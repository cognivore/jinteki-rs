//! Local mode: human vs the random-walk bot, human vs the CR bot, and human
//! vs human through the CR lobby — with sessions that survive refreshes and
//! closed tabs.
//!
//! Games live in a server-side registry keyed by an opaque token; the client
//! stores its token in localStorage and resumes over any fresh WebSocket.
//! Wire protocol (JSON text frames):
//!   client → server:
//!     {"type":"start","side":"runner"|"corp","seed":123?,"runner_id"?}
//!     {"type":"start","engine":"cr","side":…,"seed":…}   ← the CR engine
//!     {"type":"resume","token":"..."}
//!     {"type":"action","command":"<jnet command>","args":{...}}
//!     {"type":"say","msg":"..."}                          ← chat (CR games)
//!     {"type":"lobby-list"}                               ← the CR lobby
//!     {"type":"lobby-create","side":…,"title":…,"seed":…}
//!     {"type":"lobby-join","gameid":"..."}
//!     {"type":"lobby-cancel"}
//!   server → client:
//!     {"type":"session","token":"...","side":"runner"|"corp","engine"?:"cr"}
//!     {"type":"state","state":{...jnet-shaped...},"actions":[...legal...]}
//!     {"type":"error","error":"...","cr_readiness"?:{…}}
//!     {"type":"lobby-list","list":[{gameid,title,creator,side,…}]}
//!     {"type":"lobby-waiting","lobby":{…},"token":"..."}
//!
//! TWO engines ride this one socket. `engine:"cr"` on the start message hosts
//! a `jinteki-cr` VM (the Comprehensive Rules machine, eternal decks, the
//! plan driver's neutral policy as the bot — see `crate::cr`); anything else
//! is the original local engine below, unchanged and still the default. Both
//! keep their own registry, and `resume` finds a token in either — or in the
//! lobby, where a token can also be a seat still waiting for an opponent.
//!
//! A two-human game needs the server to speak first: when your opponent moves,
//! nothing arrives on YOUR socket unless something tells it to look. That is
//! `crate::lobby`'s nudge bus, and this loop selects over it and the socket.
//! A nudge names a game; the answer is always this connection serializing its
//! OWN seat's view (SYS-S-1) — no state ever travels over the bus.

use crate::db::Db;
use crate::decks;
use crate::lobby::{self, Nudge};
use axum::extract::ws::{Message, WebSocket};
use jinteki_core::state::{GameState, TurnState};
use jinteki_core::view::{render_state, Viewer};
use jinteki_core::{enumerate_actions, process_command, random_walk_step, Command, ServerId, Side};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::Mutex;

struct LocalGame {
    st: GameState,
    human: Side,
    bot_rng: ChaCha8Rng,
    last_seen: Instant,
    /// Whether the game's outcome has been written to the `games` table yet.
    outcome_recorded: bool,
}

type Registry = Arc<Mutex<HashMap<String, Arc<Mutex<LocalGame>>>>>;

fn registry() -> Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

/// Sessions idle longer than this are pruned (phones sleep; give them days).
const SESSION_TTL: Duration = Duration::from_secs(72 * 3600);

async fn prune_and_insert(token: String, game: Arc<Mutex<LocalGame>>) {
    let reg = registry();
    let mut map = reg.lock().await;
    let mut dead = Vec::new();
    for (t, g) in map.iter() {
        if let Ok(g) = g.try_lock() {
            if g.last_seen.elapsed() > SESSION_TTL {
                dead.push(t.clone());
            }
        }
    }
    for t in dead {
        map.remove(&t);
    }
    map.insert(token, game);
}

/// Titles for one side of a game: identity + deck list.
struct SideDeck {
    identity: String,
    cards: Vec<String>,
    deck_id: Option<String>,
}

fn demo_deck(side: Side) -> SideDeck {
    match side {
        Side::Corp => SideDeck {
            identity: jinteki_core::carddb::CORP_ID.into(),
            cards: jinteki_core::carddb::corp_deck().iter().map(|s| s.to_string()).collect(),
            deck_id: None,
        },
        Side::Runner => SideDeck {
            identity: jinteki_core::carddb::RUNNER_ID.into(),
            cards: jinteki_core::carddb::runner_deck().iter().map(|s| s.to_string()).collect(),
            deck_id: None,
        },
    }
}

