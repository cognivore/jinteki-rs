//! Runner — Apex.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).
//!
//! CR 2.13.2 gives Apex a faction of its own with a single identity in it.

use jinteki_cr::Subtype;

use crate::edsl::*;

/// Apex: Invasive Predator — Identity: Digital. Link 0.
/// "You cannot install non-virtual resources.
///  When your turn begins, you may install 1 card from your grip facedown."
///
/// COMPLETE.
///
/// The first sentence is a permanent fact rather than something that happens,
/// so it is a declaration: 1.2.2 says a "cannot" takes precedence over
/// anything that directs the thing to happen, which makes it reach every
/// install at once — 5.2.7d's basic action, an ability that installs from the
/// grip, an ability that installs out of the heap — without naming any of
/// them. The description is the sentence's own words: a resource that is not
/// **virtual**, written as the type word and the negated subtype word beside
/// each other, because several description words together mean all of them.
///
/// The second sentence installs a card FACEDOWN (4.6.4d), which is a
/// stipulation about the status step 8.5.16a places the card in, and the whole
/// of what makes the rest of the sentence work. It is why the description
/// names no card type: 8.1.4a leaves a facedown installed Runner card with no
/// characteristics at all, so there is no type for the description to
/// stipulate and an event goes into the rig as readily as a program. It is
/// also why nothing is paid — 8.5.11a puts "facedown Runner cards" beside
/// agendas and upgrades among the cards that have no install cost.
///
/// THE READING WHERE THE TWO SENTENCES MEET, stated because it decides a
/// case: the first sentence does NOT forbid the second one a non-virtual
/// resource. What a facedown install produces is 8.1.4a's blank object — no
/// name, no card type, no subtypes — so the thing this identity's second
/// sentence installs is never a "non-virtual resource" for the first
/// sentence to describe: the prohibition has nothing to read (1.15.3), and
/// it reads the card's printed face only for FACEUP installs, where the
/// installed object keeps those characteristics. The kernel asks the
/// question with the declared face in hand (`Vm::install_prohibited`'s
/// facedown arm), which is why every card in the grip is a candidate here
/// while 5.2.7d's faceup basic action still refuses the same resource.
pub fn apex() -> Card {
    card("Apex: Invasive Predator")
        .runner()
        .identity()
        .faction("Apex")
        .subtypes(&[Subtype::Digital])
        .text("You cannot install non-virtual resources.")
        .text("When your turn begins, you may install 1 card from your grip facedown.")
        .declares([cannot_install(&[of_type(CardType::Resource), non(with_subtype(Subtype::Virtual))])])
        .may_when(
            turn_begins(Runner),
            [install_facedown(choose(1, &[in_hand_of(Runner)]), InstallDest::Rig)],
        )
        .named("invasive predator")
        .build()
}

/// Every Apex identity this module carries, in queue order.
pub fn identities() -> Vec<Card> {
    vec![apex()]
}
