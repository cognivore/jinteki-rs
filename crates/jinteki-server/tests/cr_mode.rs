//! CR mode end to end: the completeness gate as an endpoint, and the
//! decision→prompt adapter driven over the real WebSocket protocol.
//!
//! Three tests: one for each half of the mandate, and one for the log.
//!
//! * `cr_readiness_endpoint_reports_the_true_fraction` hits
//!   `GET /api/cr-readiness` over a real listener and checks the payload
//!   against `jinteki-cards` itself — the endpoint may not invent a number,
//!   and every card it calls incomplete must say which printed sentences it
//!   cannot express (SYS-D-12).
//!
//! * `cr_ws_loop_drives_a_game_human_vs_bot` builds a VM from two SMALL
//!   ALL-COMPLETE decks (implemented cards only, so the gate is beside the
//!   point) and plays several turns human-vs-bot over the ws frames the
//!   browser actually sends. That is the adapter loop — decision → prompt →
//!   command → answer → next decision — proven end to end, plus the
//!   redaction invariant on every state it produces.
//!
//! * `cr_the_runner_reads_what_they_accessed_and_the_corp_only_what_it_may`
//!   runs two centrals and reads BOTH seats' logs off the live game: CR 7.1.2
//!   gives the Runner the card they accessed, 4.2.2 keeps R&D from the Corp,
//!   and one record produces both sentences.

use futures_util::{SinkExt, StreamExt};
use jinteki_cr::object::{CardType, PrintedCard, Side};
use jinteki_cr::{cards, GameSetup};
use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, cr, decks, guard, local, mail};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

async fn spawn_app() -> String {
    let db = Arc::new(Db::open_in_memory().expect("db"));
    {
        let conn = db.lock().await;
        auth::ensure_system_user(&conn).unwrap();
        decks::seed_starter_decks(&conn).unwrap();
    }
    let http = reqwest::Client::new();
    let state = AppState {
        db: db.clone(),
        mailer: Arc::new(mail::Mailer::from_env(http.clone())),
        guard: Arc::new(guard::Guard::new()),
        http,
        secure_cookies: false,
    };
    let app = axum::Router::new()
        .route("/ws/local", axum::routing::any(ws_local))
        .merge(api::router())
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("{addr}")
}

async fn ws_local(
    ws: axum::extract::ws::WebSocketUpgrade,
    axum::extract::State(st): axum::extract::State<AppState>,
) -> axum::response::Response {
    let db = st.db.clone();
    ws.on_upgrade(move |socket| local::handle(socket, db, None))
}