/// Load a stored deck for the human side: must be owned by the connected
/// user or published in the library (§8.3).
async fn load_side_deck(
    db: &Db,
    user: Option<&str>,
    deck_id: &str,
    want_side: Side,
) -> Result<SideDeck, String> {
    let conn = db.lock().await;
    let row = decks::get(&conn, deck_id).ok_or("no such deck")?;
    let owned = user == Some(row.owner_id.as_str());
    if !owned && row.published_at.is_none() {
        return Err("no such deck".into());
    }
    let side = match row.side.as_str() {
        "corp" => Side::Corp,
        _ => Side::Runner,
    };
    if side != want_side {
        return Err(format!("that deck is a {} deck", row.side));
    }
    let cards: Vec<decks::WireLine> = serde_json::from_str(&row.cards_json).unwrap_or_default();
    Ok(SideDeck {
        identity: row.identity_title,
        cards: decks::expand_titles(&cards),
        deck_id: Some(row.id),
    })
}

/// If the game just ended, write the outcome onto its `games` row (§5.2:
/// state stays in the registry; the table records existence + outcome).
async fn record_outcome_if_over(db: &Db, token: &str, g: &mut LocalGame) {
    if g.outcome_recorded || !g.st.game_over() {
        return;
    }
    g.outcome_recorded = true;
    let winner = g.st.winner.map(|w| w.as_str().to_string());
    let reason = g.st.reason.clone();
    let conn = db.lock().await;
    let _ = conn.execute(
        "UPDATE games SET finished_at = datetime('now'), winner = ?1, reason = ?2
         WHERE id = ?3 AND finished_at IS NULL",
        rusqlite::params![winner, reason, token],
    );
}

/// Which engine a connection is attached to. The three are peers on the wire
/// and strangers everywhere else.
enum Attached {
    Local(String, Arc<Mutex<LocalGame>>),
    Cr(crate::cr::Seat),
    /// A seat taken in the CR lobby, waiting for an opponent (its token).
    Waiting(String),
}

/// The seat label the other player sees. The accounts subsystem owns the
/// name; a cookieless visitor plays as "guest".
async fn display_name(db: &Db, user: Option<&str>) -> String {
    match user {
        Some(uid) => {
            let conn = db.lock().await;
            crate::auth::display_name(&conn, uid).unwrap_or_else(|| "guest".into())
        }
        None => "guest".into(),
    }
}

/// Answer a nudge: re-list the lobby if this socket is watching it, push this
/// seat's own view if the nudge was about its game, and notice the moment a
/// waiting seat becomes a game. Nothing here reads another seat's state.
async fn on_nudge(
    ws: &mut WebSocket,
    db: &Db,
    n: Option<&Nudge>,
    attached: &mut Option<Attached>,
    watching_lobby: bool,
) {
    let all = n.is_none(); // a lagged receiver: refresh everything
    let lobby_moved = all || matches!(n, Some(Nudge::Lobby));
    if lobby_moved && watching_lobby {
        let list = lobby::list_json().await;
        let _ = ws.send(Message::Text(list.to_string().into())).await;
    }
    // A waiting seat finds out it is a game the same way anyone would: by
    // asking the registry whether its token resolves yet.
    if lobby_moved {
        if let Some(Attached::Waiting(token)) = attached.as_ref() {
            let token = token.clone();
            if let Some(seat) = crate::cr::lookup(&token).await {
                crate::cr::attach(ws, db, &token, &seat).await;
                lobby::nudge(Nudge::Game(seat.key.clone()));
                *attached = Some(Attached::Cr(seat));
                return;
            }
        }
    }
    if let Some(Attached::Cr(seat)) = attached.as_ref() {
        if all || matches!(n, Some(Nudge::Game(k)) if *k == seat.key) {
            crate::cr::push_seat(seat, ws).await;
        }
    }
}

