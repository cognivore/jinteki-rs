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

/// Spark Agency: Worldswide Reach — Identity: Division.
/// "The first time each turn you rez an advertisement, the Runner loses
///  1[credit]."
///
/// COMPLETE. 8.1.2's rez with 2.16's subtype stipulation as content on the
/// condition, and 9.6.5c's ordinal about the occurrence. The sentence says
/// nothing about the card's TYPE — an advertisement may be an asset, an
/// upgrade or a piece of ice — so the condition stipulates none either.
///
/// "Loses 1[credit]" is 1.10.4's loss, not a payment: the Runner chooses
/// nothing and a Runner with an empty pool simply loses what they have.
pub fn spark_agency() -> Card {
    card("Spark Agency: Worldswide Reach")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("The first time each turn you rez an advertisement, the Runner loses 1[credit].")
        .when_first_each_turn(corp_rezzes_a_subtyped("Advertisement"), [lose(Runner, 1)])
        .named("the first advertisement rez of the turn")
        .build()
}

/// Editorial Division: Ad Nihilum — Identity: Division.
/// "The first time each turn you take bad publicity, you may search R&D for 1
///  non-agenda black ops, gray ops, or liability card and reveal it. (Shuffle
///  R&D after searching it.) Add that card to HQ."
///
/// COMPLETE. 10.6.1's taking of bad publicity with 9.6.5c's ordinal on the
/// occurrence — the sentence says "1 or more" nowhere, so a card that gives
/// two at once is still one taking and spends the ordinal once.
///
/// The printed line splits where 9.11.4 says it does: (d) a search is its own
/// instruction, (e) a reveal ends one, and the move to HQ is what is left.
/// The parenthetical is 8.7.3 restated — searching a deck shuffles it — so it
/// is part of the search instruction rather than a fourth one.
///
/// "**Black ops**, **gray ops**, or **liability**" is a printed "or" between
/// subtypes, which is the disjunction word; "non-agenda" is the ordinary
/// description vocabulary negated, and R&D is the zone the search names.
pub fn editorial_division() -> Card {
    card("Editorial Division: Ad Nihilum")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("The first time each turn you take bad publicity, you may search R&D for 1 non-agenda black ops, gray ops, or liability card and reveal it. (Shuffle R&D after searching it.) Add that card to HQ.")
        .may_when_first_each_turn(
            takes_bad_publicity(Corp),
            [
                search_rnd(
                    &[
                        non(of_type(CardType::Agenda)),
                        with_any_subtype(&["Black Ops", "Gray Ops", "Liability"]),
                    ],
                    1,
                ),
                reveal(found_by_search()),
                add_to_hand(found_by_search()),
            ],
        )
        .named("ad nihilum")
        .build()
}

/// Information Dynamics: All You Need To Know — Identity: Division.
/// "Draft format only.
///  If you have more [nbn] cards rezzed than any other faction, whenever an
///  agenda is scored or stolen, give the runner 1 tag."
///
/// COMPLETE. The format restriction, then one printed sentence with TWO
/// conditions — 1.17.3a's score and 1.17.3b's steal are different occurrences
/// — so two conditional abilities with the same effect, the shape Jinteki:
/// Personal Evolution takes from the same sentence. Each carries its own copy
/// of 9.6.5c's additional requirement, which is exact here and not merely
/// convenient: the requirement is a question about the board, so asking it
/// twice is asking it once per occurrence, and no ordinal is being shared.
///
/// The faction partition is drawn over the rezzed cards, and the Runner's
/// theft is as much an occurrence of it as the Corp's score.
pub fn information_dynamics() -> Card {
    let more_nbn_rezzed = || {
        more_cards_of_this_faction_than_any_other("NBN", &[installed_corp_card(), rezzed()])
    };
    card("Information Dynamics: All You Need To Know")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("Draft format only.")
        .text("If you have more [nbn] cards rezzed than any other faction, whenever an agenda is scored or stolen, give the runner 1 tag.")
        .when(corp_scores_agenda_if(&[more_nbn_rezzed()]), [give_tags(1)])
        .named("an agenda was scored")
        .when(runner_steals_agenda_if(&[more_nbn_rezzed()]), [give_tags(1)])
        .named("an agenda was stolen")
        .build()
}

/// New Angeles Sol: Your News — Identity: Division.
/// "Whenever an agenda is scored or stolen, you may play 1 current from HQ or
///  Archives (paying its play cost)."
///
/// COMPLETE. One printed sentence with two conditions — 1.17.3a's score and
/// 1.17.3b's steal — so two declinable conditional abilities with the same
/// effect, the Leela Patel shape.
///
/// "From HQ **or** Archives" is a printed "or" between two whole
/// descriptions, so it is the disjunction word and not two instructions:
/// where the card is is a criterion about the card, exactly as its subtype
/// is. Both branches name a zone, which is what lifts 1.15.2c for the whole
/// description — a disjunction with one silent branch would leave the
/// play-area default standing for that branch and describe nothing at all.
///
/// The parenthesis is 8.6.7b restated rather than a second sentence: an
/// effect that plays a card pays the play cost unless it says otherwise. So
/// the Corp who cannot afford the current is simply not able to play it, and
/// 8.6.6c is what then keeps the played current in the play area instead of
/// trashing it.
pub fn new_angeles_sol() -> Card {
    let a_current_in_hq_or_archives = || {
        choose(
            1,
            &[with_subtype("Current"), any_of(&[&[in_hand_of(Corp)], &[in_archives()]])],
        )
    };
    card("New Angeles Sol: Your News")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("Whenever an agenda is scored or stolen, you may play 1 current from HQ or Archives (paying its play cost).")
        .may_when(corp_scores_agenda(), [play_card(a_current_in_hq_or_archives())])
        .named("an agenda was scored")
        .may_when(runner_steals_agenda(), [play_card(a_current_in_hq_or_archives())])
        .named("an agenda was stolen")
        .build()
}

/// Every NBN identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        new_angeles_sol(),
        information_dynamics(),
        editorial_division(),
        nbn_reality_plus(),
        nbn_the_world_is_yours(),
        pravdivost_consulting(),
        spark_agency(),
    ]
}