// ───────────────────────────────────────────────────────────────────────────
// (a) the gate, as an endpoint
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cr_readiness_endpoint_reports_the_true_fraction() {
    let addr = spawn_app().await;
    let r: Value = reqwest::get(format!("http://{addr}/api/cr-readiness"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // The fraction, recomputed here from the card layer itself.
    let mut total = 0usize;
    let mut complete = 0usize;
    let mut incomplete: Vec<(String, Vec<String>)> = Vec::new();
    // The SAME decks the endpoint counts, asked of the server rather than
    // listed here. This test exists to recompute the fraction independently;
    // a hardcoded pair stops being independent the moment a deck is seated,
    // and then it fails for the one reason that is not a defect.
    for key in jinteki_server::cr::deck_specs().map(|s| s.key) {
        let deck = jinteki_cards::deck_named(key).expect("the deck exists");
        // CR 1.5.4a: the additional identities come to the table with the
        // deck, so the gate counts them with it.
        let pile = jinteki_cards::pile_named(key).expect("the pile exists");
        for c in deck.into_iter().chain(pile) {
            total += 1;
            if c.is_complete() {
                complete += 1;
            } else {
                incomplete.push((
                    c.name().to_string(),
                    c.unimplemented.iter().map(|s| s.to_string()).collect(),
                ));
            }
        }
    }
    eprintln!("CR readiness: {complete}/{total} cards implemented");

    assert_eq!(r["total"].as_u64().unwrap() as usize, total, "distinct cards of both decks");
    assert_eq!(r["complete"].as_u64().unwrap() as usize, complete);
    assert_eq!(r["ready"].as_bool().unwrap(), complete == total);
    assert!(
        r["problems"].as_array().unwrap().is_empty(),
        "the printed deck lists and the card layer disagree: {:?}",
        r["problems"]
    );

    // Every incomplete card is named, with its exact unsayable sentences.
    let missing = r["missing"].as_array().unwrap();
    assert_eq!(missing.len(), incomplete.len());
    for (title, sentences) in &incomplete {
        let m = missing
            .iter()
            .find(|m| m["title"] == json!(title))
            .unwrap_or_else(|| panic!("{title} is incomplete but the endpoint does not say so"));
        let said: Vec<String> = m["unimplemented"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        assert_eq!(&said, sentences, "{title}: the refusal must quote the card");
        assert!(!said.is_empty(), "{title} is incomplete for no stated reason");
        assert!(m["copies"].as_u64().unwrap() >= 1, "{title} has a copy count");
    }

    // The two decks, with the printed copy counts (46 and 50 including
    // identities) — a deck list that drifted would show up here.
    let decks = r["decks"].as_array().unwrap();
    let by_key = |k: &str| decks.iter().find(|d| d["key"] == json!(k)).unwrap().clone();
    assert_eq!(by_key("andromeda")["copies"], json!(46));
    assert_eq!(by_key("gauntlet")["copies"], json!(50));
    assert_eq!(by_key("andromeda")["side"], json!("runner"));
    assert_eq!(by_key("gauntlet")["side"], json!("corp"));
}

/// The gate is honoured on the socket too: while the decks are incomplete a
/// CR start is refused, and the refusal carries the same payload.
#[tokio::test]
async fn cr_start_refuses_while_the_decks_are_incomplete() {
    let addr = spawn_app().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .expect("ws connects");
    send(&mut ws, json!({"type":"start","engine":"cr","side":"runner","seed":7})).await;
    let frames = drain(&mut ws, 1500).await;
    let ready = cr::readiness().ready;
    if ready {
        assert!(
            frames.iter().any(|f| f["type"] == json!("session")),
            "the decks are complete, so the game must start"
        );
    } else {
        let err = frames
            .iter()
            .find(|f| f["type"] == json!("error"))
            .expect("an incomplete deck must be refused, not silently vanilla'd");
        assert!(err["error"].as_str().unwrap().contains("not playable yet"));
        assert_eq!(err["cr_readiness"]["ready"], json!(false));
        assert!(!err["cr_readiness"]["missing"].as_array().unwrap().is_empty());
    }
}

// ───────────────────────────────────────────────────────────────────────────
// (b) the adapter loop, over the wire
// ───────────────────────────────────────────────────────────────────────────

type Ws = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

/// Read frames until the socket goes quiet — the server pushes a state per
/// bot move, so a turn is several frames.
async fn drain(ws: &mut Ws, ms: u64) -> Vec<Value> {
    let mut out = Vec::new();
    loop {
        match tokio::time::timeout(Duration::from_millis(ms), ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                out.push(serde_json::from_str(&t).expect("frames are json"))
            }
            Ok(Some(Ok(_))) => continue,
            _ => break,
        }
    }
    out
}

fn last_state(frames: &[Value]) -> Value {
    frames
        .iter()
        .rev()
        .find(|f| f["type"] == json!("state"))
        .unwrap_or_else(|| panic!("no state frame in {frames:#?}"))["state"]
        .clone()
}

/// Two small decks of cards whose behavior the VM implements — the gate is
/// about the eternal decks, the adapter is about any game at all.
fn small_setup(seed: u64) -> GameSetup {
    let mut corp_deck = Vec::new();
    for _ in 0..6 {
        corp_deck.push(cards::hedge_fund());
        corp_deck.push(cards::ice_wall());
        corp_deck.push(cards::hostile_takeover());
        corp_deck.push(cards::pad_campaign());
    }
    let mut runner_deck = Vec::new();
    for _ in 0..8 {
        runner_deck.push(cards::sure_gamble());
        runner_deck.push(cards::easy_mark());
        runner_deck.push(cards::diesel());
    }
    for c in corp_deck.iter().chain(runner_deck.iter()) {
        assert!(!c.name.is_empty());
    }
    GameSetup {
        corp_deck,
        runner_deck,
        corp_identity: Some(PrintedCard::vanilla(
            "Test Corp",
            Side::Corp,
            CardType::Identity,
        )),
        runner_identity: Some(PrintedCard::vanilla(
            "Test Runner",
            Side::Runner,
            CardType::Identity,
        )),
        // CR 1.5.4a: no additional identities brought.
        additional_identities: Default::default(),
        extra_cards: Default::default(),
        seed,
        shuffle: true,
    }
}

/// SYS-S-1, checked on every state the adapter emits: the opponent's grip and
/// both decks travel as counts, never as cards, and never even as card ids.
fn assert_redacted(state: &Value, human: &str) {
    let opp = if human == "corp" { "runner" } else { "corp" };
    assert_eq!(
        state[opp]["hand"].as_array().unwrap().len(),
        0,
        "the opponent's grip must not travel"
    );
    assert!(state[opp]["hand-count"].as_u64().unwrap() <= 9);
    for side in ["corp", "runner"] {
        assert_eq!(
            state[side]["deck"].as_array().unwrap().len(),
            0,
            "no deck ever travels"
        );
        assert!(state[side]["deck-count"].as_u64().is_some());
    }
    // The opponent's prompt is not our business either.
    assert!(state[opp]["prompt-state"].is_null());
}

#[tokio::test]
async fn cr_ws_loop_drives_a_game_human_vs_bot() {
    let addr = spawn_app().await;
    // Bot delay 0: the pacing is a UX choice, not a protocol one.
    let token = cr::create_session(small_setup(20_260_803), Side::Runner, 0).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .expect("ws connects");
    send(&mut ws, json!({"type":"resume","token": token})).await;
    let frames = drain(&mut ws, 1500).await;

    let session = frames.iter().find(|f| f["type"] == json!("session")).expect("session frame");
    assert_eq!(session["engine"], json!("cr"));
    assert_eq!(session["side"], json!("runner"));

    // 1. The first thing asked of a player is the mulligan (CR 1.6.6a). The
    //    Corp's was answered by the bot before we ever saw a frame.
    let st = last_state(&frames);
    assert_redacted(&st, "runner");
    let p = &st["runner"]["prompt-state"];
    assert!(
        p["msg"].as_str().unwrap().contains("Keep this opening hand"),
        "expected the mulligan prompt, got {p:#?}"
    );
    let choices = p["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 2, "keep or mulligan");
    assert_eq!(st["runner"]["hand-count"], json!(5), "1.6.6: five cards");

    // 2. Keep. The Corp's whole turn then plays itself out and the machine
    //    comes back to us at OUR action window.
    let keep = choices[0]["uuid"].as_str().unwrap().to_string();
    send(
        &mut ws,
        json!({"type":"action","command":"choice","args":{"choice":{"uuid": keep}}}),
    )
    .await;
    let frames = drain(&mut ws, 3000).await;
    let st = last_state(&frames);
    assert_redacted(&st, "runner");
    assert_eq!(st["active-player"], json!("runner"), "the Runner's turn");
    assert!(
        st["runner"]["prompt-state"].is_null(),
        "an action window is the board, not a prompt sheet: {:#?}",
        st["runner"]["prompt-state"]
    );

    // 3. The action window arrives as the same command vocabulary the local
    //    engine emits, so the board's chips and card sheets need no new code.
    let actions = frames
        .iter()
        .rev()
        .find(|f| f["type"] == json!("state"))
        .unwrap()["actions"]
        .as_array()
        .unwrap()
        .clone();
    let has = |c: &str| actions.iter().any(|a| a["command"] == json!(c));
    assert!(has("credit"), "5.2.7b basic credit: {actions:#?}");
    assert!(has("draw"), "5.2.7c basic draw");
    assert!(has("run"), "5.2.7f basic run");
    assert!(
        actions.iter().any(|a| a["command"] == json!("runner-install")
            || a["command"] == json!("play")),
        "a card in the grip is playable: {actions:#?}"
    );
    for a in &actions {
        assert!(a["label"].is_string(), "every affordance is labelled: {a}");
    }

    // 4. Take the basic credit action. Clicks down one, credits up one.
    let before_cred = st["runner"]["credit"].as_u64().unwrap();
    let before_click = st["runner"]["click"].as_u64().unwrap();
    assert_eq!(before_click, 4, "1.11.2: the Runner is allotted four clicks");
    send(&mut ws, json!({"type":"action","command":"credit","args":{}})).await;
    let frames = drain(&mut ws, 3000).await;
    let st = last_state(&frames);
    assert_eq!(st["runner"]["credit"].as_u64().unwrap(), before_cred + 1);
    assert_eq!(st["runner"]["click"].as_u64().unwrap(), before_click - 1);
    assert_redacted(&st, "runner");

    // 5. Draw a card: the grip grows, the stack shrinks.
    let hand = st["runner"]["hand"].as_array().unwrap().len();
    let deck = st["runner"]["deck-count"].as_u64().unwrap();
    send(&mut ws, json!({"type":"action","command":"draw","args":{}})).await;
    let frames = drain(&mut ws, 3000).await;
    let st = last_state(&frames);
    assert_eq!(st["runner"]["hand"].as_array().unwrap().len(), hand + 1);
    assert_eq!(st["runner"]["deck-count"].as_u64().unwrap(), deck - 1);

    // 5b. The log is not a list of clicks any more: the kernel's change
    //     stream is narrated into it, ONCE PER VIEWER. The same draw is a
    //     title in the drawer's copy and "a card" in their opponent's
    //     (CR 4.3.2 / 10.2.2b) — which is the whole reason there are two
    //     logs and not one. Both copies are read here from the live game,
    //     because only one of them ever goes down this socket.
    {
        let seat = cr::lookup(&token).await.expect("the session is registered");
        let g = seat.game.lock().await;
        let lines = |side: Side| -> Vec<String> {
            cr::state_json(&g, side)["log"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l["text"].as_str().unwrap_or("").to_string())
                .collect()
        };
        let mine = lines(Side::Runner);
        let theirs = lines(Side::Corp);
        let drawn = |ls: &[String], who: &str| -> Vec<String> {
            ls.iter().filter(|l| l.starts_with(&format!("{who}: draws "))).cloned().collect()
        };

        // The Runner's own draw, in the Runner's log, by name.
        let mine_drew = drawn(&mine, "Runner");
        assert!(!mine_drew.is_empty(), "the log narrates draws now: {mine:#?}");
        assert!(
            mine_drew.iter().any(|l| l != "Runner: draws a card."),
            "and names the card the drawer is entitled to: {mine_drew:#?}"
        );
        // The same event in the Corp's log, unnamed.
        assert!(
            drawn(&theirs, "Runner").iter().all(|l| l == "Runner: draws a card."),
            "4.3.2: the Corp may not read the grip: {theirs:#?}"
        );
        // And the mirror image: the Corp's mandatory draws.
        let theirs_drew = drawn(&theirs, "Corp");
        assert!(
            theirs_drew.iter().any(|l| l != "Corp: draws a card."),
            "the Corp reads its own draws: {theirs_drew:#?}"
        );
        assert!(
            drawn(&mine, "Corp").iter().all(|l| l == "Corp: draws a card."),
            "the Runner may not read HQ: {mine:#?}"
        );
        // Turn boundaries, credit swings and the run structure are all there
        // — the log the player asked for.
        assert!(
            mine.iter().any(|l| l.contains("'s turn")),
            "turns are marked: {mine:#?}"
        );
        assert!(
            mine.iter().any(|l| l == "Runner: gains 1[c]."),
            "credit swings are narrated: {mine:#?}"
        );
    }

    // 6. Our own grip is fully readable — title, type, oracle text and the
    //    NRDB code the board fetches art with.
    let card = &st["runner"]["hand"].as_array().unwrap()[0];
    assert!(card["title"].as_str().unwrap().len() > 2);
    assert!(card["type"].is_string());
    assert!(card["cid"].is_number());
    assert_eq!(card["facedown"], json!(false));

    // 7. An illegal command is refused, not applied.
    send(&mut ws, json!({"type":"action","command":"purge","args":{}})).await;
    let frames = drain(&mut ws, 1500).await;
    assert!(
        frames.iter().any(|f| f["type"] == json!("error")),
        "the Runner has no purge action: {frames:#?}"
    );

    // 8. Spend the rest of the turn: the machine keeps asking, the bot keeps
    //    answering, and the turn passes back to the Corp.
    for _ in 0..3 {
        send(&mut ws, json!({"type":"action","command":"credit","args":{}})).await;
        drain(&mut ws, 3000).await;
    }
    send(&mut ws, json!({"type":"resume","token": ""})).await; // no-op: expired
    let frames = drain(&mut ws, 3000).await;
    assert!(frames.iter().any(|f| f["type"] == json!("error")));

    // 9. The session survives a fresh socket — refresh/resume, as the local
    //    engine's registry has always done.
    let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .unwrap();
    send(&mut ws2, json!({"type":"resume","token": token})).await;
    let frames = drain(&mut ws2, 3000).await;
    let st = last_state(&frames);
    assert_redacted(&st, "runner");
    assert!(
        st["runner"]["credit"].as_u64().unwrap() >= before_cred + 1,
        "the resumed game is the same game"
    );
    assert!(
        st["log"].as_array().unwrap().iter().any(|l| l["text"]
            .as_str()
            .unwrap_or("")
            .contains("CR engine")),
        "the log came back with it"
    );
}

/// "I don't see what I'm accessing while accessing HQ."
///
/// The regression test for the headline complaint, driven the way a player
/// drives it: run a central, walk the run controls to the breach, and read
/// BOTH seats' logs off the live game.
///
/// CR 7.1.2 gives the Runner the card they are accessing; 4.2.2 keeps R&D
/// from the Corp. So an R&D access is named in the Runner's log and not in
/// the Corp's, an HQ access is named in both (the Corp is looking at its own
/// grip), and neither is ever the bare "a card" for the Runner — which is
/// what the log said before, because the entitlement had lapsed by the time
/// the line was rendered.
#[tokio::test]
async fn cr_the_runner_reads_what_they_accessed_and_the_corp_only_what_it_may() {
    let addr = spawn_app().await;
    let token = cr::create_session(small_setup(20_260_804), Side::Runner, 0).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .expect("ws connects");
    send(&mut ws, json!({"type":"resume","token": token})).await;
    let frames = drain(&mut ws, 1500).await;

    // Keep the opening hand.
    let st = last_state(&frames);
    let keep = st["runner"]["prompt-state"]["choices"][0]["uuid"]
        .as_str()
        .expect("the mulligan prompt")
        .to_string();
    send(
        &mut ws,
        json!({"type":"action","command":"choice","args":{"choice":{"uuid": keep}}}),
    )
    .await;
    drain(&mut ws, 1500).await;

    // Run R&D, then HQ, answering whatever the run puts in front of us —
    // "continue" past the jack-out choice, otherwise the first offered
    // choice — until the run is over.
    for server in ["rd", "hq"] {
        send(
            &mut ws,
            json!({"type":"action","command":"run","args":{"server": server}}),
        )
        .await;
        let mut frames = drain(&mut ws, 1200).await;
        for _ in 0..12 {
            let Some(last) = frames.iter().rev().find(|f| f["type"] == json!("state")).cloned()
            else {
                break;
            };
            let actions = last["actions"].as_array().cloned().unwrap_or_default();
            let prompt = last["state"]["runner"]["prompt-state"].clone();
            if actions.iter().any(|a| a["command"] == json!("continue")) {
                send(&mut ws, json!({"type":"action","command":"continue","args":{}})).await;
            } else if let Some(uuid) = prompt["choices"][0]["uuid"].as_str() {
                let uuid = uuid.to_string();
                send(
                    &mut ws,
                    json!({"type":"action","command":"choice","args":{"choice":{"uuid": uuid}}}),
                )
                .await;
            } else {
                break;
            }
            frames = drain(&mut ws, 1200).await;
        }
    }

    let seat = cr::lookup(&token).await.expect("the session is registered");
    let g = seat.game.lock().await;
    let accesses = |side: Side| -> Vec<String> {
        cr::state_json(&g, side)["log"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|l| l["text"].as_str().map(|s| s.to_string()))
            .filter(|t| t.starts_with("Runner: accesses "))
            .collect()
    };
    let mine = accesses(Side::Runner);
    let theirs = accesses(Side::Corp);

    assert_eq!(mine.len(), 2, "one access per central: {mine:#?}");
    assert_eq!(theirs.len(), mine.len(), "both logs saw both accesses");
    // 7.1.2: the Runner is allowed to look at the card they are accessing,
    // and their log says which — for both servers, and still after the run.
    for l in &mine {
        assert!(
            !l.contains("accesses a card"),
            "the Runner sees what they accessed: {l:?}"
        );
    }
    assert!(mine.iter().any(|l| l.contains("from R&D")));
    assert!(mine.iter().any(|l| l.contains("from HQ")));
    // 4.2.2: R&D is hidden from the Corp as much as from the Runner.
    assert_eq!(
        theirs.iter().find(|l| l.contains("from R&D")),
        Some(&"Runner: accesses a card from R&D.".to_string()),
        "the Corp is told an access happened and where, never what: {theirs:#?}"
    );
    // 4.3.2 the other way: HQ is the Corp's own grip, so its copy names it —
    // and names the same card the Runner's does.
    let hq_mine = mine.iter().find(|l| l.contains("from HQ")).unwrap();
    let hq_theirs = theirs.iter().find(|l| l.contains("from HQ")).unwrap();
    assert_eq!(hq_mine, hq_theirs, "an HQ access reads the same to both");
}
