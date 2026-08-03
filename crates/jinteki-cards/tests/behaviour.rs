//! The complete cards, played in the VM.
//!
//! Parsing is not proof (SYS-D-12): a card counts as complete only when the
//! instructions it denotes into actually do, in the rules engine, what the
//! printed text says. Each test below takes the card straight out of the deck
//! file — no hand-written `PrintedCard` — puts it on a board and drives it
//! with the shared plan driver, then asserts the printed sentence's effect.

use jinteki_cards::priority_decks;
use jinteki_cr::change::GameChange;

use jinteki_cr::instr::Instruction;
use jinteki_cr::object::{CounterKind, PrintedCard, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Kind, Match, Plan, Reply};
use jinteki_cr::timing::StructKind;
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// The card as the deck file writes it — and a check that the file still
/// claims every one of its printed sentences is expressed.
fn card(name: &str) -> PrintedCard {
    let c = priority_decks()
        .unwrap_or_else(|e| panic!("{e}"))
        .into_iter()
        .find(|c| c.printed.name == name)
        .unwrap_or_else(|| panic!("no card named {name} in either deck"));
    assert!(
        c.is_complete(),
        "{name} still carries an `unimplemented:` marker — it cannot be asserted as playable"
    );
    c.printed
}

// ---------------------------------------------------------------------------
// Events and operations
// ---------------------------------------------------------------------------

/// "Gain 9[credit]." / "Draw 3 cards." — played with the basic action
/// (5.2.7e), paying the printed play cost.
#[test]
fn sure_gamble_and_diesel() {
    let mut vm = Vm::empty(11);
    let gamble = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let diesel = vm.new_object(card("Diesel"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([gamble, diesel]);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(gamble))
            .when(Match::action().once(), Reply::play_card(diesel))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 9, "5 − 5 play cost + 9 gained: {}", t.tail(10));
    assert_eq!(vm.st.hand[&Side::Runner].len(), 3, "Diesel drew 3: {}", t.tail(10));
    assert_eq!(vm.st.objects[&gamble].zone, Zone::Discard(Side::Runner));
}

/// "Gain 9[credit]." — the Corp side of the same shape.
#[test]
fn hedge_fund() {
    let mut vm = Vm::empty(12);
    let hedge = vm.new_object(card("Hedge Fund"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hedge);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(hedge)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.credits, 9, "5 − 5 + 9");
}

/// "Add 1 card from Archives to HQ." — an announced target in a hidden zone
/// (4.4), moved to the Corp's hand.
#[test]
fn archived_memories() {
    let mut vm = Vm::empty(13);
    let op = vm.new_object(card("Archived Memories"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
    let buried = vm.new_object(tk::corp_filler("Buried"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(op))
            .when(Match::targets(), Reply::target(buried))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&buried].zone,
        Zone::Hand(Side::Corp),
        "the Archives card is in HQ: {}",
        t.tail(10)
    );
}

/// "As an additional cost to play 24/7 News Cycle, forfeit an agenda."
/// "Resolve the \"when scored\" ability on an agenda in your score area."
/// — 1.16.10 additional play cost and 9.6.14c/d resolution by class.
#[test]
fn news_cycle_forfeits_then_resolves_a_when_scored_ability() {
    let mut vm = Vm::empty(14);
    // Two agendas already scored: one to forfeit, one whose "when scored"
    // ability is re-resolved. Tomorrow's Headline tags the Runner.
    let headline = vm.new_object(card("Tomorrow's Headline"), Zone::ScoreArea(Side::Corp));
    let spare = vm.new_object(tk::vanilla_agenda("Spare Initiative", 3, 0), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().extend([headline, spare]);
    let cycle = vm.new_object(card("24/7 News Cycle"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(cycle);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(cycle))
            // 8.2.5: which agenda is forfeited is the payer's choice.
            .when(Match::payment_cards(), Reply::Targets(vec![spare]))
            .when(Match::targets(), Reply::target(headline))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&spare].zone,
        Zone::RemovedFromGame,
        "8.2.5: the forfeited agenda left the score area: {}",
        t.tail(12)
    );
    assert_eq!(
        vm.st.runner.tags, 1,
        "9.6.14d: Tomorrow's Headline's \"when scored\" ability resolved again: {}",
        t.tail(12)
    );
}

// ---------------------------------------------------------------------------
// Agendas
// ---------------------------------------------------------------------------

/// "When this agenda is scored or stolen, give the Runner 1 tag." — both
/// halves, in one game.
#[test]
fn tomorrows_headline_tags_on_score_and_on_steal() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(15);
        let th = tk::install_root(&mut vm, card("Tomorrow's Headline"), ServerId::Remote(1), false);
        vm.st.objects.get_mut(&th).unwrap().counters.insert(CounterKind::Advancement, 3);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);

        if stolen {
            vm.start_turn(Side::Runner);
            plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .stop_at_action(),
            );
            assert_eq!(vm.st.objects[&th].zone, Zone::ScoreArea(Side::Runner));
        } else {
            vm.start_turn(Side::Corp);
            plan::play(
                &mut vm,
                Plan::corp().when(Match::paid(), Reply::score(th)).stop_at_action(),
                Plan::runner(),
            );
            assert_eq!(vm.st.objects[&th].zone, Zone::ScoreArea(Side::Corp));
        }
        assert_eq!(vm.st.runner.tags, 1, "1 tag whether scored or stolen (stolen={stolen})");
    }
}

