//! The complete cards, played in the VM.
//!
//! Parsing is not proof (SYS-D-12): a card counts as complete only when the
//! instructions it denotes into actually do, in the rules engine, what the
//! printed text says. Each test below takes the card straight out of the deck
//! file — no hand-written `PrintedCard` — puts it on a board and drives it
//! with the shared plan driver, then asserts the printed sentence's effect.

use jinteki_cr::change::GameChange;

use jinteki_cr::instr::Instruction;
use jinteki_cr::object::{CardType, CounterKind, PrintedCard, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Kind, Match, Pick, Plan, Reply};
use jinteki_cr::timing::StructKind;
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

/// The card as the deck module writes it — and a check that the module still
/// claims every one of its printed sentences is expressed.
fn card(name: &str) -> PrintedCard {
    let c = jinteki_cards::find(name)
        // Cards defined but not in a deck list (Hedge Fund left Gauntlet on
        // the printed list's authority) are still testable directly.
        .or_else(|| match name {
            "Hedge Fund" => Some(jinteki_cards::decks::gauntlet::hedge_fund()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no card named {name} in either deck"));
    assert!(
        c.is_complete(),
        "{name} still carries an `.unimplemented(…)` marker — it cannot be asserted as playable"
    );
    c.printed
}

/// A card that is still partial, for asserting the parts that ARE expressed.
fn card_partial(name: &str) -> PrintedCard {
    jinteki_cards::find(name)
        .or_else(|| match name {
            "Hedge Fund" => Some(jinteki_cards::decks::gauntlet::hedge_fund()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no card named {name} in either deck"))
        .printed
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
        assert_eq!(
            gf.abilities
                .iter()
                .filter(|a| a.kind == jinteki_cr::ability::AbilityKind::Subroutine)
                .count(),
            2,
            "two printed subroutines (the break toll is a conditional, not a subroutine)"
        );
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

/// "Threat 4 → This program gets −2 strength."
/// "Interface → 1[credit]: Break 1 code gate subroutine."
/// "2[credit]: +2 strength."
#[test]
fn shibboleth() {
    let mut vm = Vm::empty(24);
    let shib = tk::install_rig(&mut vm, card("Shibboleth"));
    assert_eq!(vm.effective_strength(shib), Some(3), "printed strength at threat 0");

    // 9.3.6f: the [threat 4] ability becomes active once ANY player has 4
    // agenda points, and stops again if that stops being true.
    let scored = vm.new_object(tk::vanilla_agenda("Big Deal", 5, 4), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(scored);
    assert_eq!(vm.effective_strength(shib), Some(1), "threat 4: −2 strength");
    vm.st.score_area.get_mut(&Side::Corp).unwrap().clear();
    assert_eq!(vm.effective_strength(shib), Some(3), "and inactive again below 4");

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
    tk::install_rig(&mut vm, card("Shibboleth"));
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

/// How many of an object's abilities are actually present, read off the
/// 9.12.1d/e pipeline (9.1.9b). A lost ability is completely ignored (9.1.9a),
/// and in this kernel `ability_present` is a mask over `printed.abilities` —
/// so this counts exactly the PRINTED abilities a card still has.
fn abilities_present(vm: &Vm, obj: jinteki_cr::object::ObjectId) -> usize {
    jinteki_cr::object::compute_effective(&vm.st.objects, &vm.char_effects(), obj)
        .ability_present
        .iter()
        .filter(|p| **p)
        .count()
}

/// Employee Strike: "The Corp's identity loses its printed abilities."
/// (9.1.9a.) The description reaches the identity through 1.14.2's controller
/// rather than through an installed-card criterion — an identity is never
/// installed — so the Runner's own identity keeps everything.
#[test]
fn employee_strike_blanks_the_corp_identity_and_leaves_the_runners_alone() {
    for played in [true, false] {
        let mut vm = Vm::empty(48);
        // "Whenever the Runner spends credits, gain 1[credit]" — a Corp
        // identity ability with something to do on the Runner's turn.
        let corp_id = tk::install_identity(&mut vm, tk::gamenet_like("GameNET-like"), Side::Corp);
        // "When a run ends, gain 1[credit]" — the Runner's own identity.
        let runner_id =
            tk::install_identity(&mut vm, tk::run_end_identity("Zahya-like"), Side::Runner);
        let es = vm.new_object(card("Employee Strike"), Zone::Hand(Side::Runner));
        let gamble = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().extend([es, gamble]);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 6;
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Runner);

        let mut g = jinteki_cr::plan::Script::new(
            Plan::corp(),
            Plan::runner()
                // The event, or a click for a credit in its place.
                .when(
                    Match::action().once(),
                    if played { Reply::play_card(es) } else { Reply::credit() },
                )
                // Halt once, to read the board with the current in play and to
                // start counting the Corp's credits from here: the play cost of
                // the event itself was paid before it reached the play area,
                // and this test is about what happens afterwards.
                .when(Match::action().once(), Reply::Halt)
                .when(Match::action().once(), Reply::play_card(gamble))
                .when(Match::action().once(), Reply::run(ServerId::Archives))
                .when(Match::reaction(), Reply::take("identity"))
                .stop_at_action(),
        );
        g.run(&mut vm);
        let corp_before = vm.st.corp.credits;
        g.run(&mut vm);
        let t = g.transcript();

        assert_eq!(
            abilities_present(&vm, corp_id),
            if played { 0 } else { 1 },
            "the Corp's identity (played={played}): {}",
            t.tail(12)
        );
        assert_eq!(
            abilities_present(&vm, runner_id),
            1,
            "the Runner's identity is never touched (played={played}): {}",
            t.tail(12)
        );
        assert_eq!(
            vm.st.corp.credits - corp_before,
            if played { 0 } else { 1 },
            "Sure Gamble's 5[credit] was spent either way; only the blanked \
             identity failed to notice (played={played}): {}",
            t.tail(16)
        );
        assert!(
            t.took("identity"),
            "…and the Runner's identity still resolved (played={played}): {}",
            t.tail(12)
        );
    }
}

/// Employee Strike: "This event is not trashed until another current is played
/// or an agenda is scored." (8.6.6c / 3.7.1b — a current EVENT waits for the
/// Corp to SCORE, where a current operation waits for the Runner to steal.)
#[test]
fn employee_strike_stays_in_the_play_area_until_the_corp_scores() {
    let mut vm = Vm::empty(49);
    let es = vm.new_object(card("Employee Strike"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(es);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Loose Agenda", 3, 1),
        ServerId::Remote(1),
        false,
    );
    vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(es))
            // Halt once, to read the board while the current is in play.
            .when(Match::action().once(), Reply::Halt)
            .otherwise_click_credit(),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&es].zone,
        Zone::PlayArea(Side::Runner),
        "8.6.6c: not trashed at step 8.6.7g"
    );

    // The Runner finishes the turn, and the Corp scores: the shield expires.
    g.run(&mut vm);
    assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Corp));
    assert_eq!(
        vm.st.objects[&es].zone,
        Zone::Discard(Side::Runner),
        "the score ended the lingering effect: {}",
        g.transcript().tail(12)
    );
}

/// Employee Strike and Targeted Marketing print the same first sentence, and
/// its other half is "until another current is played" (3.5.1b/3.7.1b): the
/// Corp's current trashes the Runner's, and its own play does not trash
/// itself.
#[test]
fn another_current_trashes_the_one_already_in_the_play_area() {
    let mut vm = Vm::empty(50);
    let es = vm.new_object(card("Employee Strike"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(es);
    let tm = vm.new_object(card_partial("Targeted Marketing"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(tm);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(tm)).stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(es))
            .otherwise_click_credit(),
    );
    assert_eq!(
        vm.st.objects[&es].zone,
        Zone::Discard(Side::Runner),
        "another current was played: {}",
        t.tail(12)
    );
    assert_eq!(
        vm.st.objects[&tm].zone,
        Zone::PlayArea(Side::Corp),
        "…and \"another\" excludes the card being played: {}",
        t.tail(12)
    );
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

// (Self-Growth Program's bounce is asserted by
// `self_growth_program_needs_a_tag_and_then_bounces_two_cards` below, which
// covers both branches of its play restriction.)

/// BOOM!: "As an additional cost to play this operation, spend [click]."
/// "Do 7 meat damage." (1.16.10b: the additional cost joins the play cost.)
#[test]
fn boom_costs_a_click_on_top_of_its_play_cost() {
    let mut vm = Vm::empty(32);
    // 9.1.8c: "Play only if the Runner has at least 2 tags."
    vm.st.runner.tags = 2;
    let boom = vm.new_object(card("BOOM!"), Zone::Hand(Side::Corp));
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
///
/// The Runner spends a click on a run first, because the card's own play
/// restriction requires it — which is what the neighbouring test is about.
#[test]
fn hard_hitting_news_traces_then_ends_the_action_phase() {
    let mut vm = Vm::empty(33);
    let hhn = vm.new_object(card("Hard-Hitting News"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hhn);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.st.corp.credits = 5;

    vm.start_turn(Side::Runner);
    plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .otherwise_click_credit(),
    );

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
                Match::paid().during(StructKind::Encounter).offering("pump").once(),
                Reply::take("pump"),
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

/// The builder calls a designer writes land where the CR puts them: a
/// `.declares(…)` is a static ability (9.4), `.paid(…)` is paid (9.5),
/// `.when(…)` is conditional (9.6), `.subroutine(…)` is a subroutine (9.8).
#[test]
fn builder_calls_denote_into_the_right_ability_kinds() {
    use jinteki_cr::ability::AbilityKind;
    let kinds = |name: &str| -> Vec<AbilityKind> {
        card_partial(name).abilities.iter().map(|a| a.kind).collect()
    };
    assert_eq!(kinds("Crisium Grid"), vec![AbilityKind::Static]);
    assert_eq!(
        kinds("Gold Farmer"),
        vec![AbilityKind::Conditional, AbilityKind::Subroutine, AbilityKind::Subroutine],
        "the break toll is a conditional ability, the two printed subroutines follow it"
    );
    assert_eq!(kinds("Hedge Fund"), vec![AbilityKind::Play]);
    assert_eq!(
        kinds("Shibboleth"),
        vec![AbilityKind::Static, AbilityKind::Paid, AbilityKind::Paid]
    );
    assert_eq!(kinds("Rebirth"), vec![AbilityKind::Static]);
    assert_eq!(kinds("Tomorrow's Headline"), vec![AbilityKind::Conditional; 2]);
    assert_eq!(kinds("Resistor"), vec![AbilityKind::Static, AbilityKind::Subroutine]);
    // 1.16.10: an additional play cost is a printed property, not a
    // declaration — so BOOM!'s cost sentence adds no ability. Its play
    // RESTRICTION (9.1.8c) is one, and the effect is the other.
    let boom = card("BOOM!");
    assert_eq!(boom.abilities.len(), 2, "the restriction and the play ability");
    assert_eq!(boom.additional_play_cost.as_ref().map(|c| c.clicks), Some(1));
}

/// The one mistake the compiler cannot catch is forgetting the printed text,
/// so `.build()` catches it — naming the card and what to do (SYS-D-3).
#[test]
#[should_panic(expected = "copy the printed text into .text")]
fn a_card_without_its_printed_text_is_refused() {
    let _ = jinteki_cards::card("Forgetful").corp().asset().cost(0).build();
}

/// …and the same for the two facts every card has.
#[test]
#[should_panic(expected = "say what type of card it is")]
fn a_card_without_a_type_is_refused() {
    let _ = jinteki_cards::card("Shapeless").corp().text("Something.").build();
}

/// Resistor: "Resistor has +1 strength for each tag the Runner has."
/// "[subroutine] Trace[4]. If successful, end the run."
///
/// The strength sentence is a calculated characteristic (9.12.1b), so it is
/// re-evaluated as the game state changes rather than fixed when the ice was
/// rezzed — which is the whole reason it is a `Quantity` and not a number.
#[test]
fn resistor_grows_with_the_runners_tags() {
    let mut vm = Vm::empty(36);
    let resistor = tk::install_ice(&mut vm, card("Resistor"), ServerId::Hq, true);
    assert_eq!(vm.effective_strength(resistor), Some(0), "printed 0 at 0 tags");

    vm.st.runner.tags = 3;
    assert_eq!(vm.effective_strength(resistor), Some(3), "+1 for each of 3 tags");
    vm.st.runner.tags = 1;
    assert_eq!(vm.effective_strength(resistor), Some(1), "and back down again");

    // And the subroutine still ends the run on a won trace.
    vm.st.runner.tags = 0;
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "trace 4 beat 0 link and the run ended: {}",
        t.tail(14)
    );
}

/// Account Siphon: "Run HQ. If successful, instead of breaching HQ, you may
/// force the Corp to lose up to 5[credit], then you gain 2[credit] for each
/// credit lost and take 2 tags."
///
/// The kernel's own suite proves the card class end to end; this proves the
/// version in the deck module — the one a game will actually deal — is that
/// card. "Up to 5" is the observed 1.10.3b loss, which is what makes the
/// gain agree with it.
#[test]
fn account_siphon_replaces_the_breach_and_pays_what_was_actually_lost() {
    for (corp_credits, expect_lost) in [(8u32, 5u32), (3u32, 3u32)] {
        let mut vm = Vm::empty(37);
        let agenda = vm.new_object(tk::vanilla_agenda("Untouched", 6, 3), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
        vm.st.corp.credits = corp_credits;
        let siphon = vm.new_object(card("Account Siphon"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(siphon);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(siphon))
                .when(Match::optional().once(), Reply::Optional(true))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.corp.credits,
            corp_credits - expect_lost,
            "1.10.3b: the Corp loses only what the pool holds: {}",
            t.tail(14)
        );
        assert_eq!(
            vm.st.runner.credits,
            2 * expect_lost,
            "2[credit] for each credit ACTUALLY lost: {}",
            t.tail(14)
        );
        assert_eq!(vm.st.runner.tags, 2, "and take 2 tags: {}", t.tail(14));
        assert_eq!(
            vm.st.objects[&agenda].zone,
            Zone::Hand(Side::Corp),
            "the breach was replaced, so nothing in HQ was accessed"
        );
        assert!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: ServerId::Hq })),
            "6.8.4: a replaced breach still leaves the run successful: {}",
            t.tail(14)
        );
    }
}

/// Desperado: "+1[mu]" / "Gain 1[credit] whenever you make a successful run."
/// The second sentence is what the classic Siphon interaction is evidence of,
/// so it is worth asserting on its own too.
#[test]
fn desperado_pays_on_a_successful_run() {
    let mut vm = Vm::empty(38);
    tk::install_rig(&mut vm, card("Desperado"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 1, "1[credit] for the successful run: {}", t.tail(12));
}

/// Clean Getaway: "Run any server. If successful, gain 6[credit]."
///
/// "Any server" is a decision, not a constant: the effect names none, so the
/// Runner announces the attacked server as the run is initiated (6.9.1a), and
/// the "if successful" clause is tied to the set the effect allowed (6.7.4a)
/// — which for this card is every server.
#[test]
fn clean_getaway_lets_the_runner_choose_the_server() {
    for server in [ServerId::Archives, ServerId::Rnd] {
        let mut vm = Vm::empty(39);
        let cg = vm.new_object(card("Clean Getaway"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(cg);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 3;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(cg))
                .when(Match::attacked_server().once(), Reply::Server(server))
                .stop_at_action(),
        );
        // The announcement really was put to the Runner, over every server.
        let announced = t.of_kind(Kind::AttackedServer);
        assert_eq!(announced.len(), 1, "one 6.9.1a announcement: {}", t.tail(14));
        assert!(
            matches!(
                &announced[0].spec,
                jinteki_cr::decision::DecisionSpec::DeclareAttackedServer { options }
                    if options.len() >= 3 && options.contains(&server)
            ),
            "6.7.4a: this card allows every server: {:?}",
            announced[0].spec
        );
        assert!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: s } if *s == server)),
            "the run went to the announced server: {}",
            t.tail(14)
        );
        // 3 − 3 play cost + 6.
        assert_eq!(vm.st.runner.credits, 6, "if successful, gain 6: {}", t.tail(14));
    }
}

/// Hard-Hitting News: "Play only if the Runner made a run during their last
/// turn." — a 9.1.8c declaration, so the proof is that the basic play action
/// does not OFFER the card when the requirement fails.
#[test]
fn hard_hitting_news_is_only_playable_after_a_runner_run() {
    for runner_ran in [false, true] {
        let mut vm = Vm::empty(40);
        let hhn = vm.new_object(card("Hard-Hitting News"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(hhn);
        tk::fill_deck(&mut vm, Side::Corp, 6);
        tk::fill_deck(&mut vm, Side::Runner, 6);
        vm.st.corp.credits = 5;

        // A whole Runner turn first, spent either on a run or on credits.
        vm.start_turn(Side::Runner);
        let runner_plan = if runner_ran {
            Plan::runner().when(Match::action().first(), Reply::run(ServerId::Archives))
        } else {
            Plan::runner()
        };
        plan::play(
            &mut vm,
            Plan::corp().when(Match::action(), Reply::Halt),
            runner_plan.otherwise_click_credit(),
        );
        assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's turn came round");

        let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
        let offered = t
            .first_window(Kind::Action, Side::Corp)
            .actions()
            .iter()
            .any(|o| matches!(o, jinteki_cr::decision::ActionOption::BasicPlayOperation { card } if *card == hhn));
        assert_eq!(
            offered, runner_ran,
            "5.2.6e offers the operation exactly when the requirement holds \
             (runner_ran={runner_ran}): {}",
            t.tail(10)
        );
    }
}

/// Self-Growth Program: "Play only if the Runner is tagged." /
/// "Add 2 installed Runner cards to the grip."
#[test]
fn self_growth_program_needs_a_tag_and_then_bounces_two_cards() {
    for tagged in [false, true] {
        let mut vm = Vm::empty(41);
        let op = vm.new_object(card("Self-Growth Program"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
        let a = tk::install_rig(&mut vm, tk::program_cost("Prog A", 0));
        let b = tk::install_rig(&mut vm, tk::program_cost("Prog B", 0));
        let c = tk::install_rig(&mut vm, tk::program_cost("Prog C", 0));
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.tags = if tagged { 1 } else { 0 };
        vm.start_turn(Side::Corp);

        let mut runner_plan = Plan::corp().stop_at_action();
        if tagged {
            runner_plan = Plan::corp()
                .when(Match::action().once(), Reply::play_card(op))
                .when(Match::targets(), Reply::Targets(vec![a, b]))
                .stop_at_action();
        }
        let t = plan::play(&mut vm, runner_plan, Plan::runner());
        let offered = t
            .first_window(Kind::Action, Side::Corp)
            .actions()
            .iter()
            .any(|o| matches!(o, jinteki_cr::decision::ActionOption::BasicPlayOperation { card } if *card == op));
        assert_eq!(offered, tagged, "playable exactly while the Runner is tagged");
        if tagged {
            assert_eq!(vm.st.objects[&a].zone, Zone::Hand(Side::Runner), "{}", t.tail(10));
            assert_eq!(vm.st.objects[&b].zone, Zone::Hand(Side::Runner));
            assert_eq!(vm.st.objects[&c].zone, Zone::Rig, "only the announced two moved");
        }
    }
}

/// Closed Accounts: "Play only if the Runner is tagged." — the half of the
/// card that IS expressed. (Its second sentence is on the gap list.)
#[test]
fn closed_accounts_is_only_playable_while_the_runner_is_tagged() {
    for tagged in [false, true] {
        let mut vm = Vm::empty(42);
        let op = vm.new_object(card_partial("Closed Accounts"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.st.runner.tags = if tagged { 2 } else { 0 };
        vm.start_turn(Side::Corp);

        let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
        let offered = t
            .first_window(Kind::Action, Side::Corp)
            .actions()
            .iter()
            .any(|o| matches!(o, jinteki_cr::decision::ActionOption::BasicPlayOperation { card } if *card == op));
        assert_eq!(offered, tagged, "9.1.8c gates the basic play action");
    }
}

/// Daily Casts: "When you install this resource, load 8[credit] onto it. When
/// it is empty, trash it." / "When your turn begins, take 2[credit] from this
/// resource."
///
/// The three sentences are one mechanism (10.9): LOADING is what links the
/// "empty" ability to the card, so both ends are asserted — a payout, and the
/// self-trash when the payouts run out.
#[test]
fn daily_casts_pays_out_and_trashes_itself_when_empty() {
    for (start_credits, left, trashed) in [(8u32, 6u32, false), (2u32, 0u32, true)] {
        let mut vm = Vm::empty(43);
        let dc = tk::install_rig(&mut vm, card("Daily Casts"));
        let o = vm.st.objects.get_mut(&dc).unwrap();
        o.counters.insert(CounterKind::Credit, start_credits);
        o.loaded_kinds.insert(CounterKind::Credit);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        vm.st.runner.credits = 0;

        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action(), Reply::Halt),
            Plan::runner().stop_at_action(),
        );
        assert_eq!(vm.st.runner.credits, 2, "1.10.3a: the credits reached the pool: {}", t.tail(10));
        if trashed {
            assert_eq!(
                vm.st.objects[&dc].zone,
                Zone::Discard(Side::Runner),
                "10.9.2: emptied, so it trashed itself: {}",
                t.tail(10)
            );
        } else {
            assert_eq!(vm.st.objects[&dc].counter(CounterKind::Credit), left);
            assert_ne!(vm.st.objects[&dc].zone, Zone::Discard(Side::Runner), "not empty yet");
        }
    }
}

/// Earthrise Hotel: "When you install this resource, load 3 power counters
/// onto it. When it is empty, trash it." / "When your turn begins, remove 1
/// hosted power counter and draw 2 cards."
#[test]
fn earthrise_hotel_spends_a_counter_a_turn_and_draws() {
    for (start_counters, trashed) in [(3u32, false), (1u32, true)] {
        let mut vm = Vm::empty(44);
        let eh = tk::install_rig(&mut vm, card("Earthrise Hotel"));
        let o = vm.st.objects.get_mut(&eh).unwrap();
        o.counters.insert(CounterKind::Power, start_counters);
        o.loaded_kinds.insert(CounterKind::Power);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 8);

        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action(), Reply::Halt),
            Plan::runner().stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&eh].counter(CounterKind::Power),
            start_counters - 1,
            "1.9.2: one counter removed: {}",
            t.tail(10)
        );
        assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "and 2 cards drawn: {}", t.tail(10));
        assert_eq!(
            vm.st.objects[&eh].zone == Zone::Discard(Side::Runner),
            trashed,
            "10.9.2: trashed exactly when the last counter came off"
        );
    }
}

