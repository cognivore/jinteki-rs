//! HTTP JSON surface: auth + decks + library + import (ACCOUNTS-AND-DECKS.md
//! §8). The games channel stays WebSocket; everything here is plain
//! request/response riding the `jrs_session` cookie.
//!
//! Conventions (§8.2): JSON bodies ≤ 1 MiB, errors `{"error": "human
//! sentence"}`, `Retry-After` on 429, SameSite=Lax + JSON bodies as the CSRF
//! story (§12.4).

use crate::auth::{self, SessionUser};
use crate::db::{audit, sha256_hex, Db};
use crate::decks;
use crate::eternal;
use crate::eternal_decks::{self, WriteOutcome};
use crate::guard::{client_ip, Guard, IpVerdict};
use crate::mail::Mailer;
use crate::nrdb;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub const COOKIE_NAME: &str = "jrs_session";
/// 400 days — the browser cap; the session row is the authority (§3.1).
const COOKIE_MAX_AGE: u64 = 400 * 24 * 3600;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub mailer: Arc<Mailer>,
    pub guard: Arc<Guard>,
    pub http: reqwest::Client,
    /// `JINTEKI_SECURE_COOKIES=1` forces Secure; X-Forwarded-Proto: https
    /// (Caddy) also sets it. Local dev over plain http sends neither.
    pub secure_cookies: bool,
}

// ── cookie plumbing ────────────────────────────────────────────────────────

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (k, v) = part.trim().split_once('=')?;
        (k == name).then(|| v.to_string())
    })
}

fn secure_flag(st: &AppState, headers: &HeaderMap) -> bool {
    st.secure_cookies
        || headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("https"))
            .unwrap_or(false)
}

fn set_cookie_header(st: &AppState, headers: &HeaderMap, value: &str, max_age: u64) -> HeaderValue {
    let secure = if secure_flag(st, headers) { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
    ))
    .expect("valid cookie header")
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

// ── session helpers ────────────────────────────────────────────────────────

/// Existing session or None. Never mints.
async fn session_of(st: &AppState, headers: &HeaderMap) -> Option<SessionUser> {
    let sid = cookie_value(headers, COOKIE_NAME)?;
    let conn = st.db.lock().await;
    auth::validate_session(&conn, &sid)
}

/// Session, minting an anonymous identity if absent/dead (§3.1: the first
/// API contact creates the account; static assets and /health never mint).
/// Returns the user and, when freshly minted, the Set-Cookie header.
async fn session_or_mint(
    st: &AppState,
    headers: &HeaderMap,
) -> Result<(SessionUser, Option<HeaderValue>), Response> {
    if let Some(su) = session_of(st, headers).await {
        return Ok((su, None));
    }
    let conn = st.db.lock().await;
    let su = auth::mint_anon(&conn)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &format!("db error: {e}")))?;
    let cookie = set_cookie_header(st, headers, &su.session_id, COOKIE_MAX_AGE);
    Ok((su, Some(cookie)))
}

fn with_cookie(mut resp: Response, cookie: Option<HeaderValue>) -> Response {
    if let Some(c) = cookie {
        resp.headers_mut().append(header::SET_COOKIE, c);
    }
    resp
}

fn me_json(su: &SessionUser) -> Value {
    // Email only ever goes to its own session (§3.4).
    json!({
        "user_id": su.user_id,
        "display_name": su.display_name,
        "kind": su.kind,
        "email": su.email,
    })
}

// ── router ─────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/me", get(api_me))
        .route("/api/auth/claim", post(api_claim))
        .route("/api/auth/logout", post(api_logout))
        .route("/api/profile", put(api_profile))
        .route("/api/account", delete(api_account_delete))
        .route("/api/catalog", get(api_catalog))
        .route("/api/decks", get(api_decks_list).post(api_decks_create))
        .route(
            "/api/decks/{id}",
            get(api_deck_get).put(api_deck_put).delete(api_deck_delete),
        )
        .route("/api/decks/validate", post(api_decks_validate))
        .route("/api/decks/import", post(api_decks_import))
        .route("/api/decks/{id}/publish", post(api_deck_publish))
        .route("/api/decks/{id}/unpublish", post(api_deck_unpublish))
        .route("/api/cards", get(api_cards))
        .route("/api/cr-readiness", get(api_cr_readiness))
        .route("/api/library", get(api_library))
        .route("/api/library/{id}", get(api_library_get))
        .route("/api/library/{id}/fork", post(api_library_fork))
        .route("/auth/verify", get(auth_verify).head(auth_verify_head))
        .layer(axum::extract::DefaultBodyLimit::max(1 << 20))
}

