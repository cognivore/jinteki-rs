//! Cards no priority deck lists.
//!
//! The two priority decks are what the odometer counts, but a wave sometimes
//! has to write a card outside them: a mechanism the CR states as a RULE — not
//! as a card — still has to be proved against a real printed card, and
//! inventing one would be exactly the overfit ARCHITECTURE §12 forbids. CR
//! 1.5.4 names its two cards outright ("Rebirth and DJ Fenris"), and only one
//! of them is in a priority deck.
//!
//! Same discipline as every other module here: printed text in `.text(…)`,
//! one call per printed sentence, `.unimplemented(…)` rather than an
//! approximation. These cards are excluded from `priority_decks()` so the
//! deck odometer keeps measuring the decks, and `jinteki_cards::find` reaches
//! them like any other card.

use crate::edsl::*;

/// Chaos Theory: Wünderkind — Identity: G-mod. Link 0.
/// "+1[mu]"
///
/// COMPLETE. Here for CR 1.5.4: a pile (1.5.4a) holding only identities of
/// the Runner's own faction proves nothing about "from the same faction", and
/// DJ Fenris needs a **g-mod** identity of ANOTHER faction to have anything
/// to reach at all. A one-line static is also the identity whose text is
/// easiest to watch arrive on another card.
pub fn chaos_theory() -> Card {
    card("Chaos Theory: Wünderkind")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["G-mod"])
        .text("+1[mu]")
        .declares([plus_memory(1)])
        .build()
}

/// Every card here, in the order the file lists it.
pub fn cards() -> Vec<Card> {
    vec![chaos_theory()]
}