/// The Source: "Trash The Source when an agenda is scored or stolen." — one
/// printed sentence with two conditions, so two conditional abilities. Each
/// branch is driven separately, because an "or" that only ever fires on one
/// side would pass a single-branch test.
#[test]
fn the_source_trashes_itself_when_an_agenda_is_scored_or_stolen() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(45);
        let src = tk::install_rig(&mut vm, card_partial("The Source"));
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Contested", 2, 1),
            ServerId::Remote(1),
            false,
        );
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 6);
        tk::fill_deck(&mut vm, Side::Runner, 6);

        let t = if stolen {
            // The Source taxes the steal 3[credit] — pay it, so the steal
            // actually happens.
            vm.st.runner.credits = 3;
            vm.start_turn(Side::Runner);
            plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .when(Match::nested_cost(), Reply::PayCost(true))
                    .stop_at_action(),
            )
        } else {
            // The printed requirement is 2, but The Source is installed and
            // raises the requirement of ALL agendas by 1 — so 2 counters is
            // no longer enough and the Corp needs a third. That the score
            // fails at 2 is the first sentence working.
            vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
            vm.start_turn(Side::Corp);
            plan::play(
                &mut vm,
                Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
                Plan::runner(),
            )
        };
        assert_eq!(
            vm.st.objects[&agenda].zone,
            if stolen { Zone::ScoreArea(Side::Runner) } else { Zone::ScoreArea(Side::Corp) },
            "the agenda changed hands (stolen={stolen}): {}",
            t.tail(12)
        );
        assert_eq!(
            vm.st.objects[&src].zone,
            Zone::Discard(Side::Runner),
            "…and The Source trashed itself (stolen={stolen}): {}",
            t.tail(12)
        );
    }
}