// ── auth endpoints ─────────────────────────────────────────────────────────

async fn api_me(State(st): State<AppState>, headers: HeaderMap) -> Response {
    match session_or_mint(&st, &headers).await {
        Ok((su, cookie)) => with_cookie(Json(me_json(&su)).into_response(), cookie),
        Err(e) => e,
    }
}

async fn api_claim(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    // Requires an existing session — a bot must first hit /api/me (§4.4).
    let Some(su) = session_of(&st, &headers).await else {
        return err(StatusCode::UNAUTHORIZED, "no session — load the app first");
    };
    // Per-IP backoff (guard.go:267-303) with Retry-After.
    if let IpVerdict::RetryAfter(secs) = st.guard.check_ip(&client_ip(&headers)) {
        let mut resp = err(StatusCode::TOO_MANY_REQUESTS, "slow down");
        resp.headers_mut().insert(
            header::RETRY_AFTER,
            HeaderValue::from_str(&secs.to_string()).unwrap(),
        );
        return resp;
    }
    let sent = Json(json!({ "sent": true })).into_response();
    // Everything below is enumeration-safe: bad-but-shaped emails, suppressed
    // sends, suspended targets — the response never changes (§4.4).
    let Some(email) = body["email"].as_str().and_then(auth::normalize_email) else {
        // Not even email-shaped: this one IS the caller's error, and telling
        // them leaks nothing about accounts.
        return err(StatusCode::BAD_REQUEST, "that does not look like an email address");
    };
    if !st.guard.allow_email(&email) {
        tracing::warn!("claim send suppressed by guard for {}", &sha256_hex(&email)[..12]);
        return sent;
    }
    let token = {
        let conn = st.db.lock().await;
        match auth::create_claim(&conn, &su.session_id, &su.user_id, &email) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("claim create failed: {e}");
                return sent; // still enumeration-safe
            }
        }
    };
    // Async send so response time doesn't oracle anything (§4.4).
    let mailer = st.mailer.clone();
    tokio::spawn(async move {
        mailer.send_magic_link(&email, &token).await;
    });
    sent
}

async fn auth_verify(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let token = q.get("token").map(String::as_str).unwrap_or("");
    let outcome = {
        let conn = st.db.lock().await;
        auth::verify_claim(&conn, token)
    };
    let mut resp = match outcome {
        Ok(auth::VerifyOutcome::Ok { session_id, .. }) => {
            let mut r = Redirect::to("/?auth=ok").into_response();
            // Fresh session for the clicking browser only (§4.5 step 3).
            r.headers_mut().append(
                header::SET_COOKIE,
                set_cookie_header(&st, &headers, &session_id, COOKIE_MAX_AGE),
            );
            r
        }
        Ok(auth::VerifyOutcome::Expired) => Redirect::to("/?auth=expired").into_response(),
        Ok(auth::VerifyOutcome::Conflict) => Redirect::to("/?auth=conflict").into_response(),
        Ok(auth::VerifyOutcome::Invalid) => Redirect::to("/?auth=invalid").into_response(),
        Err(e) => {
            tracing::error!("verify failed: {e}");
            Redirect::to("/?auth=invalid").into_response()
        }
    };
    // Keep the token out of referrer headers (§12.3).
    resp.headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    resp
}

/// HEAD is side-effect-free so mail-scanner prefetch doesn't burn the
/// single-use token (§4.2 mail-scanner note, OI-2).
async fn auth_verify_head() -> Response {
    let mut r = StatusCode::OK.into_response();
    r.headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    r
}

async fn api_logout(State(st): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(sid) = cookie_value(&headers, COOKIE_NAME) {
        let conn = st.db.lock().await;
        auth::delete_session(&conn, &sid);
        audit(&conn, None, "logout", &json!({}));
    }
    let mut resp = Json(json!({ "ok": true })).into_response();
    resp.headers_mut().append(
        header::SET_COOKIE,
        set_cookie_header(&st, &headers, "", 0), // clear
    );
    resp
}

