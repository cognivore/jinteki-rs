//! NetrunnerDB decklist import (ACCOUNTS-AND-DECKS.md §7): v2 public API
//! only, numeric and UUID ids, previous-printing resolution, and an explicit
//! import report instead of the reference's silent drop (`nrdb.clj:40`).
//!
//! Fetching is isolated from parsing/mapping so tests run on recorded
//! fixtures with no live network (SYS-N-1 verification note).

use crate::carddata;
use crate::decks::WireLine;
use serde_json::{json, Value};

const NRDB_BASE: &str = "https://netrunnerdb.com/api/2.0/public/";
/// Honest, stable User-Agent — NRDB's bot shield 403s default UAs (§1.4).
pub const USER_AGENT: &str = "jinteki-rs (netrunner.sweater.vac.fere.me)";
const MAX_BODY: usize = 1 << 20; // 1 MiB response cap

/// Which v2 endpoint(s) to try for a parsed input (`nrdb.clj:65-85`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    /// `/decklist/{id}` — public decklists.
    Decklist(String),
    /// `/deck/{id}` — private-but-published decks.
    Deck(String),
    /// Bare id: try decklist first, fall back to deck.
    Either(String),
}

/// Failure taxonomy (§7.4), each with user-facing copy at the API layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// Unparseable input — the caller's mistake (400).
    BadInput(String),
    /// NRDB 404 / total != 1 (404).
    NotFound,
    /// NRDB 403 — bot shield (502-style: the caller did nothing wrong).
    Blocked,
    /// Timeout / 5xx / network (502).
    Down(String),
    /// Deck exists but its identity cannot be resolved (422).
    NoIdentity(String),
}

/// Accepts a bare id (numeric or UUID) or any NRDB URL (§7.1). The id is
/// whatever follows `decklist/`, `deck/view/`, or `deck/`, truncated at the
/// next `/`, then validated against numeric or UUID shape before it goes
/// anywhere near a URL we fetch.
pub fn parse_input(input: &str) -> Result<Endpoint, ImportError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ImportError::BadInput("paste a NetrunnerDB decklist URL or id".into()));
    }
    let bad = || {
        ImportError::BadInput(format!(
            "could not find a decklist id in \"{}\"",
            &input.chars().take(80).collect::<String>()
        ))
    };
    let extract = |after: &str| -> Option<String> {
        let start = input.find(after)? + after.len();
        let rest = &input[start..];
        let end = rest.find('/').unwrap_or(rest.len());
        let id = rest[..end].trim();
        valid_id(id).then(|| id.to_string())
    };
    if input.contains("decklist/") {
        return extract("decklist/").map(Endpoint::Decklist).ok_or_else(bad);
    }
    if input.contains("deck/view/") {
        return extract("deck/view/").map(Endpoint::Deck).ok_or_else(bad);
    }
    if input.contains("deck/") {
        return extract("deck/").map(Endpoint::Deck).ok_or_else(bad);
    }
    if valid_id(input) {
        return Ok(Endpoint::Either(input.to_string()));
    }
    Err(bad())
}

