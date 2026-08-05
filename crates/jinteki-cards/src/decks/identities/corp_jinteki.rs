//! Corp — Jinteki identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).

use crate::edsl::*;

/// One installed piece of ice — the description Synthetic Systems' sentence
/// makes twice, written once here. 8.8.2 filters the second announcement
/// against the first, so the same description on both sides can never choose
/// the same card twice.
fn an_installed_piece_of_ice() -> TargetSpec {
    choose(1, &[installed_corp_card(), of_type(CardType::Ice)])
}

/// Jinteki: Personal Evolution — Identity: Megacorp.
/// "Whenever an agenda is scored or stolen, do 1 net damage."
///
/// COMPLETE. One printed sentence with two conditions, so two conditional
/// abilities with the same effect (9.6.1 gives an ability one primary
/// condition, and 1.17.3a's score and 1.17.3b's steal are different
/// occurrences) — the shape Leela Patel already takes from the other side of
/// the table.
///
/// 10.4.2 makes the CORP responsible for the damage on both halves, including
/// the one the Runner's own theft meets, which is what decides who wins if it
/// flatlines them.
pub fn jinteki_personal_evolution() -> Card {
    card("Jinteki: Personal Evolution")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Megacorp"])
        .text("Whenever an agenda is scored or stolen, do 1 net damage.")
        .when(corp_scores_agenda(), [net_damage(Corp, 1)])
        .named("an agenda was scored")
        .when(runner_steals_agenda(), [net_damage(Corp, 1)])
        .named("an agenda was stolen")
        .build()
}

/// Jinteki: Potential Unleashed — Identity: Megacorp.
/// "Whenever the Runner takes at least 1 net damage, trash the top card of
///  the stack."
///
/// COMPLETE. 10.4.1's damage with the sentence's stipulation about the KIND
/// riding on the condition as content. "At least 1" is not a threshold to
/// check: 10.4.1 makes damage of an amount, and every occurrence of net
/// damage the Runner suffers is at least 1 — the phrase is there to say that
/// three net damage still fires this once, which is exactly what one
/// condition met per occurrence does.
///
/// "The top card of the stack" names a zone, so 1.15.2c's play-area
/// restriction lifts and the description itself fixes the card; nothing is
/// announced.
pub fn jinteki_potential_unleashed() -> Card {
    card("Jinteki: Potential Unleashed")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Megacorp"])
        .text("Whenever the Runner takes at least 1 net damage, trash the top card of the stack.")
        .when(suffers_damage(DamageKind::Net), [trash(top_of_stack(amount(1)))])
        .named("potential unleashed")
        .build()
}

/// Pālanā Foods: Sustainable Growth — Identity: Division.
/// "The first time each turn the Runner draws a card, gain 1[credit]."
///
/// COMPLETE. CR 8.4.2 meets a draw-related condition once PER CARD DRAWN, so
/// the printed ordinal is doing real work: a Runner who draws three cards
/// with one action pays this identity once, on the first of them. That is
/// 9.6.5c's stipulation about the occurrence and not 9.3.6g's flag — 9.1.6
/// only spends a flag when a player *uses* an ability, and this one is
/// entirely mandatory.
///
/// The condition names the Runner, so the Corp's own mandatory draw at the
/// start of its turn is not one of the times counted.
pub fn palana_foods() -> Card {
    card("Pālanā Foods: Sustainable Growth")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("The first time each turn the Runner draws a card, gain 1[credit].")
        .when_first_each_turn(draws_a_card(Runner), [gain(Corp, 1)])
        .named("the first runner draw of the turn")
        .build()
}

/// Tennin Institute: The Secrets Within — Identity: Division.
/// "When your turn begins, if the Runner did not make a successful run during
///  their last turn, you may place 1 advancement counter on an installed
///  card."
///
/// COMPLETE. The "if …" clause is 9.6.5c's additional requirement listed
/// inside the trigger condition, so it is asked when the turn begins and not
/// again while the ability resolves. It is NOT "made no runs at all": an
/// unsuccessful run leaves the requirement met, which is the whole point of
/// the card.
///
/// "An installed card" makes no stipulation about whose, so 1.15.2c's default
/// — the installed cards — is the description, and a Runner card is as valid
/// a target as a Corp one. 1.18.2: the counter is PLACED, not advanced, so
/// this never meets a "whenever you advance a card" condition.
pub fn tennin_institute() -> Card {
    card("Tennin Institute: The Secrets Within")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("When your turn begins, if the Runner did not make a successful run during their last turn, you may place 1 advancement counter on an installed card.")
        .may_when(
            turn_begins_if(Corp, &[runner_made_no_successful_run_last_turn()]),
            [place_on(choose(1, &[]), CounterKind::Advancement, 1)],
        )
        .named("the secrets within")
        .build()
}

