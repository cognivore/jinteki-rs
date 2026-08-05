//! The Eternal format: legality data, the deck-builder catalog, and the
//! deck validator.
//!
//! Pure over two embedded artifacts — `carddata/formats.json` (the active
//! eternal snapshot of NSG's format data, emitted by `tools/gen-carddata.py`
//! from the netrunner-cards-json v2 tree) and `carddata/cards.json` (via
//! `carddata.rs`) — plus the card layer's completeness verdicts
//! (`jinteki_cards`). No I/O, no clock; storage lives in `eternal_decks.rs`.
//!
//! Card ids throughout are NSG v2 slugs (`accelerated_diagnostics`) — the
//! vocabulary the restriction and card-pool files speak. `cards.json` carries
//! each card's v2 id as `nsg_id` (joined by the generator on the collapsed
//! title slug; the EDN and v2 disagree on apostrophes, so neither raw slug is
//! usable directly).
//!
//! Rule sources, cited per check below:
//!   - docs/rules/CR-v26.03.md §1.4 (Deck Construction) — note the CR keeps
//!     deck construction in 1.4; 1.5 is Extra Cards;
//!   - the Eternal points list (formats.json: restriction
//!     `eternal_points_list_26_03`, point limit 7) — points are counted once
//!     per card NAME regardless of copies, identity included, matching the
//!     reference validator (`jinteki-reference/src/cljc/jinteki/
//!     validator.cljc:215-252`, `combine-id-and-cards` + `deck-point-count`).

use crate::carddata::{self, Card};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

static FORMATS_JSON: &str = include_str!("../../jinteki-core/carddata/formats.json");

/// The printed line that marks the draft-format card class (Boris "Syfr"
/// Kovac and friends). CR 1.4.2 settles format legality before the game
/// begins; a card printed for draft never reaches an Eternal table.
const DRAFT_ONLY_LINE: &str = "Draft format only.";

// ───────────────────────────────────────────────────────────────────────────
// Format data
// ───────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawFormats {
    eternal: RawEternal,
}

#[derive(Deserialize)]
struct RawEternal {
    restriction_id: String,
    point_limit: i64,
    banned: Vec<String>,
    points: BTreeMap<String, Vec<String>>,
    card_pool_id: String,
    legal_cards: Vec<String>,
}

/// The active eternal snapshot, indexed for lookup.
pub struct Eternal {
    pub restriction_id: String,
    pub point_limit: i64,
    pub card_pool_id: String,
    banned: HashSet<String>,
    /// v2 card id → its points-list value.
    points: HashMap<String, i64>,
    legal_cards: HashSet<String>,
}

pub fn eternal() -> &'static Eternal {
    static E: OnceLock<Eternal> = OnceLock::new();
    E.get_or_init(|| {
        let raw: RawFormats =
            serde_json::from_str(FORMATS_JSON).expect("carddata/formats.json is valid");
        let e = raw.eternal;
        let mut points = HashMap::new();
        for (tier, ids) in &e.points {
            let value: i64 = tier.parse().expect("points tier is a number");
            for id in ids {
                points.insert(id.clone(), value);
            }
        }
        Eternal {
            restriction_id: e.restriction_id,
            point_limit: e.point_limit,
            card_pool_id: e.card_pool_id,
            banned: e.banned.into_iter().collect(),
            points,
            legal_cards: e.legal_cards.into_iter().collect(),
        }
    })
}

impl Eternal {
    /// In the eternal card pool (by NSG v2 id; a card with no v2 id — player
    /// aids, never-NSG promos — is in no pool).
    pub fn in_pool(&self, card: &Card) -> bool {
        card.nsg_id.as_deref().is_some_and(|id| self.legal_cards.contains(id))
    }
    pub fn is_banned(&self, card: &Card) -> bool {
        card.nsg_id.as_deref().is_some_and(|id| self.banned.contains(id))
    }
    /// Eternal points-list value; 0 if unlisted.
    pub fn points_of(&self, card: &Card) -> i64 {
        card.nsg_id
            .as_deref()
            .and_then(|id| self.points.get(id).copied())
            .unwrap_or(0)
    }
}

/// The printed draft-format marker, on its own — distinct from being merely
/// outside the pool (the two produce different validation problems).
pub fn text_draft_only(card: &Card) -> bool {
    card.text.as_deref().is_some_and(|t| t.contains(DRAFT_ONLY_LINE))
}

