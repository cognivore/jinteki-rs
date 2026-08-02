//! Decklists: canonicalization, CRUD, publish/fork, the public library, and
//! seed content (ACCOUNTS-AND-DECKS.md §6).

use crate::auth::SYSTEM_USER_ID;
use crate::carddata;
use crate::db::{audit, new_token};
use crate::deckcheck::{self, DeckLine, Verdict};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Wire/storage deck line (§6.2): canonical title + latest code + qty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireLine {
    pub title: String,
    pub code: String,
    pub qty: u32,
}

/// A deck draft as accepted from clients (POST/PUT body).
#[derive(Debug, Clone, Deserialize)]
pub struct DeckDraft {
    pub name: String,
    pub identity: IdentityRef,
    #[serde(default)]
    pub cards: Vec<DraftLine>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub source: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdentityRef {
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DraftLine {
    pub title: String,
    #[serde(default)]
    pub qty: u32,
}

/// Canonicalized, validated deck ready to store.
pub struct Canonical {
    pub name: String,
    pub side: String, // 'corp' | 'runner'
    pub identity_title: String,
    pub identity_code: String,
    pub cards: Vec<WireLine>,
    pub notes: String,
    pub source: Option<Value>,
}

/// Re-canonicalize on write (§6.2, SYS-K-2): titles looked up, codes
/// force-set to the latest printing, unknown titles rejected naming the
/// offender. A client cannot smuggle a mismatched title/code pair.
pub fn canonicalize(draft: &DeckDraft) -> Result<Canonical, String> {
    let name = draft.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err("deck name must be 1-120 characters".into());
    }
    let id = carddata::by_title(&draft.identity.title)
        .ok_or_else(|| format!("unknown identity \"{}\"", draft.identity.title))?;
    if !id.is_identity() {
        return Err(format!("\"{}\" is not an identity card", id.title));
    }
    let side = match id.side.as_str() {
        "Corp" => "corp",
        "Runner" => "runner",
        other => return Err(format!("\"{}\" is a {} card, not a playable identity", id.title, other)),
    };
    let mut cards = Vec::with_capacity(draft.cards.len());
    for line in &draft.cards {
        if line.qty == 0 {
            continue;
        }
        let c = carddata::by_title(&line.title)
            .ok_or_else(|| format!("unknown card \"{}\"", line.title))?;
        cards.push(WireLine { title: c.title.clone(), code: c.code.clone(), qty: line.qty });
    }
    if draft.notes.len() > 10_000 {
        return Err("notes too long".into());
    }
    Ok(Canonical {
        name: name.to_string(),
        side: side.to_string(),
        identity_title: id.title.clone(),
        identity_code: id.code.clone(),
        cards,
        notes: draft.notes.clone(),
        source: draft.source.clone(),
    })
}

pub fn validate(identity_title: &str, cards: &[WireLine]) -> Verdict {
    let lines: Vec<DeckLine> = cards
        .iter()
        .map(|c| DeckLine { title: c.title.clone(), qty: c.qty })
        .collect();
    deckcheck::check(identity_title, &lines)
}

fn identity_json(title: &str) -> Value {
    let code = carddata::by_title(title).map(|c| c.code.clone()).unwrap_or_default();
    json!({ "title": title, "code": code })
}

/// Full deck payload (§6.2 shape) + validation + per-card impl status.
fn deck_json(row: &DeckRow, include_owner: bool) -> Value {
    let cards: Vec<WireLine> = serde_json::from_str(&row.cards_json).unwrap_or_default();
    let verdict = validate(&row.identity_title, &cards);
    let mut v = json!({
        "id": row.id,
        "name": row.name,
        "side": row.side,
        "format": row.format,
        "identity": identity_json(&row.identity_title),
        "cards": verdict.cards,
        "notes": row.notes,
        "source": row.source_json.as_deref().and_then(|s| serde_json::from_str::<Value>(s).ok()),
        "published_at": row.published_at,
        "created_at": row.created_at,
        "updated_at": row.updated_at,
        "validation": {
            "legal": verdict.legal,
            "problems": verdict.problems,
            "warnings": verdict.warnings,
            "counts": verdict.counts,
            "playable": verdict.playable,
        },
    });
    if include_owner {
        v["owner_id"] = json!(row.owner_id);
    }
    v
}

pub struct DeckRow {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub side: String,
    pub identity_title: String,
    pub format: String,
    pub cards_json: String,
    pub notes: String,
    pub source_json: Option<String>,
    pub published_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn row_from(r: &rusqlite::Row) -> rusqlite::Result<DeckRow> {
    Ok(DeckRow {
        id: r.get("id")?,
        owner_id: r.get("owner_id")?,
        name: r.get("name")?,
        side: r.get("side")?,
        identity_title: r.get("identity_title")?,
        format: r.get("format")?,
        cards_json: r.get("cards_json")?,
        notes: r.get("notes")?,
        source_json: r.get("source_json")?,
        published_at: r.get("published_at")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

const DECK_COLS: &str = "id, owner_id, name, side, identity_title, format, cards_json, notes, source_json, published_at, created_at, updated_at";

pub fn get(conn: &Connection, deck_id: &str) -> Option<DeckRow> {
    conn.query_row(
        &format!("SELECT {DECK_COLS} FROM decks WHERE id = ?1"),
        [deck_id],
        |r| row_from(r),
    )
    .optional()
    .ok()
    .flatten()
}

/// Owned deck, full payload.
pub fn get_owned_json(conn: &Connection, deck_id: &str, owner_id: &str) -> Option<Value> {
    let row = get(conn, deck_id)?;
    if row.owner_id != owner_id {
        return None;
    }
    Some(deck_json(&row, false))
}

pub fn insert(conn: &Connection, owner_id: &str, c: &Canonical) -> rusqlite::Result<Value> {
    let id = new_token();
    conn.execute(
        "INSERT INTO decks (id, owner_id, name, side, identity_title, format, cards_json,
                            notes, source_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'standard', ?6, ?7, ?8, datetime('now'), datetime('now'))",
        params![
            id,
            owner_id,
            c.name,
            c.side,
            c.identity_title,
            serde_json::to_string(&c.cards).unwrap(),
            c.notes,
            c.source.as_ref().map(|s| s.to_string()),
        ],
    )?;
    Ok(deck_json(&get(conn, &id).expect("just inserted"), false))
}

pub fn update(
    conn: &Connection,
    deck_id: &str,
    owner_id: &str,
    c: &Canonical,
) -> Option<Value> {
    let n = conn
        .execute(
            "UPDATE decks SET name=?1, side=?2, identity_title=?3, cards_json=?4, notes=?5,
                    updated_at=datetime('now')
             WHERE id = ?6 AND owner_id = ?7",
            params![
                c.name,
                c.side,
                c.identity_title,
                serde_json::to_string(&c.cards).unwrap(),
                c.notes,
                deck_id,
                owner_id
            ],
        )
        .ok()?;
    if n == 0 {
        return None;
    }
    Some(deck_json(&get(conn, deck_id)?, false))
}

pub fn delete(conn: &Connection, deck_id: &str, owner_id: &str) -> bool {
    conn.execute(
        "DELETE FROM decks WHERE id = ?1 AND owner_id = ?2",
        params![deck_id, owner_id],
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

/// Deck summaries for "my decks" (§9.2): newest first, with legality tick
/// and playable roll-up so refusal is never news (SYS-K-4).
pub fn list_mine(conn: &Connection, owner_id: &str) -> Vec<Value> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {DECK_COLS} FROM decks WHERE owner_id = ?1 ORDER BY updated_at DESC"
        ))
        .expect("prepare");
    let rows = stmt
        .query_map([owner_id], |r| row_from(r))
        .expect("query")
        .filter_map(Result::ok);
    rows.map(|row| summary_json(&row, None)).collect()
}

fn summary_json(row: &DeckRow, author: Option<&str>) -> Value {
    let cards: Vec<WireLine> = serde_json::from_str(&row.cards_json).unwrap_or_default();
    let verdict = validate(&row.identity_title, &cards);
    let mut v = json!({
        "id": row.id,
        "name": row.name,
        "side": row.side,
        "identity": identity_json(&row.identity_title),
        "legal": verdict.legal,
        "playable": verdict.playable,
        "counts": verdict.counts,
        "published_at": row.published_at,
        "updated_at": row.updated_at,
    });
    if let Some(a) = author {
        v["author_name"] = json!(a);
    }
    v
}

/// Publish (owner, claimed, legal — §6.4). Returns Err with a human reason.
pub fn publish(conn: &Connection, deck_id: &str, owner_id: &str, kind: &str) -> Result<Value, String> {
    let row = get(conn, deck_id).ok_or("no such deck")?;
    if row.owner_id != owner_id {
        return Err("no such deck".into());
    }
    if kind != "claimed" {
        return Err("claim your account with an email to publish decks".into());
    }
    let cards: Vec<WireLine> = serde_json::from_str(&row.cards_json).unwrap_or_default();
    let verdict = validate(&row.identity_title, &cards);
    if !verdict.legal {
        return Err("only legal decks can be published".into());
    }
    conn.execute(
        "UPDATE decks SET published_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ?1",
        [deck_id],
    )
    .map_err(|e| e.to_string())?;
    audit(conn, Some(owner_id), "deck_published", &json!({ "deck": deck_id }));
    Ok(deck_json(&get(conn, deck_id).unwrap(), false))
}

pub fn unpublish(conn: &Connection, deck_id: &str, owner_id: &str) -> Result<Value, String> {
    let n = conn
        .execute(
            "UPDATE decks SET published_at = NULL, updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2",
            params![deck_id, owner_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("no such deck".into());
    }
    audit(conn, Some(owner_id), "deck_unpublished", &json!({ "deck": deck_id }));
    Ok(deck_json(&get(conn, deck_id).unwrap(), false))
}

/// Library browse (§6.4): published decks, suspended authors excluded,
/// author names resolved at read time. Filters: side, faction, q substring.
pub fn library_list(
    conn: &Connection,
    side: Option<&str>,
    faction: Option<&str>,
    q: Option<&str>,
    sort: Option<&str>,
    page: u32,
) -> Value {
    const PAGE: u32 = 30;
    let order = match sort {
        Some("name") => "d.name COLLATE NOCASE ASC",
        _ => "d.published_at DESC",
    };
    let mut sql = format!(
        "SELECT {}, u.display_name AS author FROM decks d
         JOIN users u ON u.id = d.owner_id
         WHERE d.published_at IS NOT NULL AND u.kind != 'suspended'",
        DECK_COLS
            .split(", ")
            .map(|c| format!("d.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut binds: Vec<String> = Vec::new();
    if let Some(s) = side.filter(|s| *s == "corp" || *s == "runner") {
        binds.push(s.to_string());
        sql.push_str(&format!(" AND d.side = ?{}", binds.len()));
    }
    // Faction filtering happens in Rust below: identity faction lives in the
    // embedded card index, not in a DB column.
    if let Some(term) = q.filter(|t| !t.trim().is_empty()) {
        binds.push(format!("%{}%", term.trim()));
        sql.push_str(&format!(
            " AND (d.name LIKE ?{n} OR d.identity_title LIKE ?{n})",
            n = binds.len()
        ));
    }
    sql.push_str(&format!(" ORDER BY {order}"));

    let mut stmt = conn.prepare(&sql).expect("library sql");
    let rows: Vec<(DeckRow, String)> = stmt
        .query_map(rusqlite::params_from_iter(binds.iter()), |r| {
            Ok((row_from(r)?, r.get::<_, String>("author")?))
        })
        .expect("library query")
        .filter_map(Result::ok)
        .collect();
    let want_faction = faction.filter(|f| !f.is_empty());
    let filtered: Vec<&(DeckRow, String)> = rows
        .iter()
        .filter(|(row, _)| match want_faction {
            None => true,
            Some(f) => carddata::by_title(&row.identity_title)
                .and_then(|c| c.faction.as_deref())
                .map(|cf| cf.eq_ignore_ascii_case(f))
                .unwrap_or(false),
        })
        .collect();
    let total = filtered.len();
    let page_rows: Vec<Value> = filtered
        .into_iter()
        .skip((page as usize) * PAGE as usize)
        .take(PAGE as usize)
        .map(|(row, author)| summary_json(row, Some(author)))
        .collect();
    json!({ "decks": page_rows, "total": total, "page": page, "page_size": PAGE })
}

/// Published deck, redacted for public read (§6.4: no owner id).
pub fn library_get(conn: &Connection, deck_id: &str) -> Option<Value> {
    let row = get(conn, deck_id)?;
    row.published_at.as_ref()?;
    let author: String = conn
        .query_row(
            "SELECT display_name FROM users WHERE id = ?1 AND kind != 'suspended'",
            [&row.owner_id],
            |r| r.get(0),
        )
        .optional()
        .ok()??;
    let mut v = deck_json(&row, false);
    v["author_name"] = json!(author);
    Some(v)
}

/// Fork a published deck into the caller's collection (§6.4): an
/// independent copy, name suffixed " (fork)", provenance recorded.
pub fn fork(conn: &Connection, deck_id: &str, new_owner: &str) -> Result<Value, String> {
    let row = get(conn, deck_id).ok_or("no such published deck")?;
    if row.published_at.is_none() {
        return Err("no such published deck".into());
    }
    let id = new_token();
    let mut name = format!("{} (fork)", row.name);
    if name.chars().count() > 120 {
        name = name.chars().take(120).collect();
    }
    conn.execute(
        "INSERT INTO decks (id, owner_id, name, side, identity_title, format, cards_json,
                            notes, source_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'), datetime('now'))",
        params![
            id,
            new_owner,
            name,
            row.side,
            row.identity_title,
            row.format,
            row.cards_json,
            row.notes,
            json!({ "kind": "fork", "deck": deck_id }).to_string(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(deck_json(&get(conn, &id).unwrap(), false))
}

/// Seed the library at first boot (§6.4): the two built-in demo decks,
/// published under the system user, so the 100%-playable pool is one fork
/// away. Idempotent by name.
pub fn seed_starter_decks(conn: &Connection) -> rusqlite::Result<()> {
    let have: i64 = conn.query_row(
        "SELECT count(*) FROM decks WHERE owner_id = ?1",
        [SYSTEM_USER_ID],
        |r| r.get(0),
    )?;
    if have > 0 {
        return Ok(());
    }
    let mk = |id_title: &str, titles: &[&str], name: &str| -> Canonical {
        let mut counts: Vec<(String, u32)> = Vec::new();
        for t in titles {
            match counts.iter_mut().find(|(ct, _)| ct == t) {
                Some((_, n)) => *n += 1,
                None => counts.push(((*t).to_string(), 1)),
            }
        }
        let cards = counts
            .into_iter()
            .map(|(title, qty)| {
                let code = carddata::by_title(&title).map(|c| c.code.clone()).unwrap_or_default();
                WireLine { title, code, qty }
            })
            .collect();
        let id = carddata::by_title(id_title).expect("starter identity exists");
        Canonical {
            name: name.into(),
            side: if id.side == "Corp" { "corp" } else { "runner" }.into(),
            identity_title: id.title.clone(),
            identity_code: id.code.clone(),
            cards,
            notes: "jinteki-rs starter deck — every card fully implemented; forked copies are yours to edit.".into(),
            source: None,
        }
    };
    let corp = mk(
        jinteki_core::carddb::CORP_ID,
        &jinteki_core::carddb::corp_deck(),
        "Starter Corp (Weyland)",
    );
    let runner = mk(
        jinteki_core::carddb::RUNNER_ID,
        &jinteki_core::carddb::runner_deck(),
        "Starter Runner (Catalyst)",
    );
    for c in [corp, runner] {
        let v = insert(conn, SYSTEM_USER_ID, &c)?;
        let deck_id = v["id"].as_str().unwrap().to_string();
        conn.execute(
            "UPDATE decks SET published_at = datetime('now') WHERE id = ?1",
            [&deck_id],
        )?;
    }
    Ok(())
}

/// Titles expanded per qty — what the engine's `new_with_decks` takes.
pub fn expand_titles(cards: &[WireLine]) -> Vec<String> {
    let mut out = Vec::new();
    for l in cards {
        for _ in 0..l.qty {
            out.push(l.title.clone());
        }
    }
    out
}
