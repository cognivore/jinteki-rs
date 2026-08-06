//! A GAME SEED SURVIVES THE ROUND TRIP.
//!
//! A seed is a `u64`. JavaScript's only number is a double, so a browser
//! cannot hold one past `2^53` — and the client used to `parseInt` the seed
//! box, which turned `9661175140325481871` into `9661175140325482000` and
//! started a different game from the one whose seed its player had pasted in
//! to replay. The wire therefore carries the seed as a STRING and the server
//! is the only thing that parses it (`local::seed_from_wire`).
//!
//! These tests hold that promise from the outside, over the real socket:
//!
//! * `a_nineteen_digit_seed_survives_the_round_trip` sends the exact seed
//!   that broke, as the client now sends it, and reads it back out of the
//!   game's own log. All nineteen digits, or the test fails.
//!
//! * `a_seed_that_is_not_a_seed_is_refused_not_mangled` sends the things a
//!   seed box can hold that are not seeds — letters, a negative, a float,
//!   one past `u64::MAX` — and requires a refusal for each. A start that
//!   quietly picked its own number instead would pass no test here.
//!
//! * `an_absent_or_blank_seed_is_still_a_random_game` — the box says
//!   "(optional)" and it means it.

use futures_util::{SinkExt, StreamExt};
use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, decks, guard, local, mail};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// The seed from the report: nineteen digits, and the last four of them are
/// exactly what a double loses.
const BIG_SEED: &str = "9661175140325481871";

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

async fn open(addr: &str) -> Ws {
    tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .expect("ws connects")
        .0
}

async fn send(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

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

/// Every line the game has written about itself, from the last state pushed.
fn log_lines(frames: &[Value]) -> Vec<String> {
    frames
        .iter()
        .rev()
        .find(|f| f["type"] == json!("state"))
        .unwrap_or_else(|| panic!("no state frame in {frames:#?}"))["state"]["log"]
        .as_array()
        .expect("the state carries a log")
        .iter()
        .map(|e| e["text"].as_str().unwrap_or_default().to_string())
        .collect()
}

// ───────────────────────────────────────────────────────────────────────────
// The round trip
// ───────────────────────────────────────────────────────────────────────────

/// Nineteen digits in, nineteen digits back. The game says its own seed in
/// its opening line, and that sentence is the only report of it a player
/// ever gets — so it is the thing worth asserting on.
#[tokio::test]
async fn a_nineteen_digit_seed_survives_the_round_trip() {
    // The seed is past what a double can hold: the failure mode this test
    // exists for is real, and the constant proves it.
    assert!(
        BIG_SEED.parse::<u64>().unwrap() as f64 as u64 != BIG_SEED.parse::<u64>().unwrap(),
        "pick a seed a double actually loses, or this test proves nothing"
    );

    let addr = spawn_app().await;
    let mut ws = open(&addr).await;
    // Exactly what the client now sends: the digits, as a JSON string.
    send(&mut ws, json!({"type":"start","side":"runner","seed": BIG_SEED})).await;
    let frames = drain(&mut ws, 1500).await;
    let lines = log_lines(&frames);
    assert!(
        lines.iter().any(|l| l == &format!("Local game vs bot, seed {BIG_SEED}.")),
        "the game must report the seed it was given, digit for digit: {lines:?}"
    );
}

/// The same seed as a JSON number still works — `curl`, the old clients and
/// this suite all speak that — and it lands on the same game.
#[tokio::test]
async fn a_seed_sent_as_a_number_lands_on_the_same_game() {
    let addr = spawn_app().await;
    let n: u64 = BIG_SEED.parse().unwrap();
    let mut ws = open(&addr).await;
    send(&mut ws, json!({"type":"start","side":"runner","seed": n})).await;
    let lines = log_lines(&drain(&mut ws, 1500).await);
    assert!(
        lines.iter().any(|l| l == &format!("Local game vs bot, seed {BIG_SEED}.")),
        "a numeric seed is the same seed: {lines:?}"
    );
}

/// Nonsense refuses the start and says so. It must NOT start a game with
/// some other number — that is the whole complaint.
#[tokio::test]
async fn a_seed_that_is_not_a_seed_is_refused_not_mangled() {
    let addr = spawn_app().await;
    for bad in [
        json!("banana"),
        json!("-1"),
        json!("1.5"),
        json!("12 34"),
        // One past u64::MAX, as a string and as a number.
        json!("18446744073709551616"),
        json!(-1),
        json!(1.5),
        json!(true),
    ] {
        let mut ws = open(&addr).await;
        send(&mut ws, json!({"type":"start","side":"runner","seed": bad})).await;
        let frames = drain(&mut ws, 800).await;
        let err = frames.iter().find(|f| f["type"] == json!("error"));
        assert!(
            err.is_some(),
            "seed {bad} must be refused out loud, not replaced: {frames:#?}"
        );
        assert!(
            err.unwrap()["error"].as_str().unwrap().contains("seed must be a whole number"),
            "the refusal must say what a seed is: {err:?}"
        );
        assert!(
            !frames.iter().any(|f| f["type"] == json!("session")),
            "a refused seed must not start a game anyway: {frames:#?}"
        );
    }
}

/// The seed box says "(optional)". An absent, null or blank one is a random
/// game, exactly as it always was.
#[tokio::test]
async fn an_absent_or_blank_seed_is_still_a_random_game() {
    let addr = spawn_app().await;
    for none in [None, Some(json!(null)), Some(json!("")), Some(json!("  "))] {
        let mut msg = json!({"type":"start","side":"runner"});
        if let Some(s) = none.clone() {
            msg["seed"] = s;
        }
        let mut ws = open(&addr).await;
        send(&mut ws, msg).await;
        let frames = drain(&mut ws, 1500).await;
        assert!(
            frames.iter().any(|f| f["type"] == json!("session")),
            "seed {none:?} is no seed at all, and no seed is a game: {frames:#?}"
        );
        let lines = log_lines(&frames);
        assert!(
            lines.iter().any(|l| l.starts_with("Local game vs bot, seed ")),
            "the game still reports whichever seed it picked: {lines:?}"
        );
    }
}
