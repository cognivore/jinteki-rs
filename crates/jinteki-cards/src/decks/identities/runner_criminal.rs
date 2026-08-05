//! Runner — Criminal identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). Andromeda and Ken Tenma are the two Criminals that
//! already exist — they live in `decks/andromeda.rs`, because that deck plays
//! one and carries the other in its CR 1.5.4a pile — and everything else in
//! the faction lands here.

use crate::edsl::*;

/// 419: Amoral Scammer — Identity: Natural. Link 1.
/// "The first time the Corp installs a card each turn, you may expose that
///  card unless the Corp pays 1[credit]."
///
/// COMPLETE. 8.5's install with no stipulation about what was installed, and
/// 9.6.5c's ordinal about the occurrence.
///
/// "That card" is 1.15.4's back-reference to the card the OCCURRENCE named,
/// so nothing is announced — which is also what makes the sentence sayable
/// at all. 1.21.4 restricts exposing to installed UNREZZED cards, and a bare
/// "expose 1 card" would have to describe candidates, which is words the card
/// does not print; here the condition fixes the card and the restriction is
/// simply honoured when the exposure resolves.
///
/// The printed "you may" is the whole ability, so it is 9.6.9's declinable
/// conditional and the decision is the Runner's (9.1.1a); "unless the Corp
/// pays 1[credit]" is 1.16.9's alternative cost put to the player the
/// sentence names, so the Corp answers second and only if the Runner said
/// yes.
pub fn amoral_scammer() -> Card {
    card("419: Amoral Scammer")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .link(1)
        .text("The first time the Corp installs a card each turn, you may expose that card unless the Corp pays 1[credit].")
        .may_when_first_each_turn(
            installs_a_card(Corp),
            [unless_pays(Corp, credits(1), expose(the_triggering_card()))],
        )
        .named("the first corp install of the turn")
        .build()
}

/// Armand "Geist" Walker: Tech Lord — Identity: G-mod. Link 1.
/// "Whenever you use a [trash] ability, draw 1 card."
///
/// COMPLETE. The [trash] symbol is 1.19.4: a trigger cost that trashes the
/// ability's own source. That is NOT 7.1.5's basic trash ability, where the
/// Runner pays an accessed card's trash cost — two different abilities, and
/// the sentence names only the first, so the condition stipulates which.
pub fn armand_geist_walker() -> Card {
    card("Armand \"Geist\" Walker: Tech Lord")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .link(1)
        .text("Whenever you use a [trash] ability, draw 1 card.")
        .when(uses_a_trash_symbol_ability(Runner), [draw(Runner, 1)])
        .named("tech lord")
        .build()
}

/// Barry "Baz" Wong: Tri-Maf Veteran — Identity: Cyborg. Link 0.
/// "Whenever the Corp rezzes a piece of ice, you may install 1 resource or
///  piece of hardware from your grip."
///
/// COMPLETE. The condition is Los's — 8.1.2's rez with the sentence's
/// card-type stipulation — without the ordinal, so every ice rez offers it.
///
/// "1 resource **or** piece of hardware" is 2.15's type list as one
/// description word. It cannot be two: a card has exactly one type, so two
/// type words beside each other would mean a card that is both, which is no
/// card at all.
pub fn barry_baz_wong() -> Card {
    card("Barry \"Baz\" Wong: Tri-Maf Veteran")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .text("Whenever the Corp rezzes a piece of ice, you may install 1 resource or piece of hardware from your grip.")
        .may_when(
            corp_rezzes_a(CardType::Ice),
            [install(
                choose(
                    1,
                    &[
                        in_hand_of(Runner),
                        of_any_type(&[CardType::Resource, CardType::Hardware]),
                    ],
                ),
                InstallDest::RunnerChoiceHostOrRig,
            )],
        )
        .named("tri-maf veteran")
        .build()
}

/// Gabriel Santiago: Consummate Professional — Identity: Cyborg. Link 0.
/// "The first time you make a successful run on HQ each turn, gain 2[credit]."
///
/// COMPLETE. Two ordinary readings and nothing else: 6.8.4's "makes a
/// successful run", with the server the sentence names as the stipulation on
/// it, and 9.6.5c's stipulation about the OCCURRENCE for "the first time each
/// turn" — counted from the change log since the turn began, and deliberately
/// not 9.3.6g's once-per-turn flag (9.1.6: an entirely mandatory ability is
/// never *used*, so nothing would ever spend that flag).
pub fn gabriel_santiago() -> Card {
    card("Gabriel Santiago: Consummate Professional")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .text("The first time you make a successful run on HQ each turn, gain 2[credit].")
        .when_first_each_turn(makes_successful_run_on(&[ServerId::Hq]), [gain(Runner, 2)])
        .named("the first HQ run of the turn")
        .build()
}

