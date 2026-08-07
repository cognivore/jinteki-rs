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
use jinteki_cr::object::{PrintedCard, Side};
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
    cr::deck_specs()
        .into_iter()
        .find(|s| s.key == key || display_slug(s.display_name) == key)
}

/// The display name as a key-shaped slug ("Mezzie's Making Stars" →
/// "mezzie-s-making-stars" family, tolerant of the picker's own collapse to
/// "mezzie-making-stars"): a client that keys a builtin by its shown name
/// still seats the right deck. Canonical keys stay canonical everywhere the
/// server speaks; this is an accepted spelling at the door, not a second id.
fn display_slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let collapsed = out.trim_matches('-').to_string();
    // The picker drops the possessive's orphaned "s" ("mezzie-s-…" →
    // "mezzie-…"); accept both spellings.
    collapsed.replace("-s-", "-")
}

pub fn is_builtin_key(key: &str) -> bool {
    builtin_spec(key).is_some()
}

fn side_key(side: Side) -> &'static str {
    match side {
        Side::Corp => "corp",
        Side::Runner => "runner",
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
        // Complete card by card, never played as a deck. The client shows it
        // as a badge; a player who wants a known-good table can still pick
        // the two that have been played for weeks.
        "untested": spec.untested,
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
        "untested": spec.untested,
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

// ── the deck→table seam ────────────────────────────────────────────────────
//
// A lobby seat stores a deck KEY; a game needs printed cards. The resolver
// is the only place a key becomes a deck, and CR 1.4.2 stands at its door:
// "Each deck must meet all requirements in this section to be legal for
// play" — an illegal user deck is refused a table, with the same problems
// the builder shows. Built-in keys resolve through `cr::expand`, the exact
// expansion the stock setup has always used, so the defaults' behaviour is
// unchanged by construction. `cr::setup_from(corp, runner, seed)` then turns
// two resolved decks into the VM's `GameSetup`.

/// A deck key resolved to what a table seats: printed cards, one per copy.
#[derive(Debug)]
pub struct TableDeck {
    pub key: String,
    /// Display name (built-ins: the Mezzie names; user decks: their own).
    pub name: String,
    pub side: Side,
    pub identity: PrintedCard,
    pub cards: Vec<PrintedCard>,
    /// CR 1.5.4a additional identities. Runner decks bring the eternal-
    /// filtered pile (built-ins their spec's, user decks the generalised
    /// one); Corp decks bring none — 1.5.4a's pile is the Runner's.
    pub pile: Vec<PrintedCard>,
    /// CR 1.5.1/1.5.3a: the cards from outside the deck the IDENTITY requires
    /// its player to bring along with it — Adam's directives. Not the deck
    /// ("these cards are not considered part of your deck") and not the
    /// 1.5.4a pile either: these begin the game installed (1.5.3b), which is
    /// why they are brought rather than drawn. Empty for every identity that
    /// prints no such fact, which is every identity but Adam.
    pub extra: Vec<PrintedCard>,
}

/// Why a key did not become a deck. `to_json()` is the wire shape the lobby
/// turns into its ws error frame.
#[derive(Debug)]
pub enum DeckRefusal {
    /// No such deck for this account (or a key shaped like nothing).
    NotFound { key: String },
    /// Saved but not legal — CR 1.4.2 refuses it a table. The problems are
    /// the validator's, exactly as the deck builder shows them.
    Illegal { key: String, problems: Vec<eternal::Problem> },
    /// A card the engine cannot seat (defensive: the validator already
    /// reports these as `unsupported` problems, so this fires only on
    /// drift between the catalog and the card layer).
    Unbuildable { key: String, missing: Vec<String> },
    /// The deck's side is not the seat's side.
    WrongSide {
        key: String,
        want: &'static str,
        got: &'static str,
    },
}

impl DeckRefusal {
    /// `{"error":"deck-refused","reason":…,"key":…,"message":…}` with
    /// `problems` attached when the reason is `illegal` — the payload a
    /// lobby error frame carries so the seat can say WHY, not just no.
    pub fn to_json(&self) -> Value {
        match self {
            DeckRefusal::NotFound { key } => json!({
                "error": "deck-refused",
                "reason": "not-found",
                "key": key,
                "message": format!("no deck under the key {key:?} for this account"),
            }),
            DeckRefusal::Illegal { key, problems } => json!({
                "error": "deck-refused",
                "reason": "illegal",
                "key": key,
                "message": "this deck is not legal for play (CR 1.4.2) — fix the listed problems first",
                "problems": problems,
            }),
            DeckRefusal::Unbuildable { key, missing } => json!({
                "error": "deck-refused",
                "reason": "unbuildable",
                "key": key,
                "message": format!("the engine cannot seat: {}", missing.join(", ")),
            }),
            DeckRefusal::WrongSide { key, want, got } => json!({
                "error": "deck-refused",
                "reason": "wrong-side",
                "key": key,
                "message": format!("this seat plays {want} and the deck {key:?} is a {got} deck"),
            }),
        }
    }
}

/// A built-in key as a table deck — `cr::expand`, today's stock expansion,
/// wrapped in the resolver's shape. Refuses only on side mismatch or on an
/// incomplete card layer (the same condition the SYS-D-12 readiness gate
/// reports with its richer payload; this is the backstop for callers that
/// reach the resolver directly).
pub fn resolve_builtin(key: &str, want: Side) -> Result<TableDeck, DeckRefusal> {
    let Some(spec) = builtin_spec(key) else {
        return Err(DeckRefusal::NotFound { key: key.into() });
    };
    if spec.side != want {
        return Err(DeckRefusal::WrongSide {
            key: key.into(),
            want: side_key(want),
            got: side_key(spec.side),
        });
    }
    let missing: Vec<String> = jinteki_cards::deck_named(spec.key)
        .unwrap_or_default()
        .iter()
        .filter(|c| !c.is_complete())
        .map(|c| c.name().to_string())
        .collect();
    if !missing.is_empty() {
        return Err(DeckRefusal::Unbuildable { key: key.into(), missing });
    }
    let (cards, identity, pile) = cr::expand(spec);
    let Some(identity) = identity else {
        return Err(DeckRefusal::Unbuildable {
            key: key.into(),
            missing: vec!["its identity card".into()],
        });
    };
    let extra = eternal::starting_extra_cards(&identity)
        .map_err(|missing| DeckRefusal::Unbuildable { key: key.into(), missing })?;
    Ok(TableDeck {
        key: spec.key.into(),
        name: spec.display_name.into(),
        side: spec.side,
        identity,
        cards,
        pile,
        extra,
    })
}

/// THE seam: a seat's chosen deck key, resolved for its table. Built-in
/// keys are everyone's; a `user-<id>` key must belong to `owner_id`. A user
/// deck is validated here and an illegal one refused (CR 1.4.2) with the
/// builder's own problems, so the lobby can surface exactly what the deck
/// screen shows. Every seated card is engine-complete by construction: the
/// validator's `unsupported` check ran, and the build step double-checks.
pub fn resolve_for_table(
    conn: &Connection,
    key: &str,
    owner_id: &str,
    want: Side,
) -> Result<TableDeck, DeckRefusal> {
    if is_builtin_key(key) {
        return resolve_builtin(key, want);
    }
    let row = row_id_of(key)
        .and_then(|row_id| get_owned(conn, row_id, owner_id))
        .ok_or_else(|| DeckRefusal::NotFound { key: key.into() })?;
    let verdict = eternal::validate(&row.identity, &row.cards);
    if !verdict.legal {
        return Err(DeckRefusal::Illegal { key: key.into(), problems: verdict.problems });
    }
    // Legal ⇒ the identity resolves, every card resolves and is supported;
    // anything else below is catalog/card-layer drift, reported not paniced.
    let mut missing: Vec<String> = Vec::new();
    let mut seat = |id: &str| -> Option<PrintedCard> {
        let title = match carddata::by_nsg_id(id) {
            Some(c) => c.title.as_str(),
            None => {
                missing.push(id.to_string());
                return None;
            }
        };
        match jinteki_cards::find(title).filter(|c| c.is_complete()) {
            Some(c) => Some(c.printed),
            None => {
                missing.push(title.to_string());
                None
            }
        }
    };
    let identity = seat(&row.identity);
    let mut cards: Vec<PrintedCard> = Vec::new();
    for (id, qty) in &row.cards {
        if let Some(printed) = seat(id) {
            for _ in 0..*qty {
                cards.push(printed.clone());
            }
        }
    }
    if !missing.is_empty() {
        return Err(DeckRefusal::Unbuildable { key: key.into(), missing });
    }
    let identity = identity.expect("missing is empty, so the identity seated");
    if identity.side != want {
        return Err(DeckRefusal::WrongSide {
            key: key.into(),
            want: side_key(want),
            got: side_key(identity.side),
        });
    }
    let pile = match want {
        // CR 1.5.4a: the pile is the Runner's, generalised for user decks.
        Side::Runner => eternal::runner_identity_pile(identity.name),
        Side::Corp => Vec::new(),
    };
    // CR 1.5.1/1.5.3a: a user deck brings the extra cards its identity
    // requires, exactly as the built-in path does. `validate` above already
    // refused an identity whose extras the engine cannot supply, so this is
    // the same backstop `missing` is: reported, never paniced.
    let extra = eternal::starting_extra_cards(&identity)
        .map_err(|missing| DeckRefusal::Unbuildable { key: key.into(), missing })?;
    Ok(TableDeck {
        key: key.into(),
        name: row.name,
        side: want,
        identity,
        cards,
        pile,
        extra,
    })
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

    // ── the deck→table seam ────────────────────────────────────────────────

    /// Built-in keys resolve to exactly the stock expansion — same counts,
    /// same identity, same eternal-filtered pile (no Boris) — and refuse
    /// the wrong seat.
    #[test]
    fn builtin_keys_resolve_unchanged() {
        let runner = resolve_builtin("andromeda", Side::Runner).expect("stock deck resolves");
        assert_eq!(runner.name, "Mezzie's Andromeda");
        assert_eq!(runner.identity.name, "Andromeda: Dispossessed Ristie");
        assert_eq!(runner.cards.len(), 45, "the printed list, by copies");
        assert!(!runner.pile.is_empty());
        assert!(
            !runner.pile.iter().any(|c| c.name.starts_with("Boris")),
            "the eternal pile filter holds through the resolver"
        );
        let corp = resolve_builtin("gauntlet", Side::Corp).expect("stock deck resolves");
        assert_eq!(corp.cards.len(), 49);
        assert!(corp.pile.is_empty(), "CR 1.5.4a: the pile is the Runner's");
        assert!(matches!(
            resolve_builtin("andromeda", Side::Corp),
            Err(DeckRefusal::WrongSide { .. })
        ));
        let setup = cr::setup_from(corp, runner, 11);
        assert_eq!(setup.corp_deck.len(), 49);
        assert_eq!(setup.runner_deck.len(), 45);
        assert!(setup.corp_identity.is_some() && setup.runner_identity.is_some());
    }

    /// A saved LEGAL user deck starts: the table's cards match the stored
    /// map (counts and identity), and a Runner deck brings the generalised
    /// CR 1.5.4a pile — cross-faction, eternally filtered, never itself.
    #[test]
    fn a_saved_legal_user_deck_reaches_the_table() {
        let (db, user) = setup();
        let conn = db.blocking_lock();

        // Corp: a saved copy of the (legal) Gauntlet list.
        let (identity, cards) = builtin_cards(&cr::GAUNTLET);
        let draft = EternalDraft { name: "my stars".into(), identity, cards: cards.clone() }
            .checked()
            .unwrap();
        let key = create(&conn, &user, &draft).unwrap()["key"].as_str().unwrap().to_string();
        let table = resolve_for_table(&conn, &key, &user, Side::Corp).expect("legal deck seats");
        assert_eq!(table.name, "my stars");
        assert_eq!(table.identity.name, "Nebula Talent Management: Making Stars");
        assert_eq!(table.cards.len() as u32, cards.values().sum::<u32>());
        for (id, qty) in &cards {
            let title = carddata::by_nsg_id(id).unwrap().title.as_str();
            let seated = table.cards.iter().filter(|c| c.name == title).count();
            assert_eq!(seated as u32, *qty, "{title} arrives in its stored count");
        }
        assert!(table.pile.is_empty(), "a Corp deck brings no 1.5.4a pile");

        // Runner: a saved copy of the Andromeda list gets the pile.
        let (identity, cards) = builtin_cards(&cr::ANDROMEDA);
        let draft = EternalDraft { name: "my ristie".into(), identity, cards }
            .checked()
            .unwrap();
        let rkey = create(&conn, &user, &draft).unwrap()["key"].as_str().unwrap().to_string();
        let rtable = resolve_for_table(&conn, &rkey, &user, Side::Runner).expect("legal deck seats");
        assert_eq!(rtable.cards.len(), 45);
        assert!(!rtable.pile.is_empty());
        assert!(!rtable.pile.iter().any(|c| c.name.starts_with("Boris")), "no Rebirth into Boris");
        assert!(
            !rtable.pile.iter().any(|c| c.name == rtable.identity.name),
            "1.5.4: identities OTHER than the selected one"
        );
        assert!(
            rtable.pile.iter().any(|c| c.name == "Chaos Theory: W\u{fc}nderkind"),
            "the user-deck pile crosses factions (Rebirth narrows it itself)"
        );

        // The pair assembles into a playable setup.
        let setup = cr::setup_from(table, rtable, 5);
        assert_eq!(setup.corp_deck.len(), 49);
        assert_eq!(setup.runner_deck.len(), 45);
    }

    /// CR 1.4.2 at the table door: an illegal deck is refused with the
    /// builder's own problems, in the wire shape the lobby renders.
    #[test]
    fn an_illegal_user_deck_is_refused_with_its_problems() {
        let (db, user) = setup();
        let conn = db.blocking_lock();
        let draft = EternalDraft {
            name: "wip".into(),
            identity: "andromeda_dispossessed_ristie".into(),
            cards: [("sure_gamble".to_string(), 3u32)].into_iter().collect(),
        }
        .checked()
        .unwrap();
        let key = create(&conn, &user, &draft).unwrap()["key"].as_str().unwrap().to_string();
        let refusal = match resolve_for_table(&conn, &key, &user, Side::Runner) {
            Err(r @ DeckRefusal::Illegal { .. }) => r,
            other => panic!("an illegal deck must be refused, got {other:?}"),
        };
        let wire = refusal.to_json();
        assert_eq!(wire["error"], "deck-refused");
        assert_eq!(wire["reason"], "illegal");
        assert_eq!(wire["key"], key.as_str());
        assert!(wire["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["code"] == "deck_size"));

        // Ownership and side are seat conditions too.
        let other = auth::mint_anon(&conn).unwrap().user_id;
        assert!(matches!(
            resolve_for_table(&conn, &key, &other, Side::Runner),
            Err(DeckRefusal::NotFound { .. })
        ));
        assert!(matches!(
            resolve_for_table(&conn, "user-nonesuch", &user, Side::Runner),
            Err(DeckRefusal::NotFound { .. })
        ));
    }

    // ── CR 1.5.3a: the extra cards an identity brings ─────────────────────

    /// The built-in decks bring nothing from outside their decks and are
    /// untouched by the seam that carries Adam's directives.
    #[test]
    fn the_builtin_decks_bring_no_extra_cards() {
        let runner = resolve_builtin("andromeda", Side::Runner).expect("stock deck resolves");
        let corp = resolve_builtin("gauntlet", Side::Corp).expect("stock deck resolves");
        assert!(runner.extra.is_empty(), "Andromeda prints no 1.5.3 setup fact");
        assert!(corp.extra.is_empty(), "Making Stars prints no 1.5.3 setup fact");
        let setup = cr::setup_from(corp, runner, 11);
        for side in [Side::Corp, Side::Runner] {
            assert!(
                setup.extra_cards.get(&side).is_none_or(Vec::is_empty),
                "the stock table is unchanged: no {side:?} extra cards"
            );
        }
        // And the game the stock pair assembles still starts.
        let vm = jinteki_cr::Vm::new_game(setup);
        assert_eq!(vm.st.deck[&Side::Corp].len() + vm.st.hand[&Side::Corp].len(), 49);
    }

    /// A saved user deck whose identity is Adam is refused at the door, and
    /// the refusal the lobby renders says which card is in the way.
    ///
    /// SYS-D-12 is the rule: "no card shall be playable in a game unless its
    /// behavior is implemented". A directive is not a deck card that may
    /// never be drawn — CR 1.5.3b starts the game with it installed and
    /// 1.5.3d makes it an ordinary installed card from that moment — so it
    /// is played in every game it comes to, and one unimplemented sentence
    /// (Always Be Running's first) refuses the whole deck. This is the same
    /// reading `cr::readiness()` already applies to the 1.5.4a identity pile:
    /// a card that comes to the table with the deck is gated like the deck.
    #[test]
    fn an_adam_user_deck_is_refused_while_a_directive_is_incomplete() {
        let (db, user) = setup();
        let conn = db.blocking_lock();
        let draft = EternalDraft {
            name: "compulsive".into(),
            identity: "adam_compulsive_hacker".into(),
            // Adam's own faction cards, so nothing here is the reason.
            cards: [
                ("neutralize_all_threats".to_string(), 3u32),
                ("safety_first".to_string(), 3),
                ("sure_gamble".to_string(), 3),
            ]
            .into_iter()
            .collect(),
        }
        .checked()
        .unwrap();
        let key = create(&conn, &user, &draft).unwrap()["key"].as_str().unwrap().to_string();
        let refusal = match resolve_for_table(&conn, &key, &user, Side::Runner) {
            Err(r) => r,
            Ok(t) => panic!("Adam seated with {} extra cards", t.extra.len()),
        };
        let wire = refusal.to_json();
        assert_eq!(wire["error"], "deck-refused");
        let said = serde_json::to_string(&wire).unwrap();
        assert!(
            said.contains("Always Be Running"),
            "the refusal must name the card in the way: {said}"
        );
    }

    /// The seam itself, proved end to end: an identity's extra cards travel
    /// deck → `TableDeck::extra` → `GameSetup::extra_cards` → the rig, and
    /// never through the stack.
    ///
    /// The pile is handed in directly rather than through
    /// `eternal::starting_extra_cards`, because that gate refuses Adam today
    /// (see `an_adam_user_deck_is_refused_while_a_directive_is_incomplete`).
    /// What is under test is the wiring the gate opens onto: with
    /// `extra_cards: Default::default()` — the bug this replaced — the three
    /// directives never reach `Zone::OutsideGame` and 1.6.2's selection
    /// cannot be made at all.
    #[test]
    fn an_identitys_extra_cards_start_installed_and_never_enter_the_stack() {
        use jinteki_cards::decks::identities::runner_adam;
        let adam = jinteki_cards::find("Adam: Compulsive Hacker").expect("Adam is implemented");
        let directives: Vec<PrintedCard> =
            runner_adam::directives().into_iter().map(|c| c.printed).collect();
        let names: Vec<&str> = directives.iter().map(|c| c.name).collect();
        assert_eq!(names.len(), 3, "CR 1.5.3a: three differently named directives");

        let corp = resolve_builtin("gauntlet", Side::Corp).expect("stock deck resolves");
        let stock = resolve_builtin("andromeda", Side::Runner).expect("stock deck resolves");
        let runner = TableDeck {
            key: "user-adam".into(),
            name: "compulsive".into(),
            side: Side::Runner,
            identity: adam.printed,
            // 1.4.3a: the extra cards are not counted in the deck, and this
            // deck is an ordinary 45.
            cards: stock.cards,
            pile: Vec::new(),
            extra: directives.clone(),
        };
        let setup = cr::setup_from(corp, runner, 7);
        assert_eq!(
            setup.extra_cards[&Side::Runner].len(),
            3,
            "the resolver's extra cards reach the setup"
        );
        assert_eq!(setup.runner_deck.len(), 45, "the directives are not part of the deck");

        let vm = jinteki_cr::Vm::new_game(setup);
        // CR 1.5.3b: all three begin the game installed in the play area.
        let installed: Vec<&str> = vm
            .st
            .objects
            .values()
            .filter(|o| o.zone == jinteki_cr::object::Zone::Rig)
            .map(|o| o.printed.name)
            .collect();
        for name in &names {
            assert!(installed.contains(name), "{name} begins the game installed: {installed:?}");
        }
        assert_eq!(installed.len(), 3, "and nothing else is installed: {installed:?}");
        for o in vm.st.objects.values().filter(|o| names.contains(&o.printed.name)) {
            assert!(o.faceup, "{} is installed faceup (4.6.4c)", o.printed.name);
            assert_eq!(o.controller, Side::Runner);
            // 1.5.3d + 1.10.5: an installed Runner card is active.
            assert!(
                jinteki_cr::object::card_active(o),
                "{} is active from 1.6 onwards",
                o.printed.name
            );
        }
        // …and none of them is in the stack or the grip — 1.5.3a's "these
        // cards are not considered part of your deck", literally.
        let stack: Vec<&str> =
            vm.st.deck[&Side::Runner].iter().map(|id| vm.st.objects[id].printed.name).collect();
        let grip: Vec<&str> =
            vm.st.hand[&Side::Runner].iter().map(|id| vm.st.objects[id].printed.name).collect();
        assert_eq!(stack.len() + grip.len(), 45, "the deck is exactly the deck");
        for name in &names {
            assert!(!stack.contains(name), "{name} is not in the stack");
            assert!(!grip.contains(name), "{name} is not in the grip");
        }
        // The pile the directives came from is emptied by 1.6.2's selection.
        assert!(
            !vm.st
                .objects
                .values()
                .any(|o| o.zone == jinteki_cr::object::Zone::OutsideGame(Side::Runner)),
            "all three were selected; nothing is left outside the game"
        );
    }
}
