//! The influence-waiver guard: what stops the next Alliance card being
//! charged full price in silence.
//!
//! Twelve cards print a first line that waives their own influence in a deck
//! that meets a condition ("This card costs 0 influence if you have 15 or
//! fewer ice in your deck"). `jinteki_server::influence` reads that sentence
//! instead of keying off card titles, for one reason: a title-keyed table is
//! a table that omits the thirteenth card. The omission would be invisible —
//! the deck would just quietly cost more influence than NetrunnerDB says, the
//! way Mezzie's Asa did at 21 against a limit of 15.
//!
//! This file is the half that keeps that true. It reads the card database and
//! insists that EVERY card whose text says "costs 0 influence" parses to a
//! waiver. A new card of a known shape passes the day its text lands; a new
//! card of an unknown shape fails the build with the sentence it could not
//! read, which is a job ticket rather than a silent wrong answer.
//!
//! ## Where the card data comes from
//!
//! `crates/jinteki-core/carddata/cards.json`, through
//! `jinteki_server::carddata::all()`. That file is inside the workspace and
//! already `include_str!`d by the crate under test, so nothing here reaches
//! outside the tree — the same constraint `crates/jinteki-cr/tests/
//! subtypes.rs` works under, met the same way. The vendored NSG tree at
//! `crates/jinteki-server/data/nsg-v2/` was NOT extended: its `printings/`
//! carry ids, positions and illustrators but no card TEXT, and card text is
//! the only thing this guard reads. `cards.json` is also the exact data the
//! validator prices decks from, so a guard over it is a guard over what
//! actually runs.
//!
//! ## Not a CR rule
//!
//! Influence is a tournament construct and the waiver is printed card text.
//! Nothing here cites the CR, and nothing in `influence.rs` does either.

use jinteki_server::carddata::{self, Card};
use jinteki_server::eternal;
use jinteki_server::influence::{self, DeckCounts, Faction, Waiver};
use std::collections::BTreeMap;

/// The printed words that mark a waiver.
const MARKER: &str = "costs 0 influence";

/// Text with NSG's `<strong>` markup and non-breaking spaces flattened, the
/// way the parser sees it. Duplicated here on purpose: the guard reads the
/// data the way a human reads it, not the way the parser does.
fn flatten(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            c if c.is_whitespace() => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().to_string()
}

fn cards_with_marker() -> Vec<&'static Card> {
    carddata::all()
        .iter()
        .filter(|c| c.text.as_deref().is_some_and(|t| flatten(t).contains(MARKER)))
        .collect()
}

// ───────────────────────────────────────────────────────────────────────────
// The guard
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn every_printed_waiver_in_the_card_data_parses() {
    let mut unparsed: Vec<String> = Vec::new();
    let mut silent: Vec<String> = Vec::new();
    for c in cards_with_marker() {
        match influence::waiver_of(c) {
            Ok(Some(_)) => {}
            Ok(None) => silent.push(c.title.clone()),
            Err(e) => unparsed.push(format!("{}: {e}", c.title)),
        }
    }
    assert!(
        silent.is_empty(),
        "these cards print \"costs 0 influence\" and the parser returned NO waiver \
         for them, which means they are being charged full price with nothing said: \
         {silent:?}",
    );
    assert!(
        unparsed.is_empty(),
        "these cards print an influence waiver jinteki_server::influence cannot \
         read. Each is a predicate to implement in influence.rs — never a card to \
         charge full price: {unparsed:?}",
    );
}

#[test]
fn the_guard_covers_the_twelve_cards_that_print_a_waiver_today() {
    let found: Vec<&str> = cards_with_marker().iter().map(|c| c.title.as_str()).collect();
    // The eight Alliance cards plus the four one-offs. Listed so that a card
    // vanishing from the data is as loud as a card arriving in it.
    for title in [
        "Consulting Visit",
        "Executive Search Firm",
        "Heritage Committee",
        "Ibrahim Salem",
        "Jeeves Model Bioroids",
        "Mumba Temple",
        "Mumbad Virtual Tour",
        "Museum of History",
        "PAD Factory",
        "Product Recall",
        "Raman Rai",
        "Salem's Hospitality",
    ] {
        assert!(
            found.contains(&title),
            "{title} prints an influence waiver and the guard no longer sees it; \
             either the card data lost the card or its text changed shape",
        );
    }
    assert!(
        found.len() >= 12,
        "the card data carries fewer waiver cards than the twelve known ones: {found:?}",
    );
}