/// Iain Stirling: Retired Spook — Identity: Natural. Link 1.
/// "When your turn begins, gain 2[credit] if the Corp has more scored agenda
///  points than you."
///
/// COMPLETE. The "if …" clause is 9.6.5c's additional requirement listed
/// inside the trigger condition, so it is asked as the turn begins and not
/// again while the ability resolves.
///
/// It compares the two SCORE AREAS (1.17.1) rather than testing one against a
/// printed number, and "more … than" is strict: a tie leaves the requirement
/// unmet and the identity pays nothing.
pub fn iain_stirling() -> Card {
    card("Iain Stirling: Retired Spook")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .link(1)
        .text("When your turn begins, gain 2[credit] if the Corp has more scored agenda points than you.")
        .when(turn_begins_if(Runner, &[agenda_points_ahead(Corp)]), [gain(Runner, 2)])
        .named("retired spook")
        .build()
}

/// Silhouette: Stealth Operative — Identity: Natural. Link 0.
/// "The first time you make a successful run on HQ each turn, you may expose
///  1 card."
///
/// COMPLETE. Gabriel Santiago's condition exactly, and the printed "you may"
/// is the whole ability, so 9.6.9 puts the decision with the Runner.
///
/// "1 card" describes nothing else, and it does not have to: CR 1.21.4
/// restricts exposing to installed cards that are not rezzed, and that
/// restriction is the INSTRUCTION's rather than the card's words, so it
/// narrows the candidates without being written here. Writing it into the
/// description would be words the card does not print.
pub fn silhouette() -> Card {
    card("Silhouette: Stealth Operative")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("The first time you make a successful run on HQ each turn, you may expose 1 card.")
        .may_when_first_each_turn(
            makes_successful_run_on(&[ServerId::Hq]),
            [expose(choose(1, &[]))],
        )
        .named("the first hq run of the turn")
        .build()
}

/// Los: Data Hijacker — Identity: G-mod. Link 0.
/// "The first time the Corp rezzes a piece of ice each turn, gain 2[credit]."
///
/// COMPLETE. The condition is 8.1.2's rez with the sentence's card-type
/// stipulation — the same one Lt. Todachine's "whenever you rez a piece of
/// ice" states, read here from the other side of the table, which is why the
/// condition names no player: only the Corp rezzes cards (8.1.1).
///
/// "The first time … each turn" is 9.6.5c's stipulation about the occurrence,
/// so the second rez of the turn does not meet the condition at all — the
/// ability is not throttled after firing, it simply never triggers again.
pub fn los() -> Card {
    card("Los: Data Hijacker")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .text("The first time the Corp rezzes a piece of ice each turn, gain 2[credit].")
        .when_first_each_turn(corp_rezzes_a(CardType::Ice), [gain(Runner, 2)])
        .named("the first ice rez of the turn")
        .build()
}

/// Liza Talking Thunder: Prominent Legislator — Identity: G-mod. Link 0.
/// "The first time you make a successful run on a central server each turn,
///  draw 2 cards and take 1 tag."
///
/// COMPLETE. "A central server" is CR 4.6.5, which names them and no others:
/// HQ, R&D and Archives. The set is fixed by the rule, so the sentence's
/// stipulation is that list of three.
///
/// "Draw 2 cards and take 1 tag" is written as TWO instructions rather than
/// one `combined([…])`: 9.11.3 splits a sentence into an instruction per
/// class, and 9.12.2c only aggregates effects of the SAME class. A draw and a
/// tag are not, so joining them would put two unrelated atoms in one
/// imminence and give a prevention effect one window for both.
pub fn liza_talking_thunder() -> Card {
    card("Liza Talking Thunder: Prominent Legislator")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .text("The first time you make a successful run on a central server each turn, draw 2 cards and take 1 tag.")
        .when_first_each_turn(
            makes_successful_run_on_a_central_server(),
            // 9.11.3: "draw 2 cards and take 1 tag" is one SENTENCE, so one
            // instruction — a tag-avoidance effect and a draw interrupt see
            // the same imminence, exactly as Snare!'s tag and damage do.
            // 9.12.2c is about aggregating a calculated quantity ("for
            // each"), not about whether a sentence splits.
            [combined([draw(Runner, 2), give_tags(1)])],
        )
        .named("the first central run of the turn")
        .build()
}