/// Jinteki: Restoring Humanity — Identity: Megacorp.
/// "When your discard phase ends, if there is a facedown card in Archives,
///  gain 1[credit]."
///
/// COMPLETE. 5.5.4's discard phase, named as the Corp's own, with 9.6.5c's
/// additional requirement inside the condition — so the question is asked at
/// the end of the discard phase, AFTER the cards discarded there have
/// arrived, which is what makes a Corp who discarded this turn paid for it.
///
/// "A facedown card in Archives" is two ordinary description words. 10.3.1a
/// is what makes the pair meaningful: a card the CORP trashes enters Archives
/// facedown and one the RUNNER trashes enters it faceup, so the sentence
/// asks about the Corp's own discards and not about what a run left behind.
/// It is not "unrezzed", which 8.1.2 restricts to installed Corp cards.
pub fn jinteki_restoring_humanity() -> Card {
    card("Jinteki: Restoring Humanity")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Megacorp"])
        .text("When your discard phase ends, if there is a facedown card in Archives, gain 1[credit].")
        .when(
            your_discard_phase_ends_if(Corp, &[board_has(&[in_archives(), facedown()], 1)]),
            [gain(Corp, 1)],
        )
        .named("restoring humanity")
        .build()
}

/// Synthetic Systems: The World Re-imagined — Identity: Division.
/// "Draft format only.
///  If you have more [jinteki] cards rezzed than any other faction, when your
///  turn begins, you may swap 2 pieces of installed ice."
///
/// COMPLETE. The format restriction, then one declinable conditional ability
/// (9.6.9) with 9.6.5c's additional requirement inside its condition — the
/// faction partition of the Corp's rezzed cards, asked when the Corp's own
/// turn begins.
///
/// The swap is 1.15.2's two announcements for one instruction, and 8.8.2 is
/// what keeps the Corp from naming the same piece of ice twice. Both pieces
/// are just "installed ice": the sentence says nothing about servers or
/// positions, so a rezzed piece and an unrezzed one may trade places, and
/// neither one has to be on the same server as the other.
pub fn synthetic_systems() -> Card {
    card("Synthetic Systems: The World Re-imagined")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("Draft format only.")
        .text("If you have more [jinteki] cards rezzed than any other faction, when your turn begins, you may swap 2 pieces of installed ice.")
        .may_when(
            turn_begins_if(
                Corp,
                &[more_cards_of_this_faction_than_any_other(
                    "Jinteki",
                    &[installed_corp_card(), rezzed()],
                )],
            ),
            [swap(an_installed_piece_of_ice(), an_installed_piece_of_ice())],
        )
        .named("the world re-imagined")
        .build()
}

/// PT Untaian: Life's Building Blocks — Identity: Division.
/// "When your discard phase ends, if there are 3 or fewer cards in HQ, you may
///  pay 1[credit] to place 1 advancement counter on an unrezzed card you can
///  advance. (You cannot score that card this turn.)"
///
/// COMPLETE. 5.5.4's discard phase named as the Corp's own, with 9.6.5c's
/// additional requirement inside the condition — so HQ is counted AFTER the
/// cards discarded there have gone, which is what makes a Corp who discarded
/// down to three qualify.
///
/// "You may pay 1[credit] to …" is 9.6.9d: the option is inside the one
/// sentence rather than on the ability, so the ability is mandatory and the
/// INSTRUCTION is what may be declined — and 1.16.1 makes the payment part of
/// the instruction, so a Corp who cannot afford it is not offered the choice
/// at all.
///
/// "An unrezzed card you can advance" is two ordinary description words:
/// 8.1.2's facedown installed Corp card — which includes an agenda, since an
/// agenda can never be rezzed — and 1.18.3's advance permission, read through
/// the same derivation the basic advance action uses.
///
/// The parenthesis is 1.4's reminder text and not a second instruction: 5.5
/// puts the discard phase after the action phase, and 1.17.3c offers scoring
/// only in the action phase's paid windows, so the turn this counter is
/// placed in has no scoring left in it to forbid.
pub fn pt_untaian() -> Card {
    card("PT Untaian: Life's Building Blocks")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("When your discard phase ends, if there are 3 or fewer cards in HQ, you may pay 1[credit] to place 1 advancement counter on an unrezzed card you can advance. (You cannot score that card this turn.)")
        .when(
            your_discard_phase_ends_if(Corp, &[board_has_at_most(&[in_hand_of(Corp)], 3)]),
            [may_pay(
                credits(1),
                place_on(choose(1, &[unrezzed(), advanceable()]), CounterKind::Advancement, 1),
            )],
        )
        .named("life's building blocks")
        .build()
}

