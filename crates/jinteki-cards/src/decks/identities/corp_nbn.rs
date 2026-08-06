//! Corp — NBN identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). Azmari EdTech and Nebula Talent Management are the
//! two NBN identities that already exist — they live in `decks/gauntlet.rs`,
//! because that deck plays one and carries the other's back face — and
//! everything else in the faction lands here.

use jinteki_cr::Subtype;

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
        .subtypes(&[Subtype::Megacorp])
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
/// static declaration: 5.5.3's maximum hand size, read continuously, which is
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
        .subtypes(&[Subtype::Megacorp])
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
        .subtypes(&[Subtype::Division])
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
        .subtypes(&[Subtype::Division])
        .text("The first time each turn you rez an advertisement, the Runner loses 1[credit].")
        .when_first_each_turn(corp_rezzes_a_subtyped(Subtype::Advertisement), [lose(Runner, 1)])
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
        .subtypes(&[Subtype::Division])
        .text("The first time each turn you take bad publicity, you may search R&D for 1 non-agenda black ops, gray ops, or liability card and reveal it. (Shuffle R&D after searching it.) Add that card to HQ.")
        .may_when_first_each_turn(
            takes_bad_publicity(Corp),
            [
                search_rnd(
                    &[
                        non(of_type(CardType::Agenda)),
                        with_any_subtype(&[Subtype::BlackOps, Subtype::GrayOps, Subtype::Liability]),
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
        .subtypes(&[Subtype::Division])
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
            &[with_subtype(Subtype::Current), any_of(&[&[in_hand_of(Corp)], &[in_archives()]])],
        )
    };
    card("New Angeles Sol: Your News")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("Whenever an agenda is scored or stolen, you may play 1 current from HQ or Archives (paying its play cost).")
        .may_when(corp_scores_agenda(), [play_card(a_current_in_hq_or_archives())])
        .named("an agenda was scored")
        .may_when(runner_steals_agenda(), [play_card(a_current_in_hq_or_archives())])
        .named("an agenda was stolen")
        .build()
}

/// Near-Earth Hub: Broadcast Center — Identity: Division.
/// "The first time each turn you create a remote server, draw 1 card."
///
/// COMPLETE. CR 4.6.8d makes a remote server exist while a card is in its
/// root or protecting it, so the sentence names step 8.5.16e of the
/// installation that puts the FIRST card there — not the install itself. An
/// install into a server that already exists creates nothing and draws
/// nothing, which is the whole point of the card.
///
/// 4.6.5's central servers are never created: they exist for the whole game,
/// so "a remote server" is the only thing the sentence could name and there
/// is nothing further to stipulate.
///
/// The ordinal is 9.6.5c's stipulation about the occurrence (the Pālanā Foods
/// reading), not 9.3.6g's flag: the ability is entirely mandatory and 9.1.6
/// says a player never USES such an ability, so a flag would never be spent.
pub fn near_earth_hub() -> Card {
    card("Near-Earth Hub: Broadcast Center")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("The first time each turn you create a remote server, draw 1 card.")
        .when_first_each_turn(creates_a_remote_server(Corp), [draw(Corp, 1)])
        .named("broadcast center")
        .build()
}

/// Haarpsichord Studios: Entertainment Unleashed — Identity: Division.
/// "The Runner cannot steal more than one agenda each turn."
///
/// COMPLETE. A permanent fact rather than something that happens, so it is a
/// static declaration. 1.2.2 makes "cannot" absolute: once the Runner has
/// stolen an agenda this turn, 7.2.3's steal step simply does nothing for the
/// rest of the turn — the access still happens, the agenda is still accessed,
/// and nothing is put to the Runner to decline.
///
/// It is a limit on STEALING (1.17.7) and not on scoring: an agenda the Corp
/// scores is untouched, and so is one an ability ADDS to the Runner's score
/// area, since 1.17.3e/f say a card added to a score area is not stolen.
///
/// "Each turn" is 1.12.6's window, counted from the game history (10.2.1) —
/// so it resets on both players' turns, and the Corp's own turn is as much a
/// turn as the Runner's for a steal made during a Corp-turn run.
pub fn haarpsichord_studios() -> Card {
    card("Haarpsichord Studios: Entertainment Unleashed")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("The Runner cannot steal more than one agenda each turn.")
        .declares([cannot_steal_more_than_each_turn(1)])
        .named("entertainment unleashed")
        .build()
}

/// Harishchandra Ent.: Where You're the Star — Identity: Division.
/// "While the Runner is tagged, they play with the grip revealed."
///
/// COMPLETE. A permanent fact with a stated condition, so it is a static
/// declaration under 9.3.7a's "while" — the declarations apply exactly while
/// the condition holds, and the moment the Runner's last tag comes off the
/// grip is hidden again.
///
/// What it changes is 4.3.2, which is the ONLY reason a hand is hidden at
/// all: "a player may look at the cards in their own hand, but not at any of
/// the cards in their opponent's hands". Lifting it for this hand makes those
/// cards open information (10.2.3), and nothing else about the zone changes —
/// the cards are not revealed one at a time, so no "whenever you reveal a
/// card" condition is ever met by it.
pub fn harishchandra_ent() -> Card {
    card("Harishchandra Ent.: Where You're the Star")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("While the Runner is tagged, they play with the grip revealed.")
        .declares_while(&[runner_is_tagged()], [hand_revealed(Runner)])
        .named("where you're the star")
        .build()
}

/// NBN: Controlling the Message — Identity: Megacorp.
/// "The first time the Runner trashes an installed Corp card each turn, you
///  may trace[4]. If successful, give the Runner 1 tag (cannot be avoided)."
///
/// COMPLETE. One declinable conditional ability (9.6.9) carrying 9.6.5c's
/// ordinal, and one instruction: 10.10 makes a trace and everything stated
/// after "if successful" one structure, which is why the tag is written
/// inside it rather than beside it.
///
/// "An installed Corp card" is three ordinary description words — whose card
/// it was (1.14.1), who trashed it (1.14.5), and where it was trashed from.
/// The last one is doing real work: a Corp card the Runner trashes while
/// accessing it out of HQ or R&D was never installed, so it does not meet
/// this, and one trashed out of a server root or off the board does.
///
/// "(Cannot be avoided)" is 9.3.3g's restriction, and 9.4.5 makes it ride the
/// value — so the tag is still a value the trace produced, and nothing
/// offered in the interrupt window can take it away. 9.9.5 makes "prevent"
/// and "avoid" the same word, which is why one stipulation answers both.
pub fn nbn_controlling_the_message() -> Card {
    card("NBN: Controlling the Message")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Megacorp])
        .text("The first time the Runner trashes an installed Corp card each turn, you may trace[4]. If successful, give the Runner 1 tag (cannot be avoided).")
        .may_when_first_each_turn(
            runner_trashes_an_installed_corp_card(),
            [trace(4, [give_tags_that_cannot_be_avoided(1)])],
        )
        .named("controlling the message")
        .build()
}