/// The catalog's `draft_only` flag: a card that can never reach an Eternal
/// table — printed "Draft format only." (the Boris "Syfr" Kovac class) OR
/// absent from the eternal card pool.
pub fn draft_only(card: &Card) -> bool {
    text_draft_only(card) || !eternal().in_pool(card)
}

/// CR 1.4.1a (`rule_gateway_identities`): the two System Gateway Starter
/// Pack identities are "intended for use only with the decks included in
/// that pack" and are "not legal for play under the full deck construction
/// rules". The rule names the two cards outright, so this names them the
/// same way rather than inventing a characteristic they do not print.
pub fn starter_pack_only(card: &Card) -> bool {
    matches!(
        card.title.as_str(),
        "The Catalyst: Convention Breaker" | "The Syndicate: Profit over Principle"
    )
}

/// Eternal table filter for the CR 1.5.4a additional-identities pile: an
/// identity Rebirth/DJ Fenris may reach must itself be eternal-playable —
/// in the card pool and not a draft-format card. (Format legality is settled
/// before the game begins, CR 1.4.2; an identity illegal in the format
/// cannot be brought to its table, so the pile a table carries is filtered
/// here, while the card itself stays implemented and gated like any other.)
pub fn identity_playable(title: &str) -> bool {
    carddata::by_title(title).is_some_and(|c| eternal().in_pool(c) && !text_draft_only(c))
}

// ───────────────────────────────────────────────────────────────────────────
// Engine support
// ───────────────────────────────────────────────────────────────────────────

/// Titles the engine fully supports: every card the card layer carries with
/// zero unimplemented printed sentences (`is_complete()` — the same bar the
/// CR readiness gate holds deck and pile cards to). Covers both priority
/// decks, `unlisted.rs`, and the identity queue.
fn complete_titles() -> &'static HashSet<String> {
    static T: OnceLock<HashSet<String>> = OnceLock::new();
    T.get_or_init(|| {
        jinteki_cards::all_cards()
            .into_iter()
            .filter(|c| c.is_complete())
            .map(|c| c.name().to_string())
            .collect()
    })
}

pub fn is_supported(title: &str) -> bool {
    complete_titles().contains(title)
}

// ───────────────────────────────────────────────────────────────────────────
// Catalog
// ───────────────────────────────────────────────────────────────────────────

/// One catalog row — exactly the CatalogCard wire shape (nullable fields
/// null).
fn catalog_card(c: &Card) -> Value {
    json!({
        "id": c.nsg_id,
        "title": c.title,
        "side": c.side.to_lowercase(),
        "faction": c.faction,
        "type": c.card_type,
        "influence_cost": c.influence_cost,
        "deck_limit": c.deck_limit,
        "agenda_points": c.agenda_points,
        "points": eternal().points_of(c),
        "banned": eternal().is_banned(c),
        "draft_only": draft_only(c),
        "min_deck_size": c.min_deck_size,
        "influence_limit": c.influence_limit,
    })
}

/// The Eternal deck-builder catalog: every engine-supported card
/// (`is_complete()`) intersected with the eternal card pool, identities
/// split out. Draft-only identities NEVER appear in the identity list —
/// today none are in the pool anyway, but the exclusion is stated policy,
/// not an accident of the data.
pub fn catalog_json() -> Value {
    let mut titles: Vec<&str> = complete_titles().iter().map(String::as_str).collect();
    titles.sort_unstable();
    let mut identities = Vec::new();
    let mut cards = Vec::new();
    for title in titles {
        let Some(c) = carddata::by_title(title) else {
            continue; // engine-internal names (basic actions) have no printing
        };
        if !eternal().in_pool(c) {
            continue;
        }
        if c.is_identity() {
            if draft_only(c) {
                continue;
            }
            // CR 1.4.1a: The Catalyst and The Syndicate "are not legal for
            // play under the full deck construction rules" — starter-pack
            // identities, in the pool for the STARTER decks only, so the
            // constructed-deck catalog never offers them. They remain
            // implemented and playable through any preset that carries them.
            if starter_pack_only(c) {
                continue;
            }
            identities.push(catalog_card(c));
        } else {
            cards.push(catalog_card(c));
        }
    }
    json!({
        "format": "eternal",
        "point_limit": eternal().point_limit,
        "identities": identities,
        "cards": cards,
    })
}

