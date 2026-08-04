//! Runner — Anarch identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). No Anarch identity existed before the queue, so
//! every one of them lands here.

use crate::edsl::*;

/// Alice Merchant: Clan Agitator — Identity: Cyborg. Link 0.
/// "The first time you make a successful run on Archives each turn, the Corp
///  must trash 1 card from HQ."
///
/// COMPLETE. The condition is 6.8.4's "makes a successful run" with the
/// server the sentence names as its stipulation, and 9.6.5c's ordinal about
/// the occurrence — the same pair Gabriel Santiago states about HQ.
///
/// "The Corp must trash 1 card from HQ" is 1.14.5's attribution: the sentence
/// names a player other than the ability's controller, so the CORP makes the
/// choice the instruction offers, even though 9.1.1a makes the Runner the
/// controller of a Runner identity's ability. "Must" is 9.12.3: the Corp has
/// no choice about whether, only about which.
pub fn alice_merchant() -> Card {
    card("Alice Merchant: Clan Agitator")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Cyborg"])
        .text("The first time you make a successful run on Archives each turn, the Corp must trash 1 card from HQ.")
        .when_first_each_turn(
            makes_successful_run_on(&[ServerId::Archives]),
            [performed_by(Corp, trash(choose(1, &[in_hand_of(Corp)])))],
        )
        .named("the first archives run of the turn")
        .build()
}

/// Edward Kim: Humanity's Hammer — Identity: Natural. Link 1.
/// "Trash the first operation you access each turn at no cost."
///
/// COMPLETE. One sentence carrying its own condition: the occurrence is
/// 7.3.6's access with the sentence's card-type stipulation, and "the first …
/// each turn" is 9.6.5c's ordinal on it, so the second operation accessed in
/// a turn does not meet the condition at all.
///
/// "At no cost" is not an instruction — it is the absence of one. 7.5.4 lets
/// the Runner trash an accessed card by PAYING its trash cost with the basic
/// trash ability; this sentence trashes the card outright, so there is
/// nothing to pay and no card to name: 1.15.2's target is the card being
/// accessed, which the access itself already fixed.
pub fn edward_kim() -> Card {
    card("Edward Kim: Humanity's Hammer")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .link(1)
        .text("Trash the first operation you access each turn at no cost.")
        .when_first_each_turn(accesses_a(CardType::Operation), [trash(accessed_card())])
        .named("the first operation accessed this turn")
        .build()
}

/// Esâ Afontov: Eco-Insurrectionist — Identity: Cyborg. Link 0.
/// "The first time each turn you suffer core damage, you may draw 1 card and
///  sabotage 2. (The Corp trashes 2 cards of their choice from HQ and/or the
///  top of R&D.)"
///
/// COMPLETE. The parenthesis is 1.4's reminder text: it restates what 10.16's
/// sabotage keyword already is, so it is not a second instruction.
///
/// "You suffer core damage" is 10.4.1's damage with the sentence's stipulation
/// about the KIND riding on the condition as content — the same shape the
/// interrupt side already states with "…would suffer net damage" — and
/// 9.6.5c's ordinal counts the occurrences from the turn's start.
///
/// "Draw 1 card and sabotage 2" is one printed sentence, so one instruction
/// (9.11.3): splitting it would invent a checkpoint between the draw and the
/// sabotage that the card does not print.
pub fn esa_afontov() -> Card {
    card("Esâ Afontov: Eco-Insurrectionist")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Cyborg"])
        .text("The first time each turn you suffer core damage, you may draw 1 card and sabotage 2. (The Corp trashes 2 cards of their choice from HQ and/or the top of R&D.)")
        .may_when_first_each_turn(
            suffers_damage(DamageKind::Core),
            [combined([draw(Runner, 1), sabotage(2)])],
        )
        .named("the first core damage of the turn")
        .build()
}

/// MaxX: Maximum Punk Rock — Identity: G-mod. Link 0.
/// "When your turn begins, trash the top 2 cards of your stack. Draw 1 card."
///
/// COMPLETE. TWO printed sentences on one condition, so two instructions —
/// 9.11.3's ordinary reading, and the one case where the boundary is real:
/// the trash finishes, a checkpoint occurs, and only then is the draw
/// imminent, which is what lets a card trashed by the first sentence act
/// before the second.
///
/// "The top 2 cards of your stack" names a zone, so 1.15.2c's play-area
/// restriction lifts and the two cards are the targets the description
/// itself fixes — no announcement is made.
pub fn maxx() -> Card {
    card("MaxX: Maximum Punk Rock")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("When your turn begins, trash the top 2 cards of your stack. Draw 1 card.")
        .when(turn_begins(Runner), [trash(top_of_stack(amount(2))), draw(Runner, 1)])
        .named("maximum punk rock")
        .build()
}

/// Nathaniel "Gnat" Hall: One-of-a-Kind — Identity: Natural. Link 0.
/// "When your turn begins, gain 1[credit] if you have 2 or fewer cards in
///  your grip."
///
/// COMPLETE. "If you have 2 or fewer cards in your grip" is 9.6.5c's
/// additional requirement listed inside the trigger condition, so it is
/// checked when the condition would be met — at the start of the turn — and
/// not again when the ability resolves.
///
/// The grip is described with the ordinary filter words, and naming a zone is
/// what lifts 1.15.2c's play-area default: without it the count would be of
/// installed cards, which is not what the sentence says.
pub fn nathaniel_gnat_hall() -> Card {
    card("Nathaniel \"Gnat\" Hall: One-of-a-Kind")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("When your turn begins, gain 1[credit] if you have 2 or fewer cards in your grip.")
        .when(
            turn_begins_if(Runner, &[board_has_at_most(&[in_hand_of(Runner)], 2)]),
            [gain(Runner, 1)],
        )
        .named("one-of-a-kind")
        .build()
}

/// Every Anarch identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![alice_merchant(), edward_kim(), esa_afontov(), maxx(), nathaniel_gnat_hall()]
}
