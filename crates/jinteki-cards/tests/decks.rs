//! The two priority decks, as data.
//!
//! These tests are the contract between the deck modules and everything that
//! consumes them: every card builds, every card carries its printed text, the
//! doc comment and the data say the same thing, and the gap list is what the
//! deck files say it is.

use jinteki_cards::{deck_named, mezzie_decks, pile_named, priority_decks, SOURCES};

#[test]
fn both_decks_build() {
    let cards = priority_decks();
    assert_eq!(cards.len(), 71, "both decks, one entry per distinct card (Hedge Fund is not in the printed Gauntlet list), plus CR 1.5.4a's pile");
    assert_eq!(deck_named("andromeda").unwrap().len(), 24);
    assert_eq!(deck_named("gauntlet").unwrap().len(), 26, "49 printed cards, 26 distinct — Hedge Fund left on the deck photo's authority");
    assert!(deck_named("nonesuch").is_none());
    // CR 1.5.4a: the pile is beside the deck, not in it, and it is the
    // Runner's. "Any number" of identities, so it grows as the identity queue
    // completes them (docs/vm/IDENTITY-QUEUE.md).
    assert_eq!(pile_named("andromeda").unwrap().len(), 21);
    assert!(pile_named("gauntlet").unwrap().is_empty());
    assert!(pile_named("nonesuch").is_none());
}

/// Mezzie's two decks (`docs/vm/MEZZIE-QUEUE.md`) are mid-queue, so the thing
/// to hold them to is not "no partial cards" — that is what the queue's
/// tick-boxes are counting towards — but that everything already in them is
/// as honest as a priority-deck card. The gap list is PRINTED rather than
/// ratcheted at zero (SYS-D-9: the point of the marker is that it is
/// countable), so a later wave shrinking it never has to move an assertion.
#[test]
fn mezzies_decks_are_honest_as_far_as_they_go() {
    let cards = mezzie_decks();
    assert!(!cards.is_empty(), "the two modules are registered and return their cards");
    for (key, expect) in [("mezzie_asa", 13usize), ("mezzie_valencia", 10usize)] {
        let deck = deck_named(key).unwrap_or_else(|| panic!("the card layer has no deck {key:?}"));
        assert_eq!(deck.len(), expect, "{key}: the cards written so far");
        for c in &deck {
            assert!(!c.oracle_text.trim().is_empty(), "{key}: {} has no printed text", c.name());
            assert!(c.printed.faction.is_some(), "{key}: {} prints a faction (2.13)", c.name());
            for a in &c.printed.abilities {
                assert!(
                    a.label.starts_with(&c.name().to_lowercase()) || a.label == "base link",
                    "{key}: {}'s ability label {:?} does not name its card",
                    c.name(),
                    a.label
                );
            }
            // A complete card denotes into SOMETHING — an ability, a 1.16.10
            // printed additional cost, or one of 1.6's setup facts (Valencia's
            // whole card is the Corp's starting bad publicity).
            if c.is_complete() {
                assert!(
                    !c.printed.abilities.is_empty()
                        || c.printed.additional_steal_cost.is_some()
                        || c.printed.additional_play_cost.is_some()
                        || c.printed.starting_hand_size.is_some()
                        || c.printed.starting_credits.is_some()
                        || c.printed.starting_bad_publicity.is_some(),
                    "{key}: {} is marked complete but denotes into nothing",
                    c.name()
                );
            }
        }
        let complete = deck.iter().filter(|c| c.is_complete()).count();
        let sentences: usize = deck.iter().map(|c| c.unimplemented.len()).sum();
        println!(
            "{key}: {} cards written, {complete} complete, {} partial, \
             {sentences} printed sentences still unsayable",
            deck.len(),
            deck.len() - complete
        );
    }
    // CR 1.5.4a: a Corp deck brings no pile at all, and Valencia's is a
    // decision for the wave that writes her deck's own cards.
    assert!(pile_named("mezzie_asa").unwrap().is_empty(), "a Corp deck brings no 1.5.4a pile");
    assert!(pile_named("mezzie_valencia").unwrap().is_empty(), "Valencia's pile is not chosen yet");
}

/// CR 1.5.4a: every card of the pile is a Runner IDENTITY, and 1.5.4b's
/// "from the same faction" needs one to be readable off the card.
#[test]
fn the_identity_pile_is_identities_with_factions() {
    for deck in ["andromeda", "gauntlet"] {
        for c in pile_named(deck).unwrap() {
            assert_eq!(c.printed.card_type, jinteki_cr::object::CardType::Identity, "{}", c.name());
            assert_eq!(c.printed.side, jinteki_cr::object::Side::Runner, "{}", c.name());
            assert!(c.printed.faction.is_some(), "{}: 2.13.3 gives every identity a faction", c.name());
        }
    }
}

