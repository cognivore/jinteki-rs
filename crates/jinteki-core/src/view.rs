//! Per-viewpoint state rendering, jnet-shaped JSON.
//!
//! This is the redaction boundary (DESIGN.md SYS-S-1, playable-milestone
//! slice): nothing reaches a client except through `render_state(viewer)`.
//! The shape mirrors jinteki.net's public-states keys closely enough that the
//! same UI renders both this engine (local mode) and the reference server
//! (bridge mode): hands as arrays-for-owner / counts-for-opponent, facedown
//! cards stripped to cid+counters, prompt-state only for the owning side.

use crate::state::*;
use crate::types::*;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewer {
    Side(Side),
    Spectator,
}

impl Viewer {
    fn sees(&self, side: Side) -> bool {
        matches!(self, Viewer::Side(s) if *s == side)
    }
}

pub fn render_state(st: &GameState, viewer: Viewer) -> Value {
    let mut root = Map::new();
    root.insert("gameid".into(), json!("local"));
    root.insert("turn".into(), json!(st.turn));
    root.insert("active-player".into(), json!(st.active.as_str()));
    root.insert(
        "turn-state".into(),
        json!(match st.turn_state {
            TurnState::Setup => "setup",
            TurnState::AwaitingStart => "awaiting-start",
            TurnState::Acting => "acting",
            TurnState::GameOver => "game-over",
        }),
    );
    root.insert("run".into(), render_run(st));
    root.insert("corp".into(), render_corp(st, viewer));
    root.insert("runner".into(), render_runner(st, viewer));
    root.insert("log".into(), json!(st.log));
    root.insert(
        "winner".into(),
        st.winner.map(|w| json!(w.as_str())).unwrap_or(Value::Null),
    );
    root.insert(
        "reason".into(),
        st.reason.as_ref().map(|r| json!(r)).unwrap_or(Value::Null),
    );
    Value::Object(root)
}

fn render_run(st: &GameState) -> Value {
    match &st.run {
        None => Value::Null,
        Some(r) => json!({
            "server": [r.server.key()],
            "position": r.position,
            "phase": r.phase,
            "successful": r.successful,
            "run-credits": r.run_credits,
        }),
    }
}

fn render_corp(st: &GameState, viewer: Viewer) -> Value {
    let side = Side::Corp;
    let own = viewer.sees(side);
    let mut m = Map::new();
    m.insert("user".into(), json!({"username": st.username(side)}));
    m.insert(
        "identity".into(),
        card_json(st, st.identity(side), true),
    );
    m.insert("credit".into(), json!(st.credits(side)));
    m.insert("click".into(), json!(st.clicks(side)));
    m.insert("agenda-point".into(), json!(st.agenda_points(side)));
    m.insert("bad-publicity".into(), json!({"base": st.bad_pub}));
    m.insert(
        "hand".into(),
        if own {
            Value::Array(
                st.hand(side)
                    .iter()
                    .map(|&c| card_json(st, c, true))
                    .collect(),
            )
        } else {
            json!([])
        },
    );
    m.insert("hand-count".into(), json!(st.hand(side).len()));
    m.insert("deck".into(), json!([]));
    m.insert("deck-count".into(), json!(st.deck(side).len()));
    m.insert(
        "discard".into(),
        Value::Array(
            st.discard(side)
                .iter()
                .map(|&c| {
                    let visible = own || st.card(c).faceup;
                    card_json(st, c, visible)
                })
                .collect(),
        ),
    );
    m.insert(
        "scored".into(),
        Value::Array(
            st.scored(side)
                .iter()
                .map(|&c| card_json(st, c, true))
                .collect(),
        ),
    );
    let mut servers = Map::new();
    for (id, srv) in &st.servers {
        let content: Vec<Value> = srv
            .content
            .iter()
            .map(|&c| card_json(st, c, own || st.card(c).rezzed || st.card(c).faceup))
            .collect();
        let ices: Vec<Value> = srv
            .ices
            .iter()
            .map(|&c| card_json(st, c, own || st.card(c).rezzed))
            .collect();
        servers.insert(id.key(), json!({"content": content, "ices": ices}));
    }
    m.insert("servers".into(), Value::Object(servers));
    m.insert("prompt-state".into(), prompt_state(st, side, viewer));
    Value::Object(m)
}

