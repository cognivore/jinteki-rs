//! Local mode: human vs the random-walk bot, human vs the CR bot, and human
//! vs human through the CR lobby — with sessions that survive refreshes and
//! closed tabs.
//!
//! Games live in a server-side registry keyed by an opaque token; the client
//! stores its token in localStorage and resumes over any fresh WebSocket.
//! Wire protocol (JSON text frames):
//!   client → server:
//!     {"type":"start","side":"runner"|"corp","seed":"123"?,"runner_id"?}
//!     {"type":"start","engine":"cr","side":…,"seed":…}   ← the CR engine
//!     {"type":"resume","token":"..."}
//!     {"type":"action","command":"<jnet command>","args":{...}}
//!     {"type":"say","msg":"..."}                          ← chat (CR games)
//!     {"type":"lobby-list"}                               ← the CR lobby
//!     {"type":"lobby-create","side":…,"title":…,"seed":…,"deck"?,"timing"?}
//!     {"type":"lobby-join","gameid":"...","deck"?}
//!     {"type":"lobby-anyone","decks"?:{"runner"?,"corp"?},"side"?,"seed"?,"timing"?}
//!     {"type":"lobby-ready","ready":true|false}
//!     {"type":"lobby-cancel"}          ← also leaves a ready-check table
//!   server → client:
//!     {"type":"session","token":"...","side":"runner"|"corp","engine"?:"cr"}
//!     {"type":"state","state":{...jnet-shaped...},"actions":[...legal...]}
//!     {"type":"error","error":"...","cr_readiness"?:{…}}
//!     {"type":"lobby-list","list":[{gameid,title,creator,side,deck,…}]}
//!     {"type":"lobby-waiting","lobby":{…},"token":"..."}
//!     {"type":"lobby-pairing","token":"...","pairing":{id,title,count,seats:[…]}}
//!     {"type":"lobby-gone"}            ← your ready-check table dissolved
//!
//! A join is a READY CHECK, not a start: both players at the table toggle
//! ready, the server counts 5,4,3,2,1 (one tick a second, cancelled by any
//! unready or leave), and only the count reaching zero creates the game —
//! through the same `cr::eternal_setup` gate and the same
//! `cr::create_two_human_session` a game has always been created through.
//! "lobby-anyone" autopairs: it claims the oldest open seat whose free side
//! the joiner can play (sides must oppose), or opens a seat if none fits.
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

/// THE SEED IS A STRING ON THE WIRE, and this is the only place it becomes a
/// number.
///
/// A game seed is a `u64`; JavaScript's only number is a double, so `2^53` is
/// the last integer a browser can hold exactly. The client used to
/// `parseInt` the seed box, which turned `9661175140325481871` into
/// `9661175140325482000` and started a DIFFERENT GAME from the one whose seed
/// its player had pasted in to replay — silently, with the wrong number
/// echoed back in the log as if it were theirs. So the client stops
/// converting: it sends the digits the player typed, unparsed, and the one
/// parse to `u64` happens here.
///
/// A JSON number is still accepted — old clients, `curl`, the test suite —
/// but only while it is exactly a `u64`; `1e3` and `-1` are refused rather
/// than rounded or wrapped. Absent, null or blank is the seed box's
/// documented "(optional)": a random game. Everything else refuses the start
/// and says what a seed is, because a seed quietly replaced by another number
/// is precisely the bug this function exists to make impossible.
pub(crate) fn seed_from_wire(v: &Value) -> Result<u64, String> {
    fn refuse(got: &str) -> String {
        format!("seed must be a whole number from 0 to {} — got {got}", u64::MAX)
    }
    match &v["seed"] {
        Value::Null => Ok(rand::random()),
        Value::String(s) if s.trim().is_empty() => Ok(rand::random()),
        Value::String(s) => s.trim().parse::<u64>().map_err(|_| refuse(&format!("{s:?}"))),
        Value::Number(n) => n.as_u64().ok_or_else(|| refuse(&n.to_string())),
        other => Err(refuse(&other.to_string())),
    }
}

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

