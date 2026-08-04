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

/// DJ Fenris — Resource: Connection. Install 3. ◆
/// "Host a g-mod identity that does not match the faction of your identity on
///  DJ Fenris when he is installed. Remove hosted identity from the game if
///  DJ Fenris is uninstalled.
///  DJ Fenris gains the text of hosted identity.
///  Limit 1 per deck."
///
/// The first half of the first sentence is expressed: "a g-mod identity that
/// does not match the faction of your identity" is CR 1.5.4a's pile (1.5.4b
/// makes that what naming an identity means), a subtype and a faction — three
/// ordinary criteria, none of which this card had to invent — and 1.13.2a's
/// host-without-installing is what "host … on DJ Fenris" does.
///
/// UNIMPLEMENTED: the other two sentences.
///
/// "Remove hosted identity from the game if DJ Fenris is uninstalled" is a
/// destination override, and nothing can state it yet. 1.13.13 trashes a
/// host's hosted objects "during the next checkpoint", by which time DJ
/// Fenris is inactive, so neither a static declaration nor a conditional
/// ability of his can still reach the identity. Left unstated, the identity
/// goes where 1.5.4b sends any identity leaving the play area — back to the
/// pile — which is a rule and not an approximation of this sentence.
///
/// "DJ Fenris gains the text of hosted identity" is 9.1.9's other direction.
/// `Effective::ability_present` is a presence MASK over `printed.abilities`,
/// so an ability can be taken away but not added, and `AbilityRef` indexes
/// that same list — gaining abilities needs both to grow. Stating it wrongly
/// would be a card that quietly does nothing while claiming to.
///
/// ("Limit 1 per deck" is a deckbuilding restriction, not a sentence a card
/// does.)
pub fn dj_fenris() -> Card {
    card("DJ Fenris")
        .runner()
        .resource()
        .faction("Neutral")
        .subtypes(&["Connection"])
        .cost(3)
        .unique()
        .text("Host a g-mod identity that does not match the faction of your identity on DJ Fenris when he is installed. Remove hosted identity from the game if DJ Fenris is uninstalled.")
        .text("DJ Fenris gains the text of hosted identity.")
        .text("Limit 1 per deck.")
        .when(
            installed(),
            [host(
                choose(
                    1,
                    &[
                        in_identity_pile_of(Runner),
                        with_subtype("G-mod"),
                        faction_matching_identity_of(Runner, false),
                    ],
                ),
                this_card(),
            )],
        )
        .named("guest of the evening")
        .unimplemented("Remove hosted identity from the game if DJ Fenris is uninstalled.")
        .unimplemented("DJ Fenris gains the text of hosted identity.")
        .build()
}

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
    vec![dj_fenris(), chaos_theory()]
}