async fn api_profile(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let Some(name) = body["display_name"]
        .as_str()
        .and_then(auth::valid_display_name)
    else {
        return err(
            StatusCode::BAD_REQUEST,
            "display name: 1-20 characters, no links or markup",
        );
    };
    let conn = st.db.lock().await;
    if conn
        .execute(
            "UPDATE users SET display_name = ?1 WHERE id = ?2",
            rusqlite::params![name, su.user_id],
        )
        .is_err()
    {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "db error");
    }
    let mut su = su;
    su.display_name = name;
    with_cookie(Json(me_json(&su)).into_response(), cookie)
}

async fn api_account_delete(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let Some(su) = session_of(&st, &headers).await else {
        return err(StatusCode::UNAUTHORIZED, "no session");
    };
    if su.kind != "claimed" {
        return err(StatusCode::FORBIDDEN, "only claimed accounts can be deleted");
    }
    let conn = st.db.lock().await;
    if let Err(e) = auth::delete_account(&conn, &su.user_id) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("delete failed: {e}"));
    }
    let mut resp = Json(json!({ "ok": true })).into_response();
    resp.headers_mut()
        .append(header::SET_COOKIE, set_cookie_header(&st, &headers, "", 0));
    resp
}

// ── eternal catalog + deck endpoints ───────────────────────────────────────
//
// The exact deck-builder contract:
//   GET    /api/catalog?format=eternal → {"format","point_limit","identities","cards"}
//   GET    /api/decks                  → {"decks":[…]} — defaults first, builtin:true
//   POST   /api/decks                  → {"key","legal","problems"} (saves even if illegal)
//   GET    /api/decks/<key>            → {"key","name","identity","cards","legal","problems"}
//   PUT    /api/decks/<key>            → as POST; built-ins 403
//   DELETE /api/decks/<key>            → {"ok":true}; built-ins 403
// Card ids throughout are the catalog's NSG v2 slugs (`eternal.rs`).

/// The Eternal deck-builder catalog: engine-supported ∩ card pool, with ban
/// flags, points-list values and the draft-only exclusion (`eternal.rs`).
async fn api_catalog(Query(q): Query<HashMap<String, String>>) -> Response {
    match q.get("format").map(String::as_str).unwrap_or("eternal") {
        "eternal" => Json(eternal::catalog_json()).into_response(),
        other => err(
            StatusCode::BAD_REQUEST,
            &format!("unknown format \"{other}\" — this server serves \"eternal\""),
        ),
    }
}

async fn api_decks_list(State(st): State<AppState>, headers: HeaderMap) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let conn = st.db.lock().await;
    let list = eternal_decks::list_json(&conn, &su.user_id);
    with_cookie(Json(list).into_response(), cookie)
}

fn parse_eternal_draft(body: Value) -> Result<eternal_decks::EternalDraft, Response> {
    let draft: eternal_decks::EternalDraft = serde_json::from_value(body)
        .map_err(|e| err(StatusCode::BAD_REQUEST, &format!("bad deck payload: {e}")))?;
    draft.checked().map_err(|msg| err(StatusCode::BAD_REQUEST, &msg))
}

async fn api_decks_create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let draft = match parse_eternal_draft(body) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let conn = st.db.lock().await;
    // Saved even if illegal (marked by `legal` + `problems`): a deck under
    // construction is still the player's deck.
    match eternal_decks::create(&conn, &su.user_id, &draft) {
        Ok(v) => with_cookie(Json(v).into_response(), cookie),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &format!("db error: {e}")),
    }
}

async fn api_deck_get(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let conn = st.db.lock().await;
    match eternal_decks::get_json(&conn, &key, &su.user_id) {
        Some(v) => with_cookie(Json(v).into_response(), cookie),
        None => err(StatusCode::NOT_FOUND, "no such deck"),
    }
}