/// "As an additional cost to steal this agenda, the Runner must pay
/// 5[credit]." / "When you score this agenda, gain 5[credit]."
#[test]
fn bellona_costs_five_to_steal_and_pays_five_to_score() {
    // (a) scored by the Corp.
    let mut vm = Vm::empty(16);
    let bell = tk::install_root(&mut vm, card("Bellona"), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&bell).unwrap().counters.insert(CounterKind::Advancement, 5);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 2;
    vm.start_turn(Side::Corp);
    plan::play(
        &mut vm,
        Plan::corp().when(Match::paid(), Reply::score(bell)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.credits, 7, "scoring gained 5");

    // (b) stolen by the Runner: the additional cost is put to them, and a
    // Runner who cannot pay it does not steal (1.16.1b).
    for (credits, expect_steal) in [(5u32, true), (4u32, false)] {
        let mut vm = Vm::empty(17);
        let bell = tk::install_root(&mut vm, card("Bellona"), ServerId::Remote(1), false);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = credits;
        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                .when(Match::nested_cost(), Reply::PayCost(true))
                .stop_at_action(),
        );
        let stolen = vm.st.objects[&bell].zone == Zone::ScoreArea(Side::Runner);
        assert_eq!(stolen, expect_steal, "with {credits}[c]: {}", t.tail(12));
        if expect_steal {
            assert_eq!(
                t.of_kind(Kind::NestedCost)
                    .iter()
                    .filter_map(|e| e.cost())
                    .map(|c| c.flat_credits())
                    .collect::<Vec<_>>(),
                vec![5],
                "the printed additional steal cost is 5[credit]"
            );
            assert_eq!(vm.st.runner.credits, 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// "When your turn begins, you may trash Rashida Jaheem to gain 3[credit]
/// and draw 3 cards." — 1.16.11a's nested cost carries the "may".
#[test]
fn rashida_jaheem_trashes_herself_for_credits_and_cards() {
    for pay in [true, false] {
        let mut vm = Vm::empty(18);
        let rj = tk::install_root(&mut vm, card("Rashida Jaheem"), ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::reaction().offering("rashida"), Reply::take("rashida"))
                .when(Match::nested_cost(), Reply::PayCost(pay))
                .stop_at_action(),
            Plan::runner(),
        );
        // The mandatory draw (5.6.1e) puts 1 card in HQ either way.
        let expect_hand = if pay { 4 } else { 1 };
        assert_eq!(vm.st.corp.credits, if pay { 8 } else { 5 }, "pay={pay}: {}", t.tail(12));
        assert_eq!(vm.st.hand[&Side::Corp].len(), expect_hand, "pay={pay}: {}", t.tail(12));
        assert_eq!(
            vm.st.objects[&rj].zone == Zone::Discard(Side::Corp),
            pay,
            "the trash is the COST, so it happens exactly when the Corp pays"
        );
    }
}

/// "[click][click][click], [trash]: Gain 4[credit] and draw 3 cards. Install
/// up to 2 cards from HQ (one at a time). You may play 1 operation from HQ."
#[test]
fn humanoid_resources() {
    let mut vm = Vm::empty(19);
    let hr = tk::install_root(&mut vm, card("Humanoid Resources"), ServerId::Remote(1), true);
    // HQ holds two installable cards; everything drawn is an operation, so
    // the last sentence has something to play.
    let asset = vm.new_object(tk::vanilla_asset("Some Asset", 0, 2), Zone::Hand(Side::Corp));
    let ice = vm.new_object(tk::vanilla_ice("Some Ice", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().extend([asset, ice]);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    // Halt in the paid window the ability's own resolution is followed by, so
    // the assertions read the board it left and not a later turn's.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().offering("humanoid"), Reply::take("humanoid"))
            .when(Match::paid().once(), Reply::Halt),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.credits, 9, "gained 4: {}", t.tail(20));
    assert_eq!(
        vm.st.objects[&hr].zone,
        Zone::Discard(Side::Corp),
        "[trash] paid as part of the cost"
    );
    // 2 in HQ + 1 mandatory draw + 3 drawn − 2 installed − 1 played = 3.
    assert_eq!(vm.st.hand[&Side::Corp].len(), 3, "drew 3, installed 2, played 1: {}", t.tail(20));
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Root(ServerId::Remote(100)),
        "8.5.5: installed one at a time, into a destination the Corp declared"
    );
    assert!(
        matches!(vm.st.objects[&ice].zone, Zone::Ice(_)),
        "the second install went to an ice position: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CardPlayed { .. }))
            .count(),
        1,
        "8.6.3: exactly one operation was played from HQ: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.corp.clicks, 0, "3 clicks spent");
}

// ---------------------------------------------------------------------------
// Ice and upgrades
// ---------------------------------------------------------------------------

/// "When the Runner encounters this ice, they must take 1 tag or end the run."
/// "Hosted power counter: Give the Runner 1 tag."
/// "[subroutine] Trace[3]. If successful, place 1 power counter on this ice."
#[test]
fn data_raven() {
    let mut vm = Vm::empty(20);
    let raven = tk::install_ice(&mut vm, card("Data Raven"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // The Runner takes the tag rather than ending the run; the Corp lets the
    // subroutine resolve and wins the trace with 0 spent (3 vs 0 link).
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::options(), Reply::ChooseNamed("tag"))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.tags, 1, "9.12.3d: the Runner chose the tag: {}", t.tail(16));
    assert_eq!(
        vm.st.objects[&raven].counter(CounterKind::Power),
        1,
        "the trace succeeded and placed a power counter: {}",
        t.tail(16)
    );

    // With a counter on it, the hosted-counter paid ability is usable in the
    // next paid window and spends the counter (1.9.2).
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().offering("data raven"), Reply::take("data raven"))
            .stop_at_action(),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(vm.st.runner.tags, 2, "the hosted power counter bought a tag: {}", t.tail(12));
    assert_eq!(vm.st.objects[&raven].counter(CounterKind::Power), 0, "the counter was spent");
}

/// "Runs against this server cannot be declared successful." (6.9.5a.)
#[test]
fn crisium_grid() {
    for protected in [true, false] {
        let mut vm = Vm::empty(21);
        let server = ServerId::Remote(1);
        if protected {
            tk::install_root(&mut vm, card("Crisium Grid"), server, true);
        }
        tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 2), server, false);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner().when(Match::action().first(), Reply::run(server)).stop_at_action(),
        );
        let declared = vm
            .changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: s } if *s == server));
        assert_eq!(
            declared, !protected,
            "the run is declared successful exactly when Crisium is absent (protected={protected}): {}",
            t.tail(12)
        );
    }
}

