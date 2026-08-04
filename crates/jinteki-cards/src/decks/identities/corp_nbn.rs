//! Corp — NBN identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). Azmari EdTech and Nebula Talent Management are the
//! two NBN identities that already exist — they live in `decks/gauntlet.rs`,
//! because that deck plays one and carries the other's back face — and
//! everything else in the faction lands here.

use crate::edsl::*;

/// NBN: Reality Plus — Identity: Megacorp.
/// "The first time each turn the Runner takes a tag, gain 2[credit] or draw 2
///  cards."
///
/// COMPLETE. 10.2.2's tag with 9.6.5c's ordinal about the occurrence. The
/// condition is met per TAKING, not per tag, so a card that hands over two
/// tags at once pays this identity once — and so does the second tag of the
/// turn, which pays nothing at all.
///
/// "Gain 2[credit] **or** draw 2 cards" is 9.11.4g's option choice: one
/// instruction offering two, chosen by the ability's controller (9.1.1a),
/// which for a Corp identity is the Corp even though the Runner is the player
/// the condition happened to.
pub fn nbn_reality_plus() -> Card {
    card("NBN: Reality Plus")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Megacorp"])
        .text("The first time each turn the Runner takes a tag, gain 2[credit] or draw 2 cards.")
        .when_first_each_turn(
            runner_takes_a_tag(),
            [choose_one([
                ("gain 2[credit]", vec![gain(Corp, 2)]),
                ("draw 2 cards", vec![draw(Corp, 2)]),
            ])],
        )
        .named("the first tag of the turn")
        .build()
}

/// NBN: The World is Yours* — Identity: Megacorp.
/// "Your maximum hand size is increased by 1."
///
/// COMPLETE. A permanent fact rather than something that happens, so it is a
/// static declaration: 5.7.3's maximum hand size, read continuously, which is
/// what makes it correct against a core damage that lowers the same number
/// from the other direction.
///
/// "Your" is 9.1.1a's controller — the Corp, whose identity this is — and the
/// declaration says so with a scope rather than by naming a side, because
/// Cybernetics Division prints the same sentence about EACH player.
pub fn nbn_the_world_is_yours() -> Card {
    card("NBN: The World is Yours*")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Megacorp"])
        .text("Your maximum hand size is increased by 1.")
        .declares([max_hand_size_mod(1)])
        .named("the world is yours")
        .build()
}

/// Pravdivost Consulting: Political Solutions — Identity: Division.
/// "The first time each turn the Runner makes a successful run, you may place
///  1 advancement counter on an installed card you can advance."
///
/// COMPLETE. 6.8.4's successful run with no stipulation about the server —
/// the sentence makes none — and 9.6.5c's ordinal about the occurrence.
///
/// "An installed card you can advance" is 1.18.3: an agenda, a card with the
/// advanceable property, or one an effect has made advanceable. The
/// description says it with the ordinary filter word, which already names the
/// play area, so 1.15.2c needs nothing added. 1.18.2: the counter is PLACED,
/// so this never meets a "whenever you advance a card" condition.
pub fn pravdivost_consulting() -> Card {
    card("Pravdivost Consulting: Political Solutions")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("The first time each turn the Runner makes a successful run, you may place 1 advancement counter on an installed card you can advance.")
        .may_when_first_each_turn(
            makes_successful_run(),
            [place_on(choose(1, &[advanceable()]), CounterKind::Advancement, 1)],
        )
        .named("the first successful run of the turn")
        .build()
}

/// Every NBN identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![nbn_reality_plus(), nbn_the_world_is_yours(), pravdivost_consulting()]
}