pub async fn handle(mut ws: WebSocket, db: Arc<Db>, user: Option<String>) {
    // The session this connection is attached to, if any.
    let mut attached: Option<Attached> = None;
    let mut nudges = lobby::subscribe();
    let me = display_name(&db, user.as_deref()).await;
    let mut watching_lobby = false;

    loop {
        enum Ev {
            Client(Message),
            Nudge(Nudge),
            Resync,
            Gone,
        }
        // The socket and the bus, together: a two-human game only moves on
        // your screen because the other player's move said so.
        let ev = tokio::select! {
            m = ws.recv() => match m {
                Some(Ok(m)) => Ev::Client(m),
                _ => Ev::Gone,
            },
            n = nudges.recv() => match n {
                Ok(n) => Ev::Nudge(n),
                Err(RecvError::Lagged(_)) => Ev::Resync,
                Err(RecvError::Closed) => Ev::Gone,
            },
        };
        let text = match ev {
            Ev::Gone => break,
            Ev::Nudge(n) => {
                on_nudge(&mut ws, &db, Some(&n), &mut attached, watching_lobby).await;
                continue;
            }
            Ev::Resync => {
                on_nudge(&mut ws, &db, None, &mut attached, watching_lobby).await;
                continue;
            }
            Ev::Client(Message::Text(t)) => t,
            Ev::Client(_) => continue,
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            send_err(&mut ws, "bad json").await;
            continue;
        };
        match v["type"].as_str() {
            // ── the CR lobby: human vs human, same VM, same gate ──────────
            Some("lobby-list") => {
                watching_lobby = true;
                let list = lobby::list_json().await;
                let _ = ws.send(Message::Text(list.to_string().into())).await;
            }
            Some("lobby-create") => {
                // SYS-D-12, at the lobby door exactly as at the bot door.
                let r = crate::cr::readiness();
                if !r.ready {
                    crate::cr::refuse_gate(&mut ws, &r).await;
                    continue;
                }
                // One open seat per person: creating again replaces the old.
                if let Some(Attached::Waiting(t)) = attached.as_ref() {
                    lobby::cancel(t).await;
                }
                let side = lobby::side_from_key(v["side"].as_str().unwrap_or("runner"));
                let seed = v["seed"].as_u64().unwrap_or_else(rand::random);
                let o = lobby::create(
                    v["title"].as_str().unwrap_or(""),
                    &me,
                    user.clone(),
                    side,
                    seed,
                )
                .await;
                watching_lobby = true;
                let _ = ws
                    .send(Message::Text(
                        json!({
                            "type": "lobby-waiting",
                            "lobby": o.to_json(),
                            "token": o.token,
                            "side": v["side"].as_str().unwrap_or("runner"),
                            "deck": lobby::deck_title(side),
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                attached = Some(Attached::Waiting(o.token.clone()));
            }
            Some("lobby-cancel") => {
                if let Some(Attached::Waiting(t)) = attached.as_ref() {
                    lobby::cancel(t).await;
                    attached = None;
                }
                let list = lobby::list_json().await;
                let _ = ws.send(Message::Text(list.to_string().into())).await;
            }
            Some("lobby-join") => {
                let id = v["gameid"].as_str().unwrap_or("");
                let Some(open) = lobby::claim(id).await else {
                    send_err(&mut ws, "that game is no longer open").await;
                    let list = lobby::list_json().await;
                    let _ = ws.send(Message::Text(list.to_string().into())).await;
                    continue;
                };
                // You are not your own opponent.
                if attached.as_ref().is_some_and(
                    |a| matches!(a, Attached::Waiting(t) if *t == open.token),
                ) {
                    lobby::restore(open).await;
                    send_err(&mut ws, "that is your own game").await;
                    continue;
                }
                // SYS-D-12 once more, because the gate is evaluated PER START.
                let setup = match crate::cr::eternal_setup(open.seed) {
                    Ok(s) => s,
                    Err(r) => {
                        lobby::restore(open).await;
                        crate::cr::refuse_gate(&mut ws, &r).await;
                        continue;
                    }
                };
                let started = lobby::start(open, &me, user.clone(), setup).await;
                // One `games` row per seat: each player's own token.
                if let Some(uid) = user.as_deref() {
                    crate::cr::record_start(&db, &started.token, uid, started.side, started.seed)
                        .await;
                }
                if let Some(uid) = started.creator_user.as_deref() {
                    crate::cr::record_start(
                        &db,
                        &started.creator_token,
                        uid,
                        started.creator_side,
                        started.seed,
                    )
                    .await;
                }
                watching_lobby = false;
                crate::cr::attach(&mut ws, &db, &started.token, &started.seat).await;
                // Wake the creator: their seat is a game now.
                lobby::nudge(Nudge::Game(started.key.clone()));
                attached = Some(Attached::Cr(started.seat));
            }
            // Chat: both players' logs, verbatim, attributed (CR games only —
            // the game log itself stays per-side).
            Some("say") => {
                let msg = v["msg"].as_str().unwrap_or("").to_string();
                if let Some(Attached::Cr(seat)) = attached.as_ref() {
                    if crate::cr::chat(seat, &msg).await {
                        crate::cr::push_seat(seat, &mut ws).await;
                        lobby::nudge(Nudge::Game(seat.key.clone()));
                    }
                }
            }
            // The CR engine: eternal decks behind the completeness gate.
            Some("start") if v["engine"].as_str() == Some("cr") => {
                if let Some((_token, seat)) =
                    crate::cr::start(&mut ws, &db, user.as_deref(), &v).await
                {
                    watching_lobby = false;
                    attached = Some(Attached::Cr(seat));
                }
            }
            Some("start") => {
                let side = match v["side"].as_str() {
                    Some("corp") => Side::Corp,
                    _ => Side::Runner,
                };
                let seed = v["seed"].as_u64().unwrap_or_else(rand::random);
                // The human side's deck: a stored deck when deck_id is given
                // (owned or published), else the built-in demo deck — the
                // cookieless flow is exactly today's behavior (§8.3).
                let human_deck = match v["deck_id"].as_str() {
                    Some(deck_id) => {
                        match load_side_deck(&db, user.as_deref(), deck_id, side).await {
                            Ok(d) => d,
                            Err(e) => {
                                send_err(&mut ws, &e).await;
                                continue;
                            }
                        }
                    }
                    None => {
                        let mut d = demo_deck(side);
                        // Legacy knob: bot games may pick another runner id.
                        if side == Side::Runner {
                            if let Some(rid) = v["runner_id"].as_str() {
                                d.identity = rid.to_string();
                            }
                        }
                        d
                    }
                };
                let bot_deck = demo_deck(side.opponent());
                let (corp, runner) = match side {
                    Side::Corp => (&human_deck, &bot_deck),
                    Side::Runner => (&bot_deck, &human_deck),
                };
                // NO silent vanilla play: refuse any deck containing a card
                // whose behavior is not natively implemented, and say which.
                let mut missing: Vec<&str> = corp
                    .cards
                    .iter()
                    .chain(runner.cards.iter())
                    .map(String::as_str)
                    .chain([corp.identity.as_str(), runner.identity.as_str()])
                    .filter(|t| {
                        !matches!(
                            jinteki_core::printed::impl_status(t),
                            jinteki_core::printed::ImplStatus::Behavior
                        )
                    })
                    .collect();
                missing.dedup();
                if !missing.is_empty() {
                    send_err(
                        &mut ws,
                        &format!(
                            "deck contains cards without implemented behavior: {}",
                            missing.join(", ")
                        ),
                    )
                    .await;
                    continue;
                }
                let corp_cards: Vec<&str> = corp.cards.iter().map(String::as_str).collect();
                let runner_cards: Vec<&str> = runner.cards.iter().map(String::as_str).collect();
                let mut st = GameState::new_with_decks(
                    seed,
                    &corp.identity,
                    &corp_cards,
                    &runner.identity,
                    &runner_cards,
                );
                st.system_log(format!("Local game vs bot, seed {seed}."));
                let token = format!(
                    "{:016x}{:016x}",
                    rand::random::<u64>(),
                    rand::random::<u64>()
                );
                // Attribute the game to the connected user (durable "my
                // games" history); anonymous cookieless starts skip this.
                if let Some(uid) = user.as_deref() {
                    let conn = db.lock().await;
                    let _ = conn.execute(
                        "INSERT INTO games (id, owner_id, side, deck_id, seed, started_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                        rusqlite::params![
                            token,
                            uid,
                            side.as_str(),
                            human_deck.deck_id,
                            seed as i64
                        ],
                    );
                }
                let game = Arc::new(Mutex::new(LocalGame {
                    st,
                    human: side,
                    bot_rng: ChaCha8Rng::seed_from_u64(seed ^ 0xB07),
                    last_seen: Instant::now(),
                    outcome_recorded: false,
                }));
                prune_and_insert(token.clone(), game.clone()).await;
                let _ = ws
                    .send(Message::Text(
                        json!({"type":"session","token": token, "side": side.as_str()})
                            .to_string()
                            .into(),
                    ))
                    .await;
                {
                    let mut g = game.lock().await;
                    bot_moves(&mut g, &mut ws).await;
                    push_state(&g, &mut ws).await;
                    record_outcome_if_over(&db, &token, &mut g).await;
                }
                attached = Some(Attached::Local(token, game));
            }
            Some("resume") => {
                let token = v["token"].as_str().unwrap_or("").to_string();
                // A token belongs to exactly one engine; ask the CR registry
                // first so a CR session resumes as a CR session.
                if let Some(seat) = crate::cr::lookup(&token).await {
                    let seat = crate::cr::resume(&mut ws, &db, &token, seat).await;
                    watching_lobby = false;
                    lobby::nudge(Nudge::Game(seat.key.clone()));
                    attached = Some(Attached::Cr(seat));
                    continue;
                }
                // A token can also be a seat still waiting for an opponent —
                // create, close the tab, come back, still waiting.
                if let Some(o) = lobby::by_token(&token).await {
                    watching_lobby = true;
                    let _ = ws
                        .send(Message::Text(
                            json!({
                                "type": "lobby-waiting",
                                "lobby": o.to_json(),
                                "token": o.token,
                                "deck": lobby::deck_title(o.side),
                            })
                            .to_string()
                            .into(),
                        ))
                        .await;
                    attached = Some(Attached::Waiting(o.token.clone()));
                    continue;
                }
                let found = registry().lock().await.get(&token).cloned();
                match found {
                    Some(game) => {
                        {
                            let mut g = game.lock().await;
                            g.last_seen = Instant::now();
                            let side = g.human.as_str();
                            let _ = ws
                                .send(Message::Text(
                                    json!({"type":"session","token": token, "side": side})
                                        .to_string()
                                        .into(),
                                ))
                                .await;
                            // The bot may have been mid-move when the old tab died.
                            bot_moves(&mut g, &mut ws).await;
                            push_state(&g, &mut ws).await;
                            record_outcome_if_over(&db, &token, &mut g).await;
                        }
                        attached = Some(Attached::Local(token, game));
                    }
                    None => send_err(&mut ws, "session expired").await,
                }
            }
            Some("action") => {
                let (token, game) = match attached.as_ref() {
                    Some(Attached::Local(t, g)) => (t, g),
                    Some(Attached::Cr(seat)) => {
                        // The other seat learns of it the only way it can.
                        if crate::cr::action(&mut ws, &db, seat, &v).await {
                            lobby::nudge(Nudge::Game(seat.key.clone()));
                        }
                        continue;
                    }
                    Some(Attached::Waiting(..)) => {
                        send_err(&mut ws, "waiting for an opponent").await;
                        continue;
                    }
                    None => {
                        send_err(&mut ws, "no game attached").await;
                        continue;
                    }
                };
                match parse_command(&v) {
                    Ok(cmd) => {
                        let mut g = game.lock().await;
                        g.last_seen = Instant::now();
                        let side = g.human;
                        if let Err(e) = process_command(&mut g.st, side, cmd) {
                            send_err(&mut ws, &e.to_string()).await;
                        }
                        push_state(&g, &mut ws).await;
                        bot_moves(&mut g, &mut ws).await;
                        record_outcome_if_over(&db, token, &mut g).await;
                    }
                    Err(e) => send_err(&mut ws, &e).await,
                }
            }
            _ => {}
        }
    }

    // The socket is gone. A seat with nobody in it is shown as exactly that
    // to the player still at the table — a held game is honest, a silently
    // stalled one is not.
    if let Some(Attached::Cr(seat)) = attached.as_ref() {
        crate::cr::set_connected(seat, false).await;
        lobby::nudge(Nudge::Game(seat.key.clone()));
    }
}

async fn send_err(ws: &mut WebSocket, e: &str) {
    let _ = ws
        .send(Message::Text(
            json!({"type":"error","error": e}).to_string().into(),
        ))
        .await;
}

async fn push_state(g: &LocalGame, ws: &mut WebSocket) {
    let state = render_state(&g.st, Viewer::Side(g.human));
    let actions = actions_json(&g.st, g.human);
    let msg = json!({"type":"state","state": state, "actions": actions});
    let _ = ws.send(Message::Text(msg.to_string().into())).await;
}

/// Let the bot act until it has no decision; push a state after each move
/// with a small delay so the human can watch it happen.
async fn bot_moves(g: &mut LocalGame, ws: &mut WebSocket) {
    let bot = g.human.opponent();
    let mut guard = 0;
    while !g.st.game_over() {
        let Some(cmd) = random_walk_step(&g.st, bot, &mut g.bot_rng) else {
            break;
        };
        let pace = match g.st.turn_state {
            TurnState::Setup => 0,
            _ => 350,
        };
        if pace > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(pace)).await;
        }
        if process_command(&mut g.st, bot, cmd).is_err() {
            break; // enumerator/executor mismatch would be a bug; stop looping
        }
        push_state(g, ws).await;
        guard += 1;
        if guard > 500 {
            break;
        }
    }
}

fn parse_command(v: &Value) -> Result<Command, String> {
    let cmd = v["command"].as_str().ok_or("missing command")?;
    let args = &v["args"];
    let cid = || -> Result<u32, String> {
        args["card"]["cid"]
            .as_u64()
            .map(|c| c as u32)
            .ok_or_else(|| "missing card".into())
    };
    Ok(match cmd {
        "credit" => Command::Credit,
        "draw" => Command::Draw,
        "start-turn" => Command::StartTurn,
        "end-turn" => Command::EndTurn,
        "continue" => Command::Continue,
        "jack-out" => Command::JackOut,
        "concede" => Command::Concede,
        "remove-tag" => Command::RemoveTag,
        "purge" => Command::Purge,
        "trash-resource" => Command::TrashResource,
        "play" => Command::Play { cid: cid()? },
        "corp-install" => Command::InstallCorp {
            cid: cid()?,
            server: args["server"].as_str().unwrap_or("New remote").to_string(),
        },
        "runner-install" => Command::InstallRunner { cid: cid()? },
        "advance" => Command::Advance { cid: cid()? },
        "score" => Command::Score { cid: cid()? },
        "rez" => Command::Rez { cid: cid()? },
        "run" => {
            let key = args["server"].as_str().ok_or("missing server")?;
            let server = ServerId::from_key(key).ok_or("bad server")?;
            Command::Run { server }
        }
        "ability" => Command::Ability {
            cid: cid()?,
            index: args["ability"].as_u64().unwrap_or(0) as usize,
        },
        "choice" => Command::Choice {
            uuid: args["choice"]["uuid"]
                .as_str()
                .ok_or("missing choice uuid")?
                .to_string(),
        },
        "select" => Command::Select { cid: cid()? },
        other => return Err(format!("unknown command {other}")),
    })
}

/// Serialize legal actions for UI affordances.
fn actions_json(st: &GameState, side: Side) -> Value {
    let acts = enumerate_actions(st, side);
    Value::Array(
        acts.into_iter()
            .map(|a| match a {
                Command::Credit => json!({"command":"credit"}),
                Command::Draw => json!({"command":"draw"}),
                Command::StartTurn => json!({"command":"start-turn"}),
                Command::EndTurn => json!({"command":"end-turn"}),
                Command::Continue => json!({"command":"continue"}),
                Command::JackOut => json!({"command":"jack-out"}),
                Command::RemoveTag => json!({"command":"remove-tag"}),
                Command::Purge => json!({"command":"purge"}),
                Command::TrashResource => json!({"command":"trash-resource"}),
                Command::Concede => json!({"command":"concede"}),
                Command::Keep => json!({"command":"keep"}),
                Command::Mulligan => json!({"command":"mulligan"}),
                Command::Play { cid } => json!({"command":"play","cid":cid}),
                Command::InstallCorp { cid, server } => {
                    json!({"command":"corp-install","cid":cid,"server":server})
                }
                Command::InstallRunner { cid } => {
                    json!({"command":"runner-install","cid":cid})
                }
                Command::Advance { cid } => json!({"command":"advance","cid":cid}),
                Command::Score { cid } => json!({"command":"score","cid":cid}),
                Command::Rez { cid } => json!({"command":"rez","cid":cid}),
                Command::Run { server } => json!({"command":"run","server":server.key()}),
                Command::Ability { cid, index } => {
                    json!({"command":"ability","cid":cid,"ability":index})
                }
                Command::Choice { uuid } => json!({"command":"choice","uuid":uuid}),
                Command::Select { cid } => json!({"command":"select","cid":cid}),
                Command::TrashAccessed { cid } => json!({"command":"trash","cid":cid}),
            })
            .collect(),
    )
}