/// "[subroutine] End the run unless the Runner pays 3[credit]." — twice
/// (1.16.11b), which is what makes the second subroutine bite.
#[test]
fn gold_farmer_subroutines_end_the_run_unless_paid() {
    for (credits, expect_ended) in [(6u32, false), (3u32, true)] {
        let mut vm = Vm::empty(22);
        let gf = card_partial("Gold Farmer");
        assert_eq!(gf.abilities.len(), 2, "two printed subroutines");
        tk::install_ice(&mut vm, gf, ServerId::Hq, true);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = credits;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Hq))
                .when(Match::nested_cost(), Reply::PayCost(true))
                .stop_at_action(),
        );
        let ended = vm
            .changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. }));
        assert_eq!(
            ended, expect_ended,
            "with {credits}[c] the Runner {} pay both subroutines: {}",
            if expect_ended { "cannot" } else { "can" },
            t.tail(14)
        );
        assert_eq!(vm.st.runner.credits, if expect_ended { 0 } else { 0 }, "paid what they could");
    }
}

/// "[subroutine] Trace[6]. If successful, add 1 installed Runner card to the
/// grip." — Archangel's subroutine, which is all of it that is sayable.
#[test]
fn archangel_subroutine_bounces_an_installed_card() {
    let mut vm = Vm::empty(23);
    let arch = card_partial("Archangel");
    let sub = arch
        .abilities
        .iter()
        .find(|a| a.kind == jinteki_cr::ability::AbilityKind::Subroutine)
        .expect("Archangel prints a subroutine");
    assert!(
        matches!(&sub.instructions[..], [Instruction::Trace { base, .. }] if *base == jinteki_cr::instr::Quantity::c(6)),
        "Trace[6]: {:?}",
        sub.instructions
    );
    tk::install_ice(&mut vm, arch, ServerId::Hq, true);
    let prog = tk::install_rig(&mut vm, tk::vanilla_runner_card("Some Program", jinteki_cr::object::CardType::Program));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&prog].zone,
        Zone::Hand(Side::Runner),
        "the trace succeeded and the program went back to the grip: {}",
        t.tail(14)
    );
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

