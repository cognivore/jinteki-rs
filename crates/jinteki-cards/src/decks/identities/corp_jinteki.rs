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

/// Harmony Medtech: Biomedical Pioneer — Identity: Division.
/// "Each player needs 1 fewer agenda point to win the game."
///
/// COMPLETE. A permanent fact about the game rather than about any card, so it
/// is a static declaration and nothing that happens.
///
/// CR 1.17.2 states winning as a comparison — "if at any time a player's score
/// is greater than or equal to 7, they win the game at the next checkpoint" —
/// and this sentence modifies the number on the far side of it. It does not
/// touch a score: a player who needs six has gained no agenda point, so
/// 1.17.1a's threat level, an "agenda points at least" requirement and every
/// other reader of a score go on reading the real one. The only place the
/// declaration is read is 10.3.1c, which is the only place the comparison is
/// made.
///
/// "Each player" reaches both without asking whose card it is — the Runner
/// wins on six against a Jinteki Corp who printed this, which is the whole
/// bargain of the card.
pub fn harmony_medtech() -> Card {
    card("Harmony Medtech: Biomedical Pioneer")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("Each player needs 1 fewer agenda point to win the game.")
        .declares([each_player_needs_fewer_agenda_points_to_win(1)])
        .named("biomedical pioneer")
        .build()
}

/// Issuaq Adaptics: Sustaining Diversity — Identity: Division.
/// "Whenever you score an agenda that you did not install or advance this
///  turn, place 1 power counter on this identity.
///  For each hosted power counter, you need 1 less agenda point to win the
///  game."
///
/// COMPLETE. Two printed lines: a conditional ability and a static
/// declaration, with the counters the first places being what the second
/// counts.
///
/// "An agenda that you did not install or advance this turn" is what the
/// sentence says about the AGENDA, so it rides on the condition as a
/// description and not as a requirement about the game state: the question is
/// asked of the card the occurrence names. Both halves are 1.12.6's game
/// history, which 10.2.1 makes open information — and De Morgan is why the
/// two are written beside each other: "did not install OR advance" is "was not
/// installed this turn" and "was not advanced this turn" together.
///
/// 1.18.2 is what keeps the second half honest. Placing an advancement counter
/// is not advancing, so an agenda a Tennin-class ability loaded still qualifies
/// — and an agenda advanced this turn whose counters were then removed does
/// not.
///
/// The second line is a declaration about 1.17.2's comparison, exactly as
/// Harmony Medtech's is, with a calculated amount instead of a printed one —
/// re-read every time the comparison is made, so the counter that arrives with
/// a score lowers the requirement before the checkpoint that reads it.
pub fn issuaq_adaptics() -> Card {
    card("Issuaq Adaptics: Sustaining Diversity")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("Whenever you score an agenda that you did not install or advance this turn, place 1 power counter on this identity.")
        .text("For each hosted power counter, you need 1 less agenda point to win the game.")
        .when(
            corp_scores_an_agenda_matching(&[
                installed_this_turn(false),
                advanced_this_turn(false),
            ]),
            [place(CounterKind::Power, 1)],
        )
        .named("sustaining diversity")
        .declares([you_need_fewer_agenda_points_to_win(times(
            1,
            per_hosted_counter(CounterKind::Power),
        ))])
        .named("for each hosted power counter")
        .build()
}

/// Nisei Division: The Next Generation — Identity: Division.
/// "Whenever you and the Runner reveal secretly spent credits, gain
///  1[credit]."
///
/// COMPLETE. CR 10.14.6 builds the psi construction as ONE instruction — both
/// players secretly spend, the spent credits are revealed, they are spent
/// immediately, and the outcome branches — and 10.14.6c is the reveal step in
/// the middle of it. That step is what this condition names, and it is one
/// moment for both players: the sentence says "you and the Runner", the reveal
/// happens to both at once, so a psi game meets it once however the bids came
/// out.
///
/// Not the spending. 10.14.4a puts the spend immediately AFTER the reveal, and
/// a Corp who bid nothing has spent nothing — this identity is paid all the
/// same, which is the difference between naming the reveal and naming the
/// credits.
pub fn nisei_division() -> Card {
    card("Nisei Division: The Next Generation")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("Whenever you and the Runner reveal secretly spent credits, gain 1[credit].")
        .when(secretly_spent_credits_are_revealed(), [gain(Corp, 1)])
        .named("the next generation")
        .build()
}