/// Numeric or UUID (8-4-4-4-12 hex) — nothing else reaches a fetch URL.
fn valid_id(id: &str) -> bool {
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(n, p)| p.len() == *n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

impl Endpoint {
    /// (path, human URL) pairs to try in order.
    pub fn attempts(&self) -> Vec<(String, String)> {
        let mk = |kind: &str, id: &str| {
            (
                format!("{NRDB_BASE}{kind}/{id}"),
                format!("https://netrunnerdb.com/en/{kind}/{id}"),
            )
        };
        match self {
            Endpoint::Decklist(id) => vec![mk("decklist", id)],
            Endpoint::Deck(id) => vec![mk("deck", id)],
            Endpoint::Either(id) => vec![mk("decklist", id), mk("deck", id)],
        }
    }
    pub fn id(&self) -> &str {
        match self {
            Endpoint::Decklist(id) | Endpoint::Deck(id) | Endpoint::Either(id) => id,
        }
    }
}

/// Fetch one endpoint attempt. 15 s timeout, 1 MiB cap, honest UA (§7.2).
async fn fetch_one(http: &reqwest::Client, url: &str) -> Result<Value, ImportError> {
    let res = http
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                ImportError::Down("NetrunnerDB timed out".into())
            } else {
                ImportError::Down(format!("NetrunnerDB unreachable: {e}"))
            }
        })?;
    match res.status().as_u16() {
        200 => {}
        403 => return Err(ImportError::Blocked),
        404 => return Err(ImportError::NotFound),
        s if s >= 500 => return Err(ImportError::Down(format!("NetrunnerDB returned {s}"))),
        s => return Err(ImportError::Down(format!("NetrunnerDB returned {s}"))),
    }
    if res.content_length().unwrap_or(0) > MAX_BODY as u64 {
        return Err(ImportError::Down("NetrunnerDB response too large".into()));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| ImportError::Down(format!("reading NetrunnerDB response: {e}")))?;
    if bytes.len() > MAX_BODY {
        return Err(ImportError::Down("NetrunnerDB response too large".into()));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| ImportError::Down("NetrunnerDB returned non-JSON".into()))
}

