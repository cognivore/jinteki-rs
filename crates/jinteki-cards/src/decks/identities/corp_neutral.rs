//! Corp — Neutral identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).
//!
//! Three of the four print only DECK-CONSTRUCTION or FORMAT restrictions, and
//! `runner_neutral.rs` says why that is complete rather than a gap: CR 1.4.2
//! settles deck legality before the game begins, so there is no condition to
//! meet and nothing to resolve, and the writing guide's third rule of thumb
//! puts such a sentence in the facts or nowhere.

use crate::edsl::*;

/// Ampère: Cybernetics For Anyone — Identity: Corp.
/// "Your deck cannot include more than 1 copy of any card.
///  Your deck may include up to 2 different agenda cards from each Corp
///  faction."
///
/// COMPLETE. Two deck-construction restrictions (CR 1.4) and nothing else —
/// the second is a WIDENING of 1.4.5's out-of-faction rule rather than a
/// narrowing, but both are settled before the game begins and neither is read
/// again once it has.
pub fn ampere() -> Card {
    card("Ampère: Cybernetics For Anyone")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Corp"])
        .text("Your deck cannot include more than 1 copy of any card.")
        .text("Your deck may include up to 2 different agenda cards from each Corp faction.")
        .build()
}

/// The Shadow: Pulling the Strings — Identity: Megacorp.
/// "Draft format only.
///  You can use agendas from all factions in this deck."
///
/// COMPLETE. A format restriction and the deck-construction permission that
/// goes with it. 1.4.2 checks both before the game begins.
pub fn the_shadow() -> Card {
    card("The Shadow: Pulling the Strings")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Megacorp"])
        .text("Draft format only.")
        .text("You can use agendas from all factions in this deck.")
        .build()
}

/// The Syndicate: Profit over Principle — Identity: Megacorp.
/// "Starter game only."
///
/// COMPLETE. The format restriction The Catalyst prints on the Runner side,
/// and nothing else is on the card.
pub fn the_syndicate() -> Card {
    card("The Syndicate: Profit over Principle")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Megacorp"])
        .text("Starter game only.")
        .build()
}

/// Every Neutral Corp identity this module carries, in queue order.
pub fn identities() -> Vec<Card> {
    vec![ampere(), the_shadow(), the_syndicate()]
}
