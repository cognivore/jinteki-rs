//! The two priority decks, as data.
//!
//! These tests are the contract between the deck modules and everything that
//! consumes them: every card builds, every card carries its printed text, the
//! doc comment and the data say the same thing, and the gap list is what the
//! deck files say it is.

use jinteki_cards::{deck_named, pile_named, priority_decks, SOURCES};

#[test]
fn both_decks_build() {
    let cards = priority_decks();
    assert_eq!(cards.len(), 51, "both decks, one entry per distinct card (Hedge Fund is not in the printed Gauntlet list), plus CR 1.5.4a's pile");
    assert_eq!(deck_named("andromeda").unwrap().len(), 24);
    assert_eq!(deck_named("gauntlet").unwrap().len(), 26, "49 printed cards, 26 distinct — Hedge Fund left on the deck photo's authority");
    assert!(deck_named("nonesuch").is_none());
    // CR 1.5.4a: the pile is beside the deck, not in it, and it is the
    // Runner's.
    assert_eq!(pile_named("andromeda").unwrap().len(), 1);
    assert!(pile_named("gauntlet").unwrap().is_empty());
    assert!(pile_named("nonesuch").is_none());
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
    assert!(
        complete.len() >= 45,
        "45 cards are fully expressed; got {} — if a card became partial, say why in \
         docs/vm/WAVES.md's gap list",
        complete.len()
    );
    assert!(
        sentences <= 13,
        "the gap list should not grow without a reason recorded in docs/vm/WAVES.md; got {sentences}"
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
            assert!(!doc.is_empty(), "{file}: {func} has no quoted printed text in its doc comment");
            assert_eq!(
                normalise(&doc),
                normalise(&data),
                "{file}: {func}'s doc comment and .text(…) disagree"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 68, "one check per card DEFINITION (Hedge Fund is defined but not listed; Gemilang Arena is Nebula's back face; Ken Tenma is CR 1.5.4a's pile; unlisted.rs is what no deck lists)");
}

/// Collapse to one space-separated line: the doc comment wraps for width and
/// the data does not, and that difference is not a discrepancy.
fn normalise(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

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
            if let Some(arg) = text_argument(t) {
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
