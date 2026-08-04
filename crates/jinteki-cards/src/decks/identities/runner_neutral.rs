//! Runner — Neutral identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).
//!
//! All three of these print a DECK-CONSTRUCTION or FORMAT restriction and
//! nothing else. CR 1.4.2 checks deck legality before the game begins, so
//! none of it is an ability: there is no condition to meet, nothing to
//! resolve, and no game state it reads. `docs/cards/EDSL.md`'s third rule of
//! thumb says exactly this — "restrictions are not sentences you write;
//! 'Limit 1 per deck' belongs in the facts or nowhere, never as an action" —
//! so these identities denote into no abilities on purpose, and that is the
//! whole of them rather than a gap.

use crate::edsl::*;

/// Nova Initiumia: Catalyst & Impetus — Identity: Digital, Natural. Link 0.
/// "Your deck cannot include more than 1 copy of any card."
///
/// COMPLETE. A deck-construction restriction (CR 1.4), checked before the
/// game begins and never again — the same class of sentence as "Limit 1 per
/// deck", which the writing guide places in the facts or nowhere. It is not
/// an ability: 9.1.1 gives an ability a condition and instructions, and this
/// sentence has neither.
pub fn nova_initiumia() -> Card {
    card("Nova Initiumia: Catalyst & Impetus")
        .runner()
        .identity()
        .faction("Neutral")
        .subtypes(&["Digital", "Natural"])
        .text("Your deck cannot include more than 1 copy of any card.")
        .build()
}

/// The Catalyst: Convention Breaker — Identity: Natural. Link 0.
/// "Starter game only."
///
/// COMPLETE. A FORMAT restriction — which games this card may be brought to,
/// decided before deck construction. Nothing about it is read during play.
pub fn the_catalyst() -> Card {
    card("The Catalyst: Convention Breaker")
        .runner()
        .identity()
        .faction("Neutral")
        .subtypes(&["Natural"])
        .text("Starter game only.")
        .build()
}

/// The Masque: Cyber General — Identity: Natural. Link 0.
/// "Draft format only."
///
/// COMPLETE. The same format restriction The Catalyst prints, for the other
/// format. Nothing about it is read during play.
pub fn the_masque() -> Card {
    card("The Masque: Cyber General")
        .runner()
        .identity()
        .faction("Neutral")
        .subtypes(&["Natural"])
        .text("Draft format only.")
        .build()
}

/// Every Neutral Runner identity this module carries, in queue order.
pub fn identities() -> Vec<Card> {
    vec![nova_initiumia(), the_catalyst(), the_masque()]
}