#[test]
fn every_shape_the_parser_knows_is_exercised_by_real_card_data() {
    let (mut non_alliance, mut ice, mut assets, mut cards, mut named) = (0, 0, 0, 0, 0);
    for c in cards_with_marker() {
        match influence::waiver_of(c).expect("guarded above").expect("guarded above") {
            Waiver::NonAllianceFactionAtLeast { .. } => non_alliance += 1,
            Waiver::IceAtMost(_) => ice += 1,
            Waiver::AssetsAtLeast(_) => assets += 1,
            Waiver::CardsAtLeast(_) => cards += 1,
            Waiver::NamedCopiesAtLeast { .. } => named += 1,
        }
    }
    assert_eq!(non_alliance, 8, "the eight Alliance cards share one shape");
    assert_eq!(ice, 1, "Mumba Temple is the only upper-bound-on-ice waiver");
    assert_eq!(assets, 1, "Mumbad Virtual Tour is the only assets waiver");
    assert_eq!(cards, 1, "Museum of History is the only deck-size waiver");
    assert_eq!(named, 1, "PAD Factory is the only named-copies waiver");
}

#[test]
fn a_waiver_is_never_printed_on_a_back_face() {
    // `waiver_of` reads `Card::text` only. If a double-sided card ever put a
    // deckbuilding waiver on a back face, that reading would miss it — so the
    // assumption is pinned rather than assumed.
    for c in carddata::all() {
        for (i, face) in c.faces.iter().enumerate() {
            let Some(t) = face.text.as_deref() else { continue };
            assert!(
                !flatten(t).contains(MARKER),
                "{} prints an influence waiver on face {i}, which influence::waiver_of \
                 does not read; it reads Card::text only",
                c.title,
            );
        }
    }
}

#[test]
fn no_other_card_text_touches_influence_unnoticed() {
    // The wider tripwire: "influence" appears in printed text only as deck
    // construction — Netrunner has no in-game influence — so any card that
    // says the word and is not a waiver is a deckbuilding rule nothing
    // implements. One exists today and is named here; a second must be
    // decided on by a human rather than discovered in a wrong deck total.
    const KNOWN_OTHER: &[&str] = &[
        // "The first copy of each program in this deck does not count against
        // your influence limit." A reduction, not a waiver, and a different
        // predicate shape (per-title, first copy only). Still unimplemented;
        // the reference validator handles it at validator.cljc:88-92,111-113.
        "The Professor: Keeper of Knowledge",
    ];
    let mut surprises: Vec<String> = Vec::new();
    for c in carddata::all() {
        let Some(t) = c.text.as_deref() else { continue };
        let flat = flatten(t);
        if !flat.contains("influence") || flat.contains(MARKER) {
            continue;
        }
        if KNOWN_OTHER.contains(&c.title.as_str()) {
            continue;
        }
        surprises.push(format!("{}: {flat}", c.title));
    }
    assert!(
        surprises.is_empty(),
        "these cards print a deck-construction rule about influence that is neither \
         a waiver this crate parses nor one of the known-unimplemented shapes. \
         Decide which it is, in code: {surprises:?}",
    );
}

