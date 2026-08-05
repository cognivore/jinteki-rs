//! The two priority decks, as cards.
//!
//! One module per deck, one function per card, in printed order. The deck
//! functions are the only thing anything outside this crate needs.

use crate::edsl::Card;

pub mod andromeda;
pub mod gauntlet;
pub mod identities;
pub mod unlisted;

/// Every card of both decks — the deck proper, plus CR 1.5.4a's pile of
/// additional identities, which a player brings "along with their deck" and
/// which therefore has to be as playable as the deck is.
pub fn priority_decks() -> Vec<Card> {
    let mut out = andromeda::deck();
    out.extend(andromeda::additional_identities());
    out.extend(gauntlet::deck());
    out.extend(gauntlet::additional_identities());
    out
}

/// CR 1.5.4a: the additional identities a deck brings along with it.
pub fn identity_pile(deck: &str) -> Option<Vec<Card>> {
    match deck {
        "andromeda" => Some(andromeda::additional_identities()),
        "gauntlet" => Some(gauntlet::additional_identities()),
        _ => None,
    }
}

/// Card definitions no deck list carries: implemented and complete, but
/// absent from [`deck_named`] because the printed list (the deck photo) is
/// the authority on what a DECK contains. Hedge Fund is the class — defined,
/// exercised by tests, left off the Gauntlet list on the photo's authority.
/// Everything that asks "what does the engine support?" (`all_cards`,
/// `find`, the eternal catalog's completeness join) must still see them;
/// a definition reachable only through a deck list is invisible the moment
/// the list declines it, which is how Hedge Fund fell out of the catalog.
pub fn off_list_cards() -> Vec<Card> {
    vec![gauntlet::hedge_fund()]
}

/// Every card this crate carries, priority decks and all — what
/// [`crate::find`] searches.
pub fn all_cards() -> Vec<Card> {
    let mut out = priority_decks();
    out.extend(unlisted::cards());
    // Definitions the deck lists decline (Hedge Fund) still exist.
    for c in off_list_cards() {
        if !out.iter().any(|x| x.name() == c.name()) {
            out.push(c);
        }
    }
    // The identity queue. An identity already enlisted in a deck's 1.5.4a
    // pile arrived with the deck above, so it is not carried twice.
    for c in identities::cards() {
        if !out.iter().any(|x| x.name() == c.name()) {
            out.push(c);
        }
    }
    out
}

/// The source of each card module, for the manifest test — the doc comments
/// are the human-readable half of SYS-D-10 and the test checks they agree
/// with the `.text(…)` data.
pub const SOURCES: &[(&str, &str)] = &[
    ("andromeda.rs", include_str!("andromeda.rs")),
    ("gauntlet.rs", include_str!("gauntlet.rs")),
    ("unlisted.rs", include_str!("unlisted.rs")),
    (
        "identities/runner_criminal.rs",
        include_str!("identities/runner_criminal.rs"),
    ),
    (
        "identities/runner_shaper.rs",
        include_str!("identities/runner_shaper.rs"),
    ),
    (
        "identities/runner_anarch.rs",
        include_str!("identities/runner_anarch.rs"),
    ),
    (
        "identities/runner_neutral.rs",
        include_str!("identities/runner_neutral.rs"),
    ),
    (
        "identities/runner_apex.rs",
        include_str!("identities/runner_apex.rs"),
    ),
    (
        "identities/runner_sunny.rs",
        include_str!("identities/runner_sunny.rs"),
    ),
    (
        "identities/runner_adam.rs",
        include_str!("identities/runner_adam.rs"),
    ),
    (
        "identities/corp_haas_bioroid.rs",
        include_str!("identities/corp_haas_bioroid.rs"),
    ),
    (
        "identities/corp_jinteki.rs",
        include_str!("identities/corp_jinteki.rs"),
    ),
    ("identities/corp_nbn.rs", include_str!("identities/corp_nbn.rs")),
    (
        "identities/corp_weyland.rs",
        include_str!("identities/corp_weyland.rs"),
    ),
    (
        "identities/corp_neutral.rs",
        include_str!("identities/corp_neutral.rs"),
    ),
];