/// Film Critic: "[click],[click]: Add an agenda hosted on Film Critic to your
/// score area." (1.17.3e/f — the agenda is ADDED, not stolen, so nothing a
/// "when the Runner steals" condition could meet is recorded.)
#[test]
fn film_critic_adds_a_hosted_agenda_to_the_score_area() {
    let mut vm = Vm::empty(46);
    let fc = tk::install_rig(&mut vm, card_partial("Film Critic"));
    // The ability that would host it is on the gap list, so the board is
    // seeded with the agenda already hosted.
    let agenda = vm.new_object(tk::vanilla_agenda("Hostage", 3, 2), Zone::Rig);
    tk::host_on(&mut vm, agenda, fc);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().offering("film critic"), Reply::take("film critic"))
            .when(Match::targets(), Reply::target(agenda))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Runner),
        "the hosted agenda reached the score area: {}",
        t.tail(12)
    );
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::AgendaStolen { .. })),
        "1.17.3e: adding a card to a score area is not stealing it: {}",
        t.tail(12)
    );
}

/// Seamless Launch: "Place 2 advancement counters on 1 installed card that you
/// did not install this turn." (1.12.6 — a game-history criterion; 1.18.2 —
/// placing an advancement counter is not ADVANCING.)
#[test]
fn seamless_launch_cannot_target_what_was_installed_this_turn() {
    let mut vm = Vm::empty(47);
    // One card installed before this turn, one installed during it.
    let old = tk::install_root(&mut vm, tk::vanilla_agenda("Old Plan", 3, 1), ServerId::Remote(1), false);
    let op = vm.new_object(card("Seamless Launch"), Zone::Hand(Side::Corp));
    let fresh = vm.new_object(tk::vanilla_asset("Fresh Asset", 0, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().extend([op, fresh]);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(jinteki_cr::plan::Pick::InstallCard(fresh)))
            .when(Match::action().once(), Reply::play_card(op))
            .stop_at_action(),
        Plan::runner(),
    );
    // The announcement is where the criterion bites.
    let announce = t
        .entries
        .iter()
        .find(|e| e.kind() == Kind::Targets)
        .unwrap_or_else(|| panic!("no target announcement: {}", t.tail(16)));
    assert!(announce.candidates().contains(&old), "the older card is a candidate");
    assert!(
        !announce.candidates().contains(&fresh),
        "1.12.6: the card installed this turn is not: {:?}",
        announce.candidates()
    );
    assert_eq!(
        vm.st.objects[&old].counter(CounterKind::Advancement),
        2,
        "2 advancement counters placed: {}",
        t.tail(16)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAdvanced { .. })),
        "1.18.2: placing a counter is not advancing"
    );
}