/// GameNET: Where Dreams are Real — Identity: Division.
/// "Whenever a Corp card ability causes the Runner to spend or lose at least
///  1[credit] during a run, gain 1[credit]."
///
/// COMPLETE. One conditional ability whose condition carries everything the
/// sentence stipulates. "Spend or lose" is 1.10.3b's forced movement and
/// 1.10.3c's payment named together — different ways the same credits leave
/// the same pool — so it is ONE condition reaching both, the shape a "loses
/// or spends [click]" sentence already takes for the other resource.
///
/// "At least 1[credit]" is not a threshold to check: every payment and every
/// loss is of an amount, and one of 0 has not moved anything. The phrase says
/// a payment of five still pays this once, which is what one condition met
/// per occurrence does — and 1.16.2b makes a calculated cost ONE payment, so
/// a "for each" cost does not meet it twice either.
///
/// "A Corp card ability causes" is 9.1.4's source, asked in the ordinary
/// description words: the ability whose cost the Runner paid, or whose
/// instruction took the credits. The Runner's own spending — a play cost, a
/// basic action, an icebreaker's pump — comes through no Corp card and is not
/// one of these, and neither is a Corp ability's own payment.
///
/// "During a run" is 9.6.5c's stipulation inside the condition (6.1.1: a run
/// is in progress), so the additional cost 6.3.4 charges to MAKE a run is not
/// paid during one and does not meet this.
pub fn gamenet() -> Card {
    card("GameNET: Where Dreams are Real")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("Whenever a Corp card ability causes the Runner to spend or lose at least 1[credit] during a run, gain 1[credit].")
        .when(
            spends_or_loses_credits(Runner, &[controlled_by(Corp)], &[during_a_run()]),
            [gain(Corp, 1)],
        )
        .named("where dreams are real")
        .build()
}

