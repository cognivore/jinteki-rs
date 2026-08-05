//! Eternal deck storage and the deck-list surface behind `/api/decks`.
//!
//! Decks are stored in the ids the catalog speaks (NSG v2 slugs), exactly as
//! the client sent them — `{"<id>": count}` — so nothing is lost when a deck
//! arrives with problems: legality is a verdict computed on every read
//! (`eternal::validate`), never a precondition for saving (a deck under
//! construction is still the player's deck).
//!
//! The list always begins with the two built-in defaults, `andromeda` and
//! `gauntlet` — the tables the site has always served. Their INTERNAL keys
//! are load-bearing (`jinteki_cards::deck_named`, the CR readiness gate, the
//! lobby); what changed is the display layer: they are shown as
//! "Mezzie's Andromeda" and "Mezzie's Making Stars" (`cr::DeckSpec::
//! display_name`). Built-ins cannot be edited or deleted (403).
//!
//! User decks are keyed `user-<token>` and owned by the session's account.

use crate::carddata;
use crate::cr::{self, DeckSpec};
use crate::db::new_token;
use crate::eternal;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The POST/PUT body — the exact wire contract:
/// `{"name": …, "identity": "<id>", "cards": {"<id>": count, …}}`.
#[derive(Debug, Clone, Deserialize)]
pub struct EternalDraft {
    pub name: String,
    pub identity: String,
    #[serde(default)]
    pub cards: BTreeMap<String, u32>,
}

impl EternalDraft {
    /// Shape checks that make a request unsaveable (unlike deck problems,
    /// which save fine and come back in `problems`).
    pub fn checked(mut self) -> Result<EternalDraft, String> {
        let name = self.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 120 {
            return Err("deck name must be 1-120 characters".into());
        }
        if self.identity.trim().is_empty() {
            return Err("an identity card id is required".into());
        }
        if self.cards.len() > 1000 {
            return Err("too many distinct cards".into());
        }
        self.cards.retain(|_, qty| *qty > 0);
        self.name = name;
        Ok(self)
    }
}

// ── built-in defaults ──────────────────────────────────────────────────────

/// A built-in deck list as (identity id, id → copies), in the catalog's id
/// vocabulary. The lists live in `cr.rs` as titles (the card layer's key);
/// every title of both shipped decks carries an NSG id.
fn builtin_cards(spec: &DeckSpec) -> (String, BTreeMap<String, u32>) {
    let mut identity = String::new();
    let mut cards = BTreeMap::new();
    for (title, qty) in spec.list {
        let Some(c) = carddata::by_title(title) else { continue };
        let Some(id) = c.nsg_id.clone() else { continue };
        if c.is_identity() {
            identity = id;
        } else {
            cards.insert(id, *qty);
        }
    }
    (identity, cards)
}

fn builtin_spec(key: &str) -> Option<&'static DeckSpec> {
    cr::deck_specs().into_iter().find(|s| s.key == key)
}

pub fn is_builtin_key(key: &str) -> bool {
    builtin_spec(key).is_some()
}

fn side_key(side: jinteki_cr::object::Side) -> &'static str {
    match side {
        jinteki_cr::object::Side::Corp => "corp",
        jinteki_cr::object::Side::Runner => "runner",
    }
}

fn builtin_summary(spec: &'static DeckSpec) -> Value {
    let (identity, cards) = builtin_cards(spec);
    let verdict = eternal::validate(&identity, &cards);
    json!({
        "key": spec.key,
        "name": spec.display_name,
        "builtin": true,
        "side": side_key(spec.side),
        "identity": identity,
        "legal": verdict.legal,
    })
}

fn builtin_full(spec: &'static DeckSpec) -> Value {
    let (identity, cards) = builtin_cards(spec);
    let verdict = eternal::validate(&identity, &cards);
    json!({
        "key": spec.key,
        "name": spec.display_name,
        "identity": identity,
        "cards": cards,
        "legal": verdict.legal,
        "problems": verdict.problems,
    })
}

// ── user decks ─────────────────────────────────────────────────────────────

struct Row {
    id: String,
    name: String,
    identity: String,
    cards: BTreeMap<String, u32>,
    updated_at: String,
}

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    let cards_json: String = r.get("cards_json")?;
    Ok(Row {
        id: r.get("id")?,
        name: r.get("name")?,
        identity: r.get("identity")?,
        cards: serde_json::from_str(&cards_json).unwrap_or_default(),
        updated_at: r.get("updated_at")?,
    })
}