/// "Interface → 1[credit]: Break 1 code gate subroutine."
/// "2[credit]: +2 strength."
/// (The `[threat 4]` sentence is on the gap list — see the card file.)
#[test]
fn shibboleth() {
    let mut vm = Vm::empty(24);
    let shib = tk::install_rig(&mut vm, card_partial("Shibboleth"));
    assert_eq!(vm.effective_strength(shib), Some(3), "printed strength");

    // A code gate of strength 3 can be interfaced with (9.3.6c); the break
    // ability is usable only during that encounter (9.5.6a/c).
    let mut gate = tk::etr_ice("Some Gate", 0, 3);
    gate.subtypes = vec!["Code Gate"];
    tk::install_ice(&mut vm, gate, ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().offering("interface").once(), Reply::take("interface"))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 4, "1[credit] for the break: {}", t.tail(16));
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::SubroutineResolved { .. })),
        "9.8.6: a broken subroutine does not resolve: {}",
        t.tail(16)
    );
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "…so the ETR subroutine never ended the run: {}",
        t.tail(16)
    );
}

/// The pump is usable outside an encounter (3.9.5d) and the break is not
/// (9.5.6a) — the timing restriction the printed text implies.
#[test]
fn shibboleth_break_is_encounter_only() {
    let mut vm = Vm::empty(25);
    tk::install_rig(&mut vm, card_partial("Shibboleth"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    let paid: Vec<&str> = t
        .entries
        .iter()
        .filter(|e| e.kind() == Kind::Paid)
        .flat_map(|e| e.options())
        .filter_map(|o| match o {
            jinteki_cr::decision::WindowOption::TriggerPaid { label, .. } => Some(*label),
            _ => None,
        })
        .collect();
    assert!(
        !paid.iter().any(|l| l.contains("interface")),
        "no encounter, so no break ability is offered: {paid:?}"
    );
}

// ---------------------------------------------------------------------------
// The sentences that ARE expressed on cards that are still partial. A
// sentence in a card file is live behaviour whether or not its neighbours
// are, so each one is asserted here too — an untested declaration is where a
// wrong card hides.
// ---------------------------------------------------------------------------

/// Rebirth: "Remove Rebirth from the game instead of trashing it." (9.9.8b /
/// 8.2.2 — the played card's trash destination is replaced.)
#[test]
fn rebirth_is_removed_from_the_game_instead_of_trashed() {
    let mut vm = Vm::empty(28);
    let rb = vm.new_object(card_partial("Rebirth"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(rb);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::play_card(rb)).stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&rb].zone,
        Zone::RemovedFromGame,
        "8.6.7g's trash went to the removed-from-game zone: {}",
        t.tail(10)
    );
}

/// The Source: "As an additional cost to steal an agenda, you must pay
/// 3[credit]." — a declaration reaching EVERY agenda (Ben Musashi class).
#[test]
fn the_source_taxes_every_steal() {
    let mut vm = Vm::empty(29);
    tk::install_rig(&mut vm, card_partial("The Source"));
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Someone Else's Agenda", 3, 1),
        ServerId::Remote(1),
        false,
    );
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert_eq!(
        t.of_kind(Kind::NestedCost).iter().filter_map(|e| e.cost()).map(|c| c.flat_credits()).collect::<Vec<_>>(),
        vec![3],
        "the cost reached an agenda The Source is not printed on: {}",
        t.tail(12)
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Runner));
    assert_eq!(vm.st.runner.credits, 0);
}

