//! The CR lobby end to end: open games over the real ws frames, the same
//! completeness gate as the bot mode, and a two-human game driven from BOTH
//! sockets at once.
//!
//! Five tests, one per promise the lobby makes.
//!
//! * `lobby_create_is_gated_exactly_like_a_bot_start` — SYS-D-12 does not care
//!   which door you came through. While the eternal decks are incomplete,
//!   `lobby-create` refuses with the same message and the same
//!   `cr_readiness` payload `start` refuses with, byte for byte.
//!
//! * `lobby_lists_an_open_seat_and_join_honours_the_gate` — an open seat is
//!   listed with its creator, its side and the deck still going begging; a
//!   join while gated is refused AND puts the seat back, because the refusal
//!   is about the card layer, not about the player who tried.
//!
//! * `two_humans_play_and_neither_sees_the_others_grip` — the invariant that
//!   matters (SYS-S-1), asserted from both sides of the table on every state
//!   either socket is ever sent: no card id in one player's grip appears
//!   ANYWHERE in the other player's payload. Plus the thing that makes a
//!   two-human game a game at all — the opponent's move arriving on your
//!   socket without your having asked.
//!
//! * `a_seat_answers_only_its_own_decisions` — the other half of that: a
//!   command from the seat that was not asked is refused, not applied.
//!
//! * `a_dropped_socket_holds_the_game_and_a_resume_picks_it_back_up` — the
//!   disconnected-opponent state, then both seats resuming, then a concede
//!   both players see.

use futures_util::{SinkExt, StreamExt};
use jinteki_cr::object::{CardType, PrintedCard, Side};
use jinteki_cr::{cards, GameSetup};
use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, cr, decks, guard, lobby, local, mail};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

// ───────────────────────────────────────────────────────────────────────────
// Harness
// ───────────────────────────────────────────────────────────────────────────

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

type Ws = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

async fn open_ws(addr: &str) -> Ws {
    tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .expect("ws connects")
        .0
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

/// Read frames until the socket goes quiet.
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

fn frame<'a>(frames: &'a [Value], ty: &str) -> Option<&'a Value> {
    frames.iter().rev().find(|f| f["type"] == json!(ty))
}

/// Two small decks of cards whose behavior the VM implements — the gate is
/// about the eternal decks, the LOBBY is about two people at one table.
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
    GameSetup {
        corp_deck,
        runner_deck,
        corp_identity: Some(PrintedCard::vanilla("Test Corp", Side::Corp, CardType::Identity)),
        runner_identity: Some(PrintedCard::vanilla(
            "Test Runner",
            Side::Runner,
            CardType::Identity,
        )),
        // CR 1.5.4a: no additional identities brought.
        additional_identities: Default::default(),
        seed,
        shuffle: true,
    }
}

/// A started two-human game and both seats' tokens, without going through the
/// gate — the same call `local.rs` makes, with a deck pair that is entirely
/// implemented today.
async fn two_human_game(seed: u64) -> (String, String) {
    let open = lobby::create("test table", "alice", None, Side::Corp, seed).await;
    let claimed = lobby::claim(&open.id).await.expect("the seat we just opened");
    let started = lobby::start(claimed, "bob", None, small_setup(seed)).await;
    (started.creator_token, started.token)
}

// ───────────────────────────────────────────────────────────────────────────
// Redaction (SYS-S-1), from both sides of the table
// ───────────────────────────────────────────────────────────────────────────