// ───────────────────────────────────────────────────────────────────────────
// Validation
// ───────────────────────────────────────────────────────────────────────────

/// One deck-construction problem. `code` is one of the fixed vocabulary
/// `deck_size | agenda_points | influence | banned | points_limit | copies |
/// side | draft_only | off_pool | unsupported`; `card` names the offending
/// card id where one card is at fault, null for whole-deck problems.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Problem {
    pub code: &'static str,
    pub message: String,
    pub card: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Verdict {
    pub legal: bool,
    pub problems: Vec<Problem>,
}

/// Validate an Eternal deck: `identity_id` and the card ids are NSG v2 ids
/// (the catalog's `id` field); `cards` maps id → copies.
///
/// Deck-construction rules per docs/rules/CR-v26.03.md §1.4, eternal
/// restriction per formats.json (`eternal_points_list_26_03`). The builder
/// only offers supported, in-pool cards, but everything is validated
/// defensively — unknown ids, unsupported cards, off-pool and draft-format
/// cards each get their own problem rather than a panic or a silent drop.
pub fn validate(identity_id: &str, cards: &BTreeMap<String, u32>) -> Verdict {
    let e = eternal();
    let mut problems: Vec<Problem> = Vec::new();
    let mut push = |code: &'static str, message: String, card: Option<&str>| {
        problems.push(Problem { code, message, card: card.map(str::to_string) });
    };

    // CR 1.4.1: the identity determines the faction, minimum deck size and
    // influence limit of the deck.
    let identity: Option<&Card> = carddata::by_nsg_id(identity_id);
    match identity {
        None => push(
            "unsupported",
            format!("unknown card id \"{identity_id}\""),
            Some(identity_id),
        ),
        Some(c) if !c.is_identity() => push(
            "unsupported",
            format!("{} is not an identity card", c.title),
            Some(identity_id),
        ),
        Some(c) => {
            if !is_supported(&c.title) {
                push(
                    "unsupported",
                    format!("{} is not yet supported by the engine", c.title),
                    Some(identity_id),
                );
            }
            // Eternal: the identity itself must be in the card pool…
            if !e.in_pool(c) {
                push(
                    "off_pool",
                    format!("{} is outside the eternal card pool", c.title),
                    Some(identity_id),
                );
            }
            // …and a draft-format identity has no Eternal table (CR 1.4.2:
            // format legality is settled before the game begins).
            if text_draft_only(c) {
                push(
                    "draft_only",
                    format!("{} is printed \"Draft format only.\"", c.title),
                    Some(identity_id),
                );
            }
            if e.is_banned(c) {
                push("banned", format!("{} is banned in eternal", c.title), Some(identity_id));
            }
        }
    }
    let identity = identity.filter(|c| c.is_identity());
    let id_side = identity.map(|c| c.side.as_str());
    let id_faction = identity.and_then(|c| c.faction.as_deref());
    // CR 1.4.3: minimum deck size from the identity; CR 1.4.1/1.4.5:
    // influence limit from the identity (no printed limit = unlimited).
    let min_deck_size = identity.and_then(|c| c.min_deck_size).unwrap_or(45);
    let influence_limit = identity.and_then(|c| c.influence_limit);

    let mut n_cards: i64 = 0;
    let mut influence_used: i64 = 0;
    let mut agenda_points: i64 = 0;
    // Eternal points are counted once per NAME regardless of copies, and the
    // identity's own listing counts (reference validator.cljc:215-252).
    let mut points_total: i64 = identity.map(|c| e.points_of(c)).unwrap_or(0);

    for (id, &qty) in cards {
        if qty == 0 {
            continue;
        }
        let Some(card) = carddata::by_nsg_id(id) else {
            push("unsupported", format!("unknown card id \"{id}\""), Some(id));
            continue;
        };
        n_cards += i64::from(qty);
        if !is_supported(&card.title) {
            push(
                "unsupported",
                format!("{} is not yet supported by the engine", card.title),
                Some(id),
            );
        }
        // CR 1.4.4: decks cannot contain identity cards or cards from the
        // wrong side — both are placement violations, surfaced as `side`.
        if card.is_identity() {
            push(
                "side",
                format!("{} is an identity card and cannot be in the deck", card.title),
                Some(id),
            );
            continue;
        }
        if let Some(side) = id_side {
            if card.side != side {
                push(
                    "side",
                    format!(
                        "{} is a {} card in a {} deck",
                        card.title,
                        card.side,
                        side.to_lowercase()
                    ),
                    Some(id),
                );
            }
        }
        // CR 1.4.7: at most 3 copies by name unless the card stipulates an
        // alternative limit in its text (deck_limit from the card data).
        let copy_limit = card.deck_limit.unwrap_or(3);
        if i64::from(qty) > copy_limit {
            push(
                "copies",
                format!("{} × {} exceeds the limit of {} copies", qty, card.title, copy_limit),
                Some(id),
            );
        }
        // CR 1.4.5: neutral cards and cards of any faction other than the
        // identity's are all out-of-faction; their total influence cost must
        // fit the identity's influence limit, counted BY COPY (1.4.5a).
        // CR 1.4.4: an out-of-faction card with no influence cost (faction
        // agendas are the class) cannot be included at all.
        if let (Some(idf), Some(cf)) = (id_faction, card.faction.as_deref()) {
            if cf != idf {
                match card.influence_cost {
                    Some(pips) => influence_used += pips * i64::from(qty),
                    None => push(
                        "influence",
                        format!(
                            "{} is a {} card with no influence cost and cannot be \
                             included out-of-faction",
                            card.title, cf
                        ),
                        Some(id),
                    ),
                }
            }
        }
        if card.is_agenda() {
            agenda_points += card.agenda_points.unwrap_or(0) * i64::from(qty);
        }
        // Eternal restriction: bans block per card; points accrue per name.
        if e.is_banned(card) {
            push("banned", format!("{} is banned in eternal", card.title), Some(id));
        }
        if !e.in_pool(card) {
            push(
                "off_pool",
                format!("{} is outside the eternal card pool", card.title),
                Some(id),
            );
        }
        if text_draft_only(card) {
            push(
                "draft_only",
                format!("{} is printed \"Draft format only.\"", card.title),
                Some(id),
            );
        }
        points_total += e.points_of(card);
    }

    if identity.is_some() {
        // CR 1.4.3: at least the identity's minimum deck size (identity and
        // extra cards outside the deck are not counted, 1.4.3a — the map
        // never contains them).
        if n_cards < min_deck_size {
            push(
                "deck_size",
                format!("{n_cards} cards; the identity requires at least {min_deck_size}"),
                None,
            );
        }
        // CR 1.4.5: total out-of-faction influence within the identity's
        // limit.
        if let Some(limit) = influence_limit {
            if influence_used > limit {
                push(
                    "influence",
                    format!("{influence_used} influence used; the identity allows {limit}"),
                    None,
                );
            }
        }
        // CR 1.4.6a-d: the Corp agenda-point band. The printed table
        // (40-44 → 18-19, 45-49 → 20-21, 50-54 → 22-23, then +2 per full 5
        // cards over 54) is exactly min = 2 + 2·⌊n/5⌋, legal iff
        // min ≤ points ≤ min+1, with n clamped up to the minimum deck size
        // so an undersized deck fails on size, not twice.
        if id_side == Some("Corp") {
            let n = n_cards.max(min_deck_size);
            let min_pts = 2 + 2 * (n / 5);
            if agenda_points < min_pts || agenda_points > min_pts + 1 {
                push(
                    "agenda_points",
                    format!(
                        "{agenda_points} agenda points; a {n}-card deck needs {min_pts}\u{2013}{}",
                        min_pts + 1
                    ),
                    None,
                );
            }
        }
    }
    // Eternal points list: Σ over distinct card names (identity included)
    // must fit the point limit.
    if points_total > e.point_limit {
        push(
            "points_limit",
            format!(
                "{points_total} eternal points; the {} allows {}",
                e.restriction_id, e.point_limit
            ),
            None,
        );
    }

    Verdict { legal: problems.is_empty(), problems }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(v: &[(&str, u32)]) -> BTreeMap<String, u32> {
        v.iter().map(|(id, q)| (id.to_string(), *q)).collect()
    }

    /// The format data is the active snapshot with the shape the server
    /// depends on.
    #[test]
    fn format_data_loads_and_pins_the_active_snapshot() {
        let e = eternal();
        assert_eq!(e.restriction_id, "eternal_points_list_26_03");
        assert_eq!(e.point_limit, 7);
        assert_eq!(e.card_pool_id, "eternal");
        assert!(e.legal_cards.len() > 2000, "pool expanded to card ids");
        assert!(e.banned.contains("watch_the_world_burn"));
        assert_eq!(e.points.get("account_siphon"), Some(&3));
        assert_eq!(e.points.get("nebula_talent_management_making_stars"), Some(&2));
    }

    /// Boris: draft-printed, outside the pool, yet fully implemented — only
    /// the eternal surface filters him.
    #[test]
    fn boris_is_draft_only_and_off_pool_but_still_a_card() {
        let boris = carddata::by_title("Boris \"Syfr\" Kovac: Crafty Veteran").unwrap();
        assert!(text_draft_only(boris));
        assert!(!eternal().in_pool(boris));
        assert!(draft_only(boris));
        assert!(!identity_playable(boris.title.as_str()));
        // The card itself remains implemented and complete in the card layer.
        let carried = jinteki_cards::find(&boris.title).expect("Boris stays implemented");
        assert!(carried.is_complete(), "Boris stays complete; only the surface filters him");
        assert!(is_supported(&boris.title));
    }

    #[test]
    fn catalog_excludes_draft_identities_and_speaks_nsg_ids() {
        let cat = catalog_json();
        assert_eq!(cat["format"], "eternal");
        assert_eq!(cat["point_limit"], 7);
        let identities = cat["identities"].as_array().unwrap();
        assert!(!identities.is_empty(), "supported identities exist");
        for id in identities {
            assert_eq!(id["draft_only"], false, "{} leaked into the identity list", id["title"]);
            assert!(id["id"].as_str().is_some(), "identities carry v2 ids");
            assert!(id["min_deck_size"].is_i64(), "identities carry a minimum deck size");
        }
        assert!(
            !identities.iter().any(|c| c["id"] == "boris_syfr_kovac_crafty_veteran"),
            "Boris never appears for Eternal"
        );
        // Rows are the exact CatalogCard shape.
        let sample = &identities[0];
        let keys: Vec<&str> = sample.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys.len(),
            13,
            "CatalogCard carries exactly the contract fields, got {keys:?}"
        );
        // Every card row is in the pool by construction; a pointed card
        // reports its points.
        let cards = cat["cards"].as_array().unwrap();
        if let Some(siphon) = cards.iter().find(|c| c["id"] == "account_siphon") {
            assert_eq!(siphon["points"], 3);
        }
    }

    /// A fully legal 45-card corp deck from catalog cards (the Gauntlet
    /// list — the shipped default is itself a legal Eternal deck).
    fn gauntlet_map() -> (String, BTreeMap<String, u32>) {
        let mut cards = BTreeMap::new();
        let mut identity = String::new();
        for (title, qty) in crate::cr::GAUNTLET_LIST {
            let c = carddata::by_title(title).expect(title);
            let id = c.nsg_id.clone().expect(title);
            if c.is_identity() {
                identity = id;
            } else {
                cards.insert(id, *qty);
            }
        }
        (identity, cards)
    }

    #[test]
    fn a_legal_deck_passes_whole() {
        let (identity, cards) = gauntlet_map();
        let v = validate(&identity, &cards);
        assert!(v.legal, "problems: {:?}", v.problems);
    }

    #[test]
    fn each_rule_violated_once() {
        let (identity, legal) = gauntlet_map();

        // deck_size (CR 1.4.3): drop the ice.
        let mut d = legal.clone();
        d.remove("ip_block");
        d.remove("slot_machine");
        let v = validate(&identity, &d);
        assert!(v.problems.iter().any(|p| p.code == "deck_size"), "{:?}", v.problems);

        // agenda_points (CR 1.4.6): a Bellona short pushes 20 → 17 while the
        // band stays 20-21 (48 cards).
        let mut d = legal.clone();
        d.insert("bellona".into(), 2);
        let v = validate(&identity, &d);
        assert!(v.problems.iter().any(|p| p.code == "agenda_points"), "{:?}", v.problems);

        // influence (CR 1.4.5): the Gauntlet spends exactly its 15; one
        // Snare! (Jinteki, 2 pips) on top exceeds the limit.
        let mut d = legal.clone();
        d.insert("snare".into(), 1);
        let v = validate(&identity, &d);
        assert!(
            v.problems.iter().any(|p| p.code == "influence" && p.card.is_none()),
            "{:?}",
            v.problems
        );

        // influence (CR 1.4.4 corollary): an out-of-faction agenda has no
        // influence cost and cannot be included at all.
        let mut d = legal.clone();
        d.insert("nisei_mk_ii".into(), 1);
        let v = validate(&identity, &d);
        assert!(
            v.problems
                .iter()
                .any(|p| p.code == "influence" && p.card.as_deref() == Some("nisei_mk_ii")),
            "{:?}",
            v.problems
        );

        // banned (eternal restriction).
        let mut d = legal.clone();
        d.insert("watch_the_world_burn".into(), 1);
        let v = validate(&identity, &d);
        assert!(
            v.problems
                .iter()
                .any(|p| p.code == "banned" && p.card.as_deref() == Some("watch_the_world_burn")),
            "{:?}",
            v.problems
        );

        // points_limit: the Gauntlet sits at exactly 7; one more pointed
        // name (Museum of History, 2 points) tips the total to 9.
        let mut d = legal.clone();
        d.insert("museum_of_history".into(), 1);
        let v = validate(&identity, &d);
        assert!(v.problems.iter().any(|p| p.code == "points_limit"), "{:?}", v.problems);

        // copies (CR 1.4.7).
        let mut d = legal.clone();
        d.insert("petty_cash".into(), 4);
        let v = validate(&identity, &d);
        assert!(
            v.problems.iter().any(|p| p.code == "copies" && p.card.as_deref() == Some("petty_cash")),
            "{:?}",
            v.problems
        );

        // side (CR 1.4.4): a runner card in a corp deck, and an identity in
        // the deck body.
        let mut d = legal.clone();
        d.insert("sure_gamble".into(), 3);
        let v = validate(&identity, &d);
        assert!(
            v.problems.iter().any(|p| p.code == "side" && p.card.as_deref() == Some("sure_gamble")),
            "{:?}",
            v.problems
        );
        let mut d = legal.clone();
        d.insert("andromeda_dispossessed_ristie".into(), 1);
        let v = validate(&identity, &d);
        assert!(
            v.problems
                .iter()
                .any(|p| p.code == "side" && p.card.as_deref() == Some("andromeda_dispossessed_ristie")),
            "{:?}",
            v.problems
        );

        // draft_only: Boris as the identity.
        let v = validate("boris_syfr_kovac_crafty_veteran", &deck(&[("sure_gamble", 3)]));
        assert!(v.problems.iter().any(|p| p.code == "draft_only"), "{:?}", v.problems);
        assert!(v.problems.iter().any(|p| p.code == "off_pool"), "draft ids are off-pool too");

        // off_pool: a corp operation the eternal card pool does not carry
        // (Net Watchlist, a campaign-only printing).
        let mut d = legal.clone();
        d.insert("net_watchlist".into(), 1);
        let v = validate(&identity, &d);
        assert!(
            v.problems
                .iter()
                .any(|p| p.code == "off_pool" && p.card.as_deref() == Some("net_watchlist")),
            "{:?}",
            v.problems
        );

        // unsupported: an id that resolves to no card at all…
        let mut d = legal.clone();
        d.insert("definitely_not_a_card".into(), 1);
        let v = validate(&identity, &d);
        assert!(
            v.problems.iter().any(|p| p.code == "unsupported"
                && p.card.as_deref() == Some("definitely_not_a_card")),
            "{:?}",
            v.problems
        );
    }

    #[test]
    fn points_count_once_per_name_and_include_the_identity() {
        // Making Stars is itself on the points list at 2; the full Gauntlet
        // sums to exactly the limit of 7 — per-name counting, since Jackson
        // Howard (2 points) rides at 3 copies.
        let (identity, cards) = gauntlet_map();
        let e = eternal();
        let mut total = e.points_of(carddata::by_nsg_id(&identity).unwrap());
        for id in cards.keys() {
            total += e.points_of(carddata::by_nsg_id(id).unwrap());
        }
        assert_eq!(total, 7, "the shipped corp deck sits at exactly the point limit");
        let v = validate(&identity, &cards);
        assert!(!v.problems.iter().any(|p| p.code == "points_limit"));
    }
}