/// Laramy Fisk: Savvy Investor — Identity: Natural. Link 0.
/// "The first time you make a successful run on a central server each turn,
///  you may force the Corp to draw 1 card."
///
/// COMPLETE. The same condition Liza states, and the printed "you may" is the
/// WHOLE ability, so it is 9.6.9's declinable conditional — the decision goes
/// to the ability's controller, which 9.1.1a makes the Runner, since it is
/// the Runner's identity.
///
/// "Force the Corp to draw" needs no 1.14.5 attribution: §8.4's draw already
/// names the player who draws, and drawing offers that player no choices to
/// make, so naming the Corp in the instruction is the whole of it.
pub fn laramy_fisk() -> Card {
    card("Laramy Fisk: Savvy Investor")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("The first time you make a successful run on a central server each turn, you may force the Corp to draw 1 card.")
        .may_when_first_each_turn(makes_successful_run_on_a_central_server(), [draw(Corp, 1)])
        .named("the first central run of the turn")
        .build()
}

/// Leela Patel: Trained Pragmatist — Identity: Natural. Link 0.
/// "Whenever an agenda is scored or stolen, add 1 unrezzed card to HQ."
///
/// COMPLETE. One printed sentence with two conditions, so it is two
/// conditional abilities with the same effect (9.6.1: a card may have several)
/// — the same shape The Source's "trash this when an agenda is scored or
/// stolen" already takes. Writing it as one ability is not available: 9.6.1
/// gives an ability ONE primary condition, and an agenda being scored
/// (1.17.3a) and one being stolen (1.17.3b) are different occurrences.
///
/// "1 unrezzed card" is 8.1.2's other half — an installed facedown Corp card.
/// It reaches an installed agenda too, which can never be rezzed at all, and
/// it is not a stipulation about a CHARACTERISTIC (rez state is open
/// information, 1.12.1), so nothing is revealed by choosing one.
///
/// The choice belongs to the ability's controller, which 9.1.1a makes the
/// Runner for a Runner identity — even on the half that fires when the CORP
/// scores.
pub fn leela_patel() -> Card {
    card("Leela Patel: Trained Pragmatist")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("Whenever an agenda is scored or stolen, add 1 unrezzed card to HQ.")
        .when(corp_scores_agenda(), [add_to_hand(choose(1, &[unrezzed()]))])
        .named("an agenda was scored")
        .when(runner_steals_agenda(), [add_to_hand(choose(1, &[unrezzed()]))])
        .named("an agenda was stolen")
        .build()
}

/// Nyusha "Sable" Sintashta: Symphonic Prodigy — Identity: G-mod. Link 0.
/// "When your turn begins, identify your mark. (If you don’t have a mark, a
///  random central server becomes your mark for this turn.)
///  The first time each turn you make a successful run on your mark, gain
///  [click]."
///
/// COMPLETE. The parenthesis is 1.4's reminder text: it restates 10.11.2a's
/// method and 10.11.3's "if one already is, nothing happens", both of which
/// "identify your mark" already is, so it is not a second instruction.
///
/// The ordinal on the second sentence rides on the CONDITION rather than on
/// 9.6.5c's usual stipulation, because 10.11.5 counts it from a different
/// moment: a condition checking a property related to the mark only checks
/// from the designation, so a successful run on that same server earlier in
/// the turn — before it was the mark — is not one of the times counted.
pub fn nyusha_sintashta() -> Card {
    card("Nyusha \"Sable\" Sintashta: Symphonic Prodigy")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .text("When your turn begins, identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.)")
        .text("The first time each turn you make a successful run on your mark, gain [click].")
        .when(turn_begins(Runner), [identify_mark()])
        .named("identify your mark")
        .when(makes_successful_run_on_your_mark(true), [gain_clicks(Runner, 1)])
        .named("the first run on the mark")
        .build()
}