/// Fetch with the decklist→deck fallback for bare ids.
pub async fn fetch(http: &reqwest::Client, ep: &Endpoint) -> Result<(Value, String), ImportError> {
    let attempts = ep.attempts();
    let n = attempts.len();
    let mut last = ImportError::NotFound;
    for (i, (url, human)) in attempts.into_iter().enumerate() {
        match fetch_one(http, &url).await {
            Ok(v) => return Ok((v, human)),
            Err(ImportError::NotFound) if i + 1 < n => last = ImportError::NotFound,
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

/// The mapped draft + report (§7.4). Pure over the fetched payload —
/// fixture-testable with no network.
pub struct Import {
    /// Draft deck fields, unsaved: the client shows the report and saving is
    /// a second explicit POST /api/decks (SYS-N-2).
    pub draft: Value,
    pub report: Value,
}

/// Map a v2 payload `{success, total, data: [{id, name, cards: {code: qty}}]}`
/// (`nrdb.clj:57-63`) into a deck draft + import report.
pub fn map_payload(payload: &Value, human_url: &str, id: &str) -> Result<Import, ImportError> {
    if payload["success"].as_bool() != Some(true) || payload["total"].as_i64() != Some(1) {
        return Err(ImportError::NotFound);
    }
    let data = payload["data"]
        .get(0)
        .ok_or(ImportError::NotFound)?;
    let name = data["name"].as_str().unwrap_or("Imported deck").to_string();
    let cards_obj = data["cards"].as_object().ok_or(ImportError::NotFound)?;

    let mut identity: Option<&'static carddata::Card> = None;
    let mut lines: Vec<WireLine> = Vec::new();
    let mut resolved = 0u32;
    let mut via_previous = 0u32;
    let mut unknown_codes: Vec<String> = Vec::new();
    let mut rotated: Vec<String> = Vec::new();

    for (code, qty_v) in cards_obj {
        let qty = qty_v.as_u64().unwrap_or(0) as u32;
        if qty == 0 {
            continue;
        }
        match carddata::resolve_code(code) {
            None => unknown_codes.push(code.clone()),
            Some((card, via_prev)) => {
                if via_prev {
                    via_previous += 1;
                }
                // Identity splits out of the card list by type (nrdb.clj:33-40).
                if card.is_identity() {
                    identity = Some(card);
                    resolved += 1;
                    continue;
                }
                resolved += qty.min(1); // count titles resolved, not copies
                if card.rotated {
                    rotated.push(card.title.clone());
                }
                lines.push(WireLine {
                    title: card.title.clone(),
                    code: card.code.clone(),
                    qty,
                });
            }
        }
    }

    // A deck with no identity is not a deck (same predicate as
    // web/decks.clj:113); unknown identity fails the import (§7.4).
    let Some(id_card) = identity else {
        return Err(ImportError::NoIdentity(name));
    };

    let notes = format!("imported from {human_url}");
    let verdict = crate::decks::validate(&id_card.title, &lines);
    let draft = json!({
        "name": name,
        "side": if id_card.side == "Corp" { "corp" } else { "runner" },
        "identity": { "title": id_card.title, "code": id_card.code },
        "cards": lines,
        "notes": notes,
        "source": { "kind": "nrdb", "id": id, "url": human_url },
    });
    let report = json!({
        "resolved": resolved,
        "via_previous_printing": via_previous,
        "unknown_codes": unknown_codes,
        "rotated": rotated,
        "validation": {
            "legal": verdict.legal,
            "problems": verdict.problems,
            "warnings": verdict.warnings,
            "counts": verdict.counts,
            "playable": verdict.playable,
            "cards": verdict.cards,
        },
    });
    Ok(Import { draft, report })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_documented_input_forms() {
        assert_eq!(
            parse_input("https://netrunnerdb.com/en/decklist/81579/some-slug").unwrap(),
            Endpoint::Decklist("81579".into())
        );
        assert_eq!(
            parse_input(
                "https://netrunnerdb.com/en/decklist/45c2efd4-62e3-4e44-aa27-a49a3a7f6368/slug"
            )
            .unwrap(),
            Endpoint::Decklist("45c2efd4-62e3-4e44-aa27-a49a3a7f6368".into())
        );
        assert_eq!(
            parse_input("https://netrunnerdb.com/en/deck/view/123456").unwrap(),
            Endpoint::Deck("123456".into())
        );
        assert_eq!(parse_input("81579").unwrap(), Endpoint::Either("81579".into()));
        assert!(matches!(parse_input("not a deck url"), Err(ImportError::BadInput(_))));
        assert!(matches!(
            parse_input("https://netrunnerdb.com/en/decklist/../../etc/passwd"),
            Err(ImportError::BadInput(_))
        ));
    }

    fn fixture() -> Value {
        // Recorded shape of GET api/2.0/public/decklist/81579 (§1.4),
        // trimmed: current codes, a previous-printing code (01050 = Core Set
        // Sure Gamble), an unknown code, and a Hoshiko identity.
        json!({
            "success": true,
            "total": 1,
            "data": [{
                "id": 81579,
                "name": "Fixture Hoshiko",
                "cards": {
                    "26066": 1,   // Hoshiko Shiro: Untold Protagonist (identity)
                    "01050": 3,   // Sure Gamble via previous printing
                    "30006": 2,   // current-printing runner card
                    "99999": 1    // unknown code
                }
            }]
        })
    }

    #[test]
    fn maps_fixture_with_previous_printing_and_unknowns() {
        let imp = map_payload(&fixture(), "https://netrunnerdb.com/en/decklist/81579", "81579")
            .expect("import succeeds minus unknown cards");
        assert_eq!(imp.draft["identity"]["title"], "Hoshiko Shiro: Untold Protagonist");
        assert_eq!(imp.draft["side"], "runner");
        let cards = imp.draft["cards"].as_array().unwrap();
        let sg = cards
            .iter()
            .find(|c| c["title"] == "Sure Gamble")
            .expect("Sure Gamble resolved via previous printing");
        assert_eq!(sg["qty"], 3);
        assert_ne!(sg["code"], "01050", "stored line carries the LATEST code");
        assert_eq!(imp.report["via_previous_printing"], 1);
        assert_eq!(imp.report["unknown_codes"][0], "99999");
        assert!(imp.draft["notes"]
            .as_str()
            .unwrap()
            .starts_with("imported from https://netrunnerdb.com/en/decklist/81579"));
    }

    #[test]
    fn missing_identity_fails_import() {
        let payload = json!({
            "success": true, "total": 1,
            "data": [{"id": 1, "name": "No ID", "cards": {"30006": 2}}]
        });
        assert!(matches!(
            map_payload(&payload, "u", "1"),
            Err(ImportError::NoIdentity(_))
        ));
    }

    #[test]
    fn bad_payload_shapes_rejected() {
        for p in [
            json!({"success": false, "total": 1, "data": [{}]}),
            json!({"success": true, "total": 0, "data": []}),
            json!({}),
        ] {
            assert!(matches!(map_payload(&p, "u", "1"), Err(ImportError::NotFound)));
        }
    }
}
