//! jinteki-server: serves the mobile UI, hosts local games against the
//! random-walk bot, and bridges to a reference jinteki.net server for parity
//! play. One WebSocket per session; all frames are JSON text with a `type`
//! field — the UI is backend-agnostic between local and bridge modes.

mod bridge;
mod local;

use axum::{
    extract::ws::WebSocketUpgrade,
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let ui_dir = std::env::var("JINTEKI_UI_DIR").unwrap_or_else(|_| "ui".into());
    let port: u16 = std::env::var("JINTEKI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7787);

    let app = Router::new()
        .route("/ws/local", any(ws_local))
        .route("/ws/bridge", any(ws_bridge))
        .route("/health", get(|| async { "ok" }))
        .fallback_service(ServeDir::new(ui_dir).append_index_html_on_directories(true));

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

async fn ws_local(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(local::handle)
}

async fn ws_bridge(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(bridge::handle)
}