fn render_runner(st: &GameState, viewer: Viewer) -> Value {
    let side = Side::Runner;
    let own = viewer.sees(side);
    let mut m = Map::new();
    m.insert("user".into(), json!({"username": st.username(side)}));
    m.insert("identity".into(), card_json(st, st.identity(side), true));
    m.insert("credit".into(), json!(st.credits(side)));
    m.insert("click".into(), json!(st.clicks(side)));
    m.insert("agenda-point".into(), json!(st.agenda_points(side)));
    m.insert("tag".into(), json!({"base": st.tags, "total": st.tags}));
    m.insert(
        "memory".into(),
        json!({
            "base": 4,
            "limit": st.mu_limit(),
            "used": st.mu_used(),
            "available": st.mu_limit() - st.mu_used(),
        }),
    );
    m.insert(
        "hand-size".into(),
        json!({"base": 5, "total": st.max_hand_size(side)}),
    );
    m.insert(
        "hand".into(),
        if own {
            Value::Array(
                st.hand(side)
                    .iter()
                    .map(|&c| card_json(st, c, true))
                    .collect(),
            )
        } else {
            json!([])
        },
    );
    m.insert("hand-count".into(), json!(st.hand(side).len()));
    m.insert("deck".into(), json!([]));
    m.insert("deck-count".into(), json!(st.deck(side).len()));
    // The heap is public information.
    m.insert(
        "discard".into(),
        Value::Array(
            st.discard(side)
                .iter()
                .map(|&c| card_json(st, c, true))
                .collect(),
        ),
    );
    m.insert(
        "scored".into(),
        Value::Array(
            st.scored(side)
                .iter()
                .map(|&c| card_json(st, c, true))
                .collect(),
        ),
    );
    m.insert(
        "rig".into(),
        json!({
            "program": st.rig.programs.iter().map(|&c| card_json(st, c, true)).collect::<Vec<_>>(),
            "hardware": st.rig.hardware.iter().map(|&c| card_json(st, c, true)).collect::<Vec<_>>(),
            "resource": st.rig.resources.iter().map(|&c| card_json(st, c, true)).collect::<Vec<_>>(),
        }),
    );
    m.insert("prompt-state".into(), prompt_state(st, side, viewer));
    Value::Object(m)
}

/// jnet-shaped prompt-state: full for the owner, a waiting stub for the
/// opponent, null when there is nothing.
fn prompt_state(st: &GameState, side: Side, viewer: Viewer) -> Value {
    if !viewer.sees(side) {
        return Value::Null;
    }
    match st.current_prompt(side) {
        Some(p) => {
            let choices: Vec<Value> = p
                .choices
                .iter()
                .map(|c| json!({"uuid": c.uuid, "value": c.label}))
                .collect();
            json!({
                "msg": p.msg,
                "prompt-type": p.prompt_type,
                "choices": choices,
                "select": p.select.is_some(),
            })
        }
        None => {
            if st.any_prompt_open() {
                json!({
                    "msg": "Waiting for opponent",
                    "prompt-type": "waiting",
                    "choices": [],
                    "select": false,
                })
            } else {
                Value::Null
            }
        }
    }
}

/// One card, redacted when `visible` is false (jnet private-card style:
/// cid + public counters only).
fn card_json(st: &GameState, cid: Cid, visible: bool) -> Value {
    let c = st.card(cid);
    if !visible {
        return json!({
            "cid": cid,
            "facedown": true,
            "advance-counter": c.advancement,
        });
    }
    let def = c.def();
    let mut m = Map::new();
    m.insert("cid".into(), json!(cid));
    m.insert("title".into(), json!(def.title));
    // Text and stats come from the printed database for every title;
    // card_text falls back to the hand-written pool strings if needed.
    m.insert("text".into(), json!(crate::carddb::card_text(def.title)));
    m.insert(
        "implementation".into(),
        match crate::printed::impl_status(def.title) {
            crate::printed::ImplStatus::Behavior => Value::Null,
            crate::printed::ImplStatus::JnetOnly => {
                json!("rs-unimplemented: engine treats as vanilla")
            }
            crate::printed::ImplStatus::Unimplemented => {
                json!("unimplemented everywhere: isolated")
            }
        },
    );
    m.insert("type".into(), json!(def.kind.as_str()));
    m.insert("cost".into(), json!(def.cost));
    m.insert("subtypes".into(), json!(def.subtypes));
    m.insert("rezzed".into(), json!(c.rezzed));
    m.insert("facedown".into(), json!(false));
    m.insert("advance-counter".into(), json!(c.advancement));
    if c.credits > 0 {
        m.insert("counter".into(), json!({"credit": c.credits}));
    }
    if def.kind == CardType::Ice {
        m.insert("strength".into(), json!(st.ice_strength(cid)));
        if c.rezzed || matches!(c.zone, Zone::Discard | Zone::Hand) {
            let subs: Vec<Value> = def
                .subroutines
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    json!({
                        "label": s.label(),
                        "broken": *c.broken.get(i).unwrap_or(&false),
                    })
                })
                .collect();
            m.insert("subroutines".into(), Value::Array(subs));
        }
    }
    if def.breaker.is_some() {
        m.insert("strength".into(), json!(st.breaker_strength(cid)));
    }
    if let Some(tc) = def.trash_cost {
        m.insert("trash-cost".into(), json!(tc));
    }
    if let Some(ap) = def.agenda_points {
        m.insert("agendapoints".into(), json!(ap));
        m.insert(
            "advancementcost".into(),
            json!(def.advancement_requirement.unwrap_or(0)),
        );
    }
    // Ability labels for the client's action sheet.
    let mut abilities: Vec<Value> = Vec::new();
    if let Some(bd) = def.breaker {
        abilities.push(json!({"label": format!("1 [Credits]: Break {} subroutine", bd.breaks.as_str())}));
        if bd.pump.is_some() {
            abilities.push(json!({"label": "1 [Credits]: +1 strength"}));
        }
    }
    if let Some(ClickAbility::TakeCredits(n)) = def.click_ability {
        abilities.push(json!({"label": format!("[Click]: Take {n} [Credits]")}));
    }
    if !abilities.is_empty() {
        m.insert("abilities".into(), Value::Array(abilities));
    }
    Value::Object(m)
}