fn user_key(row_id: &str) -> String {
    format!("user-{row_id}")
}

/// `user-<token>` → `<token>`; None for any other key shape.
fn row_id_of(key: &str) -> Option<&str> {
    key.strip_prefix("user-").filter(|rest| !rest.is_empty())
}

fn summary(row: &Row) -> Value {
    let verdict = eternal::validate(&row.identity, &row.cards);
    let side = carddata::by_nsg_id(&row.identity)
        .map(|c| c.side.to_lowercase())
        .unwrap_or_default();
    json!({
        "key": user_key(&row.id),
        "name": row.name,
        "builtin": false,
        "side": side,
        "identity": row.identity,
        "legal": verdict.legal,
        "updated_at": row.updated_at,
    })
}

fn full(row: &Row) -> Value {
    let verdict = eternal::validate(&row.identity, &row.cards);
    json!({
        "key": user_key(&row.id),
        "name": row.name,
        "identity": row.identity,
        "cards": row.cards,
        "legal": verdict.legal,
        "problems": verdict.problems,
    })
}

fn get_owned(conn: &Connection, row_id: &str, owner_id: &str) -> Option<Row> {
    conn.query_row(
        "SELECT id, name, identity, cards_json, updated_at
         FROM eternal_decks WHERE id = ?1 AND owner_id = ?2",
        params![row_id, owner_id],
        |r| row_from(r),
    )
    .optional()
    .ok()
    .flatten()
}

/// GET /api/decks — the session's deck list, defaults always first.
pub fn list_json(conn: &Connection, owner_id: &str) -> Value {
    let mut decks: Vec<Value> = cr::deck_specs().into_iter().map(builtin_summary).collect();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, identity, cards_json, updated_at
             FROM eternal_decks WHERE owner_id = ?1 ORDER BY updated_at DESC, id",
        )
        .expect("prepare eternal deck list");
    let rows = stmt
        .query_map([owner_id], |r| row_from(r))
        .expect("query eternal decks")
        .filter_map(Result::ok);
    decks.extend(rows.map(|row| summary(&row)));
    json!({ "decks": decks })
}

/// POST /api/decks — save (even if illegal, marked) and report.
pub fn create(conn: &Connection, owner_id: &str, draft: &EternalDraft) -> rusqlite::Result<Value> {
    let id = new_token();
    conn.execute(
        "INSERT INTO eternal_decks (id, owner_id, name, identity, cards_json,
                                    created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))",
        params![
            id,
            owner_id,
            draft.name,
            draft.identity,
            serde_json::to_string(&draft.cards).expect("card map serializes"),
        ],
    )?;
    let verdict = eternal::validate(&draft.identity, &draft.cards);
    Ok(json!({
        "key": user_key(&id),
        "legal": verdict.legal,
        "problems": verdict.problems,
    }))
}

pub enum WriteOutcome {
    Ok(Value),
    Builtin,
    NotFound,
}

/// PUT /api/decks/<key> — built-ins are immutable (403 upstream).
pub fn update(
    conn: &Connection,
    key: &str,
    owner_id: &str,
    draft: &EternalDraft,
) -> WriteOutcome {
    if is_builtin_key(key) {
        return WriteOutcome::Builtin;
    }
    let Some(row_id) = row_id_of(key) else {
        return WriteOutcome::NotFound;
    };
    let n = conn
        .execute(
            "UPDATE eternal_decks SET name = ?1, identity = ?2, cards_json = ?3,
                    updated_at = datetime('now')
             WHERE id = ?4 AND owner_id = ?5",
            params![
                draft.name,
                draft.identity,
                serde_json::to_string(&draft.cards).expect("card map serializes"),
                row_id,
                owner_id
            ],
        )
        .unwrap_or(0);
    if n == 0 {
        return WriteOutcome::NotFound;
    }
    let verdict = eternal::validate(&draft.identity, &draft.cards);
    WriteOutcome::Ok(json!({
        "key": key,
        "legal": verdict.legal,
        "problems": verdict.problems,
    }))
}

/// GET /api/decks/<key> — built-in or owned user deck, full payload.
pub fn get_json(conn: &Connection, key: &str, owner_id: &str) -> Option<Value> {
    if let Some(spec) = builtin_spec(key) {
        return Some(builtin_full(spec));
    }
    let row_id = row_id_of(key)?;
    get_owned(conn, row_id, owner_id).map(|row| full(&row))
}