/// Virtual Intelligence, P.I.: "You Can Call Me Vic" — Identity: Digital.
/// Link 0.
/// "Once per turn → [click], 1[credit]: Draw 1 card and remove 1 tag."
///
/// COMPLETE. Everything printed before the colon is the cost — a click and a
/// credit, one cost with two components (1.16.2) — and "Once per turn →" is
/// 9.3.6g's flag, which a PAID ability has something to spend it with (9.1.6:
/// a player uses a paid ability, which is what expends the flag).
///
/// "Draw 1 card and remove 1 tag" is one printed sentence, so one instruction
/// (9.11.3): joining them with `combined` keeps the single checkpoint and the
/// single interrupt window the card gives. The ability is usable with no tags
/// to remove — 9.5.3 only asks that the cost be payable.
pub fn virtual_intelligence_pi() -> Card {
    card("Virtual Intelligence, P.I.: \"You Can Call Me Vic\"")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Digital"])
        .text("Once per turn → [click], 1[credit]: Draw 1 card and remove 1 tag.")
        .paid_once_per_turn(
            clicks(1).plus_cost(credits(1)),
            [combined([draw(Runner, 1), remove_tags(1)])],
        )
        .named("draw 1 card and remove 1 tag")
        .build()
}

/// Mercury: Chrome Libertador — Identity: Bioroid. Link 0.
/// "Once per turn → When you breach HQ or R&D during a run, if you did not
///  break any subroutines during that run, you may access 1 additional card."
///
/// COMPLETE. "HQ or R&D" is ONE condition with two servers in it, not two
/// abilities: "Once per turn →" is 9.3.6g's flag stated once, and two
/// abilities would each carry their own copy of it and let the identity fire
/// twice in a turn. That is the same trap Leela Patel's "scored or stolen"
/// avoids only because a score and a steal really are different occurrences —
/// a breach of HQ and a breach of R&D are the same occurrence with a
/// different server, so the server is content on the one condition.
///
/// The flag has something to spend it with because the ability is OPTIONAL:
/// 9.1.6's second sentence says a player does not *use* an entirely mandatory
/// ability, so a mandatory conditional written with the flag would never
/// expend it at all.
///
/// Both "during a run" and "if you did not break any subroutines during that
/// run" are 9.6.5c requirements listed inside the condition, checked when the
/// breach begins. The first is 7.2's point: a breach can happen without a run
/// (an Ashen Epilogue-class effect), and this sentence excludes those. The
/// second reviews the run's history for a break (9.8.7) — "did not break
/// ANY" is a count with the threshold at 0.
pub fn mercury() -> Card {
    card("Mercury: Chrome Libertador")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Bioroid"])
        .text("Once per turn → When you breach HQ or R&D during a run, if you did not break any subroutines during that run, you may access 1 additional card.")
        .may_when_once_per_turn(
            breaches_one_of_if(
                &[ServerId::Hq, ServerId::Rnd],
                &[during_a_run(), at_most(subroutines_broken_this_run(), 0)],
            ),
            [additional_accesses(1)],
        )
        .named("chrome libertador")
        .build()
}

/// MuslihaT: Multifarious Marketeer — Identity: Natural. Link 0.
/// "When your turn begins, look at the top card of your stack. If that card
///  is an icebreaker or a run event, you may reveal it and add it to your
///  grip."
///
/// COMPLETE. Two printed sentences, so two instructions — and here 9.11.4e
/// says so twice over, since looking and revealing are each one of its
/// exceptions.
///
/// "An **icebreaker** or a **run** event" is a printed "or" between two whole
/// descriptions rather than between two words of one kind: an icebreaker is
/// named by its subtype alone, a run event by a type AND a subtype. Written
/// as one flat list the two would become a conjunction and describe no card
/// at all, so the alternation is said as one criterion with two alternatives.
///
/// "That card" and "it" are 1.15.4's back-reference to the card the look
/// named, which 1.12.3 stamps for the resolving ability — nothing is
/// announced, because "the top card of your stack" already named a zone and
/// fixed the card (1.15.2c). That is also why the reveal is honest: 1.21.3
/// shows the Corp a card the Runner has already seen and nobody else had.
pub fn muslihat() -> Card {
    let an_icebreaker_or_a_run_event = || {
        any_of(&[
            &[with_subtype("Icebreaker")],
            &[of_type(CardType::Event), with_subtype("Run")],
        ])
    };
    card("MuslihaT: Multifarious Marketeer")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("When your turn begins, look at the top card of your stack. If that card is an icebreaker or a run event, you may reveal it and add it to your grip.")
        .when(
            turn_begins(Runner),
            [
                look_at(top_of_stack(amount(1)), Runner),
                if_met(
                    &[board_has(
                        &[looked_at_by_this_ability(), an_icebreaker_or_a_run_event()],
                        1,
                    )],
                    // 9.11.3: "reveal it and add it to your grip" is one
                    // sentence, so one instruction.
                    [may(combined([
                        reveal(all_matching(&[looked_at_by_this_ability()])),
                        add_to_hand(all_matching(&[looked_at_by_this_ability()])),
                    ]))],
                ),
            ],
        )
        .named("multifarious marketeer")
        .build()
}