/// Mti Mwekundu: Life Improved — Identity: Division.
/// "Once per turn → When the Runner approaches a server, you may install 1
///  piece of ice from HQ in the innermost position protecting that server,
///  ignoring all costs. The Runner moves to that ice and approaches it. If
///  this is not the first time they have approached ice this run, they may
///  jack out."
///
/// COMPLETE. One conditional ability with 9.3.6g's once-per-turn flag, met at
/// 6.9.4g's step — the moment every piece of ice protecting the attacked
/// server has been passed, or the run's first moment when none is.
///
/// The printed "you may" is in the first sentence, and 9.6.9 reads it as the
/// ability's: "if a conditional ability gives its controller a choice of
/// whether to apply its effects, such that the ability could potentially have
/// no effects at all, it is considered an optional conditional ability". Both
/// later sentences are stated about what the first did — "that ice", and the
/// approach "this" names — so declining the install is declining all of it.
///
/// "The innermost position protecting that server" is 6.2.2b, which the CR
/// states as its own rule beside 6.2.2a's outermost — the default 8.5.2d
/// installs to and the one "unless otherwise indicated" is written for. "That
/// server" is the attacked one (6.1.2), because 6.9.4g approaches the attacked
/// server; it cannot be named when the card is written, since 4.6.8's remotes
/// are created during play. "Ignoring all costs" is 1.16.5c, which includes
/// 8.5.11a's 1[credit] per piece of ice already protecting the server — every
/// one of which this ice is going inside of.
///
/// "The Runner moves to that ice and approaches it" is 6.2.8a pointed at the
/// card this ability installed (8.5.16f), so it announces nothing; an ability
/// that installed nothing moves nobody.
///
/// The third sentence is 6.1.5a from the other end. The opportunity to jack
/// out belongs to a Runner who has PASSED a piece of ice, and this ability
/// puts them back in front of one without a pass — so the sentence hands back
/// what the run's structure would have given them, and withholds it in exactly
/// 6.1.5b's case, a server they approached with no ice protecting it. The
/// count is of approaches already made: the run's own Approach Ice Phase has
/// not been reached when this instruction resolves, so "not the first time"
/// is "they have approached ice this run already". "They may" names the
/// Runner, so the Runner is who the choice is put to (1.14.5).
pub fn mti_mwekundu() -> Card {
    card("Mti Mwekundu: Life Improved")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("Once per turn → When the Runner approaches a server, you may install 1 piece of ice from HQ in the innermost position protecting that server, ignoring all costs. The Runner moves to that ice and approaches it. If this is not the first time they have approached ice this run, they may jack out.")
        .may_when_once_per_turn(
            runner_approaches_a_server(),
            [
                install_ignoring_all_costs(
                    choose(1, &[in_hand_of(Corp), of_type(CardType::Ice)]),
                    InstallDest::InnermostProtectingAttackedServer,
                ),
                move_runner_to_ice(the_card_this_ability_installed()),
                if_met(
                    &[
                        approached_ice_this_run_already(),
                        board_has(&[installed_by_this_ability()], 1),
                    ],
                    [performed_by(Runner, may(jack_out()))],
                ),
            ],
        )
        .named("life improved")
        .build()
}

