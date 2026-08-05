//! The ready check and the countdown, end to end: a join puts two people at
//! one table, both must say ready, the SERVER counts 5,4,3,2,1, and only the
//! count reaching zero makes a game — through the same code path every
//! two-human game has always been created through.
//!
//! * `the_countdown_finish_creates_the_game_through_the_usual_path` — the
//!   gate-free half of the promise, on small fully-implemented decks: a
//!   finished countdown turns both pairing tokens into CR seats on opposite
//!   sides of ONE game, and the ready check's deck keys ride the pairing.
//!
//! * `an_unready_mid_count_cancels_and_the_table_survives` — the countdown
//!   is cancellable the whole way down: an unready 1.2s in stops the count,
//!   no game is ever created, and the table stands exactly as it was (both
//!   players still seated, count withdrawn).
//!
//! * `a_join_is_a_ready_check_and_the_count_drops_both_into_the_game` — the
//!   whole flow over real sockets (gate permitting): create with a deck key,
//!   join with a deck key, both ready, ticks arriving on BOTH sockets
//!   unbidden, then a session and a state each, in the right seats.
//!
//! * `a_dying_socket_withdraws_its_lobby` (gate permitting) — an open seat
//!   whose socket dies stops being an invitation; a paired joiner's death
//!   puts the creator back on the open list.

use futures_util::{SinkExt, StreamExt};
use jinteki_cr::object::{CardType, PrintedCard, Side};
use jinteki_cr::{cards, GameSetup};
use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, cr, decks, guard, lobby, local, mail};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

// ───────────────────────────────────────────────────────────────────────────
// Harness (the same shape as cr_lobby.rs)
// ───────────────────────────────────────────────────────────────────────────

async fn test_db() -> Arc<Db> {
    let db = Arc::new(Db::open_in_memory().expect("db"));
    {
        let conn = db.lock().await;
        auth::ensure_system_user(&conn).unwrap();
        decks::seed_starter_decks(&conn).unwrap();
    }
    db
}