/// Zahya Sadeghi: Versatile Smuggler — Identity: Cyborg. Link 0.
/// "Once per turn → When a run on HQ or R&D ends, you may gain 1[credit] for
///  each time you accessed a card during that run."
///
/// COMPLETE. The condition is 6.9.6's end of the run with the servers the
/// sentence names as content on it — one condition, for the reason Mercury's
/// is one: the printed "Once per turn →" is 9.3.6g's flag stated once, and a
/// pair of abilities would carry a flag each.
///
/// "For each time you accessed a card during that run" spans the whole run
/// rather than one breach: 7.3.6 counts the accesses PERFORMED, so a run that
/// breached twice counts both breaches' accesses, and an access replaced by
/// another effect never happened and is not counted.
///
/// The CR uses this card as its own example of 9.3.6g (`example_rule_once_
/// per_turn_flag_1`): the ability being declined does not spend the flag,
/// because 9.1.6 makes using it the thing that does.
pub fn zahya_sadeghi() -> Card {
    card("Zahya Sadeghi: Versatile Smuggler")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .text("Once per turn → When a run on HQ or R&D ends, you may gain 1[credit] for each time you accessed a card during that run.")
        .may_when_once_per_turn(
            run_on_ends(&[ServerId::Hq, ServerId::Rnd]),
            [gain_q(Runner, times(1, accesses_this_run()))],
        )
        .named("versatile smuggler")
        .build()
}

/// Az McCaffrey: Mechanical Prodigy — Identity: Cyborg. Link 1.
/// "The first job resource, connection resource, or piece of hardware you
///  install each turn costs 1[credit] less to install."
///
/// COMPLETE. A DECLARATION, not an ability that resolves: 9.3.5 applies it
/// continuously, and it is read wherever an install cost is calculated — by
/// 8.7.2b's affordability query as much as by 8.5.16d's payment, which is
/// what lets it make an otherwise unaffordable card installable.
///
/// The reduction is automatic. Nothing is paid for it and nothing is chosen,
/// which is the whole difference from Patchwork's 1.16.6 reduction: that one
/// is only available while its own cost is payable and the installer must
/// decide to use it.
///
/// "Job resource, connection resource, or piece of hardware" is one
/// description with three alternatives — 2.15's type AND 2.16's subtype for
/// the first two, a type alone for the third — so it is the printed "or"
/// between whole descriptions and not between single words.
///
/// "The first … each turn" is the same 9.6.5c ordinal a trigger condition
/// states, read here of the install: the declaration reaches the install only
/// while no earlier matching one has happened this turn. "You install" needs
/// no words of its own — 2.15 partitions the card types by side, and
/// resources and hardware are the Runner's.
pub fn az_mccaffrey() -> Card {
    card("Az McCaffrey: Mechanical Prodigy")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .link(1)
        .text("The first job resource, connection resource, or piece of hardware you install each turn costs 1[credit] less to install.")
        .declares([first_installed_each_turn_costs_less(
            &[any_of(&[
                &[of_type(CardType::Resource), with_subtype("Job")],
                &[of_type(CardType::Resource), with_subtype("Connection")],
                &[of_type(CardType::Hardware)],
            ])],
            1,
        )])
        .build()
}