/// Every `cid` anywhere in a payload — the set of cards this player's client
/// could point at, however deeply nested.
fn all_cids(v: &Value) -> HashSet<u64> {
    let mut out = HashSet::new();
    fn walk(v: &Value, out: &mut HashSet<u64>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if k == "cid" {
                        if let Some(n) = x.as_u64() {
                            out.insert(n);
                        }
                    }
                    walk(x, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

fn grip_cids(state: &Value, side: &str) -> HashSet<u64> {
    state[side]["hand"]
        .as_array()
        .map(|a| a.iter().filter_map(|c| c["cid"].as_u64()).collect())
        .unwrap_or_default()
}

/// The shape half: counts travel, cards do not, and the other player's prompt
/// is nobody else's business.
fn assert_redacted(state: &Value, viewer: &str) {
    let opp = if viewer == "corp" { "runner" } else { "corp" };
    assert_eq!(
        state[opp]["hand"].as_array().unwrap().len(),
        0,
        "the opponent's grip must not travel"
    );
    assert!(state[opp]["hand-count"].as_u64().is_some(), "its size is open (10.2.3a)");
    for side in ["corp", "runner"] {
        assert_eq!(state[side]["deck"].as_array().unwrap().len(), 0, "no deck ever travels");
        assert!(state[side]["deck-count"].as_u64().is_some());
    }
    assert!(state[opp]["prompt-state"].is_null(), "the opponent's prompt is not ours");
}

/// The identity half, and the one the second human makes possible: no card
/// that is in MY grip appears ANYWHERE in what was sent to YOU — not as a
/// card, not as a bare id, not nested in a prompt.
fn assert_no_grip_leak(corp_view: &Value, runner_view: &Value) {
    for (mine, theirs, who) in [
        (corp_view, runner_view, "corp"),
        (runner_view, corp_view, "runner"),
    ] {
        let grip = grip_cids(mine, who);
        let seen = all_cids(theirs);
        let leaked: Vec<u64> = grip.intersection(&seen).copied().collect();
        assert!(
            leaked.is_empty(),
            "the {who}'s grip leaked into the opponent's payload: {leaked:?}"
        );
    }
}

/// A seat with a socket in it, and the last state that socket was sent.
/// Frames arrive unbidden in a two-human game (that is the point), so a
/// player remembers its newest state rather than demanding one per read.
struct P {
    ws: Ws,
    side: &'static str,
    last: Value,
    frames: Vec<Value>,
}

impl P {
    async fn sit(addr: &str, side: &'static str, token: &str) -> P {
        let mut ws = open_ws(addr).await;
        send(&mut ws, json!({"type":"resume","token": token})).await;
        let mut p = P { ws, side, last: Value::Null, frames: Vec::new() };
        p.pump(1500).await;
        p
    }
    /// Read everything the server has to say, and keep the newest state.
    async fn pump(&mut self, ms: u64) {
        self.frames = drain(&mut self.ws, ms).await;
        if let Some(f) = self.frames.iter().rev().find(|f| f["type"] == json!("state")) {
            self.last = f["state"].clone();
        }
    }
    async fn send(&mut self, v: Value) {
        send(&mut self.ws, v).await;
    }
    async fn choose(&mut self, uuid: &str) {
        self.send(json!({"type":"action","command":"choice","args":{"choice":{"uuid": uuid}}}))
            .await;
    }
    fn prompt(&self) -> &Value {
        &self.last[self.side]["prompt-state"]
    }
    fn msg(&self) -> String {
        self.prompt()["msg"].as_str().unwrap_or("").to_string()
    }
    fn state(&self) -> &Value {
        &self.last
    }
}

/// Both sockets drained to quiet, then BOTH invariants checked on the pair —
/// the shape half and the identity half, from both sides of the table, on
/// every state either player is ever sent.
async fn settle(corp: &mut P, runner: &mut P) {
    corp.pump(900).await;
    runner.pump(900).await;
    assert_redacted(&corp.last, "corp");
    assert_redacted(&runner.last, "runner");
    assert_no_grip_leak(&corp.last, &runner.last);
}

// ───────────────────────────────────────────────────────────────────────────
// (a) the gate, at the lobby door
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn lobby_create_is_gated_exactly_like_a_bot_start() {
    let addr = spawn_app().await;
    let ready = cr::readiness().ready;

    let mut a = open_ws(&addr).await;
    send(&mut a, json!({"type":"lobby-create","side":"runner","title":"gate test","seed":5})).await;
    let created = drain(&mut a, 1500).await;

    let mut b = open_ws(&addr).await;
    send(&mut b, json!({"type":"start","engine":"cr","side":"runner","seed":5})).await;
    let started = drain(&mut b, 1500).await;

    if ready {
        let w = frame(&created, "lobby-waiting").expect("a seat, taken, waiting");
        assert_eq!(w["lobby"]["side"], json!("runner"));
        assert!(w["token"].as_str().unwrap().len() >= 16, "a resume token for the wait");
        assert!(frame(&started, "session").is_some());
        send(&mut a, json!({"type":"lobby-cancel"})).await;
        drain(&mut a, 800).await;
    } else {
        let lobby_err = frame(&created, "error").expect("SYS-D-12 refuses the lobby too");
        let bot_err = frame(&started, "error").expect("and the bot start");
        assert_eq!(
            lobby_err["error"], bot_err["error"],
            "the two doors refuse in the same words"
        );
        assert_eq!(
            lobby_err["cr_readiness"], bot_err["cr_readiness"],
            "and hand back the same gap, so the UI shows one screen"
        );
        assert_eq!(lobby_err["cr_readiness"]["ready"], json!(false));
        assert!(!lobby_err["cr_readiness"]["missing"].as_array().unwrap().is_empty());
        assert!(
            frame(&created, "lobby-waiting").is_none(),
            "a refused create must not leave a seat lying around"
        );
    }
}

#[tokio::test]
async fn lobby_lists_an_open_seat_and_join_honours_the_gate() {
    let addr = spawn_app().await;
    let open = lobby::create("a game of two halves", "carol", None, Side::Corp, 11).await;

    let mut ws = open_ws(&addr).await;
    send(&mut ws, json!({"type":"lobby-list"})).await;
    let frames = drain(&mut ws, 1200).await;
    let list = frame(&frames, "lobby-list").expect("a list frame");
    let row = list["list"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["gameid"] == json!(open.id))
        .expect("our open game is listed")
        .clone();
    assert_eq!(row["title"], json!("a game of two halves"));
    assert_eq!(row["creator"], json!("carol"));
    assert_eq!(row["side"], json!("corp"), "the side the creator took");
    assert_eq!(row["open-side"], json!("runner"), "the seat going begging");
    assert_eq!(row["open-deck"], json!(cr::ANDROMEDA.title), "…and its deck");
    assert_eq!(row["started"], json!(false));
    assert_eq!(row["format"], json!("eternal"));
    // Bridge-shaped players array, so a lobby row renders with one renderer.
    assert_eq!(row["players"][0]["user"]["username"], json!("carol"));

    // Joining evaluates the gate again, because the gate is per start.
    send(&mut ws, json!({"type":"lobby-join","gameid": open.id})).await;
    let frames = drain(&mut ws, 2000).await;
    if cr::readiness().ready {
        assert!(frame(&frames, "session").is_some(), "both seats filled — a game");
    } else {
        let err = frame(&frames, "error").expect("refused");
        assert!(err["cr_readiness"]["ready"] == json!(false));
        // The seat is still there: the refusal was about the card layer.
        send(&mut ws, json!({"type":"lobby-list"})).await;
        let frames = drain(&mut ws, 1200).await;
        let list = frame(&frames, "lobby-list").unwrap();
        assert!(
            list["list"].as_array().unwrap().iter().any(|r| r["gameid"] == json!(open.id)),
            "a gated join must put the seat back"
        );
    }
    lobby::cancel(&open.token).await;
}

// ───────────────────────────────────────────────────────────────────────────
// (b) two people at one table
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn two_humans_play_and_neither_sees_the_others_grip() {
    let addr = spawn_app().await;
    let (corp_token, runner_token) = two_human_game(20_260_803).await;

    let mut corp = P::sit(&addr, "corp", &corp_token).await;
    let mut runner = P::sit(&addr, "runner", &runner_token).await;

    // Each seat is told which seat it is, and gets its own token back.
    assert_eq!(frame(&runner.frames, "session").unwrap()["side"], json!("runner"));
    assert_eq!(frame(&runner.frames, "session").unwrap()["engine"], json!("cr"));

    settle(&mut corp, &mut runner).await;
    // Each sees a person across the table, named, present.
    assert_eq!(corp.state()["opponent"], json!("bob"));
    assert_eq!(runner.state()["opponent"], json!("alice"));
    assert_eq!(corp.state()["opponent-bot"], json!(false));
    assert_eq!(corp.state()["opponent-connected"], json!(true));
    assert_eq!(runner.state()["opponent-connected"], json!(true));
    assert_eq!(corp.state()["corp"]["user"]["username"], json!("you"));
    assert_eq!(corp.state()["runner"]["user"]["username"], json!("bob"));
    assert_eq!(runner.state()["corp"]["user"]["username"], json!("alice"));

    // 1. The mulligan (CR 1.6.6a) is asked of each player in turn, and NOBODY
    //    answers it for them. Exactly one seat is on the clock at a time.
    for round in 0..2 {
        settle(&mut corp, &mut runner).await;
        let asked_corp = corp.msg().contains("Keep this opening hand");
        let asked_runner = runner.msg().contains("Keep this opening hand");
        assert!(
            asked_corp ^ asked_runner,
            "round {round}: exactly one seat is on the clock — corp {:?} / runner {:?}",
            corp.msg(),
            runner.msg()
        );
        // The seat NOT asked is told what it is waiting for, not left blank.
        let waiting = if asked_corp { runner.prompt() } else { corp.prompt() };
        assert_eq!(waiting["prompt-type"], json!("waiting"));
        let p = if asked_corp { &mut corp } else { &mut runner };
        let uuid = p.prompt()["choices"][0]["uuid"].as_str().unwrap().to_string();
        p.choose(&uuid).await;
    }

    // 2. Both hands kept: the Corp's turn, with the Corp on the clock and the
    //    Runner's socket ALREADY UPDATED — the other seat's move arrives
    //    without having been asked for. That is the whole point of the bus.
    settle(&mut corp, &mut runner).await;
    assert_eq!(corp.state()["active-player"], json!("corp"));
    assert_eq!(
        runner.state()["active-player"],
        json!("corp"),
        "the Runner's socket was told, unprompted"
    );
    // Five kept, plus the mandatory draw that opens a Corp turn (CR 5.6.1a).
    assert_eq!(corp.state()["corp"]["hand"].as_array().unwrap().len(), 6);
    assert_eq!(runner.state()["runner"]["hand"].as_array().unwrap().len(), 5);
    // The other player's hand is a NUMBER on both screens (CR 10.2.3a).
    assert_eq!(corp.state()["runner"]["hand-count"], json!(5), "size open, identity not");
    assert_eq!(runner.state()["corp"]["hand-count"], json!(6));
    assert!(
        runner.msg().contains("Waiting for the Corp"),
        "the waiting variant carries who: {:?}",
        runner.msg()
    );
    assert!(corp.prompt().is_null(), "an action window is the board, not a sheet");

    // 3. The Corp spends its allotment; every state either socket receives
    //    keeps both halves of the invariant (checked inside `settle`).
    let mut credits = corp.state()["corp"]["credit"].as_u64().unwrap();
    for _ in 0..3 {
        corp.send(json!({"type":"action","command":"credit","args":{}})).await;
        settle(&mut corp, &mut runner).await;
        credits += 1;
        assert_eq!(corp.state()["corp"]["credit"].as_u64().unwrap(), credits);
    }

    // 4. Three clicks later the Corp is over its maximum hand size (the
    //    mandatory draw), so it discards — a decision the Runner is shown
    //    nothing of, not even that it is happening to cards it could name.
    settle(&mut corp, &mut runner).await;
    // 5.5.4c is staged and then confirmed, so the wording of its first phase
    // is "choose … to discard"; matched case-insensitively so a sentence that
    // is allowed to be rewritten cannot silently skip this whole step (it
    // did: the `if` stopped matching, the discard was never answered, and the
    // failure surfaced three assertions later as "it is still the Corp's turn").
    if corp.msg().to_lowercase().contains("discard") {
        assert!(runner.msg().contains("Waiting"), "the Runner just waits");
        let uuid = corp.prompt()["choices"][0]["uuid"].as_str().unwrap().to_string();
        corp.choose(&uuid).await;
        settle(&mut corp, &mut runner).await;
    }

    // …and then it is the Runner's turn, on both screens.
    assert_eq!(corp.state()["active-player"], json!("runner"));
    assert_eq!(runner.state()["active-player"], json!("runner"));
    assert!(runner.prompt().is_null(), "the Runner's action window");
    assert_eq!(corp.prompt()["prompt-type"], json!("waiting"));

    // 5. Chat crosses the table verbatim; the game log does not. The Runner
    //    draws, and the Corp's log never learns which card.
    runner.send(json!({"type":"say","msg":"gl hf"})).await;
    settle(&mut corp, &mut runner).await;
    let said = |st: &Value| {
        st["log"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["text"] == json!("gl hf") && l["user"] == json!("bob"))
    };
    assert!(
        said(corp.state()) && said(runner.state()),
        "a chat line lands in both logs, attributed"
    );

    runner.send(json!({"type":"action","command":"draw","args":{}})).await;
    settle(&mut corp, &mut runner).await;
    assert_eq!(runner.state()["runner"]["hand"].as_array().unwrap().len(), 6);
    assert_eq!(corp.state()["runner"]["hand-count"], json!(6));
    let runner_titles: Vec<String> = runner.state()["runner"]["hand"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["title"].as_str().map(str::to_string))
        .collect();
    for line in corp.state()["log"].as_array().unwrap() {
        if line["user"] != json!("__system__") {
            continue; // chat is what the sender chose to give away
        }
        let t = line["text"].as_str().unwrap_or("");
        for title in &runner_titles {
            assert!(
                !t.contains(title.as_str()),
                "the Corp's log named {title}, which is in the Runner's grip: {t}"
            );
        }
    }
}

#[tokio::test]
async fn a_seat_answers_only_its_own_decisions() {
    let addr = spawn_app().await;
    let (corp_token, runner_token) = two_human_game(4242).await;
    let mut corp = P::sit(&addr, "corp", &corp_token).await;
    let mut runner = P::sit(&addr, "runner", &runner_token).await;
    settle(&mut corp, &mut runner).await;

    // The Corp is on the mulligan; the Runner is not, and may not answer it.
    assert!(corp.msg().contains("Keep this opening hand"), "the Corp is asked first");
    let uuid = corp.prompt()["choices"][0]["uuid"].as_str().unwrap().to_string();
    runner.choose(&uuid).await;
    runner.pump(1200).await;
    let err = frame(&runner.frames, "error").expect("the wrong seat is refused");
    assert_eq!(err["error"], json!("it is not your decision right now"));

    // And it was refused, not applied: the Corp still holds the same prompt.
    settle(&mut corp, &mut runner).await;
    assert_eq!(corp.prompt()["choices"][0]["uuid"], json!(uuid));
    assert!(corp.msg().contains("Keep this opening hand"));
}

#[tokio::test]
async fn a_dropped_socket_holds_the_game_and_a_resume_picks_it_back_up() {
    let addr = spawn_app().await;
    let (corp_token, runner_token) = two_human_game(909).await;
    let mut corp = P::sit(&addr, "corp", &corp_token).await;
    let mut runner = P::sit(&addr, "runner", &runner_token).await;
    settle(&mut corp, &mut runner).await;
    assert_eq!(corp.state()["opponent-connected"], json!(true));

    // The Runner's phone sleeps.
    runner.ws.close(None).await.unwrap();
    drop(runner);
    corp.pump(1500).await;
    assert_eq!(corp.state()["opponent-connected"], json!(false), "the Corp is told");
    assert_eq!(corp.state()["opponent"], json!("bob"));

    // The Corp answers its own mulligan anyway; the game is then held at the
    // Runner's empty seat, and the Corp's view says so rather than looking
    // stuck (the waiting prompt, with a reason).
    let uuid = corp.prompt()["choices"][0]["uuid"].as_str().unwrap().to_string();
    corp.choose(&uuid).await;
    corp.pump(1500).await;
    let held = corp.msg();
    assert!(
        held.contains("disconnected") && held.contains("bob"),
        "a held game says so: {held:?}"
    );
    assert_eq!(corp.prompt()["prompt-type"], json!("waiting"));

    // The Runner comes back on a fresh socket with the same token: same seat,
    // same game, its own mulligan waiting for it.
    let mut runner = P::sit(&addr, "runner", &runner_token).await;
    assert_eq!(frame(&runner.frames, "session").unwrap()["side"], json!("runner"));
    settle(&mut corp, &mut runner).await;
    assert_eq!(
        corp.state()["opponent-connected"],
        json!(true),
        "and the Corp is told that too"
    );
    assert!(
        runner.msg().contains("Keep this opening hand"),
        "the Runner's own decision survived the outage: {:?}",
        runner.msg()
    );

    // The Corp's socket survives a refresh too — both seats resume, either
    // side, any number of times.
    let mut corp = P::sit(&addr, "corp", &corp_token).await;
    assert_eq!(frame(&corp.frames, "session").unwrap()["side"], json!("corp"));
    assert_redacted(corp.state(), "corp");
    assert!(
        corp.state()["log"].as_array().unwrap().iter().any(|l| l["text"]
            .as_str()
            .unwrap_or("")
            .contains("CR engine")),
        "the log came back with it"
    );

    // Conceding ends it for both of them.
    corp.send(json!({"type":"action","command":"concede","args":{}})).await;
    corp.pump(1500).await;
    runner.pump(1500).await;
    assert_eq!(corp.state()["winner"], json!("runner"));
    assert_eq!(corp.state()["reason"], json!("conceded"));
    assert_eq!(runner.state()["winner"], json!("runner"), "the other seat sees it too");
    assert!(runner.state()["log"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["text"].as_str().unwrap_or("").contains("concedes")));
}