/// DELETE /api/decks/<key>.
pub fn delete(conn: &Connection, key: &str, owner_id: &str) -> WriteOutcome {
    if is_builtin_key(key) {
        return WriteOutcome::Builtin;
    }
    let Some(row_id) = row_id_of(key) else {
        return WriteOutcome::NotFound;
    };
    let n = conn
        .execute(
            "DELETE FROM eternal_decks WHERE id = ?1 AND owner_id = ?2",
            params![row_id, owner_id],
        )
        .unwrap_or(0);
    if n == 0 {
        WriteOutcome::NotFound
    } else {
        WriteOutcome::Ok(json!({ "ok": true }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth;
    use crate::db::Db;

    fn setup() -> (Db, String) {
        let db = Db::open_in_memory().unwrap();
        let user = {
            let conn = db.blocking_lock();
            auth::mint_anon(&conn).unwrap().user_id
        };
        (db, user)
    }

    #[test]
    fn defaults_lead_the_list_with_display_names_and_are_legal() {
        let (db, user) = setup();
        let conn = db.blocking_lock();
        let v = list_json(&conn, &user);
        let decks = v["decks"].as_array().unwrap();
        assert_eq!(decks[0]["key"], "andromeda");
        assert_eq!(decks[0]["name"], "Mezzie's Andromeda");
        assert_eq!(decks[0]["builtin"], true);
        assert_eq!(decks[0]["side"], "runner");
        assert_eq!(decks[1]["key"], "gauntlet");
        assert_eq!(
            decks[1]["name"], "Mezzie's Making Stars",
            "the display layer names the deck; the internal key stays gauntlet"
        );
        assert_eq!(decks[1]["builtin"], true);
        // Both shipped decks are legal Eternal decks (15/15 influence, 7/7
        // points, the corp band at 20/49).
        assert_eq!(decks[0]["legal"], true);
        assert_eq!(decks[1]["legal"], true);
    }

    #[test]
    fn builtin_full_payload_speaks_catalog_ids() {
        let (db, user) = setup();
        let conn = db.blocking_lock();
        let v = get_json(&conn, "gauntlet", &user).unwrap();
        assert_eq!(v["identity"], "nebula_talent_management_making_stars");
        assert_eq!(v["cards"]["astroscript_pilot_program"], 3);
        assert_eq!(v["legal"], true);
        assert_eq!(v["problems"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn crud_round_trip_saves_illegal_decks_marked() {
        let (db, user) = setup();
        let conn = db.blocking_lock();
        let draft = EternalDraft {
            name: "wip".into(),
            identity: "andromeda_dispossessed_ristie".into(),
            cards: [("sure_gamble".to_string(), 3u32)].into_iter().collect(),
        }
        .checked()
        .unwrap();
        let saved = create(&conn, &user, &draft).unwrap();
        let key = saved["key"].as_str().unwrap().to_string();
        assert!(key.starts_with("user-"));
        assert_eq!(saved["legal"], false, "3 cards < 45 — saved anyway, marked");
        assert!(saved["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["code"] == "deck_size"));

        let got = get_json(&conn, &key, &user).unwrap();
        assert_eq!(got["cards"]["sure_gamble"], 3);
        assert_eq!(got["name"], "wip");

        // Another account cannot see it.
        let other = auth::mint_anon(&conn).unwrap().user_id;
        assert!(get_json(&conn, &key, &other).is_none());

        match update(&conn, &key, &user, &draft) {
            WriteOutcome::Ok(v) => assert_eq!(v["key"], key.as_str()),
            _ => panic!("update owned deck"),
        }
        match delete(&conn, &key, &user) {
            WriteOutcome::Ok(_) => {}
            _ => panic!("delete owned deck"),
        }
        assert!(get_json(&conn, &key, &user).is_none());
    }

    #[test]
    fn builtins_refuse_writes() {
        let (db, user) = setup();
        let conn = db.blocking_lock();
        assert!(matches!(delete(&conn, "gauntlet", &user), WriteOutcome::Builtin));
        let draft = EternalDraft {
            name: "x".into(),
            identity: "andromeda_dispossessed_ristie".into(),
            cards: BTreeMap::new(),
        };
        assert!(matches!(update(&conn, "andromeda", &user, &draft), WriteOutcome::Builtin));
    }
}