// ---------------------------------------------------------------------------
// Wave 1 of the coordinator's direct card drive: identities and staples
// ---------------------------------------------------------------------------

/// Andromeda: "You draw a starting hand of 9 cards." — through the real game
/// setup (1.6.6), not a fixture: her side draws 9, the Corp still draws 5.
#[test]
fn andromeda_draws_a_starting_hand_of_nine() {
    use jinteki_cr::vm::GameSetup;
    let corp_deck: Vec<PrintedCard> = (0..20).map(|_| tk::corp_filler("C-filler")).collect();
    let runner_deck: Vec<PrintedCard> =
        (0..20).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    let vm = Vm::new_game(GameSetup {
        seed: 7,
        corp_identity: None,
        runner_identity: Some(card("Andromeda: Dispossessed Ristie")),
        corp_deck,
        runner_deck,
        shuffle: true,
    });
    assert_eq!(vm.st.hand[&Side::Runner].len(), 9, "Andromeda's opening nine");
    assert_eq!(vm.st.hand[&Side::Corp].len(), 5, "the Corp's ordinary five");
}

/// Nebula, front face: the Corp plays an operation, so when the action
/// phase ends the identity pays 1[credit] and flips (and only once —
/// the ability is not "per operation").
#[test]
fn nebula_flips_after_an_operation_turn() {
    let mut vm = Vm::empty(4400);
    let id = tk::install_identity(&mut vm, card("Nebula Talent Management: Making Stars"), Side::Corp);
    let hf = vm.new_object(card("Hedge Fund"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hf);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(hf)),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "flipped exactly once at the phase end: {}",
        t.tail(12)
    );
    assert!(vm.st.objects[&id].flipped, "the back face is up");
    // 5 − 5 (Hedge Fund) + 9 + 2 (two remaining basic credits) + 1 (Nebula).
    assert_eq!(vm.st.corp.credits, 12, "{}", t.tail(12));
}