/// Synapse Global: Faster than Thought — Identity: Division.
/// "The first time each turn a tag is removed, you may reveal and install 1
///  card from HQ, ignoring all costs.
///  [click], remove 1 tag: Gain 2[credit]."
///
/// COMPLETE. Two printed lines, two abilities — and the paid one feeds the
/// conditional one, because 1.16.10b records a payment's own changes where
/// conditions can meet them. The Corp spends a click and a tag, and the tag
/// going back to the bank is exactly what the other ability is waiting for:
/// the identity turns the Runner's tag into a free install, once a turn.
///
/// The condition names no player. 10.5.1 puts every tag on the Runner, so a
/// tag removed by the RUNNER's own 10.5.4 basic action meets this just as
/// well as one the Corp removed — which is the point of a card that would
/// otherwise never see its own tags leave.
///
/// "Reveal and install" is 9.11.4e's split, not one instruction: making the
/// card visible ends an instruction, a checkpoint occurs while it is visible,
/// and the install is what remains of the sentence — acting on the card the
/// reveal announced (1.15.4). "Ignoring all costs" is 1.16.5c, which removes
/// 8.5.11a's 1[credit] per piece of ice already protecting the destination
/// along with every additional cost.
///
/// The install states no destination, so 8.5.16b leaves the choice to the
/// installer: every location the card could legally occupy is on offer.
pub fn synapse_global() -> Card {
    card("Synapse Global: Faster than Thought")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("The first time each turn a tag is removed, you may reveal and install 1 card from HQ, ignoring all costs.")
        .text("[click], remove 1 tag: Gain 2[credit].")
        .may_when_first_each_turn(
            a_tag_is_removed(),
            [
                reveal(choose(1, &[in_hand_of(Corp)])),
                install_ignoring_all_costs(earlier_choice(0), InstallDest::DeclaredByInstaller),
            ],
        )
        .named("faster than thought")
        .paid(clicks(1).plus_cost(remove_a_tag(1)), [gain(Corp, 2)])
        .named("remove a tag")
        .build()
}

/// NBN: Making News — Identity: Megacorp.
/// "2[recurring-credit]
///  Use these credits during trace attempts."
///
/// COMPLETE. The first line is 1.10.5's shorthand — two credits on the
/// identity from the moment it is active (1.10.5b, which for an identity is
/// 1.6: it is never installed and never rezzed), refilled to two at step
/// 5.6.1c of every Corp turn and never past it (1.10.5d).
///
/// The second line is 1.10.3c, and it names a MOMENT rather than a card: the
/// credits are allowed at 10.8.6c and 10.8.6d, the two steps of a trace
/// attempt where credits are spent, and nowhere else. That is why it needs no
/// description — unlike "to trash cards" or "to pay for using icebreakers",
/// there is no object for a description to be about.
///
/// 10.8.6c is the Corp's own step, but the restriction says nothing about
/// whose spend it is: the sentence is about the payment, and only the Corp
/// ever has this card's credits to spend anyway (1.10.3c reaches only the
/// controller's own cards).
pub fn nbn_making_news() -> Card {
    card("NBN: Making News")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Megacorp])
        .text("2[recurring-credit]")
        .text("Use these credits during trace attempts.")
        .recurring_credits(2)
        .credits_only_during_trace_attempts()
        .build()
}