/// Which engine a connection is attached to. The four are peers on the wire
/// and strangers everywhere else.
enum Attached {
    Local(String, Arc<Mutex<LocalGame>>),
    Cr(crate::cr::Seat),
    /// A seat taken in the CR lobby, waiting for an opponent (its token).
    Waiting(String),
    /// A seat at a ready-check table (its token, the pairing's id).
    Paired { token: String, id: String },
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

/// This viewer's ready-check table, as a frame. The token rides along so
/// the client can store it exactly like a game's (a refresh resumes into
/// the same seat at the same table).
async fn push_pairing(ws: &mut WebSocket, token: &str, p: &lobby::Pairing) {
    let _ = ws
        .send(Message::Text(
            json!({"type":"lobby-pairing","token": token, "pairing": p.to_json(token)})
                .to_string()
                .into(),
        ))
        .await;
}

/// Answer a nudge: re-list the lobby if this socket is watching it, push this
/// seat's own view if the nudge was about its game, notice the moment a
/// waiting seat becomes a ready check, and the moment a ready check becomes a
/// game. Nothing here reads another seat's state.
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
    // A waiting seat finds out somebody sat down the same way anyone would:
    // by asking the registries whether its token resolves yet. (The pair
    // and the start both announce themselves with Nudge::Lobby.)
    if lobby_moved {
        if let Some(Attached::Waiting(token)) = attached.as_ref() {
            let token = token.clone();
            if let Some(seat) = crate::cr::lookup(&token).await {
                crate::cr::attach(ws, db, &token, &seat).await;
                lobby::nudge(Nudge::Game(seat.key.clone()));
                *attached = Some(Attached::Cr(seat));
                return;
            }
            if let Some(p) = lobby::pairing_by_token(&token).await {
                push_pairing(ws, &token, &p).await;
                *attached = Some(Attached::Paired { token, id: p.id.clone() });
                return;
            }
        }
    }
    // A seat at a ready-check table follows the table wherever it went: it
    // became a game (attach), it moved (repaint), the joiner left and I am
    // the creator (back to waiting), or it dissolved under me (gone). Only
    // its OWN table's nudges move it — every transition out of a pairing
    // announces itself with Nudge::Pair(id), so the list's churn is not a
    // reason to repaint the table.
    if let Some(Attached::Paired { token, id }) = attached.as_ref() {
        if all || matches!(n, Some(Nudge::Pair(p)) if p == id) {
            let token = token.clone();
            if let Some(seat) = crate::cr::lookup(&token).await {
                crate::cr::attach(ws, db, &token, &seat).await;
                lobby::nudge(Nudge::Game(seat.key.clone()));
                *attached = Some(Attached::Cr(seat));
                return;
            }
            if let Some(p) = lobby::pairing_by_token(&token).await {
                push_pairing(ws, &token, &p).await;
                return;
            }
            if let Some(o) = lobby::by_token(&token).await {
                let _ = ws
                    .send(Message::Text(
                        json!({
                            "type": "lobby-waiting",
                            "lobby": o.to_json(),
                            "token": o.token,
                            "side": lobby::side_key(o.side),
                            "deck": lobby::deck_title(o.side),
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                *attached = Some(Attached::Waiting(token));
                return;
            }
            let _ = ws
                .send(Message::Text(json!({"type":"lobby-gone"}).to_string().into()))
                .await;
            let list = lobby::list_json().await;
            let _ = ws.send(Message::Text(list.to_string().into())).await;
            *attached = None;
            return;
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
                // One seat per person: creating again replaces the old, and
                // a ready-check table is walked away from first.
                if let Some(Attached::Paired { token, .. }) = attached.as_ref() {
                    lobby::leave_pairing(token).await;
                    attached = None;
                }
                if let Some(Attached::Waiting(t)) = attached.as_ref() {
                    lobby::cancel(t).await;
                }
                let side = lobby::side_from_key(v["side"].as_str().unwrap_or("runner"));
                let seed = match seed_from_wire(&v) {
                    Ok(s) => s,
                    Err(e) => {
                        send_err(&mut ws, &e).await;
                        continue;
                    }
                };
                let deck = v["deck"].as_str().map(String::from);
                // Absent timing is the default mode (timed 30 + rope).
                let timing = crate::timing::TimingConfig::from_wire(&v["timing"]);
                let o = lobby::create(
                    v["title"].as_str().unwrap_or(""),
                    &me,
                    user.clone(),
                    side,
                    deck,
                    timing,
                    seed,
                )
                .await;
                o.hold();
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
                match attached.as_ref() {
                    Some(Attached::Waiting(t)) => {
                        lobby::cancel(t).await;
                        attached = None;
                    }
                    // Cancel at a ready-check table is leaving it (which
                    // also cancels any running countdown).
                    Some(Attached::Paired { token, .. }) => {
                        lobby::leave_pairing(token).await;
                        attached = None;
                    }
                    _ => {}
                }
                watching_lobby = true;
                let list = lobby::list_json().await;
                let _ = ws.send(Message::Text(list.to_string().into())).await;
            }
            Some("lobby-join") => {
                let id = v["gameid"].as_str().unwrap_or("");
                let deck = v["deck"].as_str().map(String::from);
                let Some(open) = lobby::claim(id).await else {
                    send_err(&mut ws, "that game is no longer open").await;
                    let list = lobby::list_json().await;
                    let _ = ws.send(Message::Text(list.to_string().into())).await;
                    continue;
                };
                // Joining your own lobby is a no-op: the seat goes straight
                // back and you are simply told you are still waiting.
                if attached.as_ref().is_some_and(
                    |a| matches!(a, Attached::Waiting(t) if *t == open.token),
                ) {
                    let frame = json!({
                        "type": "lobby-waiting",
                        "lobby": open.to_json(),
                        "token": open.token,
                        "side": lobby::side_key(open.side),
                        "deck": lobby::deck_title(open.side),
                    });
                    lobby::restore(open).await;
                    let _ = ws.send(Message::Text(frame.to_string().into())).await;
                    continue;
                }
                // SYS-D-12 at the door (it is evaluated once more when the
                // countdown reaches zero, because the gate is per START).
                let r = crate::cr::readiness();
                if !r.ready {
                    lobby::restore(open).await;
                    crate::cr::refuse_gate(&mut ws, &r).await;
                    continue;
                }
                // One seat per person: taking this table gives up any seat
                // or table you already held.
                if let Some(Attached::Paired { token, .. }) = attached.as_ref() {
                    lobby::leave_pairing(token).await;
                }
                if let Some(Attached::Waiting(t)) = attached.as_ref() {
                    lobby::cancel(t).await;
                }
                // Both seats filled: the READY CHECK, not yet the game.
                let p = lobby::pair(open, &me, user.clone(), deck).await;
                let my_token = p.seat(p.joiner_side()).token.clone();
                watching_lobby = true;
                push_pairing(&mut ws, &my_token, &p).await;
                attached = Some(Attached::Paired { token: my_token, id: p.id.clone() });
            }
            // "Play anyone": autopair with the oldest open seat whose free
            // side this player can take — or open a seat if none fits.
            Some("lobby-anyone") => {
                if matches!(attached.as_ref(), Some(Attached::Cr(_)) | Some(Attached::Local(..)))
                {
                    send_err(&mut ws, "you are already in a game").await;
                    continue;
                }
                let r = crate::cr::readiness();
                if !r.ready {
                    crate::cr::refuse_gate(&mut ws, &r).await;
                    continue;
                }
                // {"decks":{"runner":key|null,"corp":key|null}} — a side is
                // playable iff its entry exists; absent map means both, with
                // each side's default deck.
                let decks = &v["decks"];
                let deck_for = |s: &str| decks[s].as_str().map(String::from);
                let can_sides: Vec<jinteki_cr::object::Side> = match decks.as_object() {
                    Some(m) => ["corp", "runner"]
                        .into_iter()
                        .filter(|s| m.contains_key(*s))
                        .map(lobby::side_from_key)
                        .collect(),
                    None => vec![
                        jinteki_cr::object::Side::Corp,
                        jinteki_cr::object::Side::Runner,
                    ],
                };
                if can_sides.is_empty() {
                    send_err(&mut ws, "pick a deck for at least one side").await;
                    continue;
                }
                let my_open = match attached.as_ref() {
                    Some(Attached::Waiting(t)) => Some(t.clone()),
                    _ => None,
                };
                let my_pairing = match attached.as_ref() {
                    Some(Attached::Paired { token, .. }) => Some(token.clone()),
                    _ => None,
                };
                match lobby::claim_oldest_compatible(&can_sides, my_open.as_deref()).await {
                    Some(open) => {
                        // One seat per person: pairing up gives up whatever
                        // seat or table this player already held.
                        if let Some(t) = my_pairing.as_deref() {
                            lobby::leave_pairing(t).await;
                        }
                        if let Some(t) = my_open.as_deref() {
                            lobby::cancel(t).await;
                        }
                        let my_side = open.side.other();
                        let deck = deck_for(lobby::side_key(my_side));
                        let p = lobby::pair(open, &me, user.clone(), deck).await;
                        let my_token = p.seat(my_side).token.clone();
                        watching_lobby = true;
                        push_pairing(&mut ws, &my_token, &p).await;
                        attached =
                            Some(Attached::Paired { token: my_token, id: p.id.clone() });
                    }
                    None if my_open.is_some() || my_pairing.is_some() => {
                        // Nobody to pair with and this player already holds
                        // a seat: keep waiting where they are.
                        let list = lobby::list_json().await;
                        let _ = ws.send(Message::Text(list.to_string().into())).await;
                    }
                    None => {
                        // Nobody to play: open a seat and wait to be found.
                        let side = match v["side"].as_str().map(lobby::side_from_key) {
                            Some(s) if can_sides.contains(&s) => s,
                            _ => can_sides[0],
                        };
                        let seed = match seed_from_wire(&v) {
                            Ok(s) => s,
                            Err(e) => {
                                send_err(&mut ws, &e).await;
                                continue;
                            }
                        };
                        let deck = deck_for(lobby::side_key(side));
                        let timing = crate::timing::TimingConfig::from_wire(&v["timing"]);
                        let o = lobby::create("", &me, user.clone(), side, deck, timing, seed)
                            .await;
                        o.hold();
                        watching_lobby = true;
                        let _ = ws
                            .send(Message::Text(
                                json!({
                                    "type": "lobby-waiting",
                                    "lobby": o.to_json(),
                                    "token": o.token,
                                    "side": lobby::side_key(side),
                                    "deck": lobby::deck_title(side),
                                })
                                .to_string()
                                .into(),
                            ))
                            .await;
                        attached = Some(Attached::Waiting(o.token.clone()));
                    }
                }
            }
            // The ready toggle. Both seats ready starts the server's count;
            // the nudge bus repaints both tables either way.
            Some("lobby-ready") => {
                let Some(Attached::Paired { token, .. }) = attached.as_ref() else {
                    send_err(&mut ws, "you are not at a table").await;
                    continue;
                };
                let ready = v["ready"].as_bool().unwrap_or(true);
                match lobby::set_ready(token, ready).await {
                    lobby::ReadyOutcome::BothReadyNow(id) => {
                        lobby::spawn_countdown(id, db.clone()).await;
                    }
                    lobby::ReadyOutcome::Updated(_) => {}
                    lobby::ReadyOutcome::NotPaired => {
                        send_err(&mut ws, "that table is gone").await;
                        attached = None;
                    }
                }
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
                let seed = match seed_from_wire(&v) {
                    Ok(s) => s,
                    Err(e) => {
                        send_err(&mut ws, &e).await;
                        continue;
                    }
                };
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
                // A token can also be a seat at a ready-check table — a
                // refresh mid-check resumes into the same seat.
                if let Some(p) = lobby::pairing_by_token(&token).await {
                    p.hold(&token);
                    watching_lobby = true;
                    push_pairing(&mut ws, &token, &p).await;
                    attached = Some(Attached::Paired { token, id: p.id.clone() });
                    continue;
                }
                // A token can also be a seat still waiting for an opponent —
                // create, close the tab, come back, still waiting.
                if let Some(o) = lobby::by_token(&token).await {
                    o.hold();
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
                    Some(Attached::Paired { .. }) => {
                        send_err(&mut ws, "the ready check has not finished").await;
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
    // stalled one is not. A LOBBY seat, though, is only an invitation, and
    // an invitation from a dead socket is a lie: it is withdrawn, and a
    // ready-check table is walked away from (which cancels any countdown
    // and puts a still-present creator back on the open list).
    match attached.as_ref() {
        Some(Attached::Cr(seat)) => {
            crate::cr::set_connected(seat, false).await;
            lobby::nudge(Nudge::Game(seat.key.clone()));
        }
        // Not an instant withdrawal: a refresh is also a dead socket, so
        // the seat survives a grace period in case its player comes back.
        Some(Attached::Waiting(token)) | Some(Attached::Paired { token, .. }) => {
            lobby::drop_holder(token.clone());
        }
        _ => {}
    }
}

pub(crate) async fn send_err(ws: &mut WebSocket, e: &str) {
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

#[cfg(test)]
mod seed_tests {
    use super::seed_from_wire;
    use serde_json::json;

    /// The seed from the report, and the proof that it needs a string: a
    /// round trip through `f64` — which is every number a browser has —
    /// comes back as a different game.
    const BIG: u64 = 9661175140325481871;

    #[test]
    fn a_u64_seed_does_not_fit_a_javascript_number() {
        assert_ne!(BIG as f64 as u64, BIG, "the whole reason the wire says string");
    }

    #[test]
    fn the_digits_a_client_types_parse_to_that_exact_u64() {
        assert_eq!(seed_from_wire(&json!({"seed": "9661175140325481871"})), Ok(BIG));
        assert_eq!(seed_from_wire(&json!({"seed": " 9661175140325481871 "})), Ok(BIG));
        assert_eq!(seed_from_wire(&json!({"seed": "0"})), Ok(0));
        assert_eq!(seed_from_wire(&json!({"seed": "007"})), Ok(7));
        assert_eq!(
            seed_from_wire(&json!({"seed": "18446744073709551615"})),
            Ok(u64::MAX),
            "the last seed there is"
        );
    }

    /// A number is still a seed — old clients, curl, this suite — as long as
    /// it is exactly a u64.
    #[test]
    fn a_json_number_is_still_read_when_it_is_exactly_a_u64() {
        assert_eq!(seed_from_wire(&json!({"seed": BIG})), Ok(BIG));
        assert_eq!(seed_from_wire(&json!({"seed": 7})), Ok(7));
    }

    /// Absent, null and blank all mean the box was left empty: a random
    /// game, which is what "(optional)" has always promised.
    #[test]
    fn no_seed_at_all_is_a_random_game() {
        for v in [json!({}), json!({"seed": null}), json!({"seed": ""}), json!({"seed": "   "})] {
            assert!(seed_from_wire(&v).is_ok(), "{v} must be a game, not a refusal");
        }
    }

    /// Everything else refuses and says what a seed is. Not one of these may
    /// silently become some other number.
    #[test]
    fn anything_else_refuses_and_says_what_a_seed_is() {
        for v in [
            json!({"seed": "banana"}),
            json!({"seed": "-1"}),
            json!({"seed": "1.5"}),
            json!({"seed": "1e3"}),
            json!({"seed": "12 34"}),
            json!({"seed": "18446744073709551616"}), // u64::MAX + 1
            json!({"seed": -1}),
            json!({"seed": 1.5}),
            json!({"seed": 1e300}),
            json!({"seed": true}),
            json!({"seed": ["7"]}),
        ] {
            let e = seed_from_wire(&v).expect_err(&format!("{v} is not a seed"));
            assert!(e.contains("seed must be a whole number from 0 to 18446744073709551615"), "{e}");
        }
    }
}