/// Gemilang (the back face): the first operation of the turn pays [click] —
/// only the first — and the Runner's successful central run flips it home.
#[test]
fn gemilang_pays_a_click_once_and_flips_back_on_a_central_run() {
    let mut vm = Vm::empty(4401);
    let id = tk::install_identity(&mut vm, card("Nebula Talent Management: Making Stars"), Side::Corp);
    // Setup state: the identity begins on its back face (as after a Nebula
    // turn) — placement, not effect-by-fiat.
    vm.st.objects.get_mut(&id).unwrap().flipped = true;
    let hf1 = vm.new_object(card("Hedge Fund"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hf1);
    let hf2 = vm.new_object(card("Hedge Fund"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hf2);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(hf1))
            .when(Match::action().once(), Reply::play_card(hf2))
            .when(Match::action().once(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Rnd)))
            .when(Match::action().once(), Reply::Halt),
    );
    script.run(&mut vm);

    // Halted after both operations: 3 allotted − 2 spent + 1 gained (ONCE).
    assert_eq!(vm.st.corp.clicks, 2, "one click for two operations: {}", script.transcript().tail(10));
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(
            c,
            GameChange::ClicksGained { side: Side::Corp, amount: 1 }
        )).count(),
        1,
        "the 'first time each turn' fired once"
    );
    assert!(vm.st.objects[&id].flipped, "still Gemilang — no flip yet");

    // Resume: the Corp drains, the Runner runs R&D. The successful run flips
    // the identity home; the Runner's halt right after proves the timing.
    script.run(&mut vm);
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "flipped back on the successful central run: {}",
        script.transcript().tail(14)
    );
    assert!(!vm.st.objects[&id].flipped, "front face up again");
}

