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
//! Influence is priced through [`crate::influence`], not read straight off
//! the card: twelve cards print a waiver that makes them cost 0 influence in
//! a deck that meets a condition of their own, so a line's cost depends on
//! the whole deck. Nothing about that is a CR rule — it is printed card text.
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
use crate::influence;
use jinteki_cr::object::PrintedCard;
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
/// rules". The rule names the two cards outright; this matches them by
/// their NSG v2 ids — the one vocabulary the whole eternal surface speaks —
/// rather than by display title, which drifts with subtitle punctuation and
/// printings. They stay implemented and preset-playable; the constructed
/// catalog never offers them.
pub fn starter_pack_only(card: &Card) -> bool {
    matches!(
        card.nsg_id.as_deref(),
        Some("the_catalyst_convention_breaker" | "the_syndicate_profit_over_principle")
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

/// The CR 1.5.4a pile a user-built Runner deck brings to an eternal table:
/// every complete, eternal-playable Runner identity the engine carries,
/// except the deck's own (1.5.4 is about identities OTHER than the selected
/// one) and the starter-pack pair (CR 1.4.1a). "Any number of additional
/// Runner identity cards" is the printed allowance; the cards that consume
/// the pile narrow it themselves at resolution (Rebirth to the same faction
/// per 1.5.4b, DJ Fenris to g-mods), so the pile's job is only to make those
/// choices possible — the same reasoning `cr::ANDROMEDA_PILE` documents for
/// the stock deck, generalised across factions because a user deck's Rebirth
/// may be any faction's.
pub fn runner_identity_pile(except_title: &str) -> Vec<jinteki_cr::object::PrintedCard> {
    jinteki_cards::all_cards()
        .into_iter()
        .filter(|c| {
            c.printed.card_type == jinteki_cr::object::CardType::Identity
                && c.printed.side == jinteki_cr::object::Side::Runner
                && c.is_complete()
                && c.name() != except_title
                && identity_playable(c.name())
                && carddata::by_title(c.name()).is_some_and(|cd| !starter_pack_only(cd))
        })
        .map(|c| c.printed)
        .collect()
}

/// Does this printed card meet the criterion, decided from PRINTED
/// characteristics alone? `None` is "not from here" — a criterion that reads
/// a position, a board or a run cannot be read before a game exists, and
/// 1.5.3a's question ("what must the player bring along with their deck?") is
/// asked while the deck is still a list.
///
/// Only the §2 card-characteristic atoms belong here. The authoritative
/// selection is still the VM's at 1.6.2, over the pile this decides to bring.
fn printed_meets(card: &PrintedCard, f: &jinteki_cr::instr::TargetFilter) -> Option<bool> {
    use jinteki_cr::instr::TargetFilter as F;
    match f {
        // CR 2.16: an effective subtype — for a card outside any game, the
        // printed one, since no modifier has had a chance to speak.
        F::HasSubtype(s) => Some(card.subtypes.contains(s)),
        F::HasAnySubtype(list) => Some(list.iter().any(|s| card.subtypes.contains(s))),
        // CR 2.15: the card's type.
        F::CardTypeIs(t) => Some(card.card_type == *t),
        _ => None,
    }
}

/// CR 1.5.1/1.5.3a: the cards from OUTSIDE the deck that an identity requires
/// its player to bring along with it — "these cards are not considered part of
/// your deck". Adam's three directives are the only printed case, and
/// `PrintedCard::starting_extra_installs` is the fact that states it; an
/// identity without one brings nothing, which is every other identity.
///
/// `Ok` is the pile to hand `GameSetup::extra_cards`, from which the VM
/// selects at 1.6.2. `Err` names what stands in the way, the way
/// `DeckRefusal::Unbuildable` names cards it cannot seat:
///
///   * a required card the engine carries INCOMPLETE. SYS-D-12: "no card
///     shall be playable in a game unless its behavior is implemented" — and
///     a directive is not merely in a deck, it BEGINS the game installed and
///     active (1.5.3b/1.5.3d), so it is played in every game it comes to.
///     This is the same bar `cr::readiness()` already holds the 1.5.4a
///     identity pile to, for the same stated reason: a card that comes to the
///     table with the deck is gated exactly like the deck is.
///   * a count 1.5.3b cannot settle. Fewer matching cards than the fact needs
///     and there is nothing to install; more, and "selects exactly N of their
///     provided cards" is a real decision that game setup has no seat for —
///     so the honest answer is to refuse rather than to choose for the player.
pub fn starting_extra_cards(identity: &PrintedCard) -> Result<Vec<PrintedCard>, Vec<String>> {
    let Some(fact) = identity.starting_extra_installs.as_ref() else {
        return Ok(Vec::new());
    };
    let mut brought: Vec<PrintedCard> = Vec::new();
    let mut incomplete: Vec<String> = Vec::new();
    let mut seen: Vec<&'static str> = Vec::new();
    for c in jinteki_cards::all_cards() {
        let mut meets = true;
        for f in &fact.criteria {
            match printed_meets(&c.printed, f) {
                Some(ok) => meets &= ok,
                None => {
                    return Err(vec![format!(
                        "{} describes the cards it starts the game with in terms no deck \
                         list can be read against",
                        identity.name
                    )])
                }
            }
        }
        if !meets {
            continue;
        }
        // 1.5.3a's "differently named": one card per name is what must be
        // brought, so a second printing of the same name is not a second card.
        if fact.distinct_names && seen.contains(&c.name()) {
            continue;
        }
        seen.push(c.name());
        if c.is_complete() {
            brought.push(c.printed);
        } else {
            incomplete.push(c.name().to_string());
        }
    }
    if !incomplete.is_empty() {
        incomplete.sort();
        return Err(incomplete);
    }
    if brought.len() != fact.count as usize {
        return Err(vec![format!(
            "{} starts the game with {} of them and the engine carries {}",
            identity.name,
            fact.count,
            brought.len()
        )]);
    }
    Ok(brought)
}

/// The 1.5.3a blockers as one problem sentence, or `None` when the identity
/// can be seated. Shared by the catalog (which never offers such an identity)
/// and `validate` (which explains a deck already saved with one).
fn extra_cards_blocker(title: &str) -> Option<String> {
    let card = jinteki_cards::find(title)?;
    let blockers = starting_extra_cards(&card.printed).err()?;
    Some(format!(
        "{title} starts the game with cards from outside the deck that the engine cannot \
         supply: {}",
        blockers.join(", ")
    ))
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
            // CR 1.4.1a: the starter-pack identities are not legal for
            // constructed play; the builder never offers them. They remain
            // implemented and playable through any preset that carries them.
            if starter_pack_only(c) {
                continue;
            }
            // CR 1.5.3a: an identity whose player must bring cards from
            // outside the deck is only as seatable as those cards are. Adam
            // is the case: complete himself, but a directive of his is not,
            // and every Adam deck would therefore be built to be refused.
            // The identity stays implemented and inspectable (SYS-D-12's
            // other half); the builder simply does not offer it yet.
            if extra_cards_blocker(&c.title).is_some() {
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
            // CR 1.5.3a: the identity is not the whole of what it brings.
            // A deck whose identity starts the game with cards the engine
            // cannot supply cannot be played, and the reason belongs on the
            // deck screen where the choice was made, not at the table door.
            if let Some(why) = extra_cards_blocker(&c.title) {
                push("unsupported", why, Some(identity_id));
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

    // Influence is a function of the DECK, not of one card: twelve cards
    // print a waiver that zeroes their own influence when the deck around
    // them satisfies a condition (`influence.rs`). The tally has to exist
    // before any line is priced, so it is taken in a pre-pass over the same
    // ids the loop below resolves. Unknown ids contribute nothing here and
    // are reported there.
    let counts = influence::DeckCounts::tally(
        cards
            .iter()
            .filter(|(_, &qty)| qty > 0)
            .filter_map(|(id, &qty)| carddata::by_nsg_id(id).map(|c| (c, qty))),
    );

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
                    // …except where the card's own printed text waives it.
                    // The waiver makes the line cost ZERO, not less
                    // (`influence.rs`); an unreadable waiver sentence is
                    // reported as a problem rather than quietly charged.
                    Some(pips) => match influence::line_cost(card, pips, qty, &counts) {
                        Ok(spent) => influence_used += spent,
                        Err((why, full)) => {
                            influence_used += full;
                            push(
                                "influence",
                                format!(
                                    "{} prints an influence waiver this server cannot \
                                     read ({why}); it is charged {full} influence until \
                                     the condition is implemented",
                                    card.title
                                ),
                                Some(id),
                            );
                        }
                    },
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

    /// CR 1.4.1a: neither System Gateway Starter Pack identity is offered
    /// for constructed play — matched by v2 id, so a subtitle respelling
    /// cannot quietly re-admit them. Nova Initiumia stays: "Catalyst" in a
    /// title is not the starter Catalyst, which is why the match is on ids.
    #[test]
    fn starter_pack_identities_never_reach_the_catalog() {
        let catalyst = carddata::by_title("The Catalyst: Convention Breaker").unwrap();
        let syndicate = carddata::by_title("The Syndicate: Profit over Principle").unwrap();
        assert!(starter_pack_only(catalyst));
        assert!(starter_pack_only(syndicate));
        let cat = catalog_json();
        let identities = cat["identities"].as_array().unwrap();
        for banned in ["the_catalyst_convention_breaker", "the_syndicate_profit_over_principle"] {
            assert!(
                !identities.iter().any(|c| c["id"] == banned),
                "{banned} is starter-pack-only (CR 1.4.1a) and must not be offered"
            );
        }
        assert!(
            identities.iter().any(|c| c["id"] == "nova_initiumia_catalyst_impetus"),
            "Nova Initiumia is a constructed-legal identity and stays listed"
        );
        // The cards remain implemented for the preset decks that carry them.
        assert!(is_supported("The Catalyst: Convention Breaker"));
    }

    /// The Hedge Fund class: a definition no deck list carries must still be
    /// engine-supported and catalog-visible (`off_list_cards` feeds
    /// `all_cards`). The floor pins the catalog's magnitude so a silently
    /// shrunken completeness join screams instead of shipping.
    #[test]
    fn off_list_cards_reach_the_catalog_and_the_floor_holds() {
        assert!(is_supported("Hedge Fund"), "Hedge Fund is implemented and complete");
        let cat = catalog_json();
        let cards = cat["cards"].as_array().unwrap();
        let hedge = cards
            .iter()
            .find(|c| c["id"] == "hedge_fund")
            .expect("Hedge Fund is in the eternal catalog");
        assert_eq!(hedge["banned"], false);
        assert_eq!(hedge["points"], 0);
        // Validation agrees: three copies in a corp deck raise no
        // `unsupported` problem for it.
        let v = validate(
            "nebula_talent_management_making_stars",
            &deck(&[("hedge_fund", 3)]),
        );
        assert!(
            !v.problems
                .iter()
                .any(|p| p.code == "unsupported" && p.card.as_deref() == Some("hedge_fund")),
            "{:?}",
            v.problems
        );
        // Magnitude floors: both priority decks' non-identity cards plus the
        // complete half of unlisted.rs plus Hedge Fund clear 60; the
        // identity queue clears 130.
        assert!(cards.len() >= 60, "catalog cards collapsed to {}", cards.len());
        let identities = cat["identities"].as_array().unwrap();
        assert!(identities.len() >= 130, "catalog identities collapsed to {}", identities.len());
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

    // ── CR 1.5.3a: the cards an identity brings from outside the deck ──────

    /// Every identity but Adam brings nothing from outside its deck, and the
    /// engine says so without consulting the card layer's completeness.
    #[test]
    fn an_ordinary_identity_brings_no_extra_cards() {
        for title in [
            "Andromeda: Dispossessed Ristie",
            "Nebula Talent Management: Making Stars",
            "Chaos Theory: W\u{fc}nderkind",
        ] {
            let c = jinteki_cards::find(title).expect(title);
            assert!(
                c.printed.starting_extra_installs.is_none(),
                "{title} prints no 1.5.3 setup fact"
            );
            assert_eq!(
                starting_extra_cards(&c.printed).expect("no fact, nothing to bring").len(),
                0
            );
            assert!(extra_cards_blocker(title).is_none(), "{title} is seatable");
        }
    }

    /// Adam is the one identity that requires cards from outside the deck,
    /// and today exactly one of the three directives blocks him. SYS-D-12:
    /// a directive BEGINS the game installed and active (CR 1.5.3b/d), so an
    /// unimplemented one is a card played in a game whose behaviour is not
    /// implemented — the same bar `cr::readiness()` holds the 1.5.4a pile to.
    ///
    /// This assertion is also the ratchet: the day Always Be Running's first
    /// printed sentence lands, this test fails and the gate above it opens.
    #[test]
    fn adam_is_refused_by_exactly_one_unimplemented_directive() {
        let adam = jinteki_cards::find("Adam: Compulsive Hacker").expect("Adam is implemented");
        assert!(adam.is_complete(), "the identity card itself is complete");
        let fact = adam.printed.starting_extra_installs.clone().expect("Adam prints 1.5.3");
        assert_eq!(fact.count, 3);
        assert!(fact.distinct_names);

        // The three directives the engine carries — the whole printed set.
        let directives: Vec<(String, bool)> = jinteki_cards::all_cards()
            .into_iter()
            .filter(|c| c.printed.subtypes.contains(&jinteki_cr::Subtype::Directive))
            .map(|c| (c.name().to_string(), c.is_complete()))
            .collect();
        assert_eq!(directives.len(), 3, "1.5.3a needs 3 differently named: {directives:?}");

        match starting_extra_cards(&adam.printed) {
            Ok(cards) => panic!(
                "Always Be Running has an unimplemented sentence; Adam must not seat: {:?}",
                cards.iter().map(|c| c.name).collect::<Vec<_>>()
            ),
            Err(blockers) => assert_eq!(blockers, vec!["Always Be Running".to_string()]),
        }
        let why = extra_cards_blocker("Adam: Compulsive Hacker").expect("a stated reason");
        assert!(why.contains("Always Be Running"), "{why}");
    }

    /// SYS-D-12's other half — the card stays visible and inspectable — but
    /// the deck builder never offers an identity every deck of which would
    /// be refused, and a deck saved with one comes back with the reason.
    #[test]
    fn adam_is_off_the_builder_and_a_saved_adam_deck_says_why() {
        let adam = carddata::by_title("Adam: Compulsive Hacker").expect("Adam has a printing");
        // Nothing about Adam himself excludes him: pool, points, format all fine.
        assert!(eternal().in_pool(adam));
        assert!(!draft_only(adam));
        assert!(!starter_pack_only(adam));
        assert!(is_supported(&adam.title), "the identity card is complete and inspectable");

        let cat = catalog_json();
        let identities = cat["identities"].as_array().unwrap();
        assert!(
            !identities.iter().any(|c| c["id"] == "adam_compulsive_hacker"),
            "an identity whose extra cards the engine cannot supply is not offered"
        );

        // A deck saved with him anyway (an older save, or a direct API call)
        // is not legal, and the problem names the directive.
        let v = validate("adam_compulsive_hacker", &deck(&[("sure_gamble", 3)]));
        assert!(!v.legal);
        let p = v
            .problems
            .iter()
            .find(|p| p.code == "unsupported" && p.message.contains("Always Be Running"))
            .unwrap_or_else(|| panic!("no 1.5.3a problem in {:?}", v.problems));
        assert_eq!(p.card.as_deref(), Some("adam_compulsive_hacker"));
        assert!(
            p.message.contains("outside the deck"),
            "the reason says where the cards come from: {}",
            p.message
        );
    }
}