/// Khan: Savvy Skiptracer — Identity: Natural. Link 0.
/// "The first time you pass a piece of ice each turn, you may install an
///  icebreaker from your hand, lowering the install cost by 1."
///
/// COMPLETE. The condition is run step 6.9.4a's pass with NO stipulation
/// about it — not this card's ice, not one fully broken, not one whose
/// subroutines resolved — and 9.6.5c's ordinal about the occurrence. A piece
/// of ice bypassed without an encounter is still passed, and still meets it.
///
/// "Lowering the install cost by 1" is 1.16.6's reduction stated by the
/// installing ability itself, so it needs no declaration and nothing is paid
/// for it; 1.16.2a floors the lowered cost at 0. 9.11.4b is why the install
/// is its own instruction.
///
/// "From your hand" names a zone, which is what lifts 1.15.2c's installed-
/// cards default, and "an icebreaker" is 2.16's subtype.
pub fn khan() -> Card {
    card("Khan: Savvy Skiptracer")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("The first time you pass a piece of ice each turn, you may install an icebreaker from your hand, lowering the install cost by 1.")
        .may_when_first_each_turn(
            passes_any_ice(),
            [install_paying_less(
                choose(1, &[in_hand_of(Runner), with_subtype("Icebreaker")]),
                InstallDest::RunnerChoiceHostOrRig,
                1,
            )],
        )
        .named("savvy skiptracer")
        .build()
}

/// Boris "Syfr" Kovac: Crafty Veteran — Identity: Cyborg. Link 0.
/// "Draft format only.
///  If you have more [criminal] cards installed than any other faction, when
///  your turn begins, remove 1 tag."
///
/// COMPLETE. Two printed lines and one ability: the first is a FORMAT
/// restriction, settled before deck construction and never read during play
/// (The Masque's whole card is that sentence), and the second is one
/// conditional ability whose leading "if" is 9.6.5c's additional requirement
/// listed inside the trigger condition — so it is asked when the turn begins
/// and not again while the ability resolves.
///
/// The requirement is a comparison across the FACTION PARTITION of the
/// Runner's installed cards (2.13), which is why no threshold word can say
/// it: the sentence prints no number, and what it wants is one group being
/// strictly larger than every other. "Installed" is the ordinary description
/// word, so the described set is the play area and nothing else — cards in
/// the grip, the heap and the stack have no faction group here.
///
/// The removal is MANDATORY and the sentence says "1 tag", so a Runner with
/// none simply removes as much as possible (1.15.3) and nothing happens.
pub fn boris_syfr_kovac() -> Card {
    card("Boris \"Syfr\" Kovac: Crafty Veteran")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .text("Draft format only.")
        .text("If you have more [criminal] cards installed than any other faction, when your turn begins, remove 1 tag.")
        .when(
            turn_begins_if(
                Runner,
                &[more_cards_of_this_faction_than_any_other(
                    "Criminal",
                    &[installed_runner_card()],
                )],
            ),
            [remove_tags(1)],
        )
        .named("crafty veteran")
        .build()
}

/// Nero Severn: Information Broker — Identity: Natural. Link 1.
/// "Once per turn → When you encounter a sentry, you may jack out."
///
/// COMPLETE. One conditional ability with 9.3.6g's once-per-turn flag, which
/// an optional ability has something to spend it with (9.1.6: an entirely
/// mandatory ability is never used, so the flag would never come off one).
/// The "may" is the whole ability, not a component of one sentence.
///
/// The condition is 6.9.3's encounter with the sentence's subtype stipulation
/// (2.16) riding on it as content — the same condition Paperclip states about
/// a barrier.
///
/// "Jack out" is 6.1.5's PROCESS, and the card is why it is an instruction at
/// all: 6.1.5b opens the opportunity after passing a piece of ice, which is
/// nowhere near an encounter, so an ability that offers the choice has to be
/// able to say the process itself. 6.1.5 says it "follows the usual process
/// for ending the run", so the run ends here exactly as the step's yes-branch
/// ends it — and the encounter, being inside the run, ends with it (6.1.4b).
pub fn nero_severn() -> Card {
    card("Nero Severn: Information Broker")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .link(1)
        .text("Once per turn → When you encounter a sentry, you may jack out.")
        .may_when_once_per_turn(encounters_a("Sentry", &[]), [jack_out()])
        .named("information broker")
        .build()
}

/// Every Criminal identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        boris_syfr_kovac(),
        nero_severn(),
        amoral_scammer(),
        az_mccaffrey(),
        khan(),
        armand_geist_walker(),
        barry_baz_wong(),
        gabriel_santiago(),
        iain_stirling(),
        silhouette(),
        los(),
        liza_talking_thunder(),
        laramy_fisk(),
        leela_patel(),
        nyusha_sintashta(),
        virtual_intelligence_pi(),
        mercury(),
        muslihat(),
        zahya_sadeghi(),
    ]
}