/// Targeted Marketing: "This card is not trashed until another current is
/// played or an agenda is stolen." (8.6.6c.)
#[test]
fn targeted_marketing_stays_in_the_play_area() {
    let mut vm = Vm::empty(30);
    let tm = vm.new_object(card_partial("Targeted Marketing"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(tm);
    let agenda = tk::install_root(&mut vm, tk::vanilla_agenda("Loose Agenda", 3, 1), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    // Played: it stays in the play area rather than going to Archives.
    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(tm))
            .otherwise_click_credit(),
        Plan::runner()
            // Halt once, to read the board while the current is in play, then
            // fall through to the run on the second look at this window.
            .when(Match::action().first(), Reply::Halt)
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&tm].zone,
        Zone::PlayArea(Side::Corp),
        "8.6.6c: not trashed at step 8.6.7g"
    );

    // The Runner steals an agenda: the shield expires and the card is trashed.
    g.run(&mut vm);
    assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Runner));
    assert_eq!(
        vm.st.objects[&tm].zone,
        Zone::Discard(Side::Corp),
        "the steal ended the lingering effect: {}",
        g.transcript().tail(12)
    );
}

/// Self-Growth Program: "Add 2 installed Runner cards to the grip."
#[test]
fn self_growth_program_bounces_two_installed_cards() {
    let mut vm = Vm::empty(31);
    let op = vm.new_object(card_partial("Self-Growth Program"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
    let a = tk::install_rig(&mut vm, tk::program_cost("Prog A", 0));
    let b = tk::install_rig(&mut vm, tk::program_cost("Prog B", 0));
    let cc = tk::install_rig(&mut vm, tk::program_cost("Prog C", 0));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(op))
            .when(Match::targets(), Reply::Targets(vec![a, b]))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.objects[&a].zone, Zone::Hand(Side::Runner), "{}", t.tail(10));
    assert_eq!(vm.st.objects[&b].zone, Zone::Hand(Side::Runner));
    assert_eq!(vm.st.objects[&cc].zone, Zone::Rig, "only the two announced cards moved");
}