/// Closed Accounts: "Play only if the Runner is tagged. The Runner loses all
/// credits in their credit pool." — the whole pool, whatever its size.
#[test]
fn closed_accounts_wipes_the_whole_pool() {
    let mut vm = Vm::empty(4401);
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 7;
    let ca = vm.new_object(card("Closed Accounts"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ca);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.st.corp.credits = 2;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(ca)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.runner.credits, 0, "all 7 gone: {}", t.tail(10));
    assert_eq!(vm.st.corp.credits, 1, "the Corp paid only the printed 1");
}

/// BOOM!'s play restriction: with one tag the action is not even offered
/// (9.1.8c removes the option, it does not merely refuse it).
#[test]
fn boom_is_not_offered_below_two_tags() {
    let mut vm = Vm::empty(4402);
    vm.st.runner.tags = 1;
    let boom = vm.new_object(card("BOOM!"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(boom);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.st.corp.credits = 6;
    vm.start_turn(Side::Corp);

    let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
    let offered = t.entries.iter().any(|e| {
        e.actions().iter().any(|a| matches!(
            a,
            jinteki_cr::decision::ActionOption::BasicPlayOperation { card } if *card == boom
        ))
    });
    assert!(!offered, "one tag is not 'at least 2 tags': {}", t.tail(6));
}

// ---------------------------------------------------------------------------
// Wave 2: the discard-phase pack and the zone movers
// ---------------------------------------------------------------------------

/// Breaking News, both sentences in one scored game: 2 tags on scoring, and
/// when the discard phase of that same turn ends, the Runner removes them.
#[test]
fn breaking_news_tags_blow_over_at_end_of_turn() {
    let mut vm = Vm::empty(4500);
    let bn = vm.new_object(card("Breaking News"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(bn);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(bn)))
            .when(Match::action().once(), Reply::Take(Pick::Advance(bn)))
            .when(Match::action().once(), Reply::Take(Pick::Advance(bn)))
            .when(Match::paid().once(), Reply::score(bn)),
        Plan::runner().stop_at_action(),
    );

    assert_eq!(vm.st.objects[&bn].zone, Zone::ScoreArea(Side::Corp), "{}", t.tail(12));
    // The tags were given on scoring AND removed at the discard phase's end —
    // by the Runner, per the printed text.
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::TagsTaken { amount: 2 })),
        "2 tags on scoring: {}",
        t.tail(14)
    );
    assert_eq!(vm.st.runner.tags, 0, "the tags blew over at end of turn: {}", t.tail(14));
}

/// Jackson Howard: the RFG ability shuffles up to 3 Archives cards into R&D —
/// "up to" proven by answering with 2.
#[test]
fn jackson_shuffles_archives_back_and_leaves_the_game() {
    let mut vm = Vm::empty(4501);
    let jh = tk::install_root(&mut vm, card("Jackson Howard"), ServerId::Remote(1), true);
    let d1 = vm.new_object(tk::corp_filler("Dead-1"), Zone::Discard(Side::Corp));
    let d2 = vm.new_object(tk::corp_filler("Dead-2"), Zone::Discard(Side::Corp));
    let d3 = vm.new_object(tk::corp_filler("Dead-3"), Zone::Discard(Side::Corp));
    for d in [d1, d2, d3] {
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(d);
    }
    tk::fill_deck(&mut vm, Side::Corp, 2);
    vm.start_turn(Side::Corp);

    let deck_before = vm.st.deck[&Side::Corp].len();
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("jackson: shuffle archives into r&d"))
            .when(Match::targets().once(), Reply::Targets(vec![d1, d2]))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(vm.st.objects[&jh].zone, Zone::RemovedFromGame, "the cost was paid: {}", t.tail(12));
    // deck_before − 1 (the turn's mandatory draw) + the 2 shuffled in.
    assert_eq!(vm.st.deck[&Side::Corp].len(), deck_before + 1, "two of up to three went back: {}", t.tail(14));
    assert_eq!(vm.st.discard[&Side::Corp].len(), 1, "the third stayed in Archives: {}", t.tail(6));
}

