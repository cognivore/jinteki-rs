//! Local mode: human vs the random-walk bot, with sessions that survive
//! refreshes and closed tabs.
//!
//! Games live in a server-side registry keyed by an opaque token; the client
//! stores its token in localStorage and resumes over any fresh WebSocket.
//! Wire protocol (JSON text frames):
//!   client → server:
//!     {"type":"start","side":"runner"|"corp","seed":123?,"runner_id"?}
//!     {"type":"resume","token":"..."}
//!     {"type":"action","command":"<jnet command>","args":{...}}
//!   server → client:
//!     {"type":"session","token":"...","side":"runner"|"corp"}
//!     {"type":"state","state":{...jnet-shaped...},"actions":[...legal...]}
//!     {"type":"error","error":"..."}

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
use tokio::sync::Mutex;

struct LocalGame {
    st: GameState,
    human: Side,
    bot_rng: ChaCha8Rng,
    last_seen: Instant,
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

pub async fn handle(mut ws: WebSocket) {
    // The session this connection is attached to, if any.
    let mut attached: Option<(String, Arc<Mutex<LocalGame>>)> = None;

    while let Some(Ok(msg)) = ws.recv().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            send_err(&mut ws, "bad json").await;
            continue;
        };
        match v["type"].as_str() {
            Some("start") => {
                let side = match v["side"].as_str() {
                    Some("corp") => Side::Corp,
                    _ => Side::Runner,
                };
                let seed = v["seed"].as_u64().unwrap_or_else(rand::random);
                let runner_id = v["runner_id"]
                    .as_str()
                    .unwrap_or(jinteki_core::carddb::RUNNER_ID)
                    .to_string();
                let corp_deck = jinteki_core::carddb::corp_deck();
                let runner_deck = jinteki_core::carddb::runner_deck();
                // NO silent vanilla play: refuse any deck containing a card
                // whose behavior is not natively implemented, and say which.
                let missing: Vec<&str> = corp_deck
                    .iter()
                    .chain(runner_deck.iter())
                    .chain([jinteki_core::carddb::CORP_ID, runner_id.as_str()].iter())
                    .filter(|t| {
                        !matches!(
                            jinteki_core::printed::impl_status(t),
                            jinteki_core::printed::ImplStatus::Behavior
                        )
                    })
                    .copied()
                    .collect();
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
                let mut st = GameState::new_with_decks(
                    seed,
                    jinteki_core::carddb::CORP_ID,
                    &corp_deck,
                    &runner_id,
                    &runner_deck,
                );
                st.system_log(format!("Local game vs bot, seed {seed}."));
                let token = format!(
                    "{:016x}{:016x}",
                    rand::random::<u64>(),
                    rand::random::<u64>()
                );
                let game = Arc::new(Mutex::new(LocalGame {
                    st,
                    human: side,
                    bot_rng: ChaCha8Rng::seed_from_u64(seed ^ 0xB07),
                    last_seen: Instant::now(),
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
                }
                attached = Some((token, game));
            }
            Some("resume") => {
                let token = v["token"].as_str().unwrap_or("").to_string();
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
                        }
                        attached = Some((token, game));
                    }
                    None => send_err(&mut ws, "session expired").await,
                }
            }
            Some("action") => {
                let Some((_, game)) = attached.as_ref() else {
                    send_err(&mut ws, "no game attached").await;
                    continue;
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
                    }
                    Err(e) => send_err(&mut ws, &e).await,
                }
            }
            _ => {}
        }
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
