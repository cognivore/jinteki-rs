//! Server-side card index over the generated card database.
//!
//! `crates/jinteki-core/carddata/cards.json` carries deck-construction fields
//! (influence pips, identity ceilings, previous-printing codes, standard ban
//! flags — emitted by `tools/gen-carddata.py`, ACCOUNTS-AND-DECKS.md §6.1)
//! that the engine core's `printed.rs` does not yet surface. The core is
//! another workstream's territory, so this module reads the same JSON file
//! directly and builds the three indexes deck validation and NRDB import
//! need: `by_title`, `by_code` (latest printings), `by_previous_code`
//! (reference lookup parity: `web/nrdb.clj:29-31`).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

static CARDS_JSON: &str = include_str!("../../jinteki-core/carddata/cards.json");

/// One card title, latest printing, with deck-construction fields.
#[derive(Debug, Clone, Deserialize)]
pub struct Card {
    pub title: String,
    pub code: String,
    pub side: String,
    #[serde(rename = "type")]
    pub card_type: String,
    pub faction: Option<String>,
    #[serde(default)]
    pub subtypes: Vec<String>,
    pub cost: Option<i64>,
    pub agenda_points: Option<i64>,
    pub deck_limit: Option<i64>,
    #[serde(default)]
    pub rotated: bool,
    /// Influence pips (`:factioncost`). None on identities and agendas.
    pub influence_cost: Option<i64>,
    /// Identity influence ceiling (`:influencelimit`). None = unlimited.
    pub influence_limit: Option<i64>,
    /// Identity minimum deck size (`:minimumdecksize`).
    pub min_deck_size: Option<i64>,
    /// Codes of earlier printings of this title (`:previous-versions`).
    #[serde(default)]
    pub previous_codes: Vec<String>,
    /// Banned in the standard format (`:format {:standard {:banned true}}`).
    #[serde(default)]
    pub standard_banned: bool,
    /// Printed card text (oracle, HTML-ish markup as the EDN carries it).
    #[serde(default)]
    pub text: Option<String>,
    /// The EDN `:normalizedtitle` slug (hyphenated).
    #[serde(default)]
    pub slug: Option<String>,
    /// The NSG v2 card id — the vocabulary format legality (formats.json)
    /// speaks. None for cards the NSG v2 tree does not carry (player aids,
    /// two never-NSG promo identities).
    #[serde(default)]
    pub nsg_id: Option<String>,
}

impl Card {
    pub fn is_identity(&self) -> bool {
        self.card_type == "Identity"
    }
    pub fn is_agenda(&self) -> bool {
        self.card_type == "Agenda"
    }
}

fn cards() -> &'static Vec<Card> {
    static DATA: OnceLock<Vec<Card>> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(CARDS_JSON).expect("carddata/cards.json is valid"))
}

struct Indexes {
    by_title: HashMap<&'static str, &'static Card>,
    by_code: HashMap<&'static str, &'static Card>,
    by_previous_code: HashMap<&'static str, &'static Card>,
    by_nsg_id: HashMap<&'static str, &'static Card>,
}

fn indexes() -> &'static Indexes {
    static IDX: OnceLock<Indexes> = OnceLock::new();
    IDX.get_or_init(|| {
        let mut by_title = HashMap::new();
        let mut by_code = HashMap::new();
        let mut by_previous_code = HashMap::new();
        let mut by_nsg_id = HashMap::new();
        for c in cards() {
            by_title.insert(c.title.as_str(), c);
            by_code.insert(c.code.as_str(), c);
            for pc in &c.previous_codes {
                by_previous_code.insert(pc.as_str(), c);
            }
            if let Some(id) = c.nsg_id.as_deref() {
                by_nsg_id.insert(id, c);
            }
        }
        Indexes { by_title, by_code, by_previous_code, by_nsg_id }
    })
}

/// Card by canonical title.
pub fn by_title(title: &str) -> Option<&'static Card> {
    indexes().by_title.get(title).copied()
}

/// Card by latest-printing NRDB code.
pub fn by_code(code: &str) -> Option<&'static Card> {
    indexes().by_code.get(code).copied()
}

/// Card by an earlier printing's code (NRDB import of old decklists).
pub fn by_previous_code(code: &str) -> Option<&'static Card> {
    indexes().by_previous_code.get(code).copied()
}

/// Card by NSG v2 id — the id vocabulary of formats.json and the eternal
/// catalog/deck API.
pub fn by_nsg_id(id: &str) -> Option<&'static Card> {
    indexes().by_nsg_id.get(id).copied()
}

/// Resolve an NRDB code: latest printings first, then previous printings.
/// Returns the card and whether the previous-printing fallback was used.
pub fn resolve_code(code: &str) -> Option<(&'static Card, bool)> {
    by_code(code)
        .map(|c| (c, false))
        .or_else(|| by_previous_code(code).map(|c| (c, true)))
}

/// Every card (sorted by title, as generated).
pub fn all() -> &'static [Card] {
    cards()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_resolve_current_and_previous_codes() {
        let sg = by_title("Sure Gamble").expect("Sure Gamble exists");
        assert_eq!(by_code(&sg.code).unwrap().title, "Sure Gamble");
        // 01050 is the original Core Set printing of Sure Gamble.
        let (c, via_prev) = resolve_code("01050").expect("previous printing resolves");
        assert_eq!(c.title, "Sure Gamble");
        assert!(via_prev);
        assert_eq!(c.code, sg.code, "resolution lands on the latest printing");
    }

    #[test]
    fn identity_fields_present() {
        let wey = by_title("Weyland Consortium: Building a Better World").unwrap();
        assert!(wey.is_identity());
        assert_eq!(wey.influence_limit, Some(15));
        assert_eq!(wey.min_deck_size, Some(45));
    }
}