/// Epiphany Analytica: Nations Undivided — Identity: Division.
/// "The first time each turn the Runner steals or trashes a Corp card, place 1
///  power counter on this identity.
///  [click], hosted power counter: Look at the top 3 cards of R&D. You may
///  install 1 of those cards."
///
/// COMPLETE. Two printed lines, two abilities: a conditional one and a paid
/// one, with the counters the first places being what the second spends.
///
/// "Steals **or** trashes" is ONE condition and not two abilities. 9.6.1a
/// gives an ability one primary condition, and a sentence stating two is
/// ordinarily written as two abilities — which is right for Leela Patel, whose
/// sentence prints no ordinal, and wrong here: `AbilityDef::ordinal` belongs
/// to one ability, so a pair would each spend their own "first time each turn"
/// and a Runner who stole an agenda and trashed an asset in the same turn
/// would pay twice. One condition describing two kinds of occurrence, met by
/// either, is what the printed "or" says.
///
/// The trash half stipulates nothing but whose card it was and who trashed it,
/// so a card trashed on access counts as readily as an installed one; the
/// steal half is 1.17.7's steal. An agenda the Runner steals meets only the
/// steal half — 1.17.7 moves it to the score area and does not trash it — so
/// the two halves cannot both fire on one card.
///
/// "Hosted power counter" is 1.9.2's cost spent off the source, which is what
/// makes the paid ability unusable while the identity is empty rather than
/// free. "1 of those cards" names the cards the ability is looking at, which
/// is a zone specification (1.15.2c lifts for it), and stipulates nothing else
/// — an agenda among the three may be installed like anything else.
pub fn epiphany_analytica() -> Card {
    card("Epiphany Analytica: Nations Undivided")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("The first time each turn the Runner steals or trashes a Corp card, place 1 power counter on this identity.")
        .text("[click], hosted power counter: Look at the top 3 cards of R&D. You may install 1 of those cards.")
        .when_first_each_turn(
            either_of(&[runner_steals_agenda(), runner_trashes_a_corp_card()]),
            [place(CounterKind::Power, 1)],
        )
        .named("nations undivided")
        .paid(
            clicks(1).plus_cost(hosted_counters(CounterKind::Power, 1)),
            [
                look_at(top_of_rnd(amount(3)), Corp),
                may(install(
                    choose(1, &[looked_at_by_this_ability()]),
                    InstallDest::DeclaredByInstaller,
                )),
            ],
        )
        .named("look at the top 3 cards of R&D")
        .build()
}

/// SYNC: Everything, Everywhere — Identity: Division.
/// "[click]: Flip this identity.
///  The Runner pays 1[credit] more when spending a [click] to remove a tag
///  (not through a card ability)."
///
/// COMPLETE, both faces. The first line is Earth Station's paid ability: the
/// whole cost is the [click], and `Instruction::FlipIdentity` is
/// rule_identity_double_sided's turn-over — the 10.3.1a checkpoint after it
/// re-derives every ability from the face now showing, so the tax below ends
/// the moment the card turns.
///
/// The second line is `StaticDecl::BasicActionCostMod`: a modification
/// (1.16.2) of the credit part of 5.2.7g's basic remove-tag action, +1 for
/// the printed "pays 1[credit] more". The parenthetical "(not through a card
/// ability)" is the declaration's whole scope — 5.2.5a identifies actions by
/// the basic action they are, so naming it reaches every taking of that
/// action and no card ability, which is why the reader lives exactly where
/// the basic action counts and pays its credits.
pub fn sync_everything_everywhere() -> Card {
    card("SYNC: Everything, Everywhere")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("[click]: Flip this identity.")
        .text("The Runner pays 1[credit] more when spending a [click] to remove a tag (not through a card ability).")
        .paid(clicks(1), [flip_identity(Corp)])
        .named("sync: flip")
        .declares([remove_tag_basic_action_costs_more(1)])
        .named("sync: the tag tax")
        .flip_face(sync_everything_everywhere_flipped())
        .build()
}

