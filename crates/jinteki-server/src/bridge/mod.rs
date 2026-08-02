//! Bridge mode: the UI plays on a real jinteki.net-protocol server (the
//! dockerized reference, or any jnet instance) through this translator.
//!
//! UI ⇄ bridge: same JSON envelope as local mode ({"type":"state",...}).
//! bridge ⇄ reference: sente over /chsk, msgpack packer, differ diffs —
//! implemented from source (see msgpack.rs / differ.rs).
//!
//! Every inbound event and outbound command is appended to
//! parity-logs/bridge-<epoch>.jsonl so reference sessions can be compared
//! against local-engine sessions offline. That file IS the parity artifact.

mod differ;
mod msgpack;

use axum::extract::ws::{Message as UiMsg, WebSocket};
use futures_util::{SinkExt, StreamExt};
use msgpack::{decode_frame, encode_event, json_to_mp, kw, uuid as mp_uuid};
use rmpv::Value as Mp;
use serde_json::{json, Value as Js};
use std::collections::HashMap;
use std::io::Write;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::COOKIE;
use tokio_tungstenite::tungstenite::Message as WsMsg;

pub async fn handle(mut ui: WebSocket) {
    // Phase 1: wait for the connect request.
    let (host, username, password) = loop {
        match ui.recv().await {
            Some(Ok(UiMsg::Text(t))) => {
                let v: Js = match serde_json::from_str(&t) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if v["type"] == "connect" {
                    break (
                        v["host"].as_str().unwrap_or("http://localhost:1042").trim_end_matches('/').to_string(),
                        v["username"].as_str().unwrap_or("").to_string(),
                        v["password"].as_str().unwrap_or("").to_string(),
                    );
                }
            }
            Some(Ok(_)) => continue,
            _ => return,
        }
    };

    match session(&mut ui, &host, &username, &password).await {
        Ok(()) => {}
        Err(e) => {
            let _ = ui
                .send(UiMsg::Text(
                    json!({"type":"error","error": e}).to_string().into(),
                ))
                .await;
        }
    }
}

