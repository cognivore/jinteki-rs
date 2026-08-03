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

pub use decks::{priority_decks, SOURCES};
pub use edsl::{card, Card, CardBuilder};

/// One deck, by name — what a deck list will ask for at cutover.
pub fn deck_named(name: &str) -> Option<Vec<Card>> {
    match name {
        "andromeda" => Some(decks::andromeda::deck()),
        "gauntlet" => Some(decks::gauntlet::deck()),
        _ => None,
    }
}

/// Find one card of either priority deck by its printed name.
pub fn find(name: &str) -> Option<Card> {
    priority_decks().into_iter().find(|c| c.printed.name == name)
}