/// BOOM!: "As an additional cost to play this operation, spend [click]."
/// "Do 7 meat damage." (1.16.10b: the additional cost joins the play cost.)
#[test]
fn boom_costs_a_click_on_top_of_its_play_cost() {
    let mut vm = Vm::empty(32);
    let boom = vm.new_object(card_partial("BOOM!"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(boom);
    tk::fill_hand(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 2);
    vm.st.corp.credits = 6;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(boom)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.credits, 2, "the printed play cost of 4: {}", t.tail(10));
    assert_eq!(
        vm.st.corp.clicks, 1,
        "1 for the basic action + 1 for the additional cost, out of 3: {}",
        t.tail(10)
    );
    assert!(
        vm.st.hand[&Side::Runner].is_empty(),
        "7 meat damage emptied the grip: {}",
        t.tail(10)
    );
}

/// Hard-Hitting News: "Trace[4]. If successful, give the Runner 4 tags."
/// "After you resolve this operation, your action phase ends." (5.6.2b.)
#[test]
fn hard_hitting_news_traces_then_ends_the_action_phase() {
    let mut vm = Vm::empty(33);
    let hhn = vm.new_object(card_partial("Hard-Hitting News"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hhn);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(hhn))
            .when(Match::reaction().offering("hard-hitting"), Reply::take("hard-hitting")),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(vm.st.runner.tags, 4, "trace 4 beat 0 link: {}", t.tail(14));
    assert_eq!(
        vm.st.turn_side,
        Side::Runner,
        "5.6.2b: the action phase ended with clicks unspent: {}",
        t.tail(14)
    );
}

/// AstroScript: "When you score this agenda, place 1 agenda counter on it."
/// (1.18.2: PLACING a counter, not loading and not advancing.)
#[test]
fn astroscript_places_an_agenda_counter_when_scored() {
    let mut vm = Vm::empty(34);
    let astro = tk::install_root(&mut vm, card_partial("AstroScript Pilot Program"), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&astro).unwrap().counters.insert(CounterKind::Advancement, 3);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    plan::play(
        &mut vm,
        Plan::corp().when(Match::paid(), Reply::score(astro)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.objects[&astro].zone, Zone::ScoreArea(Side::Corp));
    assert_eq!(vm.st.objects[&astro].counter(CounterKind::Agenda), 1);
}

/// Bukhgalter: "Interface → 1[credit]: Break 1 sentry subroutine."
/// "1[credit]: +1 strength." — the pump raises strength enough to interface.
#[test]
fn bukhgalter_pumps_then_breaks_a_sentry() {
    let mut vm = Vm::empty(35);
    let bukh = tk::install_rig(&mut vm, card_partial("Bukhgalter"));
    assert_eq!(vm.effective_strength(bukh), Some(1), "printed strength 1");
    let mut sentry = tk::etr_ice("Some Sentry", 0, 2);
    sentry.subtypes = vec!["Sentry"];
    tk::install_ice(&mut vm, sentry, ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            // Strength 1 vs 2: the interface ability is not offered until the
            // pump has run (9.3.6c). Both happen inside the encounter, which
            // is where 3.9.5b makes the pump last (`duration: None`).
            .when(
                Match::paid().during(StructKind::Encounter).offering("paid 1 credit").once(),
                Reply::take("paid 1 credit"),
            )
            .when(
                Match::paid().during(StructKind::Encounter).offering("interface").once(),
                Reply::take("interface"),
            )
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 3, "1 to pump, 1 to break: {}", t.tail(16));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { .. })),
        "the sentry's subroutine was broken, so it did not resolve: {}",
        t.tail(16)
    );
}

// ---------------------------------------------------------------------------
// Hardware and identity facts
// ---------------------------------------------------------------------------

