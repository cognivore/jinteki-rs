//! End-to-end HTTP flow over a real TCP listener: cookie mint on /api/me,
//! deck CRUD + validation payloads, library fork, the publish gate, and the
//! magic-link verify redirect — all against an in-memory database with the
//! mailer in dev mode (no network leaves the process).

use jinteki_server::api::{self, AppState};
use jinteki_server::db::Db;
use jinteki_server::{auth, decks, guard, mail};
use std::sync::Arc;

async fn spawn_app() -> (String, Arc<Db>) {
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
    let app = axum::Router::new().merge(api::router()).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), db)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

#[tokio::test]
async fn me_mints_anon_once_and_cookie_persists_identity() {
    let (base, _db) = spawn_app().await;
    let c = client();
    let r1: serde_json::Value = c.get(format!("{base}/api/me")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(r1["kind"], "anon");
    assert!(r1["display_name"].as_str().unwrap().starts_with("guest-"));
    let r2: serde_json::Value = c.get(format!("{base}/api/me")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(r1["user_id"], r2["user_id"], "same cookie, same identity");

    // A cookieless client gets a different identity.
    let c2 = client();
    let r3: serde_json::Value = c2.get(format!("{base}/api/me")).send().await.unwrap()
        .json().await.unwrap();
    assert_ne!(r1["user_id"], r3["user_id"]);
}

#[tokio::test]
async fn library_seeded_and_forkable() {
    let (base, _db) = spawn_app().await;
    let c = client();

    // Library carries the two seeded starter decks without any session.
    let lib: serde_json::Value = c.get(format!("{base}/api/library")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(lib["total"], 2);
    let first = &lib["decks"][0];
    assert_eq!(first["author_name"], "jinteki-rs");
    let starter_id = first["id"].as_str().unwrap().to_string();

    // Fork it (mints a session on the way — the phone flow).
    let forked: serde_json::Value = c
        .post(format!("{base}/api/library/{starter_id}/fork"))
        .send().await.unwrap().json().await.unwrap();
    assert!(forked["name"].as_str().unwrap().ends_with("(fork)"));
    assert_eq!(forked["source"]["kind"], "fork");
}

/// The exact eternal deck contract: catalog, defaults-first list with
/// display names, CRUD keyed `user-<id>`, save-even-if-illegal, built-in 403.
#[tokio::test]
async fn eternal_catalog_and_deck_contract() {
    let (base, _db) = spawn_app().await;
    let c = client();

    // Catalog: eternal only, identities split out, draft-only never listed.
    let cat: serde_json::Value = c
        .get(format!("{base}/api/catalog?format=eternal")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(cat["format"], "eternal");
    assert_eq!(cat["point_limit"], 7);
    let identities = cat["identities"].as_array().unwrap();
    assert!(identities.iter().all(|i| i["draft_only"] == false));
    assert!(!identities.iter().any(|i| i["id"] == "boris_syfr_kovac_crafty_veteran"),
        "draft-only identities never appear for Eternal");
    assert!(!identities.iter().any(|i| i["id"] == "the_catalyst_convention_breaker"
        || i["id"] == "the_syndicate_profit_over_principle"),
        "CR 1.4.1a: the starter-pack identities are never offered");
    assert!(cat["cards"].as_array().unwrap().iter().any(|x| x["id"] == "sure_gamble"));
    assert!(cat["cards"].as_array().unwrap().iter().any(|x| x["id"] == "hedge_fund"),
        "off-list definitions (Hedge Fund) are engine-supported and listed");
    let unknown = c.get(format!("{base}/api/catalog?format=startup")).send().await.unwrap();
    assert_eq!(unknown.status(), 400);

    // Deck list: the two defaults first, display-named, builtin:true.
    let mine: serde_json::Value = c.get(format!("{base}/api/decks")).send().await.unwrap()
        .json().await.unwrap();
    let decks = mine["decks"].as_array().unwrap();
    assert_eq!(decks[0]["key"], "andromeda");
    assert_eq!(decks[0]["name"], "Mezzie's Andromeda");
    assert_eq!(decks[0]["builtin"], true);
    assert_eq!(decks[1]["key"], "gauntlet");
    assert_eq!(decks[1]["name"], "Mezzie's Making Stars");
    assert_eq!(decks[1]["builtin"], true);
    assert_eq!(decks.len(), 2, "a fresh account has only the defaults");

    // A built-in reads whole, in catalog ids, and refuses writes.
    let gauntlet: serde_json::Value = c
        .get(format!("{base}/api/decks/gauntlet")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(gauntlet["identity"], "nebula_talent_management_making_stars");
    assert_eq!(gauntlet["cards"]["jackson_howard"], 3);
    assert_eq!(gauntlet["legal"], true);
    let del = c.delete(format!("{base}/api/decks/gauntlet")).send().await.unwrap();
    assert_eq!(del.status(), 403, "built-ins cannot be deleted");
    let put = c.put(format!("{base}/api/decks/andromeda"))
        .json(&serde_json::json!({
            "name": "x", "identity": "andromeda_dispossessed_ristie", "cards": {}
        }))
        .send().await.unwrap();
    assert_eq!(put.status(), 403, "built-ins cannot be edited");

    // Create: saves even if illegal, marked; unknown ids get problems, not 400.
    let created: serde_json::Value = c
        .post(format!("{base}/api/decks"))
        .json(&serde_json::json!({
            "name": "smoke",
            "identity": "hoshiko_shiro_untold_protagonist",
            "cards": {"sure_gamble": 3, "definitely_not_a_card": 1},
        }))
        .send().await.unwrap().json().await.unwrap();
    let key = created["key"].as_str().unwrap().to_string();
    assert!(key.starts_with("user-"));
    assert_eq!(created["legal"], false);
    let problems = created["problems"].as_array().unwrap();
    assert!(problems.iter().any(|p| p["code"] == "deck_size"));
    assert!(problems.iter().any(|p| p["code"] == "unsupported"
        && p["card"] == "definitely_not_a_card"));

    // It joins the list after the defaults.
    let mine: serde_json::Value = c.get(format!("{base}/api/decks")).send().await.unwrap()
        .json().await.unwrap();
    let decks = mine["decks"].as_array().unwrap();
    assert_eq!(decks.len(), 3);
    assert_eq!(decks[2]["key"], key.as_str());
    assert_eq!(decks[2]["builtin"], false);
    assert_eq!(decks[2]["legal"], false);

    // Update, read whole, delete.
    let updated: serde_json::Value = c
        .put(format!("{base}/api/decks/{key}"))
        .json(&serde_json::json!({
            "name": "smoke v2",
            "identity": "hoshiko_shiro_untold_protagonist",
            "cards": {"sure_gamble": 2},
        }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(updated["key"], key.as_str());
    let got: serde_json::Value = c.get(format!("{base}/api/decks/{key}")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(got["name"], "smoke v2");
    assert_eq!(got["cards"]["sure_gamble"], 2);

    // Another client cannot read my deck (ownership).
    let other = client();
    other.get(format!("{base}/api/me")).send().await.unwrap();
    let denied = other.get(format!("{base}/api/decks/{key}")).send().await.unwrap();
    assert_eq!(denied.status(), 404);

    let del = c.delete(format!("{base}/api/decks/{key}")).send().await.unwrap();
    assert_eq!(del.status(), 200);
    let gone = c.get(format!("{base}/api/decks/{key}")).send().await.unwrap();
    assert_eq!(gone.status(), 404);

    // Malformed shape (not deck content) is the one 400.
    let bad = c.post(format!("{base}/api/decks"))
        .json(&serde_json::json!({"name": "", "identity": "x", "cards": {}}))
        .send().await.unwrap();
    assert_eq!(bad.status(), 400);
}

#[tokio::test]
async fn publish_requires_claimed_account() {
    let (base, _db) = spawn_app().await;
    let c = client();
    let lib: serde_json::Value = c.get(format!("{base}/api/library")).send().await.unwrap()
        .json().await.unwrap();
    let starter_id = lib["decks"][0]["id"].as_str().unwrap().to_string();
    let forked: serde_json::Value = c
        .post(format!("{base}/api/library/{starter_id}/fork"))
        .send().await.unwrap().json().await.unwrap();
    let deck_id = forked["id"].as_str().unwrap();
    let resp = c.post(format!("{base}/api/decks/{deck_id}/publish")).send().await.unwrap();
    assert_eq!(resp.status(), 403, "anonymous users get the claim nudge");
    assert!(resp.json::<serde_json::Value>().await.unwrap()["error"]
        .as_str().unwrap().contains("claim"));
}

#[tokio::test]
async fn claim_endpoint_is_enumeration_safe_and_verify_sets_cookie() {
    let (base, db) = spawn_app().await;
    let c = client();
    let me: serde_json::Value = c.get(format!("{base}/api/me")).send().await.unwrap()
        .json().await.unwrap();
    let anon_user = me["user_id"].as_str().unwrap().to_string();

    // Claim responds {"sent":true} (dev-mode mailer logs, sends nothing).
    let r: serde_json::Value = c
        .post(format!("{base}/api/auth/claim"))
        .json(&serde_json::json!({"email": "smoke@example.com"}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(r["sent"], true);

    // The raw token is not retrievable over HTTP (by design); take a claim
    // minted directly against the DB and walk the link like a mail client.
    let raw = {
        let conn = db.lock().await;
        // token must never appear in any API response; mint our own.
        let session_id: String = conn.query_row(
            "SELECT id FROM sessions WHERE user_id = ?1", [&anon_user], |row| row.get(0)).unwrap();
        auth::create_claim(&conn, &session_id, &anon_user, "smoke@example.com").unwrap()
    };

    // HEAD is side-effect-free (mail-scanner prefetch).
    let head = c.head(format!("{base}/auth/verify?token={raw}")).send().await.unwrap();
    assert_eq!(head.status(), 200);

    // GET consumes: 303 to /?auth=ok with a fresh session cookie.
    let resp = c.get(format!("{base}/auth/verify?token={raw}")).send().await.unwrap();
    assert_eq!(resp.status(), 303);
    assert_eq!(resp.headers()["location"], "/?auth=ok");
    assert_eq!(resp.headers()["referrer-policy"], "no-referrer");
    let set_cookie = resp.headers()["set-cookie"].to_str().unwrap();
    assert!(set_cookie.starts_with("jrs_session="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));

    // The clicking browser is now the claimed user.
    let me2: serde_json::Value = c.get(format!("{base}/api/me")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(me2["kind"], "claimed");
    assert_eq!(me2["email"], "smoke@example.com");
    assert_eq!(me2["user_id"], anon_user, "upgraded in place, same id");

    // Replay of the same link: invalid.
    let replay = c.get(format!("{base}/auth/verify?token={raw}")).send().await.unwrap();
    assert_eq!(replay.headers()["location"], "/?auth=invalid");

    // Logout clears the session server-side.
    let out: serde_json::Value = c.post(format!("{base}/api/auth/logout")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(out["ok"], true);
}

#[tokio::test]
async fn profile_and_cards_endpoints() {
    let (base, _db) = spawn_app().await;
    let c = client();
    c.get(format!("{base}/api/me")).send().await.unwrap();
    let upd: serde_json::Value = c
        .put(format!("{base}/api/profile"))
        .json(&serde_json::json!({"display_name": "wyrm"}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(upd["display_name"], "wyrm");
    let bad = c
        .put(format!("{base}/api/profile"))
        .json(&serde_json::json!({"display_name": "https://x.example"}))
        .send().await.unwrap();
    assert_eq!(bad.status(), 400);

    // Card search for the editor.
    let cards: serde_json::Value = c
        .get(format!("{base}/api/cards?q=sure%20gamble")).send().await.unwrap()
        .json().await.unwrap();
    assert!(cards.as_array().unwrap().iter().any(|x| x["title"] == "Sure Gamble"));
    let ids: serde_json::Value = c
        .get(format!("{base}/api/cards?q=hoshiko&type=Identity")).send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(ids[0]["type"], "Identity");

    // Validate endpoint works sessionless on unsaved drafts.
    let v: serde_json::Value = c
        .post(format!("{base}/api/decks/validate"))
        .json(&serde_json::json!({
            "name": "draft",
            "identity": {"title": "Weyland Consortium: Building a Better World"},
            "cards": [{"title": "Hedge Fund", "qty": 4}],
        }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(v["legal"], false);
    assert!(v["problems"].as_array().unwrap().iter().any(|p| p["code"] == "copy-limit"));
}