async fn api_deck_put(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let draft = match parse_eternal_draft(body) {
        Ok(d) => d,
        Err(e) => return e,
    };
    let conn = st.db.lock().await;
    match eternal_decks::update(&conn, &key, &su.user_id, &draft) {
        WriteOutcome::Ok(v) => with_cookie(Json(v).into_response(), cookie),
        WriteOutcome::Builtin => err(StatusCode::FORBIDDEN, "built-in decks cannot be edited"),
        WriteOutcome::NotFound => err(StatusCode::NOT_FOUND, "no such deck"),
    }
}

async fn api_deck_delete(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let conn = st.db.lock().await;
    match eternal_decks::delete(&conn, &key, &su.user_id) {
        WriteOutcome::Ok(v) => with_cookie(Json(v).into_response(), cookie),
        WriteOutcome::Builtin => err(StatusCode::FORBIDDEN, "built-in decks cannot be deleted"),
        WriteOutcome::NotFound => err(StatusCode::NOT_FOUND, "no such deck"),
    }
}

async fn api_decks_validate(
    State(_st): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    // Pure check for unsaved drafts (§6.2): no session needed, nothing stored.
    let draft: decks::DeckDraft = match serde_json::from_value(body) {
        Ok(d) => d,
        Err(e) => return err(StatusCode::BAD_REQUEST, &format!("bad deck payload: {e}")),
    };
    let lines: Vec<crate::deckcheck::DeckLine> = draft
        .cards
        .iter()
        .map(|c| crate::deckcheck::DeckLine { title: c.title.clone(), qty: c.qty })
        .collect();
    let v = crate::deckcheck::check(&draft.identity.title, &lines);
    Json(json!({
        "legal": v.legal,
        "problems": v.problems,
        "warnings": v.warnings,
        "counts": v.counts,
        "playable": v.playable,
        "cards": v.cards,
    }))
    .into_response()
}

async fn api_decks_import(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let input = body["input"].as_str().unwrap_or("");
    if !st.guard.allow_nrdb(&su.user_id) {
        let mut resp = err(
            StatusCode::TOO_MANY_REQUESTS,
            "import limit reached — try again in a minute",
        );
        resp.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
        return resp;
    }
    let ep = match nrdb::parse_input(input) {
        Ok(ep) => ep,
        Err(nrdb::ImportError::BadInput(m)) => return err(StatusCode::BAD_REQUEST, &m),
        Err(_) => return err(StatusCode::BAD_REQUEST, "bad input"),
    };
    let fetched = nrdb::fetch(&st.http, &ep).await;
    let (payload, human_url) = match fetched {
        Ok(x) => x,
        Err(e) => return import_error_response(e),
    };
    match nrdb::map_payload(&payload, &human_url, ep.id()) {
        Ok(imp) => with_cookie(
            Json(json!({ "deck": imp.draft, "report": imp.report })).into_response(),
            cookie,
        ),
        Err(e) => import_error_response(e),
    }
}

/// §7.4 failure taxonomy → status + user-facing copy. Upstream failures are
/// 502-style, not 400 — the caller did nothing wrong.
fn import_error_response(e: nrdb::ImportError) -> Response {
    match e {
        nrdb::ImportError::BadInput(m) => err(StatusCode::BAD_REQUEST, &m),
        nrdb::ImportError::NotFound => err(
            StatusCode::NOT_FOUND,
            "NetrunnerDB has no decklist with that id",
        ),
        nrdb::ImportError::Blocked => err(
            StatusCode::BAD_GATEWAY,
            "NetrunnerDB refused the request; try again later",
        ),
        nrdb::ImportError::Down(m) => err(StatusCode::BAD_GATEWAY, &m),
        nrdb::ImportError::NoIdentity(name) => err(
            StatusCode::UNPROCESSABLE_ENTITY,
            &format!("\"{name}\" has no identity card we recognize — a deck with no identity is not a deck"),
        ),
    }
}

async fn api_deck_publish(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(su) = session_of(&st, &headers).await else {
        return err(StatusCode::UNAUTHORIZED, "no session");
    };
    let conn = st.db.lock().await;
    match decks::publish(&conn, &id, &su.user_id, &su.kind) {
        Ok(v) => Json(v).into_response(),
        Err(m) if m == "no such deck" => err(StatusCode::NOT_FOUND, &m),
        Err(m) => err(StatusCode::FORBIDDEN, &m),
    }
}