#[test]
fn every_card_carries_its_printed_text() {
    // SYS-D-10: behaviour is checked against the text, so the text must be
    // there — and `.build()` refuses a card without it, so this also proves
    // no card slipped through by another route.
    for c in priority_decks() {
        assert!(!c.oracle_text.trim().is_empty(), "{} has no printed text", c.name());
    }
}

#[test]
fn every_card_is_named_and_typed_consistently() {
    for c in priority_decks() {
        assert!(!c.printed.name.is_empty());
        // 2.3: an agenda has no play/install/rez cost; it has a requirement
        // and a point value instead.
        if c.printed.card_type == jinteki_cr::object::CardType::Agenda {
            assert!(c.printed.cost.is_none(), "{}: agendas have no cost", c.name());
            assert!(c.printed.advancement_requirement.is_some(), "{}", c.name());
            assert!(c.printed.agenda_points.is_some(), "{}", c.name());
        }
        if c.printed.card_type == jinteki_cr::object::CardType::Ice {
            assert!(c.printed.strength.is_some(), "{}: ice has a strength", c.name());
        }
        // 2.13: every card is in a faction — 2.13.2 gives even a card with no
        // logo one ("if a card has a white background and no logo, it is
        // neutral"). It is an in-game characteristic, not deck metadata.
        assert!(c.printed.faction.is_some(), "{}: every card prints a faction", c.name());
    }
}

/// The gap list, printed and ratcheted. The point of the marker is that it is
/// countable, not that it is zero yet (SYS-D-9).
#[test]
fn the_gap_list_is_measurable_and_honest() {
    let cards = priority_decks();
    let complete: Vec<&str> = cards.iter().filter(|c| c.is_complete()).map(|c| c.name()).collect();
    let sentences: usize = cards.iter().map(|c| c.unimplemented.len()).sum();
    println!(
        "priority decks: {} cards, {} complete, {} partial, {} printed sentences still unsayable",
        cards.len(),
        complete.len(),
        cards.len() - complete.len(),
        sentences
    );
    println!("complete: {complete:?}");
    assert_eq!(
        complete.len(),
        cards.len(),
        "every printed sentence of both decks is expressed; got {} of {} — if a card became \
         partial, say why in docs/vm/WAVES.md's gap list",
        complete.len(),
        cards.len()
    );
    assert_eq!(
        sentences, 0,
        "the gap list is empty and must stay empty: a sentence that cannot be said again \
         needs a reason recorded in docs/vm/WAVES.md"
    );
}

/// No card is complete by saying nothing: a card with no marker and no
/// abilities would be a silent lie.
#[test]
fn no_card_is_complete_by_saying_nothing() {
    for c in priority_decks() {
        if !c.is_complete() {
            continue;
        }
        assert!(
            !c.printed.abilities.is_empty()
                || c.printed.additional_steal_cost.is_some()
                || c.printed.additional_play_cost.is_some(),
            "{} is marked complete but denotes into nothing",
            c.name()
        );
    }
}

/// Every ability carries a label naming its card, so plan-driver tests and
/// game logs can pick abilities out by name.
#[test]
fn every_ability_is_labelled_with_its_card() {
    for c in priority_decks() {
        for a in &c.printed.abilities {
            assert!(!a.label.is_empty(), "{}: an unlabelled ability", c.name());
            assert!(
                a.label.starts_with(&c.name().to_lowercase()) || a.label == "base link",
                "{}: ability label {:?} does not name its card",
                c.name(),
                a.label
            );
        }
    }
}

// ---------------------------------------------------------------------------
// SYS-D-10's two halves have to agree
// ---------------------------------------------------------------------------

