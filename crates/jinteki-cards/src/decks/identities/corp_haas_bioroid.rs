//! Corp — Haas-Bioroid identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).

use crate::edsl::*;

/// Haas-Bioroid: Engineering the Future — Identity: Megacorp.
/// "The first time you install a card each turn, gain 1[credit]."
///
/// COMPLETE. 8.5's install with no stipulation about what was installed — the
/// sentence makes none — and 9.6.5c's ordinal about the occurrence, counted
/// from the change log since the turn began. The Corp installs ice, assets,
/// upgrades and agendas, and this condition reaches all four because the
/// sentence names no type.
///
/// It is not 9.3.6g's once-per-turn flag: 9.1.6 says a player *uses* a paid
/// ability, and an entirely mandatory conditional ability is never used, so
/// nothing would ever spend the flag.
pub fn haas_bioroid_engineering_the_future() -> Card {
    card("Haas-Bioroid: Engineering the Future")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Megacorp"])
        .text("The first time you install a card each turn, gain 1[credit].")
        .when_first_each_turn(installs_a_card(Corp), [gain(Corp, 1)])
        .named("the first install of the turn")
        .build()
}

/// Sportsmetal: Go Big or Go Home — Identity: Subsidiary.
/// "Whenever an agenda is scored or stolen, gain 2[credit] or draw 2 cards."
///
/// COMPLETE. One printed sentence with two conditions, so two conditional
/// abilities with the same effect — the shape Leela Patel and Tāo Salonga
/// already take, and for the same reason: 9.6.1 gives an ability ONE primary
/// condition, and an agenda being scored (1.17.3a) and one being stolen
/// (1.17.3b) are different occurrences.
///
/// "Gain 2[credit] **or** draw 2 cards" is 9.11.4g's option choice, so it is
/// one instruction offering two, not two instructions. 9.1.1a puts the choice
/// with the ability's controller — the CORP, whose identity this is — even on
/// the half the Runner's theft meets.
pub fn sportsmetal() -> Card {
    card("Sportsmetal: Go Big or Go Home")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Subsidiary"])
        .text("Whenever an agenda is scored or stolen, gain 2[credit] or draw 2 cards.")
        .when(corp_scores_agenda(), [go_big_or_go_home()])
        .named("an agenda was scored")
        .when(runner_steals_agenda(), [go_big_or_go_home()])
        .named("an agenda was stolen")
        .build()
}

/// Sportsmetal's option choice, written once because the sentence states it
/// once and only its two conditions differ.
fn go_big_or_go_home() -> Instruction {
    choose_one([("gain 2[credit]", vec![gain(Corp, 2)]), ("draw 2 cards", vec![draw(Corp, 2)])])
}

/// Thule Subsea: Safety Below — Identity: Division.
/// "Whenever the Runner steals an agenda, do 1 core damage unless they spend
///  [click] and 2[credit]."
///
/// COMPLETE. "Unless they spend …" is 1.16.9's alternative cost put to the
/// player the sentence names: the Runner may pay to stop the damage, and the
/// payment is ONE cost with two components (1.16.2), so a Runner who cannot
/// pay both pays neither and takes the damage.
///
/// The damage is the Corp's — 10.4.2 makes the player who caused it
/// responsible, which is what decides who wins a flatline — even though the
/// occurrence that meets the condition is the Runner's theft.
pub fn thule_subsea() -> Card {
    card("Thule Subsea: Safety Below")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Whenever the Runner steals an agenda, do 1 core damage unless they spend [click] and 2[credit].")
        .when(
            runner_steals_agenda(),
            [unless_pays(Runner, clicks(1).plus_cost(credits(2)), core_damage(Corp, 1))],
        )
        .named("safety below")
        .build()
}

/// Every Haas-Bioroid identity this module carries, in the order the queue
/// reached them.
pub fn identities() -> Vec<Card> {
    vec![haas_bioroid_engineering_the_future(), sportsmetal(), thule_subsea()]
}
