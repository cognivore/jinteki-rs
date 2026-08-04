//! A dev harness for the CR adapter: the whole app, with one CR session
//! already sitting in the registry, built from SMALL ALL-COMPLETE decks.
//!
//! The eternal decks are behind the completeness gate (SYS-D-12) and will stay
//! there until the card layer closes — but the adapter, the redaction shim and
//! the board renderer are finished now, and this is how you look at them.
//!
//!     cargo run -p jinteki-server --example cr_demo
//!     open http://127.0.0.1:7788/cr-demo
//!
//! `/cr-demo` hands the browser the session token and bounces to `/`, which
//! resumes it exactly as a refresh would. Nothing here is reachable from the
//! shipped binary: examples are not compiled into it.

use jinteki_cr::object::{CardType, PrintedCard, Side};
use jinteki_cr::{cards, GameSetup};
use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, cr, decks, guard, local, mail};
use std::sync::Arc;

fn setup(seed: u64) -> GameSetup {
    let mut corp_deck = Vec::new();
    for _ in 0..6 {
        corp_deck.push(cards::hedge_fund());
        corp_deck.push(cards::ice_wall());
        corp_deck.push(cards::enigma());
        corp_deck.push(cards::hostile_takeover());
        corp_deck.push(cards::pad_campaign());
        corp_deck.push(cards::project_beale());
    }
    let mut runner_deck = Vec::new();
    for _ in 0..8 {
        runner_deck.push(cards::sure_gamble());
        runner_deck.push(cards::easy_mark());
        runner_deck.push(cards::diesel());
        runner_deck.push(cards::corroder());
        runner_deck.push(cards::magnum_opus());
    }
    GameSetup {
        corp_deck,
        runner_deck,
        corp_identity: Some(PrintedCard::vanilla("Demo Corp", Side::Corp, CardType::Identity)),
        runner_identity: Some(PrintedCard::vanilla(
            "Demo Runner",
            Side::Runner,
            CardType::Identity,
        )),
        // CR 1.5.4a: no additional identities brought.
        additional_identities: Default::default(),
        seed,
        shuffle: true,
    }
}

#[tokio::main]
async fn main() {
    let ui_dir = std::env::var("JINTEKI_UI_DIR").unwrap_or_else(|_| "ui".into());
    let port: u16 = std::env::var("JINTEKI_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7788);
    let side = match std::env::var("JINTEKI_SIDE").as_deref() {
        Ok("corp") => Side::Corp,
        _ => Side::Runner,
    };
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
    let token = cr::create_session(setup(20_260_803), side, 300).await;
    println!("CR demo session: {token} (you are the {side:?})");
    println!("open http://127.0.0.1:{port}/cr-demo");

    let handoff = format!(
        "<!doctype html><meta charset=utf-8><script>\
         localStorage.setItem('jinteki_local', JSON.stringify(\
           {{token:'{token}', side:'{}', engine:'cr'}}));\
         location.replace('/');</script>resuming…",
        match side {
            Side::Corp => "corp",
            Side::Runner => "runner",
        }
    );
    let app = axum::Router::new()
        .route("/ws/local", axum::routing::any(ws_local))
        .route(
            "/cr-demo",
            axum::routing::get(move || {
                let h = handoff.clone();
                async move { axum::response::Html(h) }
            }),
        )
        .merge(api::router())
        .fallback_service(tower_http::services::ServeDir::new(ui_dir))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_local(
    ws: axum::extract::ws::WebSocketUpgrade,
    axum::extract::State(st): axum::extract::State<AppState>,
) -> axum::response::Response {
    let db = st.db.clone();
    ws.on_upgrade(move |socket| local::handle(socket, db, None))
}