/// The printed text is written twice on purpose: in the doc comment, for
/// whoever is reading the file, and in `.text(…)`, for whatever is checking
/// behaviour against it. This test is what keeps the two from drifting — the
/// only thing that makes writing it twice safe.
#[test]
fn the_doc_comment_and_the_data_carry_the_same_printed_text() {
    let mut checked = 0;
    for (file, src) in SOURCES {
        for (func, doc, data) in cards_in(src) {
            // A card whose text box really is blank says so with
            // `.no_printed_text()`. Then the doc comment must quote nothing
            // either — the two halves still have to agree.
            if data == BLANK_TEXT_BOX {
                assert!(
                    doc.is_empty(),
                    "{file}: {func} says its text box is blank but its doc comment quotes text"
                );
                checked += 1;
                continue;
            }
            assert!(!doc.is_empty(), "{file}: {func} has no quoted printed text in its doc comment");
            assert_eq!(
                normalise(&doc),
                normalise(&data),
                "{file}: {func}'s doc comment and .text(…) disagree"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 243, "one check per card DEFINITION (Hedge Fund is defined but not listed; Gemilang Arena is Nebula's back face, Ascending to Orbit is Earth Station's; Ken Tenma is CR 1.5.4a's pile; unlisted.rs is what no deck lists; identities/ is the CR 1.5.4a queue; mezzie_asa.rs is 6 of Mezzie's ice and 5 of her assets, mezzie_valencia.rs is Zer0 and three programs)");
}

/// Collapse to one space-separated line: the doc comment wraps for width and
/// the data does not, and that difference is not a discrepancy.
fn normalise(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Every definition is reachable
// ---------------------------------------------------------------------------

/// Every `card("Title")` a module opens, by scanning the same SOURCES the
/// manifest test reads. The builder call sits alone on its line in every
/// module, and escaped quotes (Boris "Syfr" Kovac) are part of the title.
fn defined_titles(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| {
            let t = l.trim();
            let rest = t.strip_prefix("card(\"")?;
            let end = rest.rfind("\")")?;
            Some(rest[..end].replace("\\\"", "\""))
        })
        .collect()
}

/// A definition no runtime surface reaches is a bug of the Hedge Fund class:
/// implemented, complete, and invisible to everything that asks "what does
/// the engine support?" — `find`, and the eternal catalog's completeness
/// join, which is how Hedge Fund silently fell out of the deck builder.
/// Every definition must be reachable through `all_cards()`, either as a
/// card of its own (a deck list, `unlisted.rs`, the identity queue, or
/// `off_list_cards()`) or as a flip face of one (Gemilang Arena is Nebula's
/// back, Ascending to Orbit is Earth Station's).
#[test]
fn every_definition_is_reachable_from_all_cards() {
    let all = jinteki_cards::all_cards();
    let mut reachable: std::collections::HashSet<String> =
        all.iter().map(|c| c.name().to_string()).collect();
    for c in &all {
        for f in &c.printed.flip_faces {
            reachable.insert(f.name.to_string());
        }
    }
    let mut checked = 0;
    for (file, src) in SOURCES {
        for title in defined_titles(src) {
            assert!(
                reachable.contains(&title),
                "{file}: card({title:?}) is defined but no surface reaches it — list it in \
                 a deck, unlisted.rs, the identity queue, or off_list_cards()"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 243, "one reachability check per card definition");
}

/// What `cards_in` reports for a card that declared `.no_printed_text()` —
/// distinguishable from a card that merely forgot to copy its text in.
const BLANK_TEXT_BOX: &str = "\u{0}blank text box";

/// Walk a deck module: for each `pub fn … -> Card`, the quoted block of its
/// doc comment and the concatenation of its `.text(…)` arguments.
fn cards_in(src: &str) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    let mut doc: Vec<String> = Vec::new();
    let mut in_quote = false;
    let mut quote_done = false;
    let mut current: Option<(String, String)> = None;
    let mut data = String::new();

    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("///") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            // The quoted block runs from the line opening the quote to the one
            // closing it. The header line above it is not printed text, and
            // neither is the UNIMPLEMENTED prose below — which quotes the card
            // back at itself, so the scan has to stop rather than reopen.
            if quote_done {
                continue;
            }
            if !in_quote && rest.starts_with('"') {
                in_quote = true;
                doc.push(unescape(rest.trim_start_matches('"')));
            } else if in_quote {
                doc.push(unescape(rest));
            }
            if in_quote && ends_quote(rest) {
                in_quote = false;
                quote_done = true;
                if let Some(last) = doc.last_mut() {
                    *last = last.trim_end().trim_end_matches('"').to_string();
                }
            }
            continue;
        }
        if t.starts_with("pub fn ") {
            if let Some((name, d)) = current.take() {
                out.push((name, d, std::mem::take(&mut data)));
            }
            data.clear();
            if t.contains("-> Card") {
                let name = t["pub fn ".len()..].split('(').next().unwrap_or("").to_string();
                current = Some((name, doc.join("\n")));
            }
            doc.clear();
            in_quote = false;
            quote_done = false;
            continue;
        }
        if current.is_some() {
            if t.starts_with(".no_printed_text()") {
                data = BLANK_TEXT_BOX.to_string();
            } else if let Some(arg) = text_argument(t) {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(&arg);
            }
        }
    }
    if let Some((name, d)) = current.take() {
        out.push((name, d, data));
    }
    out
}

/// Does this doc-comment line close the quoted block?
fn ends_quote(s: &str) -> bool {
    let s = s.trim_end();
    s.ends_with('"') && !s.ends_with("\\\"")
}

/// The argument of a `.text("…")` call, unescaped.
fn text_argument(line: &str) -> Option<String> {
    let rest = line.strip_prefix(".text(\"")?;
    let end = closing_quote(rest)?;
    Some(unescape(&rest[..end]))
}

/// Index of the string literal's closing quote, skipping escaped ones.
fn closing_quote(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("\\\"", "\"").replace("\\\\", "\\")
}