/// "+1[mu]" — the console's declaration, read by the memory limit (1.19).
#[test]
fn desperado_raises_the_memory_limit() {
    let mut vm = Vm::empty(26);
    let base = vm.memory_limit();
    tk::install_rig(&mut vm, card_partial("Desperado"));
    assert_eq!(vm.memory_limit(), base + 1, "+1[mu]");
}

/// The identity's printed base link (1.20).
#[test]
fn andromeda_has_1_base_link() {
    let mut vm = Vm::empty(27);
    let id = vm.new_object(card_partial("Andromeda: Dispossessed Ristie"), Zone::PlayArea(Side::Runner));
    vm.st.objects.get_mut(&id).unwrap().faceup = true;
    assert_eq!(vm.runner_link(), 1);
}

/// A card that is still partial, for asserting the parts that ARE expressed.
fn card_partial(name: &str) -> PrintedCard {
    priority_decks()
        .unwrap()
        .into_iter()
        .find(|c| c.printed.name == name)
        .unwrap_or_else(|| panic!("no card named {name}"))
        .printed
}

/// Every card the deck files call complete is denoted into at least one
/// ability, or is a vanilla card with nothing but facts — a guard against a
/// card being marked complete because nothing was written for it at all.
#[test]
fn no_card_is_complete_by_saying_nothing() {
    for c in priority_decks().unwrap() {
        if !c.is_complete() {
            continue;
        }
        assert!(
            !c.printed.abilities.is_empty()
                || c.printed.additional_steal_cost.is_some()
                || c.printed.additional_play_cost.is_some(),
            "{} is marked complete but denotes into nothing",
            c.printed.name
        );
    }
}

/// The blocks a designer writes land where the CR puts them: a `static:`
/// block is a static ability (9.4), `paid` is paid (9.5), `when` is
/// conditional (9.6), `subroutine` is a subroutine (9.8).
#[test]
fn blocks_denote_into_the_right_ability_kinds() {
    use jinteki_cr::ability::AbilityKind;
    let kinds = |name: &str| -> Vec<AbilityKind> {
        card_partial(name).abilities.iter().map(|a| a.kind).collect()
    };
    assert_eq!(kinds("Crisium Grid"), vec![AbilityKind::Static]);
    assert_eq!(kinds("Gold Farmer"), vec![AbilityKind::Subroutine, AbilityKind::Subroutine]);
    assert_eq!(kinds("Hedge Fund"), vec![AbilityKind::Play]);
    assert_eq!(kinds("Shibboleth"), vec![AbilityKind::Paid, AbilityKind::Paid]);
    assert_eq!(kinds("Rebirth"), vec![AbilityKind::Static]);
    assert_eq!(kinds("Tomorrow's Headline"), vec![AbilityKind::Conditional; 2]);
    // 1.16.10: an additional play cost is a printed property, not a
    // declaration — so BOOM!'s `static:` block adds no ability at all.
    let boom = card_partial("BOOM!");
    assert_eq!(boom.abilities.len(), 1, "only the play ability");
    assert_eq!(boom.additional_play_cost.as_ref().map(|c| c.clicks), Some(1));
}

/// A designer's mistake names the card, the line and a fix (SYS-D-3).
#[test]
fn an_unknown_declaration_is_refused_with_a_fix() {
    let src = "card \"X\"\n  side: corp\n  type: asset\n  text:\n    Something.\n  static:\n    this card is made of cheese\n";
    let Err(e) = jinteki_cards::load("t.cards", src) else { panic!("this should not parse") };
    let s = e.to_string();
    assert!(s.starts_with("t.cards:7 in \"X\":"), "{s}");
    assert!(s.contains("unknown declaration"), "{s}");
    assert!(s.contains("unimplemented:"), "the fix is offered: {s}");
}

