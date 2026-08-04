//! jinteki-server: serves the mobile UI, hosts local games against the
//! random-walk bot, bridges to a reference jinteki.net server for parity
//! play, and carries the native account/deck subsystem (accounts, magic-link
//! claims, decklists, library, NRDB import — ACCOUNTS-AND-DECKS.md).
//!
//! Games ride one WebSocket per session (JSON text frames with a `type`
//! field); auth and decks ride plain HTTP JSON under /api/*.

use jinteki_server::{api, auth, bridge, db, decks, guard, lobby, local, mail, transcript};

use axum::{
    extract::ws::WebSocketUpgrade,
    http::HeaderMap,
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::ServeDir;

/// The build this binary was compiled from (nix passes it; "dev" otherwise).
fn build_rev() -> &'static str {
    option_env!("JINTEKI_BUILD_REV").unwrap_or("dev")
}

/// Serve index.html with its asset URLs stamped with the build id. The file on
/// disk carries the neutral `?v=dev`; the response carries `?v=<rev>`, so a
/// deploy changes every asset URL and no stale copy can be reused. Read per
/// request (one small file) so editing the UI in dev needs no restart.
async fn index_page(dir: Arc<str>) -> axum::response::Response {
    let path = std::path::Path::new(dir.as_ref()).join("index.html");
    match std::fs::read_to_string(&path) {
        Ok(html) => (
            [(axum::http::header::CACHE_CONTROL, "no-store")],
            axum::response::Html(html.replace("?v=dev", &format!("?v={}", build_rev()))),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("index.html unreadable at {}: {e}", path.display());
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "no UI").into_response()
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let ui_dir = std::env::var("JINTEKI_UI_DIR").unwrap_or_else(|_| "ui".into());
    let port: u16 = std::env::var("JINTEKI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7787);

    // Storage: one SQLite file under JINTEKI_DATA_DIR (the vacationvm state
    // dir in deployment, ./data in development).
    let data_dir = PathBuf::from(std::env::var("JINTEKI_DATA_DIR").unwrap_or_else(|_| "data".into()));
    let db_path = data_dir.join("jinteki.db");
    let db = Arc::new(db::Db::open(&db_path).expect("open jinteki.db"));
    tracing::info!("database at {}", db_path.display());
    // Per-game debug transcripts, beside the database. OFF unless a process
    // asks for them, which is why this call lives in the binary and not in
    // the library: the test suite never writes one. They are never served —
    // no route reads this directory (see `transcript`).
    match transcript::configure(&data_dir) {
        Some(dir) => tracing::info!("game transcripts at {}", dir.display()),
        None => tracing::warn!("game transcripts disabled"),
    }
    {
        let conn = db.lock().await;
        auth::ensure_system_user(&conn).expect("system user");
        decks::seed_starter_decks(&conn).expect("seed starter decks");
    }

    let http = reqwest::Client::new();
    let state = api::AppState {
        db: db.clone(),
        mailer: Arc::new(mail::Mailer::from_env(http.clone())),
        guard: Arc::new(guard::Guard::new()),
        http,
        secure_cookies: std::env::var("JINTEKI_SECURE_COOKIES")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    };

    // Daily GC: expired sessions/claims + long-idle empty anonymous users
    // (claim state is a GC veto — SYS-A-6).
    {
        let db = db.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
            tick.tick().await; // first tick fires immediately; skip it
            loop {
                tick.tick().await;
                {
                    let conn = db.lock().await;
                    match auth::gc_sweep(&conn) {
                        Ok(n) if n > 0 => tracing::info!("anon GC pruned {n} users"),
                        Ok(_) => {}
                        Err(e) => tracing::warn!("gc sweep failed: {e}"),
                    }
                }
                // An open lobby seat nobody took in a day is not an
                // invitation any more.
                match lobby::gc().await {
                    0 => {}
                    n => tracing::info!("lobby GC dropped {n} stale open games"),
                }
            }
        });
    }

    let ui_root: Arc<str> = Arc::from(ui_dir.as_str());
    let app = Router::new()
        .route("/ws/local", any(ws_local))
        .route("/ws/bridge", any(ws_bridge))
        .route("/health", get(|| async { "ok" }))
        // Build id baked in by nix (env.JINTEKI_BUILD_REV); "dev" locally.
        .route("/version", get(|| async { build_rev() }))
        // The page is served, not shipped: its asset URLs get the build id so
        // every deploy is a new URL. Header-only cache control cannot save a
        // cache that was poisoned before the headers existed (nix-store mtimes
        // are 1970, and browsers infer freshness from age) — a changed URL can.
        .route("/", get({
            let d = ui_root.clone();
            move || index_page(d.clone())
        }))
        .route("/index.html", get({
            let d = ui_root.clone();
            move || index_page(d.clone())
        }))
        .merge(api::router())
        .fallback_service(ServeDir::new(ui_dir).append_index_html_on_directories(true))
        .with_state(state);

    // Deployment mode (vacationvm): serve over a Unix socket that Caddy fronts.
    if let Ok(sock) = std::env::var("JINTEKI_SOCKET") {
        let _ = std::fs::remove_file(&sock);
        if let Some(dir) = std::path::Path::new(&sock).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let listener = tokio::net::UnixListener::bind(&sock).expect("bind unix socket");
        // World-writable so the reverse proxy in its own group can connect.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&sock, std::fs::Permissions::from_mode(0o666));
        tracing::info!("jinteki-rs server on unix:{sock}");
        axum::serve(listener, app).await.expect("serve");
        return;
    }

    let bind = std::env::var("JINTEKI_BIND").unwrap_or_else(|_| "0.0.0.0".into());
    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    tracing::info!("jinteki-rs server on http://localhost:{port} (LAN: http://<your-ip>:{port})");
    axum::serve(listener, app).await.expect("serve");
}

/// The WS upgrade request carries the same-origin `jrs_session` cookie;
/// resolve it here so game sessions can be attributed to the user (§8.3).
/// No cookie / dead session = anonymous play, exactly today's behavior.
async fn ws_local(
    ws: WebSocketUpgrade,
    axum::extract::State(st): axum::extract::State<api::AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user = match api::cookie_value(&headers, api::COOKIE_NAME) {
        Some(sid) => {
            let conn = st.db.lock().await;
            auth::validate_session(&conn, &sid).map(|su| su.user_id)
        }
        None => None,
    };
    let db = st.db.clone();
    ws.on_upgrade(move |socket| local::handle(socket, db, user))
}

async fn ws_bridge(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(bridge::handle)
}
