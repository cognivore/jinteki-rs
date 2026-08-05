//! The operator's transcript, end to end: play a real CR game over the real
//! socket and read what the server wrote to disk.
//!
//! The complaint this answers is not a player's — it is the operator's. "I
//! really hope you have DETAILED logs on the remote server which can
//! reconstruct things that are going wrong during the trial game I am
//! playing." So the assertions here are exactly the reconstruction: the seed
//! and both deck lists in the opening record, then every kernel change, every
//! decision offered and every answer taken, in one monotonic sequence.
//!
//! It runs in its own process because `transcript::configure` is a
//! once-per-process switch, and the rest of the suite must stay silent.

use futures_util::{SinkExt, StreamExt};
use jinteki_cr::object::{CardType, PrintedCard, Side};
use jinteki_cr::{cards, GameSetup};
use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, cr, decks, guard, local, mail, transcript};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

type Ws = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

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
        additional_identities: Default::default(),
        extra_cards: Default::default(),
        seed,
        shuffle: true,
    }
}

#[tokio::test]
async fn a_played_game_leaves_a_reconstructable_transcript_on_disk() {
    let dir = std::env::temp_dir()
        .join(format!("jinteki-cr-transcript-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let tdir = transcript::configure(&dir).expect("a writable data dir");
    assert_eq!(tdir, dir.join("transcripts"), "transcripts live beside the database");

    let addr = spawn_app().await;
    // The game is created AFTER transcripts are configured — a game started
    // by a process that never configured any writes nothing at all.
    let token = cr::create_session(small_setup(20_260_804), Side::Runner, 0).await;
    let seat = cr::lookup(&token).await.expect("the session is registered");
    let key = seat.key.clone();
    drop(seat);

    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws/local"))
        .await
        .expect("ws connects");
    send(&mut ws, json!({"type":"resume","token": token})).await;
    let frames = drain(&mut ws, 1500).await;

    // Keep the opening hand, then spend the turn.
    let st = frames
        .iter()
        .rev()
        .find(|f| f["type"] == json!("state"))
        .expect("a state frame")["state"]
        .clone();
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
    // Five clicks: the Runner's whole turn, the Corp's bot turn in between,
    // and the start of the next — enough that both seats appear as answerers.
    for _ in 0..5 {
        send(&mut ws, json!({"type":"action","command":"credit","args":{}})).await;
        drain(&mut ws, 800).await;
    }

    // ── what is on disk ────────────────────────────────────────────────
    let path = tdir.join(format!("{key}.jsonl"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no transcript at {}: {e}", path.display()));
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("plain JSONL, one record per line"))
        .collect();
    assert!(
        lines.len() > 60,
        "two turns of a real game are a lot of records ({})",
        lines.len()
    );

    // One monotonic sequence, every line stamped with the game it belongs to.
    for (i, l) in lines.iter().enumerate() {
        assert_eq!(l["seq"], json!(i as u64 + 1), "monotonic: {l}");
        assert_eq!(l["game"], json!(&key), "every line names its game");
        assert!(l["ms"].as_u64().unwrap() > 0, "and when it happened");
    }

    // The opening record is what the game was BUILT from — with the seed and
    // both deck lists, the answers below replay it.
    let start = &lines[0];
    assert_eq!(start["kind"], json!("start"));
    assert_eq!(start["seed"], json!(20_260_804u64));
    assert_eq!(start["decks"]["corp_deck"].as_array().unwrap().len(), 24);
    assert_eq!(start["decks"]["runner_deck"].as_array().unwrap().len(), 24);
    assert_eq!(start["decks"]["runner_identity"], json!("Test Runner"));

    let of_kind = |k: &str| -> Vec<&Value> {
        lines.iter().filter(|l| l["kind"] == json!(k)).collect()
    };
    // Every kernel change, unfiltered — including the ones the player's log
    // deliberately swallows.
    let changes = of_kind("change");
    assert!(changes.len() > 40, "the whole change stream: {}", changes.len());
    assert!(
        changes.iter().any(|l| l["detail"].as_str().unwrap().starts_with("ClickSpent")),
        "including the bookkeeping no player would read"
    );
    assert!(changes
        .iter()
        .any(|l| l["detail"].as_str().unwrap().starts_with("CardDrawn")));

    // Every decision offered, and every answer taken, with whose it was and
    // whether a person or the bot answered it.
    let decisions = of_kind("decision");
    let answers = of_kind("answer");
    assert!(decisions.len() >= 8, "decisions: {}", decisions.len());
    assert!(answers.len() >= 8, "answers: {}", answers.len());
    // The offer is recorded WHOLE — every option the machine put on the
    // table, not just the one taken. A game that went wrong usually went
    // wrong in what was offered.
    assert!(
        decisions
            .iter()
            .any(|l| l["detail"].as_str().unwrap().contains("BasicRun { server: Hq }")),
        "a decision carries its full option list"
    );
    assert!(
        decisions.iter().any(|l| l["detail"] == json!("Mulligan")),
        "the first decision of the game is in there"
    );
    assert!(
        answers.iter().any(|l| l["by"] == json!("human") && l["side"] == json!("runner")),
        "the person's own answers are attributed to them"
    );
    assert!(
        answers.iter().any(|l| l["by"] == json!("bot")),
        "and the bot's to it"
    );
    // A decision is always followed by its answer, so the pairing an operator
    // reads the file for is actually there.
    assert!(answers.len() <= decisions.len(), "no answer without a decision");

    let _ = std::fs::remove_dir_all(&dir);
}
