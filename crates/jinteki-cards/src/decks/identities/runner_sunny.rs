//! Runner — Sunny Lebeau's own faction, which holds exactly one identity.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).

use crate::edsl::*;

/// Sunny Lebeau: Security Specialist — Identity: Natural. Link 2.
///
/// COMPLETE. The whole card is its base link: NSG's card data records no text
/// at all for it, and the printed card's text box is empty. `.no_printed_text()`
/// says that deliberately, so a blank box is told apart from a card whose
/// text was forgotten.
///
/// CR 5.5.2's link is a characteristic, not an ability the player uses, and
/// `.link(2)` is where the other identities put theirs — so this identity
/// denotes into the same one static declaration Chaos Theory's `+1[mu]` does,
/// and into nothing else.
pub fn sunny_lebeau() -> Card {
    card("Sunny Lebeau: Security Specialist")
        .runner()
        .identity()
        .faction("Sunny Lebeau")
        .subtypes(&["Natural"])
        .link(2)
        .no_printed_text()
        .build()
}

/// Every identity this module carries.
pub fn identities() -> Vec<Card> {
    vec![sunny_lebeau()]
}
