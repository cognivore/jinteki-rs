//! Corp — Weyland Consortium identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).

use crate::edsl::*;

/// Argus Security: Protection Guaranteed — Identity: Corp.
/// "Whenever the Runner steals an agenda, they must take 1 tag or suffer 2
///  meat damage."
///
/// COMPLETE. The option choice is 9.11.4g's, but "**they** must" is 1.14.5's
/// attribution putting it to the RUNNER — the only thing that separates this
/// card from Sportsmetal's, whose sentence names nobody and so leaves the
/// choice with 9.1.1a's controller.
///
/// "Must" is 9.12.3: the Runner has no choice about whether, only about
/// which, so neither option may be declined. The meat damage stays the
/// CORP's under 10.4.2 even though the Runner chose it — the sentence is on
/// the Corp's identity, and responsibility is what decides who wins a
/// flatline.
pub fn argus_security() -> Card {
    card("Argus Security: Protection Guaranteed")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Whenever the Runner steals an agenda, they must take 1 tag or suffer 2 meat damage.")
        .when(
            runner_steals_agenda(),
            [performed_by(
                Runner,
                choose_one([
                    ("take 1 tag", vec![give_tags(1)]),
                    ("suffer 2 meat damage", vec![meat_damage(Corp, 2)]),
                ]),
            )],
        )
        .named("protection guaranteed")
        .build()
}

/// GRNDL: Power Unleashed — Identity: Division, Liability.
/// "You start the game with 10[credit] and 1 bad publicity."
///
/// COMPLETE. One sentence, two facts about the game's SETUP — there is no
/// condition to meet and nothing to resolve, and both are already true when
/// the first turn begins. They go where Andromeda's starting hand of nine
/// goes, among the printed facts, which is also why "and" here is not
/// 9.11.3's instruction question at all.
pub fn grndl() -> Card {
    card("GRNDL: Power Unleashed")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Division", "Liability"])
        .text("You start the game with 10[credit] and 1 bad publicity.")
        .starting_credits(10)
        .starting_bad_publicity(1)
        .build()
}

/// The Outfit: Family Owned and Operated — Identity: Subsidiary.
/// "Whenever you take 1 or more bad publicity, gain 3[credit]."
///
/// COMPLETE. 10.6.1's bad publicity, taken by the player the sentence names.
/// "1 or more" is not a threshold to check: bad publicity is taken in an
/// amount and the condition is met once per TAKING, so a card handing over
/// two at once pays this identity once — which is exactly what the phrase is
/// there to say.
pub fn the_outfit() -> Card {
    card("The Outfit: Family Owned and Operated")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Subsidiary"])
        .text("Whenever you take 1 or more bad publicity, gain 3[credit].")
        .when(takes_bad_publicity(Corp), [gain(Corp, 3)])
        .named("family owned and operated")
        .build()
}

/// Titan Transnational: Investing In Your Future — Identity: Corp.
/// "Whenever you score an agenda, you may place 1 agenda counter on it."
///
/// COMPLETE. "It" is 1.15.4's back-reference to the card the OCCURRENCE
/// named — the agenda that was just scored — and it is not a target: nothing
/// is announced, because the condition already fixed which card the sentence
/// is about, exactly as an access fixes "the card you are accessing".
///
/// The counter is an AGENDA counter (1.9), which is why it goes on a card in
/// the score area at all: 4.5.1 keeps a scored agenda there as an object, and
/// 9.1.8a keeps its abilities active, so a counter placed on it is something
/// a later ability can spend.
pub fn titan_transnational() -> Card {
    card("Titan Transnational: Investing In Your Future")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Whenever you score an agenda, you may place 1 agenda counter on it.")
        .may_when(corp_scores_agenda(), [place_on(the_triggering_card(), CounterKind::Agenda, 1)])
        .named("investing in your future")
        .build()
}

/// Weyland Consortium: Building a Better World — Identity: Megacorp.
/// "Whenever you play a transaction operation, gain 1[credit]."
///
/// COMPLETE. 8.6's play with two stipulations about the card played, 2.15's
/// type and 2.16's subtype, both riding on the one condition as content. The
/// ability is not once per turn, so every transaction pays.
pub fn weyland_building_a_better_world() -> Card {
    card("Weyland Consortium: Building a Better World")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Megacorp"])
        .text("Whenever you play a transaction operation, gain 1[credit].")
        .when(plays_a_subtyped(Corp, CardType::Operation, "Transaction"), [gain(Corp, 1)])
        .named("building a better world")
        .build()
}

/// Weyland Consortium: Built to Last — Identity: Megacorp.
/// "Whenever you advance a card, gain 2[credit] if it had no advancement
///  counters."
///
/// COMPLETE. CR 1.18.2 is what makes the condition narrow: only an ADVANCE
/// meets it, so a card that merely places an advancement counter (Mushin No
/// Shin class) pays nothing. "If it had no advancement counters" is 9.6.5c's
/// additional requirement inside the condition, asked of the card as it was
/// BEFORE the advance — which is why the second advance of the same card
/// pays nothing and the first pays.
pub fn weyland_built_to_last() -> Card {
    card("Weyland Consortium: Built to Last")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Megacorp"])
        .text("Whenever you advance a card, gain 2[credit] if it had no advancement counters.")
        .when(advances_a_card(true), [gain(Corp, 2)])
        .named("built to last")
        .build()
}

/// Every Weyland Consortium identity this module carries, in the order the
/// queue reached them.
pub fn identities() -> Vec<Card> {
    vec![
        argus_security(),
        grndl(),
        the_outfit(),
        titan_transnational(),
        weyland_building_a_better_world(),
        weyland_built_to_last(),
    ]
}