/// Chronos Protocol: Selective Mind-mapping — Identity: Division.
/// "For the first net damage the Runner suffers each turn, you may look at the
///  Runner's grip and select the card that is trashed."
///
/// COMPLETE. One printed sentence, and it never happens: 10.4.2a trashes "1
/// randomly-chosen card from the grip" for each point of damage, and this
/// sentence is 10.4.3a's modification OF that procedure, so it is a static
/// declaration (9.4.1 — it never resolves) rather than anything the Corp does.
///
/// Everything the sentence stipulates is content on that declaration:
///
/// - **"net damage"** names one of 10.4.2's three types, so a meat or core
///   damage goes on being random while this identity is out.
/// - **"the first … each turn"** is the printed ordinal, read from the change
///   log (10.2.1 open information) rather than from 9.3.6g's once-per-turn
///   flag: a static ability never resolves and so never spends one. The count
///   is taken before this damage's own occurrence is recorded, so the damage
///   being decided is never one of the earlier ones it is counted against —
///   and a *prevented* damage was never suffered, so it does not use the turn
///   up.
/// - **"you may"** governs the looking as well as the selecting, so the Corp
///   is asked before the grip is named. Naming the candidates first and
///   letting the Corp choose none would show them a grip the card only lets
///   them see when they use it — 4.3.2 keeps the grip hidden otherwise.
///
/// "The card that is trashed" is singular because one net damage trashes one
/// card. The number is the declaration's own, not the damage's: the VM takes
/// the smaller of the two, so a Corp who somehow did two net damage at once
/// selects one of the pair and the other stays random.
///
/// 9.12.1c is what happens when the Runner declares the same thing (Titanium
/// Ribs): the choice can only be made once, so the active player makes it.
pub fn chronos_protocol_selective_mind_mapping() -> Card {
    card("Chronos Protocol: Selective Mind-mapping")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("For the first net damage the Runner suffers each turn, you may look at the Runner's grip and select the card that is trashed.")
        .declares([may_select_first_damage_trashes_each_turn(
            Corp,
            DamageKind::Net,
            amount(1),
        )])
        .named("selective mind-mapping")
        .build()
}

/// Saraswati Mnemonics: Endless Exploration — Identity: Division.
/// "[click], 1[credit]: Install 1 card from HQ in the root of a remote server,
///  then place 1 advancement counter on it. You cannot score or rez that card
///  until your next turn begins."
///
/// COMPLETE. One paid ability whose trigger cost is everything printed before
/// the colon, and two instructions after it.
///
/// The first sentence is ONE instruction. 9.11.4b splits a sentence at every
/// install "after the first" and there is only one here, and "then" is not
/// among 9.11.4's exceptions at all — so the install and the placement are a
/// single instruction, which is what `combined` says.
///
/// "In the root of a remote server" is 8.5.16b's declaration with 4.6.8's
/// remotes as the only servers on offer and 4.6.6b's root as the only half of
/// them, 8.5.2a's brand-new remote included: the Corp still says WHICH, because
/// a remote server is created during play and a card written before the game
/// cannot name one. A card that could occupy no such root — a piece of ice —
/// leaves no destination to identify, and 8.5.14 is what stops the install
/// then; the sentence says "1 card" and describes nothing else, so the
/// announcement is HQ and the legality is the destination's business.
///
/// 1.18.2: the counter is PLACED, not advanced, so this never meets a "whenever
/// you advance a card" condition and never pays a Built-to-Last-class ability.
///
/// The second sentence is a lingering effect (9.10.1) and not a static
/// declaration, because "that card" NAMES the card the first instruction
/// installed rather than describing it — another copy of the same card is
/// untouched. 1.2.2 gives it precedence over every ability that would score or
/// rez: the (S) and (R) options are simply not offered for it, and an ability
/// directing either is refused.
///
/// "Until your next turn begins" is a span 5.1 makes longer than "this turn" by
/// exactly one turn — through the rest of this turn, through the whole of the
/// Runner's, and gone the moment the Corp's next turn begins, which is before
/// anything in that turn can happen.
pub fn saraswati_mnemonics() -> Card {
    card("Saraswati Mnemonics: Endless Exploration")
        .corp()
        .identity()
        .faction("Jinteki")
        .subtypes(&["Division"])
        .text("[click], 1[credit]: Install 1 card from HQ in the root of a remote server, then place 1 advancement counter on it. You cannot score or rez that card until your next turn begins.")
        .paid(
            clicks(1).plus_cost(credits(1)),
            [
                combined([
                    install(
                        choose(1, &[in_hand_of(Corp)]),
                        InstallDest::DeclaredByInstallerInRemoteRoot,
                    ),
                    place_on(
                        the_card_this_ability_installed(),
                        CounterKind::Advancement,
                        1,
                    ),
                ]),
                cannot_be(
                    the_card_this_ability_installed(),
                    &[ProhibitedAction::Score, ProhibitedAction::Rez],
                    until_next_turn_begins_of(Corp),
                ),
            ],
        )
        .named("endless exploration")
        .build()
}

/// Every Jinteki identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        harmony_medtech(),
        issuaq_adaptics(),
        nisei_division(),
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
        mti_mwekundu(),
        chronos_protocol_selective_mind_mapping(),
        saraswati_mnemonics(),
    ]
}