#[test]
fn faction_all_is_every_faction_the_card_data_names() {
    let mut missing: Vec<&str> = Vec::new();
    for c in carddata::all() {
        let Some(f) = c.faction.as_deref() else { continue };
        if Faction::from_name(f).is_none() && !missing.contains(&f) {
            missing.push(f);
        }
    }
    assert!(
        missing.is_empty(),
        "the card data names factions Faction::ALL does not: {missing:?}. A waiver \
         naming one of them could never be parsed.",
    );
    for f in Faction::ALL {
        assert!(
            carddata::all().iter().any(|c| c.faction.as_deref() == Some(f.name())),
            "Faction::{f:?} spells itself {:?}, which no card in the database carries",
            f.name(),
        );
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Evaluated against real decks, through the real validator
// ───────────────────────────────────────────────────────────────────────────

fn deck(v: &[(&str, u32)]) -> BTreeMap<String, u32> {
    v.iter().map(|(id, q)| (id.to_string(), *q)).collect()
}

/// Mezzie's Asa as `deck_specs()` seats it, in the validator's id vocabulary.
fn mezzie_asa() -> (String, BTreeMap<String, u32>) {
    let mut identity = String::new();
    let mut cards = BTreeMap::new();
    for (title, qty) in jinteki_server::cr::MEZZIE_ASA.list {
        let c = carddata::by_title(title)
            .unwrap_or_else(|| panic!("{title} is in the card database"));
        let id = c.nsg_id.clone().unwrap_or_else(|| panic!("{title} has an NSG id"));
        if c.is_identity() {
            identity = id;
        } else {
            cards.insert(id, *qty);
        }
    }
    (identity, cards)
}

/// The influence problems of a verdict, message-first. Other problems (engine
/// support, agenda band) are a different subject and are not filtered out of
/// existence — they are simply not what these tests are about.
fn influence_problems(v: &eternal::Verdict) -> Vec<String> {
    v.problems.iter().filter(|p| p.code == "influence").map(|p| p.message.clone()).collect()
}

#[test]
fn mezzies_asa_is_legal_and_the_three_mumba_temples_are_free() {
    let (identity, cards) = mezzie_asa();
    let v = eternal::validate(&identity, &cards);
    assert!(
        v.legal,
        "Mezzie's Asa runs 12 ice, so all three Mumba Temples cost 0 influence and \
         the deck spends 15 against Asa Group's limit of 15. It is legal, and \
         NetrunnerDB agrees. Problems: {:?}",
        v.problems,
    );
}

#[test]
fn mezzies_asa_spends_exactly_fifteen_influence() {
    // The verdict reports a total only when the total is over the limit, so
    // the number is pinned from above: add exactly one pip of out-of-faction
    // influence (Hired Help, Neutral, 1) in place of one Vanilla (Neutral, 0)
    // and the deck must report SIXTEEN. Legal at 15 (above) and 16 with one
    // pip more is 15 exactly, with no arithmetic taken on trust.
    let (identity, mut cards) = mezzie_asa();
    cards.insert("vanilla".to_string(), 2);
    cards.insert("hired_help".to_string(), 1);
    let v = eternal::validate(&identity, &cards);
    assert_eq!(
        influence_problems(&v),
        vec!["16 influence used; the identity allows 15".to_string()],
        "one extra influence pip on top of Mezzie's Asa must read 16, which makes \
         the deck's own total 15 — not the 21 it read when Mumba Temple was charged",
    );
}

#[test]
fn sixteen_ice_makes_mumba_temple_cost_full_price() {
    // The polarity case. Mumba Temple is free at "15 OR FEWER ice"; a
    // reversed comparison would make it free in exactly the decks where it
    // should cost. Swap four Haas-Bioroid assets (in-faction, no influence)
    // for four Haas-Bioroid ice (in-faction, no influence): the deck is still
    // 49 cards and every other line prices identically, so the whole
    // difference is the Temples.
    let (identity, mut cards) = mezzie_asa();
    cards.remove("estelle_moon"); // −3 assets
    cards.remove("marilyn_campaign"); // −1 asset
    cards.insert("vertigo".to_string(), 3); // +2 ice
    cards.insert("drafter".to_string(), 3); // +1 ice
    cards.insert("fairchild_3_0".to_string(), 3); // +1 ice → 16
    let v = eternal::validate(&identity, &cards);
    assert_eq!(
        influence_problems(&v),
        vec!["21 influence used; the identity allows 15".to_string()],
        "at 16 ice the waiver does not fire and the three Temples cost 2 each: the \
         deck is back to 21 influence, which is what it must be",
    );
    assert!(!v.legal, "and a 21-influence deck under a 15 limit is not legal");
}

#[test]
fn fifteen_ice_is_still_within_mumba_temples_bound() {
    // The boundary itself: "15 or fewer" includes 15.
    let (identity, mut cards) = mezzie_asa();
    cards.remove("estelle_moon"); // −3 assets
    cards.insert("vertigo".to_string(), 3); // +2 ice
    cards.insert("drafter".to_string(), 3); // +1 ice → 15
    let v = eternal::validate(&identity, &cards);
    assert_eq!(
        influence_problems(&v),
        Vec::<String>::new(),
        "15 ice satisfies \"15 or fewer\" — the bound is inclusive, and one ice \
         either side of it is the whole difference between 15 and 21 influence",
    );
}

/// An Alliance deck built to sit either side of the six-card threshold: NBN
/// with three Jeeves Model Bioroids (Haas-Bioroid, 3 influence each) and six
/// non-Alliance Haas-Bioroid cards at 2 influence each.
///
/// With six, Jeeves is free and the deck spends 12 of NBN's 15. Take one
/// away and Jeeves costs 9, so the deck spends 19 and is over. Nothing else
/// changes, so the flip is the waiver and only the waiver.
///
/// The list is a probe, not a tournament deck: it is short and its cards are
/// not all engine-complete, so it carries deck-size and support problems that
/// these assertions deliberately do not look at. The influence arithmetic is
/// the subject.
fn alliance_probe(drafters: u32) -> eternal::Verdict {
    let cards = deck(&[
        ("jeeves_model_bioroids", 3),
        ("estelle_moon", 3),
        ("drafter", drafters),
    ]);
    eternal::validate("nbn_making_news", &cards)
}

#[test]
fn an_alliance_card_is_free_at_its_threshold_and_charged_below_it() {
    assert_eq!(
        influence_problems(&alliance_probe(3)),
        Vec::<String>::new(),
        "six non-Alliance Haas-Bioroid cards waive Jeeves entirely: the deck spends \
         12 (six cards at 2) of NBN: Making News's 15",
    );
    assert_eq!(
        influence_problems(&alliance_probe(2)),
        vec!["19 influence used; the identity allows 15".to_string()],
        "five is below the threshold, so three Jeeves cost 3 each: 10 for the five \
         Haas-Bioroid cards plus 9 is 19, over NBN: Making News's 15",
    );
}

#[test]
fn an_alliance_card_cannot_pay_for_itself() {
    // "non-alliance" is the load-bearing word. A deck of nothing but Alliance
    // cards of one faction has none of them, however many copies it runs.
    let cards =
        deck(&[("jeeves_model_bioroids", 3), ("product_recall", 3), ("tour_guide", 1)]);
    let v = eternal::validate("nbn_making_news", &cards);
    assert_eq!(
        influence_problems(&v),
        vec!["17 influence used; the identity allows 15".to_string()],
        "six Haas-Bioroid cards, every one of them Alliance, waive nothing: three \
         Jeeves at 3 plus three Product Recalls at 2 plus a Tour Guide at 2 is 17, \
         because the count \"non-alliance\" asks for is still zero",
    );
}

// ───────────────────────────────────────────────────────────────────────────
// The three waivers no seated deck exercises
// ───────────────────────────────────────────────────────────────────────────

#[test]
fn the_one_off_waivers_evaluate_on_the_decks_they_describe() {
    let card = |t: &str| {
        carddata::by_title(t).unwrap_or_else(|| panic!("{t} is in the card database"))
    };
    let waiver = |t: &str| {
        influence::waiver_of(card(t))
            .unwrap_or_else(|e| panic!("{t} prints a readable waiver, got: {e}"))
            .unwrap_or_else(|| panic!("{t} prints a waiver at all"))
    };

    // Museum of History: 50 or more CARDS, counting itself.
    let fifty = DeckCounts::tally([(card("Museum of History"), 3), (card("Hedge Fund"), 47)]);
    assert!(fifty.satisfies(&waiver("Museum of History")), "50 cards is \"50 or more\"");
    let forty_nine =
        DeckCounts::tally([(card("Museum of History"), 3), (card("Hedge Fund"), 46)]);
    assert!(
        !forty_nine.satisfies(&waiver("Museum of History")),
        "49 cards is not, and the Museums themselves are three of the fifty",
    );

    // Mumbad Virtual Tour: 7 or more ASSETS, and it is an upgrade, so it is
    // not one of them.
    let seven = DeckCounts::tally([(card("Mumbad Virtual Tour"), 3), (card("PAD Campaign"), 7)]);
    assert!(seven.satisfies(&waiver("Mumbad Virtual Tour")), "7 assets is \"7 or more\"");
    let six = DeckCounts::tally([(card("Mumbad Virtual Tour"), 3), (card("PAD Campaign"), 6)]);
    assert!(
        !six.satisfies(&waiver("Mumbad Virtual Tour")),
        "an upgrade does not count itself among the assets, so six is six",
    );

    // PAD Factory: three PAD Campaigns, by name.
    let three = DeckCounts::tally([(card("PAD Factory"), 3), (card("PAD Campaign"), 3)]);
    assert!(three.satisfies(&waiver("PAD Factory")), "three PAD Campaigns is the condition");
    let two = DeckCounts::tally([(card("PAD Factory"), 3), (card("PAD Campaign"), 2)]);
    assert!(
        !two.satisfies(&waiver("PAD Factory")),
        "two is not three, and PAD Factory is not a PAD Campaign",
    );
}