async fn session(
    ui: &mut WebSocket,
    host: &str,
    username: &str,
    password: &str,
) -> Result<(), String> {
    // ── HTTP session dance: CSRF scrape + login ────────────────────────────
    let jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
    let http = reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .danger_accept_invalid_certs(false)
        .build()
        .map_err(|e| e.to_string())?;

    let index = http
        .get(format!("{host}/"))
        .send()
        .await
        .map_err(|e| format!("cannot reach {host}: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let csrf = scrape_csrf(&index).ok_or("no csrf token on index page")?;

    if !username.is_empty() {
        // The reference reads ring :params — form-encoded, not JSON.
        let resp = http
            .post(format!("{host}/login"))
            .header("X-CSRF-Token", &csrf)
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(|e| format!("login request failed: {e}"))?;
        if !resp.status().is_success() {
            let code = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("login rejected ({code}): {body}"));
        }
    }

    // ── WebSocket connect with cookies + csrf param ────────────────────────
    let base_url: reqwest::Url = host.parse().map_err(|e| format!("bad host: {e}"))?;
    let client_id = format!("{:016x}{:016x}", rand::random::<u64>(), rand::random::<u64>());
    let ws_scheme = if base_url.scheme() == "https" { "wss" } else { "ws" };
    let ws_url = format!(
        "{ws_scheme}://{}{}/chsk?client-id={client_id}&csrf-token={}",
        base_url.host_str().unwrap_or("localhost"),
        base_url
            .port()
            .map(|p| format!(":{p}"))
            .unwrap_or_default(),
        percent_encode(&csrf),
    );
    let mut req = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| e.to_string())?;
    use reqwest::cookie::CookieStore;
    if let Some(cookies) = jar.cookies(&base_url) {
        req.headers_mut().insert(
            COOKIE,
            cookies
                .to_str()
                .map_err(|e| e.to_string())?
                .parse()
                .map_err(|_| "cookie header")?,
        );
    }
    let (mut sente, _resp) = tokio_tungstenite::connect_async(req)
        .await
        .map_err(|e| format!("ws connect failed: {e} (url {ws_url})"))?;

    // ── Parity log ─────────────────────────────────────────────────────────
    std::fs::create_dir_all("parity-logs").ok();
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut plog = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("parity-logs/bridge-{epoch}.jsonl"))
        .ok();
    let mut log_line = move |dir: &str, val: &Js| {
        if let Some(f) = plog.as_mut() {
            let _ = writeln!(f, "{}", json!({"dir": dir, "v": val}));
        }
    };

    // ── Session state ──────────────────────────────────────────────────────
    let mut gameid: Option<String> = None;
    let mut game_state: Option<Js> = None;
    let mut pending: HashMap<String, String> = HashMap::new();
    let mut cb_counter: u64 = 0;
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(15));
    let my_username = username.to_string();

    macro_rules! ui_send {
        ($v:expr) => {{
            let _ = ui.send(UiMsg::Text($v.to_string().into())).await;
        }};
    }
    macro_rules! sente_send {
        ($bytes:expr) => {{
            let _ = sente.send(WsMsg::Binary($bytes.into())).await;
        }};
    }

    loop {
        tokio::select! {
            _ = ping.tick() => {
                cb_counter += 1;
                let cb = format!("p{cb_counter}");
                pending.insert(cb.clone(), "ping".into());
                sente_send!(encode_event("chsk/ws-ping", None, Some(&cb)));
            }
            frame = sente.next() => {
                let Some(frame) = frame else {
                    ui_send!(json!({"type":"disconnected"}));
                    return Ok(());
                };
                let frame = frame.map_err(|e| format!("sente ws error: {e}"))?;
                let bytes = match frame {
                    WsMsg::Binary(b) => b,
                    WsMsg::Close(_) => {
                        ui_send!(json!({"type":"disconnected"}));
                        return Ok(());
                    }
                    _ => continue,
                };
                let f = match decode_frame(&bytes) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::warn!("undecodable frame: {e}");
                        continue;
                    }
                };
                if let Some(cb) = f.cb_uuid {
                    let purpose = pending.remove(&cb).unwrap_or_default();
                    if purpose != "ping" {
                        log_line("reply", &f.payload);
                        ui_send!(json!({"type":"reply","purpose":purpose,"data":f.payload}));
                    }
                    continue;
                }
                if f.payload == json!("chsk/ws-ping") {
                    sente_send!(encode_event("chsk/ws-pong", None, None));
                    continue;
                }
                let events: Vec<Js> = match &f.payload {
                    Js::Array(items) if items.first().map(|x| x.is_string()).unwrap_or(false) => {
                        vec![f.payload.clone()]
                    }
                    Js::Array(items) => items.clone(),
                    _ => continue,
                };
                for ev in events {
                    let id = ev[0].as_str().unwrap_or("");
                    let data = ev.get(1).cloned().unwrap_or(Js::Null);
                    log_line(id, &data);
                    match id {
                        "chsk/handshake" => {
                            ui_send!(json!({"type":"connected","uid": data.get(0).cloned().unwrap_or(Js::Null)}));
                            sente_send!(encode_event("lobby/list", None, None));
                        }
                        "lobby/list" => ui_send!(json!({"type":"lobbies","list": data})),
                        "lobby/state" => {
                            // The creator learns its gameid from the lobby broadcast.
                            if let Some(gid) = data.get("gameid").and_then(|g| g.as_str()) {
                                gameid = Some(gid.to_string());
                            }
                            ui_send!(json!({"type":"lobby","lobby": data}));
                        }
                        "lobby/notification" => {}
                        "lobby/toast" => ui_send!(json!({"type":"toast","toast": data})),
                        "game/start" | "game/resync" => {
                            if let Some(s) = data.as_str() {
                                if let Ok(st) = serde_json::from_str::<Js>(s) {
                                    game_state = Some(st.clone());
                                    if gameid.is_none() {
                                        gameid = st["gameid"].as_str().map(|s| s.to_string());
                                    }
                                    push_bridge_state(ui, &st, &my_username).await;
                                }
                            }
                        }
                        "game/diff" => {
                            if let (Some(s), Some(cur)) = (data.as_str(), game_state.as_ref()) {
                                if let Ok(d) = serde_json::from_str::<Js>(s) {
                                    let patched = differ::patch(cur, &d["diff"]);
                                    game_state = Some(patched.clone());
                                    push_bridge_state(ui, &patched, &my_username).await;
                                }
                            }
                        }
                        "game/error" => ui_send!(json!({"type":"toast","toast":{"message":"server error; resyncing"}})),
                        "system/force-disconnect" => {
                            ui_send!(json!({"type":"disconnected"}));
                            return Ok(());
                        }
                        _ => ui_send!(json!({"type":"event","id":id,"data":data})),
                    }
                }
            }
            msg = ui.recv() => {
                let Some(Ok(msg)) = msg else { return Ok(()) };
                let UiMsg::Text(t) = msg else { continue };
                let Ok(v) = serde_json::from_str::<Js>(&t) else { continue };
                let typ = v["type"].as_str().unwrap_or("");
                log_line(&format!("ui/{typ}"), &v);
                match typ {
                    "lobbies" => sente_send!(encode_event("lobby/list", None, None)),
                    "create" => {
                        let side = v["side"].as_str().unwrap_or("Corp");
                        let title = v["title"].as_str().unwrap_or("jinteki-rs parity game");
                        let payload = json_to_mp(&json!({
                            "title": title,
                            "side": side,
                            "format": "casual",
                            "room": "casual",
                            "allow-spectator": true,
                            "save-replay": true,
                            "spectatorhands": false,
                            "password": "",
                            "timer": Js::Null,
                        }));
                        sente_send!(encode_event("lobby/create", Some(payload), None));
                    }
                    "join" | "watch" => {
                        let gid = v["gameid"].as_str().unwrap_or("").to_string();
                        gameid = Some(gid.clone());
                        let mut pairs = vec![
                            (kw("gameid"), mp_uuid(&gid)),
                            (kw("password"), Mp::Nil),
                        ];
                        if let Some(side) = v["side"].as_str() {
                            pairs.push((kw("request-side"), Mp::String(side.to_string().into())));
                        }
                        cb_counter += 1;
                        let cb = format!("c{cb_counter}");
                        pending.insert(cb.clone(), typ.to_string());
                        let ev = if typ == "join" { "lobby/join" } else { "lobby/watch" };
                        sente_send!(encode_event(ev, Some(Mp::Map(pairs)), Some(&cb)));
                    }
                    "decks" => {
                        match http.get(format!("{host}/data/decks")).send().await {
                            Ok(r) => {
                                let decks: Js = r.json().await.unwrap_or(json!([]));
                                ui_send!(json!({"type":"decks","list": decks}));
                            }
                            Err(e) => ui_send!(json!({"type":"error","error": format!("decks: {e}")})),
                        }
                    }
                    "deck" => {
                        if let (Some(gid), Some(did)) = (gameid.clone(), v["deck-id"].as_str()) {
                            let pairs = vec![
                                (kw("gameid"), mp_uuid(&gid)),
                                (kw("deck-id"), Mp::String(did.to_string().into())),
                            ];
                            cb_counter += 1;
                            let cb = format!("c{cb_counter}");
                            pending.insert(cb.clone(), "deck".into());
                            sente_send!(encode_event("lobby/deck", Some(Mp::Map(pairs)), Some(&cb)));
                        }
                    }
                    "start" => {
                        if let Some(gid) = gameid.clone() {
                            let pairs = vec![(kw("gameid"), mp_uuid(&gid))];
                            sente_send!(encode_event("game/start", Some(Mp::Map(pairs)), None));
                        }
                    }
                    "action" => {
                        if let Some(gid) = gameid.clone() {
                            let cmd = v["command"].as_str().unwrap_or("");
                            let pairs = vec![
                                (kw("gameid"), mp_uuid(&gid)),
                                (kw("command"), Mp::String(cmd.to_string().into())),
                                (kw("args"), json_to_mp(&v["args"])),
                            ];
                            sente_send!(encode_event("game/action", Some(Mp::Map(pairs)), None));
                        }
                    }
                    "say" => {
                        if let Some(gid) = gameid.clone() {
                            let pairs = vec![
                                (kw("gameid"), mp_uuid(&gid)),
                                (kw("msg"), Mp::String(v["msg"].as_str().unwrap_or("").to_string().into())),
                            ];
                            sente_send!(encode_event("game/say", Some(Mp::Map(pairs)), None));
                        }
                    }
                    "resync" => {
                        if let Some(gid) = gameid.clone() {
                            let pairs = vec![(kw("gameid"), mp_uuid(&gid))];
                            sente_send!(encode_event("game/resync", Some(Mp::Map(pairs)), None));
                        }
                    }
                    "concede" => {
                        if let Some(gid) = gameid.clone() {
                            let pairs = vec![(kw("gameid"), mp_uuid(&gid))];
                            sente_send!(encode_event("game/concede", Some(Mp::Map(pairs)), None));
                        }
                    }
                    "leave" => {
                        if let Some(gid) = gameid.clone() {
                            let pairs = vec![(kw("gameid"), mp_uuid(&gid))];
                            sente_send!(encode_event("game/leave", Some(Mp::Map(pairs.clone())), None));
                            sente_send!(encode_event("lobby/leave", Some(Mp::Map(pairs)), None));
                            gameid = None;
                            game_state = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn push_bridge_state(ui: &mut WebSocket, st: &Js, my_username: &str) {
    let side = if st["corp"]["user"]["username"] == json!(my_username) {
        "corp"
    } else if st["runner"]["user"]["username"] == json!(my_username) {
        "runner"
    } else {
        "spect"
    };
    let msg = json!({"type":"state","state": st, "actions": [], "mode": "bridge", "side": side});
    let _ = ui.send(UiMsg::Text(msg.to_string().into())).await;
}

fn scrape_csrf(html: &str) -> Option<String> {
    let needle = "data-csrf-token=\"";
    let start = html.find(needle)? + needle.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