/// Bloo Moose: at turn start, exile one heap card for 2[credit] — optionally.
#[test]
fn bloo_moose_cashes_in_a_heap_card() {
    let mut vm = Vm::empty(4502);
    tk::install_rig(&mut vm, card("Bloo Moose"));
    let dead = vm.new_object(tk::vanilla_runner_card("Dead-Event", CardType::Event), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(dead);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().once(), Reply::take("cash in a memory"))
            .when(Match::optional().once(), Reply::Optional(true))
            .when(Match::targets().once(), Reply::Targets(vec![dead]))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&dead].zone, Zone::RemovedFromGame, "{}", t.tail(12));
    assert_eq!(vm.st.runner.credits, 2, "and the 2 credits: {}", t.tail(12));
}

/// Citadel Sanctuary's trace: tagged at the end of the discard phase, the
/// Corp MUST trace[1]; the Runner outbids it, so the trace is unsuccessful
/// and a tag comes off.
#[test]
fn citadel_sanctuary_traces_and_the_runner_escapes_a_tag() {
    let mut vm = Vm::empty(4503);
    tk::install_rig(&mut vm, card("Citadel Sanctuary"));
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 3;
    tk::fill_deck(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::trace_spend(), Reply::Spend(0)).stop_at_action(),
        Plan::runner().when(Match::trace_spend(), Reply::Spend(2)),
    );
    assert_eq!(vm.st.runner.tags, 0, "unsuccessful trace removed the tag: {}", t.tail(14));
}

/// Citadel Sanctuary's interrupt: trash it and the whole grip to prevent ALL
/// meat damage — BOOM!'s 7 land as zero.
#[test]
fn citadel_sanctuary_burns_everything_to_stop_the_meat() {
    let mut vm = Vm::empty(4504);
    vm.st.runner.tags = 2;
    let cs = tk::install_rig(&mut vm, card("Citadel Sanctuary"));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    let boom = vm.new_object(card("BOOM!"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(boom);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.st.corp.credits = 6;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action().once(), Reply::play_card(boom)).stop_at_action(),
        Plan::runner().when(Match::interrupt().once(), Reply::take("burn it all")),
    );

    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::DamageSuffered { .. })),
        "all 7 meat prevented: {}",
        t.tail(14)
    );
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip was the price: {}", t.tail(14));
    assert_ne!(vm.st.objects[&cs].zone, Zone::Rig, "and so was Citadel Sanctuary");
    assert_eq!(vm.st.runner.tags, 2, "alive, tagged, and broke — but alive");
}

// ---------------------------------------------------------------------------
// Wave 3: the access pack
// ---------------------------------------------------------------------------

/// Cupellation pockets a non-agenda out of an HQ access; the pocketed card is
/// no longer being accessed, so no trash prompt follows.
#[test]
fn cupellation_pockets_the_accessed_card() {
    let mut vm = Vm::empty(4600);
    let cup = tk::install_rig(&mut vm, card("Cupellation"));
    let pad = vm.new_object(tk::corp_filler("PAD-ish"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(pad);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 2);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::of(Kind::MidAccess).once(), Reply::take("pocket the evidence"))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&pad].host, Some(cup), "hosted on Cupellation: {}", t.tail(14));
    assert_eq!(vm.st.runner.credits, 2, "paid 1");
}

/// Film Critic hosts an accessed agenda — no steal, no additional steal cost
/// — and two clicks later adds it to the score area for its printed points.
#[test]
fn film_critic_shields_and_then_scores_the_agenda() {
    let mut vm = Vm::empty(4601);
    let fc = tk::install_rig(&mut vm, card("Film Critic"));
    let bellona = vm.new_object(card("Bellona"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(bellona);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 2);
    vm.st.runner.credits = 1; // could never pay Bellona's 5-credit steal cost
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::reaction().once(), Reply::take("above the fray"))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::take("publish the story"))
            .when(Match::targets().once(), Reply::Targets(vec![bellona]))
            .when(Match::action().once(), Reply::Halt),
    );
    script.run(&mut vm);
    assert_eq!(vm.st.objects[&bellona].host, Some(fc), "hosted: {}", script.transcript().tail(14));
    assert_eq!(vm.score(Side::Runner), 0, "not stolen, no cost paid");

    script.run(&mut vm); // resume into the two-click publish
    assert_eq!(
        vm.st.objects[&bellona].zone,
        Zone::ScoreArea(Side::Runner),
        "entries={} {}",
        script.transcript().entries.len(),
        script.transcript().tail(30)
    );
    assert_eq!(vm.score(Side::Runner), 3, "Bellona's printed 3 points");
}

/// Archangel from HQ: the Corp pays 3 and the Runner ENCOUNTERS an ice that
/// was never installed; its trace bounces an installed Runner card to grip.
#[test]
fn archangel_ambushes_from_hq() {
    let mut vm = Vm::empty(4602);
    let prog = tk::install_rig(&mut vm, tk::vanilla_runner_card("Some-Program", CardType::Program));
    let arch = vm.new_object(card("Archangel"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(arch);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 2);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().once(), Reply::take("the ambush"))
            .when(Match::of(Kind::NestedCost).once(), Reply::PayCost(true))
            .when(Match::trace_spend(), Reply::Spend(0)),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::trace_spend(), Reply::Spend(0))
            .stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == arch)),
        "the ambush encounter happened: {}",
        t.tail(16)
    );
    assert_eq!(vm.st.objects[&prog].zone, Zone::Hand(Side::Runner), "bounced to grip: {}", t.tail(16));
}

/// Pinhole Threading: run one server, access a card in the ROOT OF ANOTHER —
/// and an accessed agenda there can be neither stolen nor trashed.
#[test]
fn pinhole_threads_into_another_root_and_cannot_steal() {
    let mut vm = Vm::empty(4603);
    tk::install_root(&mut vm, tk::corp_filler("Decoy-Asset"), ServerId::Remote(1), true);
    let agenda = tk::install_root(&mut vm, card("Bellona"), ServerId::Remote(2), false);
    let ph = vm.new_object(card("Pinhole Threading"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(ph);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 2);
    vm.st.runner.credits = 9; // could afford Bellona's steal cost — but cannot steal
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(ph))
            .when(Match::of(Kind::AttackedServer).once(), Reply::Server(ServerId::Remote(1)))
            .when(Match::targets().once(), Reply::Targets(vec![agenda]))
            .stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == agenda)),
        "accessed the OTHER server's root: {}",
        t.tail(16)
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::Root(ServerId::Remote(2)), "not stolen, not trashed");
    assert_eq!(vm.score(Side::Runner), 0);
}

