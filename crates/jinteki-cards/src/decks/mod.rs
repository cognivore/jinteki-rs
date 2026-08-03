//! The two priority decks, as cards.
//!
//! One module per deck, one function per card, in printed order. The deck
//! functions are the only thing anything outside this crate needs.

use crate::edsl::Card;

pub mod andromeda;
pub mod gauntlet;

/// Every card of both decks.
pub fn priority_decks() -> Vec<Card> {
    let mut out = andromeda::deck();
    out.extend(gauntlet::deck());
    out
}

/// The source of each deck module, for the manifest test — the doc comments
/// are the human-readable half of SYS-D-10 and the test checks they agree
/// with the `.text(…)` data.
pub const SOURCES: &[(&str, &str)] = &[
    ("andromeda.rs", include_str!("andromeda.rs")),
    ("gauntlet.rs", include_str!("gauntlet.rs")),
];