async fn spawn_app() -> String {
    let db = test_db().await;
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

/// Every countdown value a socket was sent, in arrival order.
fn counts(frames: &[Value]) -> Vec<u64> {
    frames
        .iter()
        .filter(|f| f["type"] == json!("lobby-pairing"))
        .filter_map(|f| f["pairing"]["count"].as_u64())
        .collect()
}

/// Two small decks of cards whose behavior the VM implements — the ready
/// check is about two people deciding to play, not about the eternal decks.
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

/// A pairing built directly (no sockets): create, claim, sit down.
async fn table(seed: u64, host_deck: Option<&str>, join_deck: Option<&str>) -> lobby::Pairing {
    let open = lobby::create(
        "pairing test",
        "hostess",
        None,
        Side::Corp,
        host_deck.map(String::from),
        jinteki_server::timing::TimingConfig::default(),
        seed,
    )
    .await;
    let claimed = lobby::claim(&open.id).await.expect("our own seat");
    lobby::pair(claimed, "guest", None, join_deck.map(String::from)).await
}

// ───────────────────────────────────────────────────────────────────────────
// (a) the countdown's finish is the one true start (gate-free)
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_countdown_finish_creates_the_game_through_the_usual_path() {
    let p = table(31_337, Some("mezzie-making-stars"), Some("mezzie-andromeda")).await;
    let corp_token = p.seat(Side::Corp).token.clone();
    let runner_token = p.seat(Side::Runner).token.clone();

    // The deck keys chosen at the door ride the pairing, per seat.
    let view = p.to_json(&runner_token);
    let seats = view["seats"].as_array().unwrap();
    assert_eq!(seats[0]["side"], json!("corp"));
    assert_eq!(seats[0]["deck"], json!("mezzie-making-stars"));
    assert_eq!(seats[1]["deck"], json!("mezzie-andromeda"));
    assert_eq!(seats[1]["you"], json!(true), "the viewer knows which seat is theirs");
    assert_eq!(seats[0]["you"], json!(false));

    // Nobody is a game while the check is unresolved.
    assert!(cr::lookup(&corp_token).await.is_none());

    // Both ready → (the countdown would run here) → the finish.
    assert!(matches!(
        lobby::set_ready(&corp_token, true).await,
        lobby::ReadyOutcome::Updated(_)
    ));
    assert!(matches!(
        lobby::set_ready(&runner_token, true).await,
        lobby::ReadyOutcome::BothReadyNow(_)
    ));
    let started = lobby::finish_pairing_with(&p.id, 0, small_setup(31_337))
        .await
        .expect("both ready, generation unmoved: the game");

    // Both tokens are now CR seats at ONE game, on opposite sides — the
    // exact registry a resume or a nudge consults.
    let corp_seat = cr::lookup(&corp_token).await.expect("the creator's seat");
    let runner_seat = cr::lookup(&runner_token).await.expect("the joiner's seat");
    assert_eq!(corp_seat.side, Side::Corp);
    assert_eq!(runner_seat.side, Side::Runner);
    assert_eq!(corp_seat.key, runner_seat.key, "one game, two seats");
    assert_eq!(corp_seat.key, started.key);

    // The lobby's timing landed on the game verbatim — the hand-off the
    // in-game enforcement reads (CrGame::timing).
    assert_eq!(
        corp_seat.game.lock().await.timing,
        jinteki_server::timing::TimingConfig::default(),
        "the table's timing is the game's timing"
    );

    // And the table is gone: a finished check cannot be finished again.
    assert!(lobby::pairing_snapshot(&p.id).await.is_none());
    assert!(
        lobby::finish_pairing_with(&p.id, 0, small_setup(31_337)).await.is_none(),
        "a pairing starts at most one game"
    );
}

// ───────────────────────────────────────────────────────────────────────────
// (b) unready cancels the count (gate-free, real timers)
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_unready_mid_count_cancels_and_the_table_survives() {
    let db = test_db().await;
    let p = table(60_601, None, None).await;
    let corp_token = p.seat(Side::Corp).token.clone();
    let runner_token = p.seat(Side::Runner).token.clone();

    lobby::set_ready(&corp_token, true).await;
    let lobby::ReadyOutcome::BothReadyNow(id) = lobby::set_ready(&runner_token, true).await
    else {
        panic!("the second ready is the transition")
    };
    lobby::spawn_countdown(id.clone(), db).await;

    // 1.2s in the count is running (5, then 4)…
    tokio::time::sleep(Duration::from_millis(1200)).await;
    let mid = lobby::pairing_snapshot(&p.id).await.expect("still a table");
    let n = mid.count.expect("the server is counting");
    assert!(n <= 5 && n >= 3, "1.2s in the count reads 4ish, got {n}");

    // …until the corp thinks better of it.
    lobby::set_ready(&corp_token, false).await;
    let after = lobby::pairing_snapshot(&p.id).await.expect("the table survives");
    assert_eq!(after.count, None, "the count is withdrawn the moment anyone unreadies");

    // Long after the count would have finished: still no game, both seats
    // still at the table, the joiner still ready, the corp not.
    tokio::time::sleep(Duration::from_millis(4500)).await;
    assert!(cr::lookup(&corp_token).await.is_none(), "no game was started");
    assert!(cr::lookup(&runner_token).await.is_none());
    let survives = lobby::pairing_snapshot(&p.id).await.expect("both still seated");
    assert_eq!(survives.count, None);
    assert!(!survives.seat(Side::Corp).ready);
    assert!(survives.seat(Side::Runner).ready);

    lobby::leave_pairing(&runner_token).await;
    lobby::cancel(&corp_token).await;
}

// ───────────────────────────────────────────────────────────────────────────
// (c) the whole flow over real sockets (gate permitting)
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_join_is_a_ready_check_and_the_count_drops_both_into_the_game() {
    if !cr::readiness().ready {
        // The two doors' identical refusal is cr_lobby.rs's ground; the
        // ready check cannot be reached over the wire while the gate holds.
        eprintln!("gate closed ({}) — wire flow not exercised", cr::readiness().fraction());
        return;
    }
    let addr = spawn_app().await;

    // The host sits down as corp, deck key riding along.
    let mut a = open_ws(&addr).await;
    send(
        &mut a,
        json!({"type":"lobby-create","side":"corp","title":"count test","seed":77,
               "deck":"mezzie-making-stars"}),
    )
    .await;
    let created = drain(&mut a, 1500).await;
    let wait = frame(&created, "lobby-waiting").expect("a seat, taken, waiting");
    let gameid = wait["lobby"]["gameid"].as_str().unwrap().to_string();
    assert_eq!(wait["lobby"]["deck"], json!("mezzie-making-stars"));

    // The joiner takes the runner seat: a table for two, nobody ready.
    let mut b = open_ws(&addr).await;
    send(
        &mut b,
        json!({"type":"lobby-join","gameid": gameid, "deck":"mezzie-andromeda"}),
    )
    .await;
    let joined = drain(&mut b, 1500).await;
    let bp = frame(&joined, "lobby-pairing").expect("the joiner is seated");
    assert_eq!(bp["pairing"]["count"], Value::Null);
    assert_eq!(
        bp["pairing"]["timing-label"], json!("30m + rope"),
        "the joiner consents to the timing they can see"
    );
    let b_you: Vec<&Value> = bp["pairing"]["seats"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["you"] == json!(true))
        .collect();
    assert_eq!(b_you.len(), 1);
    assert_eq!(b_you[0]["side"], json!("runner"), "the joiner takes the free side");
    assert_eq!(b_you[0]["deck"], json!("mezzie-andromeda"));

    // The host's socket was told unbidden — that is the bus's whole point.
    let a_frames = drain(&mut a, 1500).await;
    let ap = frame(&a_frames, "lobby-pairing").expect("the creator sees the table");
    assert!(frame(&a_frames, "session").is_none(), "nothing started yet");
    assert_eq!(
        ap["pairing"]["seats"].as_array().unwrap().iter()
            .find(|s| s["you"] == json!(true)).unwrap()["side"],
        json!("corp")
    );

    // One ready is not a countdown.
    send(&mut a, json!({"type":"lobby-ready","ready":true})).await;
    let one = drain(&mut b, 1200).await;
    let p1 = frame(&one, "lobby-pairing").expect("the toggle repaints both tables");
    assert_eq!(p1["pairing"]["count"], Value::Null, "one ready does not count");
    assert_eq!(
        p1["pairing"]["seats"].as_array().unwrap().iter()
            .find(|s| s["side"] == json!("corp")).unwrap()["ready"],
        json!(true)
    );

    // Both ready: the server speaks 5,4,3,2,1 — one tick a second, to both
    // sockets — and then each seat lands in the game.
    send(&mut b, json!({"type":"lobby-ready","ready":true})).await;
    let a_run = drain(&mut a, 7000).await;
    let b_run = drain(&mut b, 2500).await;
    for (who, run) in [("creator", &a_run), ("joiner", &b_run)] {
        let ticks = counts(run);
        assert!(
            ticks.windows(2).all(|w| w[0] > w[1]),
            "{who}'s ticks descend: {ticks:?}"
        );
        assert_eq!(ticks.first(), Some(&5), "{who} heard the count begin: {ticks:?}");
        assert_eq!(ticks.last(), Some(&1), "{who} heard the count out: {ticks:?}");
        let sess = frame(run, "session").expect("the count reaching zero starts the game");
        assert_eq!(sess["engine"], json!("cr"));
        assert!(frame(run, "state").is_some(), "and a first state arrives");
    }
    assert_eq!(
        frame(&a_run, "session").unwrap()["side"], json!("corp"),
        "the creator lands in the seat they chose"
    );
    assert_eq!(frame(&b_run, "session").unwrap()["side"], json!("runner"));
}

#[tokio::test]
async fn a_dying_socket_withdraws_its_lobby() {
    if !cr::readiness().ready {
        eprintln!("gate closed ({}) — socket-death wire flow not exercised", cr::readiness().fraction());
        return;
    }
    let addr = spawn_app().await;

    // An open seat dies with its socket: a dead socket's invitation is
    // withdrawn rather than left to catch a joiner nobody will play — but
    // only after the reconnect grace, because a refresh is also a dead
    // socket and must NOT cost the seat.
    let mut a = open_ws(&addr).await;
    send(&mut a, json!({"type":"lobby-create","side":"corp","title":"doomed seat","seed":9})).await;
    let created = drain(&mut a, 1500).await;
    let gameid = frame(&created, "lobby-waiting").unwrap()["lobby"]["gameid"]
        .as_str().unwrap().to_string();
    a.close(None).await.unwrap();
    drop(a);
    // Inside the grace the seat still stands (a refresh would find it)…
    let mut w = open_ws(&addr).await;
    send(&mut w, json!({"type":"lobby-list"})).await;
    let frames = drain(&mut w, 1000).await;
    let list = frame(&frames, "lobby-list").unwrap();
    assert!(
        list["list"].as_array().unwrap().iter().any(|r| r["gameid"] == json!(gameid)),
        "inside the grace the seat survives (refreshes must not lose it)"
    );
    // …and past it, with nobody having come back, it is withdrawn.
    tokio::time::sleep(lobby::ABANDON_GRACE + Duration::from_millis(700)).await;
    send(&mut w, json!({"type":"lobby-list"})).await;
    let frames = drain(&mut w, 1200).await;
    let list = frame(&frames, "lobby-list").unwrap();
    assert!(
        !list["list"].as_array().unwrap().iter().any(|r| r["gameid"] == json!(gameid)),
        "the dead socket's seat is gone"
    );

    // A paired joiner dying puts the creator back on the open list, still
    // holding the same lobby.
    let mut host = open_ws(&addr).await;
    send(&mut host, json!({"type":"lobby-create","side":"corp","title":"resilient seat","seed":8})).await;
    let created = drain(&mut host, 1500).await;
    let gameid = frame(&created, "lobby-waiting").unwrap()["lobby"]["gameid"]
        .as_str().unwrap().to_string();
    let mut joiner = open_ws(&addr).await;
    send(&mut joiner, json!({"type":"lobby-join","gameid": gameid})).await;
    drain(&mut joiner, 1200).await;
    let host_frames = drain(&mut host, 1200).await;
    assert!(frame(&host_frames, "lobby-pairing").is_some(), "a table for two");
    joiner.close(None).await.unwrap();
    drop(joiner);
    // The joiner gets the same grace a refresh would need; past it, the
    // creator is back on the open list, same lobby, same token.
    let host_frames =
        drain(&mut host, lobby::ABANDON_GRACE.as_millis() as u64 + 1500).await;
    let back = frame(&host_frames, "lobby-waiting")
        .expect("the joiner died: the creator waits again");
    assert_eq!(back["lobby"]["gameid"], json!(gameid), "same lobby, same seat");
    send(&mut host, json!({"type":"lobby-cancel"})).await;
    drain(&mut host, 800).await;
}
