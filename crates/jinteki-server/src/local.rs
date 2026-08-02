//! Local mode: one game per WebSocket, human vs the random-walk bot.
//!
//! Wire protocol (JSON text frames):
//!   client → server:
//!     {"type":"start","side":"runner"|"corp","seed":123?,"runner_id"?,"corp_id"?}
//!     {"type":"action","command":"<jnet command>","args":{...}}
//!   server → client:
//!     {"type":"state","state":{...jnet-shaped...},"actions":[...legal...]}
//!     {"type":"error","error":"..."}
//! Legal actions ride with every state so the UI can glow exactly what is
//! playable (MTGA lesson: affordances, not error toasts).

use axum::extract::ws::{Message, WebSocket};
use jinteki_core::state::{GameState, TurnState};
use jinteki_core::view::{render_state, Viewer};
use jinteki_core::{enumerate_actions, process_command, random_walk_step, Command, ServerId, Side};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};

pub async fn handle(mut ws: WebSocket) {
    let mut game: Option<Game> = None;
    while let Some(Ok(msg)) = ws.recv().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            let _ = ws
                .send(Message::Text(
                    json!({"type":"error","error":"bad json"}).to_string().into(),
                ))
                .await;
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
                    let _ = ws
                        .send(Message::Text(
                            json!({
                                "type": "error",
                                "error": format!(
                                    "deck contains cards without implemented behavior: {}",
                                    missing.join(", ")
                                ),
                            })
                            .to_string()
                            .into(),
                        ))
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
                let bot_rng = ChaCha8Rng::seed_from_u64(seed ^ 0xB07);
                let mut g = Game { st, human: side, bot_rng };
                // Bot answers its mulligan immediately if it can.
                g.bot_moves(&mut ws).await;
                g.push_state(&mut ws).await;
                game = Some(g);
            }
            Some("action") => {
                if let Some(g) = game.as_mut() {
                    match parse_command(&v) {
                        Ok(cmd) => {
                            let side = g.human;
                            if let Err(e) = process_command(&mut g.st, side, cmd) {
                                let _ = ws
                                    .send(Message::Text(
                                        json!({"type":"error","error": e.to_string()})
                                            .to_string()
                                            .into(),
                                    ))
                                    .await;
                            }
                            g.push_state(&mut ws).await;
                            g.bot_moves(&mut ws).await;
                        }
                        Err(e) => {
                            let _ = ws
                                .send(Message::Text(
                                    json!({"type":"error","error": e}).to_string().into(),
                                ))
                                .await;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

struct Game {
    st: GameState,
    human: Side,
    bot_rng: ChaCha8Rng,
}

impl Game {
    async fn push_state(&self, ws: &mut WebSocket) {
        let state = render_state(&self.st, Viewer::Side(self.human));
        let actions = actions_json(&self.st, self.human);
        let msg = json!({"type":"state","state": state, "actions": actions});
        let _ = ws.send(Message::Text(msg.to_string().into())).await;
    }

    /// Let the bot act until it has no decision; push a state after each move
    /// with a small delay so the human can watch it happen.
    async fn bot_moves(&mut self, ws: &mut WebSocket) {
        let bot = self.human.opponent();
        let mut guard = 0;
        while !self.st.game_over() {
            // The bot also auto-starts its turn and never idles at AwaitingStart.
            let Some(cmd) = random_walk_step(&self.st, bot, &mut self.bot_rng) else {
                break;
            };
            let pace = match self.st.turn_state {
                TurnState::Setup => 0,
                _ => 350,
            };
            if pace > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(pace)).await;
            }
            if process_command(&mut self.st, bot, cmd).is_err() {
                break; // enumerator/executor mismatch would be a bug; stop looping
            }
            self.push_state(ws).await;
            guard += 1;
            if guard > 500 {
                break;
            }
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