/// SYNC: Everything, Everywhere — Identity: Division; the back face of the
/// same card (oracle: netrunner-cards-json v2, `faces[0]` — its `title` is
/// null, so the back keeps the front's name).
/// "[click]: Flip this identity.
///  You may pay 2[credit] fewer when spending a [click] to trash a resource
///  (not through a card ability)."
///
/// The same [click] on this side turns the card home. The second line is the
/// same `StaticDecl::BasicActionCostMod` about 5.2.6g's basic trash-resource
/// action, −2, floored at 0 by 1.16.2a; the parenthetical scopes it to the
/// basic action exactly as the front's does. The printed "may": the
/// reduction costs nothing, so declining it is never anything but a smaller
/// credit pool — the same affordable-anyway choice the kernel's install
/// payments already decline to offer (`Vm::install_payment`'s KERNEL
/// APPROXIMATION note), so the reduction applies of its own accord.
pub fn sync_everything_everywhere_flipped() -> Card {
    card("SYNC: Everything, Everywhere")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("[click]: Flip this identity.")
        .text("You may pay 2[credit] fewer when spending a [click] to trash a resource (not through a card ability).")
        .paid(clicks(1), [flip_identity(Corp)])
        .named("sync: flip home")
        .declares([trash_resource_basic_action_costs_less(2)])
        .named("sync: the trash discount")
        .build()
}

/// Every NBN identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        epiphany_analytica(),
        nbn_controlling_the_message(),
        nbn_making_news(),
        gamenet(),
        synapse_global(),
        near_earth_hub(),
        haarpsichord_studios(),
        harishchandra_ent(),
        new_angeles_sol(),
        information_dynamics(),
        editorial_division(),
        nbn_reality_plus(),
        nbn_the_world_is_yours(),
        pravdivost_consulting(),
        spark_agency(),
        acme_consulting(),
        sync_everything_everywhere(),
    ]
}

/// Acme Consulting: The Truth You Need — Identity: Subsidiary.
/// "The Runner is considered to have 1 additional tag (even if they have 0)
///  during encounters with the outermost piece of ice protecting any server."
///
/// COMPLETE. A declaration about the NUMBER of tags the Runner is considered
/// to have (10.5.2), not a tag: no counter is taken, nothing is recorded, and
/// the moment the encounter ends the number is what it was. Which readers see
/// the modified number is settled on `StaticDecl::ConsideredTagsMod`:
/// `Quantity::RunnerTags`, "the Runner is tagged" in every 9.6.5c requirement
/// (so Harishchandra Ent. opens the grip during such an encounter) and
/// 5.2.6g's trash-a-resource gate read it; 5.2.6e's remove-tag action and
/// every cost that removes a tag read the real count, because "(even if they
/// have 0)" means there may be nothing to remove.
///
/// "During encounters with…" is 9.3.7a's stated condition — the declaration
/// applies exactly while an encounter with a described ice is in progress —
/// and the description is two criteria about one card: the piece of ice being
/// encountered (6.5.1), which is also the outermost piece of ice protecting
/// its server (6.2.2's install position, read from the positions as they
/// stand). "Any server" is why the second criterion measures the ice against
/// its OWN server rather than a named one.
pub fn acme_consulting() -> Card {
    card("Acme Consulting: The Truth You Need")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Subsidiary])
        .text("The Runner is considered to have 1 additional tag (even if they have 0) during encounters with the outermost piece of ice protecting any server.")
        .declares_while(
            &[board_has(&[the_encountered_ice(), outermost_ice_of_its_server()], 1)],
            [considered_additional_tags(1)],
        )
        .named("the truth you need")
        .build()
}
