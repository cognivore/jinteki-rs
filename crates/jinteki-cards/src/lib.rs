//! The card layer — printed card text as data (DESIGN.md §5.5, SYS-D-1…D-12).
//!
//! Cards are written in an EMBEDDED DSL: typed builders over the CR VM's own
//! vocabulary (`crate::edsl`). A card designer copies the printed text into
//! `.text(…)` and then makes one call per printed sentence. Nothing here is
//! programming — but it *is* Rust, which means the compiler is the
//! proof-reader: a sentence the vocabulary cannot say does not compile, so it
//! cannot be quietly approximated.
//!
//! `docs/cards/EDSL.md` is the designer's guide and the only thing a designer
//! should have to read. (`docs/cards/DSL.md` is its tombstone: an external
//! text format was tried first and judged the wrong basket for these eggs —
//! covering Netrunner's real weirdness in a bespoke parser is a language
//! project, not a card project.)
//!
//! Sentences the vocabulary cannot yet express are marked, never faked:
//! `.unimplemented("<printed sentence>")` keeps the card honest, keeps it
//! visible everywhere but the table (SYS-D-12), and keeps the gap list
//! measurable — `tests/decks.rs` prints it and ratchets it.

pub mod decks;
pub mod edsl;

pub use decks::{all_cards, deck_of_the_week, mezzie_decks, priority_decks, SOURCES};
pub use edsl::{card, Card, CardBuilder};

/// One deck, by name — what a deck list will ask for at cutover.
pub fn deck_named(name: &str) -> Option<Vec<Card>> {
    match name {
        "andromeda" => Some(decks::andromeda::deck()),
        "gauntlet" => Some(decks::gauntlet::deck()),
        // Mid-queue (docs/vm/MEZZIE-QUEUE.md). Named here so a deck list can
        // ask for them; SYS-D-12 is what keeps an unfinished one off a table,
        // and `cr::readiness()` is where that gate lives.
        "mezzie_asa" => Some(decks::mezzie_asa::deck()),
        "mezzie_valencia" => Some(decks::mezzie_valencia::deck()),
        // docs/vm/DECK-OF-THE-WEEK.md.
        "notw_restoring_humanity" => Some(decks::notw_restoring_humanity::deck()),
        _ => None,
    }
}

/// CR 1.5.4a: the additional identities that deck brings along with it, kept
/// in a pile outside the game. Separate from [`deck_named`] because the pile
/// is not part of the deck — 1.5.4a puts it beside it.
pub fn pile_named(name: &str) -> Option<Vec<Card>> {
    decks::identity_pile(name)
}

/// Find one card by its printed name.
pub fn find(name: &str) -> Option<Card> {
    all_cards().into_iter().find(|c| c.printed.name == name)
}