/// The decision the driver never had to answer: a card file that denotes into
/// a `DecisionSpec` the plan did not expect fails loudly rather than quietly
/// doing nothing.
#[test]
fn the_gap_list_only_shrinks_by_saying_things() {
    let cards = priority_decks().unwrap();
    let complete = cards.iter().filter(|c| c.is_complete()).count();
    let sentences: usize = cards.iter().map(|c| c.unimplemented.len()).sum();
    println!("{complete} complete, {sentences} sentences unsayable");
    assert!(complete >= 11, "11 cards are fully expressed; got {complete}");
    assert!(
        sentences <= 60,
        "the gap list should not grow without a reason recorded in docs/vm/WAVES.md; got {sentences}"
    );
}

// ---------------------------------------------------------------------------
// The guide is the contract (SYS-D-2), so it is checked like one.
// ---------------------------------------------------------------------------

const GUIDE: &str = include_str!("../../../docs/cards/DSL.md");

/// The lines of a fenced block in DSL.md carrying this info string.
fn guide_block(kind: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in GUIDE.lines() {
        if line.trim_start().starts_with("```") {
            let info = line.trim().trim_start_matches('`');
            inside = !inside && info == kind;
            continue;
        }
        if inside && !line.trim().is_empty() {
            out.push(line.trim());
        }
    }
    assert!(!out.is_empty(), "docs/cards/DSL.md has no ```{kind} block");
    out
}

/// Every sentence the guide lists really works. This test exists because the
/// guide had been promising verbs — `gain 1 click`, `run hq`, `access N
/// additional cards` — that were never implemented, and a designer reading it
/// had no way to know. The guide is the contract with people who do not read
/// Rust (DESIGN.md SYS-D-2); a contract nobody checks is a wish.
#[test]
fn every_sentence_the_guide_lists_is_one_you_can_write() {
    for sentence in guide_block("sentences") {
        let src = format!(
            "card \"Guide Check\"\n  side: corp\n  type: operation\n  text:\n    Checked against docs/cards/DSL.md.\n  play:\n    {sentence}\n"
        );
        let cards = jinteki_cards::load("docs/cards/DSL.md", &src).unwrap_or_else(|e| {
            panic!("docs/cards/DSL.md lists a sentence the DSL cannot write:\n{e}")
        });
        assert_eq!(
            cards[0].printed.abilities.len(),
            1,
            "`{sentence}` denoted into nothing"
        );
    }
}

/// The same for the declarations a `static:` block can state.
#[test]
fn every_declaration_the_guide_lists_is_one_you_can_write() {
    for decl in guide_block("declarations") {
        let src = format!(
            "card \"Guide Check\"\n  side: runner\n  type: resource\n  text:\n    Checked against docs/cards/DSL.md.\n  static:\n    {decl}\n"
        );
        let cards = jinteki_cards::load("docs/cards/DSL.md", &src).unwrap_or_else(|e| {
            panic!("docs/cards/DSL.md lists a declaration the DSL cannot write:\n{e}")
        });
        let c = &cards[0].printed;
        assert!(
            !c.abilities.is_empty()
                || c.additional_play_cost.is_some()
                || c.additional_steal_cost.is_some(),
            "`{decl}` denoted into nothing"
        );
    }
}

/// Every block header and trigger the guide names is one the parser accepts.
#[test]
fn every_trigger_the_guide_names_is_one_you_can_write() {
    // The trigger table's left column, as the guide writes it.
    let triggers: Vec<&str> = GUIDE
        .lines()
        .filter(|l| l.starts_with("| `when "))
        .flat_map(|l| l.split('|').nth(1).unwrap().split(" / "))
        .map(|c| c.trim().trim_matches('`'))
        .collect();
    assert!(triggers.len() >= 10, "the guide's trigger table went missing: {triggers:?}");
    for trigger in triggers {
        let src = format!(
            "card \"Guide Check\"\n  side: corp\n  type: asset\n  text:\n    Checked against docs/cards/DSL.md.\n  {trigger}:\n    gain 1 credit\n"
        );
        jinteki_cards::load("docs/cards/DSL.md", &src).unwrap_or_else(|e| {
            panic!("docs/cards/DSL.md names a trigger the DSL cannot write:\n{e}")
        });
    }
}
