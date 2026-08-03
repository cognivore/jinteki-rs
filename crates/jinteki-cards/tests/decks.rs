//! The two priority decks, as data.
//!
//! These tests are the contract between the card files and everything that
//! consumes them: every card parses, every card carries its printed text, the
//! gap list is what the files say it is, and the cards that ARE complete
//! actually run in the VM.

use jinteki_cards::{load, priority_decks, ANDROMEDA, GAUNTLET};

#[test]
fn every_card_in_both_decks_parses_and_denotes() {
    let cards = priority_decks().unwrap_or_else(|e| panic!("{e}"));
    assert!(cards.len() >= 50, "both decks, one entry per distinct card: {}", cards.len());
}

#[test]
fn every_card_carries_its_printed_text() {
    // SYS-D-10: behaviour is checked against the text, so the text must be there.
    for c in priority_decks().unwrap() {
        assert!(
            !c.oracle_text.trim().is_empty(),
            "{} has no printed text",
            c.printed.name
        );
    }
}

#[test]
fn the_gap_list_is_measurable_and_honest() {
    let cards = priority_decks().unwrap();
    let complete: Vec<&str> = cards.iter().filter(|c| c.is_complete()).map(|c| c.printed.name).collect();
    let partial: usize = cards.len() - complete.len();
    let sentences: usize = cards.iter().map(|c| c.unimplemented.len()).sum();
    println!(
        "priority decks: {} cards, {} complete, {} partial, {} printed sentences still unsayable",
        cards.len(),
        complete.len(),
        partial,
        sentences
    );
    println!("complete: {complete:?}");
    // The point of the marker is that it is countable, not that it is zero yet.
    assert!(!complete.is_empty(), "at least the vanilla economy cards are complete");
}

#[test]
fn a_complete_card_denotes_into_real_vm_data() {
    let cards = load("t.cards", ANDROMEDA).unwrap();
    let gamble = cards.iter().find(|c| c.printed.name == "Sure Gamble").expect("Sure Gamble");
    assert!(gamble.is_complete());
    assert_eq!(gamble.printed.cost, Some(5));
    assert_eq!(gamble.printed.abilities.len(), 1, "one printed sentence, one ability");

    let casts = cards.iter().find(|c| c.printed.name == "Daily Casts").expect("Daily Casts");
    assert!(casts.is_complete(), "all three of its sentences are sayable");
    assert_eq!(casts.printed.abilities.len(), 3, "install, empty, turn-begins");
}

#[test]
fn corp_cards_denote_on_the_corp_side() {
    let cards = load("t.cards", GAUNTLET).unwrap();
    let news = cards.iter().find(|c| c.printed.name == "Breaking News").expect("Breaking News");
    assert_eq!(news.printed.advancement_requirement, Some(2));
    assert_eq!(news.printed.agenda_points, Some(1));
    assert!(!news.is_complete(), "its second sentence is not sayable yet");
    assert_eq!(news.unimplemented.len(), 1);
}