/// Industrial Genomics: Growing Solutions — Identity: Division.
/// "The trash cost of each card is increased by 1 for each facedown card in
///  Archives."
///
/// COMPLETE. A permanent fact about every trash cost in the game, so it is a
/// static declaration and not something that happens. The amount is a
/// calculated quantity (9.12.2), re-read every time a cost is read — a card
/// entering Archives raises every trash cost on the board at once, and one
/// leaving lowers them again.
///
/// "Each card" stipulates nothing at all, so the description is empty: the
/// Runner's own cards are covered by the sentence as written. What limits it
/// is 7.1.5a from the other end — a card with no trash cost printed on it
/// never gains one, because "if a card does not have a trash cost, the Runner
/// cannot pay its trash cost" is about the CARD and not about the number.
///
/// "Facedown card in Archives" is the pair 10.3.1a makes meaningful: a card
/// the Corp trashes enters Archives facedown and one the Runner trashes
/// enters it faceup, so what raises these costs is the Corp's own discards.
/// It is not "unrezzed", which 8.1.2 restricts to installed Corp cards.
pub fn industrial_genomics() -> Card {
    card("Industrial Genomics: Growing Solutions")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("The trash cost of each card is increased by 1 for each facedown card in Archives.")
        .declares([trash_costs_increased_by(
            &[],
            times(1, per_card(any_of(&[&[in_archives(), facedown()]]))),
        )])
        .named("growing solutions")
        .build()
}

/// Jinteki: Replicating Perfection — Identity: Megacorp.
/// "The Runner cannot run on remote servers. Ignore this ability until the
///  end of the turn whenever the Runner runs on a central server."
///
/// COMPLETE. Two printed sentences and neither of them happens: the first is
/// a permanent prohibition, the second says when the first does not apply. So
/// it is ONE static ability — the declaration, with 9.3.7a's stated condition
/// saying while what holds it applies.
///
/// The second sentence is the first's condition read from the other side.
/// "Ignore this ability until the end of the turn whenever the Runner runs on
/// a central server" and "while the Runner has not run on a central server
/// this turn" are the same span: the ability applies from the turn's
/// beginning until the first such run, and not again until the next turn
/// begins. 1.12.6's "this turn" is exactly "until the end of the turn"
/// counted from the other end, and 10.2.1 makes the run history the open
/// information the question is answered from.
///
/// 6.3.2a: what is prohibited is ANNOUNCING a remote as the attacked server,
/// so the run action is simply not offered for one. It is not a prohibition
/// on the run continuing — 6.1.2d's change of attacked server can still put a
/// run in progress onto a remote, which is what an AgInfusion-class ability
/// does.
///
/// "Runs on a central server" is the run being MADE, not one being
/// successful: an unsuccessful run on HQ lifts this for the turn just as well.
pub fn jinteki_replicating_perfection() -> Card {
    card("Jinteki: Replicating Perfection")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Megacorp"])
        .text("The Runner cannot run on remote servers. Ignore this ability until the end of the turn whenever the Runner runs on a central server.")
        .declares_while(
            &[runner_made_no_run_this_turn_on(&[
                ServerId::Hq,
                ServerId::Rnd,
                ServerId::Archives,
            ])],
            [cannot_initiate_runs_on_remote_servers()],
        )
        .named("replicating perfection")
        .build()
}

/// Hyoubu Institute: Absolute Clarity — Identity: Division.
/// "The first time each turn you reveal a card, gain 1[credit].
///  [click]: Reveal 1 card from the grip at random or the top card of the
///  stack."
///
/// COMPLETE. Two printed lines, two abilities: a conditional one and a paid
/// one. The paid ability's own reveal meets the conditional one's condition,
/// which is the card working as printed — the Corp spends a click, sees a
/// card and is paid once a turn for the first look.
///
/// "You reveal" is 9.1.1a's controller and not the card's owner: both halves
/// of the paid ability show a RUNNER card, and the Corp is the one revealing
/// it. 1.21.5 keeps this apart from looking, exposing and accessing — none of
/// those meets a reveal condition.
///
/// "At random" is what stops the grip half being a description: 1.15.2b puts
/// a target announcement to a player, and this sentence takes the choice from
/// both, so nothing is announced and an empty grip reveals nothing at all.
/// The stack half names a zone and fixes the card by position, so it
/// announces nothing either. The printed "or" between them is 9.11.4g's
/// option choice, made by the Corp when the ability resolves.
pub fn hyoubu_institute() -> Card {
    card("Hyoubu Institute: Absolute Clarity")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("The first time each turn you reveal a card, gain 1[credit].")
        .text("[click]: Reveal 1 card from the grip at random or the top card of the stack.")
        .when_first_each_turn(reveals_a_card(Corp), [gain(Corp, 1)])
        .named("absolute clarity")
        .paid(
            clicks(1),
            [choose_one([
                ("1 card from the grip at random", vec![reveal_at_random_from_hand_of(Runner, 1)]),
                ("the top card of the stack", vec![reveal(top_of_stack(amount(1)))]),
            ])],
        )
        .named("reveal a card")
        .build()
}

/// Every Jinteki identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        hyoubu_institute(),
        industrial_genomics(),
        jinteki_replicating_perfection(),
        synthetic_systems(),
        pt_untaian(),
        jinteki_personal_evolution(),
        jinteki_potential_unleashed(),
        jinteki_restoring_humanity(),
        palana_foods(),
        tennin_institute(),
    ]
}
