//! "Play anyone" over the wire. Alone in its own binary ON PURPOSE: the
//! open-seats registry is process-global, and an autopairing joiner claims
//! whatever compatible seat is oldest — a neighbouring test's seat included.
//! Every other lobby test joins by id, which cannot be raced this way.
//!
//! Which OLDEST compatible seat wins, that sides must oppose, and that your
//! own seat is never picked are `lobby::tests::autopair_*`'s pure-function
//! ground; this file is about the wire behavior — pairing when a seat fits,
//! opening one when nothing does.

use futures_util::{SinkExt, StreamExt};
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

#[tokio::test]
async fn play_anyone_pairs_over_the_wire_or_opens_a_seat() {
    if !cr::readiness().ready {
        eprintln!(
            "gate closed ({}) — autopair wire flow not exercised",
            cr::readiness().fraction()
        );
        return;
    }
    let addr = spawn_app().await;

    // An UNROPED corp host sits down first (oldest of all). Its label says
    // so, it is joinable by hand — and autopair must never volunteer
    // anyone into it, because nothing would stop it stalling forever.
    let mut u = open_ws(&addr).await;
    send(
        &mut u,
        json!({"type":"lobby-create","side":"corp","title":"house rules, no rope",
               "timing":{"main_clock_secs":1500}}),
    )
    .await;
    let uf = drain(&mut u, 1500).await;
    let uw = frame(&uf, "lobby-waiting").expect("the unroped seat waits like any other");
    assert_eq!(uw["lobby"]["timing-label"], json!("25m"), "no rope in the label");
    let unroped_gameid = uw["lobby"]["gameid"].clone();

    // A corp-deck joiner with nobody compatible waiting: "play anyone"
    // opens a seat on the side the deck plays rather than failing. (The
    // unroped table does not count even side-wise; this proves the
    // nothing-fits path.)
    let mut b = open_ws(&addr).await;
    send(&mut b, json!({"type":"lobby-anyone","decks":{"corp":"mezzie-making-stars"}})).await;
    let waited = drain(&mut b, 1500).await;
    let wait = frame(&waited, "lobby-waiting")
        .expect("no compatible seat: play-anyone waits instead");
    assert_eq!(wait["lobby"]["side"], json!("corp"), "seated on the side the deck plays");
    assert_eq!(wait["lobby"]["deck"], json!("mezzie-making-stars"));
    assert_eq!(
        wait["lobby"]["timing-label"], json!("30m + rope"),
        "a seat autopair opens is the default mode: roped"
    );
    let host_gameid = wait["lobby"]["gameid"].clone();

    // A second corp-deck "play anyone" cannot take that table — a corp host
    // needs a runner joiner, sides must oppose — so it opens ANOTHER seat.
    let mut c2 = open_ws(&addr).await;
    send(&mut c2, json!({"type":"lobby-anyone","decks":{"corp":null}})).await;
    let c2f = drain(&mut c2, 1500).await;
    let w2 = frame(&c2f, "lobby-waiting").expect("two corp decks never pair");
    assert_ne!(w2["lobby"]["gameid"], host_gameid);
    send(&mut c2, json!({"type":"lobby-cancel"})).await;
    drain(&mut c2, 800).await;

    // A runner-deck "play anyone" would fit BOTH corp hosts. The unroped
    // one is OLDER — and is skipped anyway: roped is the autopair floor.
    let mut c = open_ws(&addr).await;
    send(&mut c, json!({"type":"lobby-anyone","decks":{"runner":"mezzie-andromeda"}})).await;
    let paired = drain(&mut c, 1500).await;
    let cp = frame(&paired, "lobby-pairing").expect("autopaired");
    assert_eq!(
        cp["pairing"]["id"], host_gameid,
        "…with the ROPED host, not the older unroped one"
    );
    let me = cp["pairing"]["seats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["you"] == json!(true))
        .unwrap()
        .clone();
    assert_eq!(me["side"], json!("runner"), "the joiner takes the opposing side");
    assert_eq!(me["deck"], json!("mezzie-andromeda"));
    let b_frames = drain(&mut b, 1500).await;
    assert!(frame(&b_frames, "lobby-pairing").is_some(), "the host is at the table too");

    // The unroped table was left exactly where it was: on the list, for
    // anyone who reads its label and joins it on purpose.
    let mut w = open_ws(&addr).await;
    send(&mut w, json!({"type":"lobby-list"})).await;
    let frames = drain(&mut w, 1200).await;
    let list = frame(&frames, "lobby-list").unwrap();
    assert!(
        list["list"].as_array().unwrap().iter().any(|r| r["gameid"] == unroped_gameid),
        "the unroped seat still stands, joinable by hand"
    );
}
