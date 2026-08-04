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

/// Custom Biotics: Engineered for Success — Identity: Division.
/// "You cannot include Jinteki cards in this deck."
///
/// COMPLETE. A deck-construction restriction (CR 1.4), settled before the
/// game begins and never read again — the same class of sentence as Ampère's
/// singleton rule, and the writing guide's third rule of thumb puts it in the
/// facts or nowhere rather than in an ability.
pub fn custom_biotics() -> Card {
    card("Custom Biotics: Engineered for Success")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("You cannot include Jinteki cards in this deck.")
        .build()
}

/// Cybernetics Division: Humanity Upgraded — Identity: Division.
/// "Each player's maximum hand size is reduced by 1."
///
/// COMPLETE. The same 5.7.3 declaration NBN: The World is Yours* makes, with
/// the other polarity and the other scope — which is exactly why both are
/// content on one declaration rather than two. "Each player's" reaches the
/// Corp who plays it as well as the Runner, so this identity's own discard
/// phase is shortened too.
pub fn cybernetics_division() -> Card {
    card("Cybernetics Division: Humanity Upgraded")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("Each player's maximum hand size is reduced by 1.")
        .declares([each_players_max_hand_size_mod(-1)])
        .named("humanity upgraded")
        .build()
}

/// Haas-Bioroid: Precision Design — Identity: Megacorp.
/// "You get +1 maximum hand size.
///  Whenever you score an agenda, you may add 1 card from Archives to HQ."
///
/// COMPLETE. Two printed lines, and they are different kinds of sentence: the
/// first is permanently true and so a static declaration, the second happens
/// and so is a conditional ability.
///
/// "1 card from Archives" names a zone, which is what lifts 1.15.2c's
/// play-area default — and it says nothing about faceup or facedown, so a
/// card the Corp trashed (10.3.1a puts it there facedown) is as valid a
/// candidate as one the Runner did. The printed "you may" is the whole
/// ability, so it is 9.6.9's declinable conditional.
pub fn haas_bioroid_precision_design() -> Card {
    card("Haas-Bioroid: Precision Design")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Megacorp"])
        .text("You get +1 maximum hand size.")
        .text("Whenever you score an agenda, you may add 1 card from Archives to HQ.")
        .declares([max_hand_size_mod(1)])
        .named("precision design")
        .may_when(corp_scores_agenda(), [add_to_hand(choose(1, &[in_archives()]))])
        .named("an agenda was scored")
        .build()
}

/// Seidr Laboratories: Destiny Defined — Identity: Division.
/// "The first time each turn the Runner loses or spends [click] during a run,
///  you may add 1 card from Archives to the top of R&D."
///
/// COMPLETE. CR 5.2.1 keeps a click SPENT and a click LOST apart — a bioroid
/// subroutine broken by clicking takes them one way, an Eli-class "lose
/// [click]" the other — and this sentence names both, so the pair is content
/// on one condition rather than two abilities that would each spend their own
/// ordinal.
///
/// "During a run" is 6.3.4's game-state test, checked when the condition
/// would be met. "1 card from Archives" names a zone, which lifts 1.15.2c's
/// play-area default, and the card goes to the TOP of R&D rather than into
/// it anywhere — 4.3.2's ordered deck is what makes that a different place.
pub fn seidr_laboratories() -> Card {
    card("Seidr Laboratories: Destiny Defined")
        .corp()
        .identity()
        .faction("Haas-Bioroid")
        .subtypes(&["Division"])
        .text("The first time each turn the Runner loses or spends [click] during a run, you may add 1 card from Archives to the top of R&D.")
        .may_when_first_each_turn(
            spends_or_loses_click_during_run(Runner),
            [add_to_deck(choose(1, &[in_archives()]), true)],
        )
        .named("the first click of the run")
        .build()
}

/// Every Haas-Bioroid identity this module carries, in the order the queue
/// reached them.
pub fn identities() -> Vec<Card> {
    vec![
        custom_biotics(),
        cybernetics_division(),
        haas_bioroid_engineering_the_future(),
        haas_bioroid_precision_design(),
        seidr_laboratories(),
        sportsmetal(),
        thule_subsea(),
    ]
}