async fn api_deck_unpublish(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let Some(su) = session_of(&st, &headers).await else {
        return err(StatusCode::UNAUTHORIZED, "no session");
    };
    let conn = st.db.lock().await;
    match decks::unpublish(&conn, &id, &su.user_id) {
        Ok(v) => Json(v).into_response(),
        Err(m) => err(StatusCode::NOT_FOUND, &m),
    }
}

/// Card-pool search for the deck editor (§9.2): pure over the embedded
/// index, no session, capped result set. Not a card-data API — art and full
/// text stay on the NRDB CDN / core pipeline.
async fn api_cards(Query(q): Query<HashMap<String, String>>) -> Response {
    let term = q.get("q").map(|s| s.trim().to_lowercase()).unwrap_or_default();
    let want_type = q.get("type").map(String::as_str);
    let want_side = q.get("side").map(String::as_str); // "Corp" | "Runner"
    let want_faction = q.get("faction").map(String::as_str);
    if term.is_empty() && want_type.is_none() {
        return Json(json!([])).into_response();
    }
    let list: Vec<Value> = crate::carddata::all()
        .iter()
        .filter(|c| c.side == "Corp" || c.side == "Runner")
        .filter(|c| want_type.map_or(true, |t| c.card_type == t))
        .filter(|c| want_side.map_or(true, |s| c.side.eq_ignore_ascii_case(s)))
        .filter(|c| {
            want_faction.map_or(true, |f| {
                c.faction.as_deref().map(|cf| cf.eq_ignore_ascii_case(f)).unwrap_or(false)
            })
        })
        .filter(|c| term.is_empty() || c.title.to_lowercase().contains(&term))
        .take(50)
        .map(|c| {
            json!({
                "title": c.title,
                "code": c.code,
                "side": c.side,
                "type": c.card_type,
                "faction": c.faction,
                "influence_cost": c.influence_cost,
                "impl": crate::deckcheck::impl_status_str(&c.title),
                // What the card SAYS. A picker that offers a card to name and
                // shows nothing but its title is asking the player to
                // remember, which is the thing the picker exists to avoid.
                "subtypes": c.subtypes,
                "cost": c.cost,
                "text": crate::cr::oracle_text(&c.title),
                // CR 1.4: a double-sided card's back faces, in face order,
                // so the editor's reader can show them too. Empty for most.
                "faces": c.faces,
            })
        })
        .collect();
    Json(json!(list)).into_response()
}

/// SYS-D-12, made public: the CR mode's completeness gate as JSON, so the
/// home screen shows the true fraction and the honest "not yet" list without
/// having to open a socket. Evaluated from `jinteki-cards` per request — the
/// mode goes live the moment the card layer closes, with no deploy.
async fn api_cr_readiness() -> Response {
    Json(json!(crate::cr::readiness())).into_response()
}

// ── library endpoints ──────────────────────────────────────────────────────

async fn api_library(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    // Public browse: no session required, none minted (crawler-safe).
    let conn = st.db.lock().await;
    let page = q.get("page").and_then(|p| p.parse().ok()).unwrap_or(0);
    let v = decks::library_list(
        &conn,
        q.get("side").map(String::as_str),
        q.get("faction").map(String::as_str),
        q.get("q").map(String::as_str),
        q.get("sort").map(String::as_str),
        page,
    );
    Json(v).into_response()
}

async fn api_library_get(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    let conn = st.db.lock().await;
    match decks::library_get(&conn, &id) {
        Some(v) => Json(v).into_response(),
        None => err(StatusCode::NOT_FOUND, "no such published deck"),
    }
}

async fn api_library_fork(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    // Fork mints a session if needed: it is how a phone visitor starts
    // playing a real deck within a minute of arriving (§6.4).
    let (su, cookie) = match session_or_mint(&st, &headers).await {
        Ok(x) => x,
        Err(e) => return e,
    };
    let conn = st.db.lock().await;
    match decks::fork(&conn, &id, &su.user_id) {
        Ok(v) => with_cookie(Json(v).into_response(), cookie),
        Err(m) => err(StatusCode::NOT_FOUND, &m),
    }
}

// CORS is deliberately absent: same-origin app, Lax cookies. A stray
// cross-origin GET gets browser-default blocking; nothing to relax.