// ---------------------------------------------------------------------------
// Wave 4 — the conditional pack
// ---------------------------------------------------------------------------

/// AstroScript: "Hosted agenda counter: Place 1 advancement counter on an
/// installed card you can advance."
///
/// 1.18.3's permission is the criterion, so the counter can only land where
/// the basic advance action would also be allowed to go — and 1.18.2 keeps
/// this a PLACEMENT, not an advance.
#[test]
fn astroscript_spends_its_counter_to_place_an_advancement() {
    let mut vm = Vm::empty(4700);
    let astro = tk::put_in_score_area(&mut vm, card("AstroScript Pilot Program"), Side::Corp);
    vm.st.objects.get_mut(&astro).unwrap().counters.insert(CounterKind::Agenda, 1);
    // An agenda in a remote root is advanceable by 1.18.3 without any card
    // granting the permission.
    let bellona = tk::install_root(&mut vm, card("Bellona"), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("spend a counter to advance"))
            .when(Match::targets().once(), Reply::Targets(vec![bellona]))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&bellona].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        1,
        "1 advancement counter placed: {}",
        t.tail(12)
    );
    assert_eq!(
        vm.st.objects[&astro].counters.get(&CounterKind::Agenda).copied().unwrap_or(0),
        0,
        "the agenda counter was spent"
    );
}

/// IP Block's encounter ability is conditional on the BOARD (9.6.5d), so it
/// is checked when the instruction resolves: no installed AI, no tag.
///
/// The ice's OWN trace subroutine also hands out a tag, so the encounter
/// ability is isolated by differencing the two boards rather than by
/// asserting an absolute count — that keeps the test honest about which
/// sentence it is measuring even if the trace's outcome changes.
#[test]
fn ip_block_taxes_only_while_an_ai_is_installed() {
    let tags_with_board = |ai_installed: bool| {
        let mut vm = Vm::empty(4701);
        tk::install_ice(&mut vm, card("IP Block"), ServerId::Hq, true);
        if ai_installed {
            let mut ai = tk::runner_filler("Bogus AI");
            ai.card_type = CardType::Program;
            ai.subtypes = vec!["AI", "Icebreaker"];
            tk::install_rig(&mut vm, ai);
        }
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
        );
        vm.st.runner.tags
    };
    let without = tags_with_board(false);
    let with = tags_with_board(true);
    assert_eq!(with - without, 1, "the installed AI is worth exactly the encounter's 1 tag");
}

/// Mutual Favor with no successful run this turn: the icebreaker is found,
/// revealed, and goes to the grip rather than the rig.
#[test]
fn mutual_favor_without_a_run_puts_the_breaker_in_the_grip() {
    let mut vm = Vm::empty(4702);
    let mf = vm.new_object(card("Mutual Favor"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(mf);
    let mut breaker = tk::runner_filler("Bogus Breaker");
    breaker.card_type = CardType::Program;
    breaker.subtypes = vec!["Icebreaker"];
    breaker.cost = Some(0);
    let brk = vm.new_object(breaker, Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(brk);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(mf))
            .when(Match::targets().once(), Reply::Targets(vec![brk]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&brk].zone,
        Zone::Hand(Side::Runner),
        "no successful run this turn, so it goes to the grip: {}",
        t.tail(16)
    );
}

/// The Source: "The advancement requirement of all agendas is increased by 1."
///
/// "All agendas" reaches every agenda wherever it sits, so the raised
/// requirement is already true of an agenda the Corp has not installed yet —
/// which is what makes the reach a scope on the declaration rather than a
/// server-scoped one like SanSan City Grid's.
#[test]
fn the_source_raises_every_agendas_requirement() {
    let mut vm = Vm::empty(4703);
    let installed =
        tk::install_root(&mut vm, tk::vanilla_agenda("Contested", 2, 1), ServerId::Remote(1), false);
    let in_hq = vm.new_object(tk::vanilla_agenda("Still In Hand", 3, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(in_hq);
    assert_eq!(vm.advancement_requirement(installed), 2, "printed 2 with no Source out");
    assert_eq!(vm.advancement_requirement(in_hq), 3, "printed 3 with no Source out");

    tk::install_rig(&mut vm, card_partial("The Source"));
    assert_eq!(vm.advancement_requirement(installed), 3, "+1 while The Source is active");
    assert_eq!(vm.advancement_requirement(in_hq), 4, "…and it reaches HQ too");
}

/// Gold Farmer: "Whenever the Runner breaks a printed subroutine on this ice,
/// they lose 1[credit]." — met once PER subroutine, not once per encounter,
/// which is what distinguishes it from a fully-broken condition.
#[test]
fn gold_farmer_taxes_each_broken_subroutine() {
    let mut vm = Vm::empty(4704);
    let gf = tk::install_ice(&mut vm, card("Gold Farmer"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::break_button("Breaker"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    let before = vm.st.runner.credits;
    // Break both subroutines with a real breaker, in the encounter's paid
    // ability window — the toll is on BREAKING, so which ability did it does
    // not matter, but it has to actually happen in the run.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").times(2), Reply::take("break"))
            .when(Match::targets().times(2), Reply::Pass)
            .stop_at_action(),
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::SubroutineBroken { ice, printed: true } if *ice == gf))
            .count(),
        2,
        "both printed subroutines were broken: {}",
        t.tail(20)
    );
    assert_eq!(
        before - vm.st.runner.credits,
        2,
        "1[credit] for each of the two printed subroutines broken: {}",
        t.tail(20)
    );
}
