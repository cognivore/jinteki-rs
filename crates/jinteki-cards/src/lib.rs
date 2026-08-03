//! The card DSL — printed card text as data (DESIGN.md §5.5, SYS-D-1…D-12).
//!
//! A card file is written by a card designer, not a programmer: facts, the
//! printed text copied verbatim, and one line per printed sentence saying
//! what it does (`docs/cards/DSL.md` is the guide, and it is the only thing a
//! designer should have to read). The file is parsed to an AST and denoted
//! into the CR VM's own instruction vocabulary — the DSL has no state access
//! of its own (SYS-D-6) and adds no card-shaped kernel variants
//! (ARCHITECTURE §12).
//!
//! Sentences the vocabulary cannot yet express are marked, never faked:
//! `unimplemented: "<printed sentence>"` keeps the card honest, keeps it
//! visible everywhere but the table (SYS-D-12), and keeps the gap list
//! measurable.

pub mod denote;
pub mod parse;

pub use denote::{denote, DenotedCard};
pub use parse::{CardAst, CardError};

/// Parse and denote a whole card file.
pub fn load(file: &str, src: &str) -> Result<Vec<DenotedCard>, CardError> {
    parse::parse_file(file, src)?.iter().map(|a| denote::denote(file, a)).collect()
}

/// The two priority decks, compiled in so tests and the server share one copy.
pub const ANDROMEDA: &str = include_str!("../cards/andromeda.cards");
pub const GAUNTLET: &str = include_str!("../cards/gauntlet.cards");

/// Every card of both decks, or the first error a designer would need to fix.
pub fn priority_decks() -> Result<Vec<DenotedCard>, CardError> {
    let mut out = load("cards/andromeda.cards", ANDROMEDA)?;
    out.extend(load("cards/gauntlet.cards", GAUNTLET)?);
    Ok(out)
}
