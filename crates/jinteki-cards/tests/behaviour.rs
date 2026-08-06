//! The complete cards, played in the VM.
//!
//! Parsing is not proof (SYS-D-12): a card counts as complete only when the
//! instructions it denotes into actually do, in the rules engine, what the
//! printed text says. Each test below takes the card straight out of the deck
//! file — no hand-written `PrintedCard` — puts it on a board and drives it
//! with the shared plan driver, then asserts the printed sentence's effect.

use jinteki_cr::change::{ActionIdentity, BasicAction, GameChange};
use jinteki_cr::decision::{ActionOption, DecisionSpec};

use jinteki_cr::instr::Instruction;
use jinteki_cr::object::{CardType, CounterKind, ObjectId, PrintedCard, ServerId, Side, Zone};
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

/// A plain Runner program under a given name. Several objects sharing one
/// name is what CR 2.1.4's "a copy of that card" is a question about, and
/// nothing else about the card matters to the sentences that ask it.
fn copy_card(name: &'static str) -> PrintedCard {
    tk::vanilla_runner_card(name, CardType::Program)
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
            .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: s, .. } if *s == server));
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
    let tm = vm.new_object(card("Targeted Marketing"), Zone::Hand(Side::Corp));
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
    let tm = vm.new_object(card("Targeted Marketing"), Zone::Hand(Side::Corp));
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

/// Targeted Marketing's second sentence: "Name a card. Gain 10[credit]
/// whenever the Runner plays or installs a copy of that card."
///
/// CR 1.15.1b: the name is said when the play ability RESOLVES, not announced
/// beforehand. 2.1.4: "a copy of that card" is any card with that name, so the
/// Runner's own Sure Gamble is one; 9.10.3c keeps the name for as long as the
/// current is active, which 8.6.6c makes the rest of the game.
#[test]
fn targeted_marketing_names_a_card_and_taxes_every_copy() {
    let mut vm = Vm::empty(31);
    let tm = vm.new_object(card("Targeted Marketing"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(tm);
    // Two copies of the named card and one card that is not it: 2.1.4's
    // "copies of" has to reach both copies and neither of the others.
    let gamble_a = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let gamble_b = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let diesel = vm.new_object(card("Diesel"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([gamble_a, gamble_b, diesel]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.st.runner.credits = 20;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(tm))
            .when(Match::name_value().once(), Reply::Name("Sure Gamble"))
            .otherwise_click_credit(),
        Plan::runner()
            // A card that is NOT the named one first: it must pay nothing.
            .when(Match::action().once(), Reply::play_card(diesel))
            .when(Match::action().once(), Reply::play_card(gamble_a))
            .when(Match::action().once(), Reply::play_card(gamble_b))
            .stop_at_action(),
    );

    assert_eq!(vm.st.objects[&gamble_a].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.objects[&gamble_b].zone, Zone::Discard(Side::Runner));
    // The Corp clicked for credits through the rest of its own turn; the two
    // Sure Gambles are the only thing that could have added 10 each.
    assert_eq!(
        vm.st.corp.credits, 22,
        "2 basic credit actions + 10 for each of the two copies, and nothing \
         for Diesel: {}",
        t.tail(20)
    );
}

/// The same sentence's other half: "…or INSTALLS a copy of that card." One
/// trigger condition covers both, which is what keeps a "first time each
/// turn" from being spent twice (Azmari EdTech's reading of the same words).
#[test]
fn targeted_marketing_taxes_an_install_too() {
    let mut vm = Vm::empty(37);
    let tm = vm.new_object(card("Targeted Marketing"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(tm);
    let hotel = vm.new_object(card("Earthrise Hotel"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(hotel);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 10;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(tm))
            .when(Match::name_value().once(), Reply::Name("Earthrise Hotel"))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(hotel)))
            .stop_at_action(),
    );

    assert_eq!(vm.st.objects[&hotel].zone, Zone::Rig);
    assert_eq!(
        vm.st.corp.credits, 12,
        "2 basic credit actions + 10 for the install: {}",
        t.tail(20)
    );
}

/// 10.1.5 read the other way round: naming a card the Runner does not play is
/// simply inert, and the ability never fires. The point of the assertion is
/// that `MatchesMaintainedChoice` is a real comparison, not a wildcard.
#[test]
fn targeted_marketing_taxes_nothing_when_the_named_card_is_not_played() {
    let mut vm = Vm::empty(41);
    let tm = vm.new_object(card("Targeted Marketing"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(tm);
    let gamble = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(gamble);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 10;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(tm))
            .when(Match::name_value().once(), Reply::Name("Diesel"))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(gamble))
            .stop_at_action(),
    );

    assert_eq!(
        vm.st.corp.credits, 2,
        "the played card is not the named one: {}",
        t.tail(20)
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
    // Rebirth prints one sentence that HAPPENS (the switch, a play ability)
    // and one that is permanently true (the removal replacement, a static).
    assert_eq!(kinds("Rebirth"), vec![AbilityKind::Play, AbilityKind::Static]);
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
                .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: ServerId::Hq, .. })),
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
                .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: s, .. } if *s == server)),
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
        additional_identities: Default::default(),
        corp_identity: None,
        runner_identity: Some(card("Andromeda: Dispossessed Ristie")),
        corp_deck,
        runner_deck,
        shuffle: true,
        extra_cards: Default::default(),
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
    assert!(vm.st.objects[&id].flipped.is_some(), "the back face is up");
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
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
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
    assert!(vm.st.objects[&id].flipped.is_some(), "still Gemilang — no flip yet");

    // Resume: the Corp drains, the Runner runs R&D. The successful run flips
    // the identity home; the Runner's halt right after proves the timing.
    script.run(&mut vm);
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "flipped back on the successful central run: {}",
        script.transcript().tail(14)
    );
    assert!(vm.st.objects[&id].flipped.is_none(), "front face up again");
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
        vm.changes.log.iter().any(|c| matches!(c, GameChange::TagsTaken { amount: 2, .. })),
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
// Trickster Taka, and the 5.7.2d clock it shares with Citadel Sanctuary
// ---------------------------------------------------------------------------

/// Trickster Taka's first sentence is ONE condition met by either occurrence
/// (9.6.4b): the turn beginning pockets a credit, and so does the steal —
/// two hosted credits by the time the run comes home.
#[test]
fn trickster_taka_pockets_a_credit_either_way() {
    let mut vm = Vm::empty(4710);
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Loose Agenda", 3, 1),
        ServerId::Remote(1),
        false,
    );
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Runner), "{}", t.tail(12));
    assert_eq!(
        vm.st.objects[&taka].counters.get(&CounterKind::Credit).copied().unwrap_or(0),
        2,
        "one for the turn beginning, one for the steal: {}",
        t.tail(12)
    );
}

/// "During runs" is half the restriction (6.1.1): outside one, three hosted
/// credits buy nothing at all — the affordability question already says no.
#[test]
fn trickster_taka_credits_are_no_good_outside_a_run() {
    let mut vm = Vm::empty(4711);
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    let breaker = tk::install_rig(&mut vm, tk::pump_breaker("Breaker", 1));
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    vm.st.runner.credits = 0;
    assert_eq!(
        vm.spendable_credits_for(
            Side::Runner,
            jinteki_cr::vm::CreditPurpose::UsingAbilityOf(breaker)
        ),
        0,
        "1.10.3c: the moment is not now, so the description is never reached"
    );
}

/// The other half holds during one: a broke Runner pumps an icebreaker on
/// Taka's credits mid-encounter, and the credit comes off the card.
#[test]
fn trickster_taka_credits_pump_a_breaker_during_a_run() {
    let mut vm = Vm::empty(4712);
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    tk::install_rig(&mut vm, tk::pump_breaker("Breaker", 1));
    tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 3), ServerId::Hq, true);
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    tk::fill_hand(&mut vm, Side::Corp, 1);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("pump"))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&taka].counters.get(&CounterKind::Credit).copied().unwrap_or(0),
        3,
        "3 placed + 1 at turn begin − the pump's 1[credit]: {}",
        t.tail(14)
    );
    assert_eq!(vm.st.runner.credits, 0, "and the pool was never touched: {}", t.tail(14));
}

/// The bill: 3 or more hosted credits when the turn ends, and the choice
/// between harms is the Runner's — declining both is not on the menu.
#[test]
fn trickster_taka_the_bill_comes_due_as_a_tag() {
    let mut vm = Vm::empty(4713);
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner().when(Match::options(), Reply::ChooseNamed("take 1 tag")),
    );
    assert_eq!(vm.st.runner.tags, 1, "{}", t.tail(12));
    assert_eq!(vm.st.objects[&taka].zone, Zone::Rig, "still installed: {}", t.tail(12));
}

/// …or the resource goes instead, and no tag lands.
#[test]
fn trickster_taka_the_bill_comes_due_as_the_resource() {
    let mut vm = Vm::empty(4714);
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner().when(Match::options(), Reply::ChooseNamed("trash this resource")),
    );
    assert_eq!(vm.st.runner.tags, 0, "{}", t.tail(12));
    assert_ne!(vm.st.objects[&taka].zone, Zone::Rig, "the resource was the price: {}", t.tail(12));
}

/// 5.1.4b: the end of the turn and the end of the discard phase are the SAME
/// moment, so Citadel Sanctuary's "while you are tagged" is read at 5.7.2d —
/// when nobody is tagged — and the tag Taka hands out during that window's
/// resolution arrives too late to ever feed a trace.
#[test]
fn taka_tag_arrives_too_late_for_citadel_sanctuary() {
    let mut vm = Vm::empty(4715);
    tk::install_rig(&mut vm, card("Citadel Sanctuary"));
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner().when(Match::options(), Reply::ChooseNamed("take 1 tag")),
    );
    assert_eq!(vm.st.runner.tags, 1, "{}", t.tail(14));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::TraceInitiated { .. })),
        "9.6.5c: the stipulation was read at 5.7.2d, when nobody was tagged: {}",
        t.tail(14)
    );
    let ended = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TurnEnded { side: Side::Runner }))
        .expect("the turn ended");
    let tagged = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagsTaken { .. }))
        .expect("the tag was taken");
    assert!(tagged > ended, "the tag lands after the shared moment: {}", t.tail(14));
}

/// A tag already there at 5.7.2d puts BOTH abilities in one reaction window
/// (9.6.4b — same occurrence, two conditions, the Privileged Access shape),
/// ordered by their controller. Citadel first: the trace fails, the old tag
/// comes off, and the bill still comes due after.
#[test]
fn citadel_and_taka_share_the_clock_citadel_first() {
    let mut vm = Vm::empty(4716);
    tk::install_rig(&mut vm, card("Citadel Sanctuary"));
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 3;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::trace_spend(), Reply::Spend(0)).stop_at_action(),
        Plan::runner()
            .when(
                Match::reaction().offering("the corp must trace"),
                Reply::take("the corp must trace"),
            )
            .when(Match::trace_spend(), Reply::Spend(2))
            .when(
                Match::reaction().offering("the bill comes due"),
                Reply::take("the bill comes due"),
            )
            .when(Match::options(), Reply::ChooseNamed("take 1 tag")),
    );
    assert_eq!(vm.st.runner.tags, 1, "1 − 1 + 1: {}", t.tail(16));
    let removed = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagRemoved))
        .expect("the unsuccessful trace removed a tag");
    let taken = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagsTaken { .. }))
        .expect("the bill was paid in tags");
    assert!(removed < taken, "Citadel resolved before the bill: {}", t.tail(16));
}

/// The same window, the other order: the bill first raises the tag count to
/// 2, and Citadel's trace — read as pending at the same 5.7.2d moment —
/// still resolves after, taking one back off.
#[test]
fn citadel_and_taka_share_the_clock_taka_first() {
    let mut vm = Vm::empty(4717);
    tk::install_rig(&mut vm, card("Citadel Sanctuary"));
    let taka = tk::install_rig(&mut vm, card("Trickster Taka"));
    tk::place_counters(&mut vm, taka, CounterKind::Credit, 3);
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 3;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::trace_spend(), Reply::Spend(0)).stop_at_action(),
        Plan::runner()
            .when(
                Match::reaction().offering("the bill comes due"),
                Reply::take("the bill comes due"),
            )
            .when(Match::options(), Reply::ChooseNamed("take 1 tag"))
            .when(
                Match::reaction().offering("the corp must trace"),
                Reply::take("the corp must trace"),
            )
            .when(Match::trace_spend(), Reply::Spend(2)),
    );
    assert_eq!(vm.st.runner.tags, 1, "1 + 1 − 1: {}", t.tail(16));
    let taken = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagsTaken { .. }))
        .expect("the bill was paid in tags");
    let removed = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagRemoved))
        .expect("the trace then took one back off");
    assert!(taken < removed, "the bill resolved before Citadel: {}", t.tail(16));
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

// ---------------------------------------------------------------------------
// Wave 7 — the interrupt that reads the draw
// ---------------------------------------------------------------------------

/// The Class Act's first sentence: installed this turn, so when the discard
/// phase of that very turn ends, 4 cards come off the stack. The card is
/// installed with the basic action, because "you installed this resource this
/// turn" is a question about the game history and nothing else answers it.
#[test]
fn the_class_act_settles_in_and_draws_four() {
    let mut vm = Vm::empty(4700);
    let tca = vm.new_object(card("The Class Act"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(tca);
    tk::fill_deck(&mut vm, Side::Runner, 12);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let stack_before = vm.st.deck[&Side::Runner].len();
    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(tca)))
            // Spend the rest of the turn on nothing, so the discard phase is
            // reached with a grip small enough to need no discarding.
            .when(Match::action(), Reply::Take(Pick::Credit)),
    );

    assert_eq!(vm.st.objects[&tca].zone, Zone::Rig, "installed this turn: {}", t.tail(20));
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        4,
        "4 cards drawn when the discard phase ended: {}",
        t.tail(20)
    );
    // The interrupt fires on that same draw (it is the turn's first), so the
    // stack loses X = 5 off the top and takes 1 back at the bottom.
    assert_eq!(
        vm.st.deck[&Side::Runner].len(),
        stack_before - 4,
        "and the stack is 4 shorter: {}",
        t.tail(20)
    );
}

/// The Class Act's interrupt: X is the number of cards you WOULD draw plus 1,
/// so a basic draw of 1 looks exactly 2 deep — never 1, never 3 — and the
/// looked-at card the Runner picks goes under the stack before the draw takes
/// what is left on top.
#[test]
fn the_class_act_reads_one_card_deeper_than_the_draw() {
    let mut vm = Vm::empty(4701);
    tk::install_rig(&mut vm, card("The Class Act"));
    let stack: Vec<_> = ["Stack-1", "Stack-2", "Stack-3", "Stack-4"]
        .into_iter()
        .map(|n| {
            let id = vm.new_object(tk::vanilla_runner_card(n, CardType::Event), Zone::Deck(Side::Runner));
            vm.st.deck.get_mut(&Side::Runner).unwrap().push(id);
            id
        })
        .collect();
    let (first, second, third) = (stack[0], stack[1], stack[2]);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Draw))
            .when(Match::targets().once(), Reply::target(first))
            // 9.3.6g's "first time each turn": the second draw of the turn
            // opens no window of ours at all.
            .when(Match::action().once(), Reply::Take(Pick::Draw))
            .when(Match::action(), Reply::Take(Pick::Credit)),
    );

    let looked: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CardLookedAt { obj, .. } => Some(*obj),
            _ => None,
        })
        .collect();
    // Two looks and no more: X = 1 card drawn + 1, so the third card was
    // never seen; the Runner's SECOND draw of the turn opened no window; and
    // the Corp's mandatory draw — which the plan runs past, halting at their
    // action window — is not a draw of "yours" at all.
    assert_eq!(
        looked,
        vec![first, second],
        "exactly two cards looked at, once, on the Runner's first draw: {}",
        t.tail(24)
    );
    assert_eq!(
        *vm.st.deck[&Side::Runner].last().unwrap(),
        first,
        "the chosen one went to the BOTTOM of the stack: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner],
        vec![second, third],
        "and both draws took what was left on top: {}",
        t.tail(24)
    );
}

// ---------------------------------------------------------------------------
// CR 1.5.4: the additional identities pile
// ---------------------------------------------------------------------------

/// Ken "Express" Tenma: "The first time each turn you play a run event, gain
/// 1[credit]." — the once-per-turn ordinal is the point, so two run events are
/// played and only the first pays.
#[test]
fn ken_tenma_pays_for_the_first_run_event_each_turn() {
    let mut vm = Vm::empty(5501);
    tk::install_identity(&mut vm, card("Ken \"Express\" Tenma: Disappeared Clone"), Side::Runner);
    let cg1 = vm.new_object(card("Clean Getaway"), Zone::Hand(Side::Runner));
    let cg2 = vm.new_object(card("Clean Getaway"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([cg1, cg2]);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 6;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(cg1))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            .when(Match::action().once(), Reply::play_card(cg2))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            .stop_at_action(),
    );
    // 6 − 3 + 6 + 1 (Ken, first run event) − 3 + 6 (no second payment).
    assert_eq!(vm.st.runner.credits, 13, "the first run event only: {}", t.tail(20));
}

/// CR 1.5.4a: the pile is beside the game, not in it — an identity waiting
/// there is INACTIVE, so its abilities do nothing.
///
/// Chaos Theory prints "+1[mu]", which is visible in one number, and the
/// Runner's memory limit must not move while she sits in the pile.
#[test]
fn an_identity_in_the_pile_is_inactive() {
    use jinteki_cr::vm::GameSetup;
    let corp_deck: Vec<PrintedCard> = (0..20).map(|_| tk::corp_filler("C-filler")).collect();
    let runner_deck: Vec<PrintedCard> =
        (0..20).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    let mut pile = std::collections::BTreeMap::new();
    pile.insert(Side::Runner, vec![card("Ken \"Express\" Tenma: Disappeared Clone")]);
    let vm = Vm::new_game(GameSetup {
        seed: 9,
        additional_identities: pile,
        corp_identity: None,
        runner_identity: Some(card("Andromeda: Dispossessed Ristie")),
        corp_deck,
        runner_deck,
        shuffle: true,
        extra_cards: Default::default(),
    });
    let carried = vm.identity_pile(Side::Runner);
    assert_eq!(carried.len(), 1, "1.5.4a: the pile came to the table");
    assert_eq!(vm.st.objects[&carried[0]].zone, Zone::OutsideGame(Side::Runner));
    assert!(
        !jinteki_cr::object::card_active(&vm.st.objects[&carried[0]]),
        "1.8.3d: an identity outside the game is not active"
    );
    // 3.1.1: the identity is still the one in the play area, and it is
    // Andromeda — so her nine-card hand, not Ken's five.
    assert_eq!(vm.st.hand[&Side::Runner].len(), 9);
    let id = vm.identity_of(Side::Runner).expect("one identity in the play area");
    assert_eq!(vm.st.objects[&id].printed.name, "Andromeda: Dispossessed Ristie");
}

/// Rebirth: "Switch your identity with another identity from the same
/// faction. Remove Rebirth from the game instead of trashing it."
///
/// Three things at once: the description reaches OUTSIDE the game (1.5.4a's
/// pile, named by a criterion, which is what lifts 1.15.2c's play-area
/// restriction); "from the same faction" refuses the Shaper sitting in the
/// same pile (2.13); and the identity that leaves the play area goes BACK to
/// the pile rather than anywhere else (1.5.4b).
#[test]
fn rebirth_switches_the_identity_for_one_of_the_same_faction() {
    let mut vm = Vm::empty(5503);
    let andromeda =
        tk::install_identity(&mut vm, card("Andromeda: Dispossessed Ristie"), Side::Runner);
    // 1.5.4a: the pile the deck brought to the table — two identities, one of
    // each faction. Placement, not effect-by-fiat.
    let mut pile = |c: PrintedCard| {
        let id = vm.new_object(c, Zone::OutsideGame(Side::Runner));
        vm.st.objects.get_mut(&id).unwrap().faceup = true;
        id
    };
    let ken = pile(card("Ken \"Express\" Tenma: Disappeared Clone"));
    let chaos = pile(card("Chaos Theory: Wünderkind"));

    let rb = vm.new_object(card("Rebirth"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(rb);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(rb))
            .when(Match::targets().once(), Reply::target(ken))
            .stop_at_action(),
    );

    // 2.13 / 1.5.4b: the Shaper was in the pile and was never a candidate.
    let announced = t.of_kind(Kind::Targets);
    assert_eq!(announced.len(), 1, "one 1.15.2 announcement: {}", t.tail(16));
    assert!(
        matches!(
            &announced[0].spec,
            jinteki_cr::decision::DecisionSpec::ChooseTargets { candidates, .. }
                if candidates == &vec![ken]
        ),
        "only the same-faction identity is a candidate: {:?}",
        announced[0].spec
    );

    // 3.1.1: the play area holds the new identity, and it is the one the
    // rules now read.
    assert_eq!(vm.st.objects[&ken].zone, Zone::PlayArea(Side::Runner));
    assert_eq!(vm.identity_of(Side::Runner), Some(ken), "{}", t.tail(16));
    // 1.5.4b: "if an identity card leaves the play area, it must be returned
    // to the pile outside the game" — not to the heap, not out of the game.
    assert_eq!(
        vm.st.objects[&andromeda].zone,
        Zone::OutsideGame(Side::Runner),
        "the old identity went back to the pile: {}",
        t.tail(16)
    );
    assert!(vm.identity_pile(Side::Runner).contains(&andromeda));
    assert!(vm.identity_pile(Side::Runner).contains(&chaos));
    // The switch is not cosmetic: Andromeda's printed link is 1 and Ken's is
    // 0, so the live characteristics are the new identity's.
    assert_eq!(vm.runner_link(), 0, "Andromeda's link left with her: {}", t.tail(16));
    assert!(
        !jinteki_cr::object::card_active(&vm.st.objects[&andromeda]),
        "1.8.3d: back in the pile, and inactive again"
    );
    // The second printed sentence, undisturbed.
    assert_eq!(vm.st.objects[&rb].zone, Zone::RemovedFromGame);
}

/// Rebirth with nothing to switch to: 9.11.2's "do as much as you can" — the
/// event is still played and still removed from the game, and the identity in
/// the play area does not move.
#[test]
fn rebirth_with_no_legal_identity_leaves_the_one_in_play() {
    let mut vm = Vm::empty(5504);
    let andromeda =
        tk::install_identity(&mut vm, card("Andromeda: Dispossessed Ristie"), Side::Runner);
    // Only a Shaper in the pile — "from the same faction" reaches nothing.
    let chaos = vm.new_object(card("Chaos Theory: Wünderkind"), Zone::OutsideGame(Side::Runner));
    vm.st.objects.get_mut(&chaos).unwrap().faceup = true;
    let rb = vm.new_object(card("Rebirth"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(rb);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(rb))
            .stop_at_action(),
    );
    // 1.15.2b: the announcement is made with as many targets as there are,
    // and there are none — the Shaper is in the pile and is not a candidate.
    // So the Runner is not asked. An announcement that could only ever be
    // empty is not a choice, and putting it to the player produced a prompt
    // with nothing to draw and nothing to press: on the live server, exactly
    // this card and exactly this board stopped the game dead.
    assert!(
        t.of_kind(Kind::Targets).is_empty(),
        "1.15.2b: nothing to name, so nothing was asked: {}",
        t.tail(12)
    );
    assert_eq!(vm.identity_of(Side::Runner), Some(andromeda), "{}", t.tail(12));
    assert_eq!(vm.st.objects[&chaos].zone, Zone::OutsideGame(Side::Runner));
    assert_eq!(vm.st.objects[&rb].zone, Zone::RemovedFromGame, "{}", t.tail(12));
}

/// DJ Fenris: "Host a g-mod identity that does not match the faction of your
/// identity on DJ Fenris when he is installed."
///
/// Three stipulations, all of them ordinary criteria: CR 1.5.4a's pile (which
/// 1.5.4b makes what naming an identity means), a subtype (2.16.7a lists
/// G-mod among the identity subtypes) and a faction that must NOT match
/// (2.13). The Criminal in the same pile fails on both counts, so the
/// candidate list is the whole proof.
#[test]
fn dj_fenris_hosts_a_g_mod_identity_of_another_faction() {
    let mut vm = Vm::empty(5505);
    tk::install_identity(&mut vm, card("Andromeda: Dispossessed Ristie"), Side::Runner);
    let mut pile = |c: PrintedCard| {
        let id = vm.new_object(c, Zone::OutsideGame(Side::Runner));
        vm.st.objects.get_mut(&id).unwrap().faceup = true;
        id
    };
    // Criminal AND not g-mod: fails both stipulations.
    let ken = pile(card_partial("Ken \"Express\" Tenma: Disappeared Clone"));
    let chaos = pile(card_partial("Chaos Theory: Wünderkind"));

    let dj = vm.new_object(card_partial("DJ Fenris"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(dj);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(dj)))
            .when(Match::destination().once(), Reply::Destination(jinteki_cr::instr::InstallDest::Rig))
            .when(Match::reaction().once(), Reply::take("guest of the evening"))
            .when(Match::targets().once(), Reply::target(chaos))
            .stop_at_action(),
    );
    let announced = t.of_kind(Kind::Targets);
    assert_eq!(announced.len(), 1, "one announcement: {}", t.tail(20));
    assert!(
        matches!(
            &announced[0].spec,
            jinteki_cr::decision::DecisionSpec::ChooseTargets { candidates, .. }
                if candidates == &vec![chaos]
        ),
        "the Criminal non-g-mod in the same pile is not a candidate: {:?}",
        announced[0].spec
    );
    // 1.13.1/1.13.12: hosted, and therefore in the host's zone.
    assert_eq!(vm.st.objects[&chaos].host, Some(dj));
    assert_eq!(vm.st.objects[&chaos].zone, Zone::Rig);
    // 1.13.2a: hosted "without reference to installing it" — so not installed,
    // and 3.1.1b would say so anyway.
    assert!(vm.st.objects[&chaos].hosted_not_installed);
    // 3.1.1: and it is still not the Runner's identity — that is the card in
    // the play area, which is still Andromeda.
    assert_eq!(
        vm.st.objects[&vm.identity_of(Side::Runner).unwrap()].printed.name,
        "Andromeda: Dispossessed Ristie",
        "{}",
        t.tail(20)
    );
    assert!(!vm.identity_pile(Side::Runner).contains(&chaos));
    assert!(vm.identity_pile(Side::Runner).contains(&ken));
}

/// DJ Fenris: "DJ Fenris gains the text of hosted identity."
///
/// CR 9.1.9's other direction, and the one the kernel could not say: an
/// object can now GAIN abilities, not only lose them. The hosted identity is
/// inactive where it sits (1.13.2a: hosted without being installed, so
/// 4.6.5h makes it inactive), so its own "+1[mu]" does nothing — but DJ
/// Fenris, who is active, HAS the ability now, and the memory limit moves by
/// exactly one, not two.
#[test]
fn dj_fenris_gains_the_text_of_the_hosted_identity() {
    let mut vm = Vm::empty(5507);
    tk::install_identity(&mut vm, card("Andromeda: Dispossessed Ristie"), Side::Runner);
    let base = vm.memory_limit();
    let chaos =
        vm.new_object(card("Chaos Theory: Wünderkind"), Zone::OutsideGame(Side::Runner));
    vm.st.objects.get_mut(&chaos).unwrap().faceup = true;
    let dj = vm.new_object(card_partial("DJ Fenris"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(dj);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);
    assert_eq!(vm.memory_limit(), base, "1.5.4a: the identity in the pile is inactive");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(dj)))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Rig),
            )
            .when(Match::reaction().once(), Reply::take("guest of the evening"))
            .when(Match::targets().once(), Reply::target(chaos))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&chaos].host, Some(dj), "hosted: {}", t.tail(16));
    assert!(vm.st.objects[&chaos].hosted_not_installed);
    assert!(
        !jinteki_cr::object::card_active(&vm.st.objects[&chaos]),
        "1.13.2a/4.6.5h: the hosted identity is not active, so its own text does nothing"
    );
    assert_eq!(
        vm.memory_limit(),
        base + 1,
        "9.1.9b: DJ Fenris has the hosted identity's ability, once: {}",
        t.tail(16)
    );
}

/// DJ Fenris: "Remove hosted identity from the game if DJ Fenris is
/// uninstalled."
///
/// The sentence belongs to the hosting ability — the same paragraph, and the
/// same card it chose — so it is a delayed conditional (9.6.13) created when
/// the hosting happens: 9.10.1 keeps the effect alive after its source has
/// left the play area, and 1.15.4 lets it act on the card the ability already
/// chose, which is the only way to still know WHICH identity once the hosting
/// relationship is gone.
///
/// The identity therefore does NOT go back to the pile (1.5.4b), where a
/// later Rebirth could take it: it is removed from the game.
#[test]
fn dj_fenris_removes_the_hosted_identity_when_he_is_uninstalled() {
    let mut vm = Vm::empty(5509);
    tk::install_identity(&mut vm, card("Andromeda: Dispossessed Ristie"), Side::Runner);
    let chaos =
        vm.new_object(card("Chaos Theory: Wünderkind"), Zone::OutsideGame(Side::Runner));
    vm.st.objects.get_mut(&chaos).unwrap().faceup = true;
    let dj = vm.new_object(card("DJ Fenris"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(dj);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.runner.credits = 5;
    vm.st.corp.credits = 5;
    // Setup state, not effect: the Runner is tagged, which is what 5.2.6g
    // requires for the Corp's basic trash-resource action.
    vm.st.runner.tags = 1;
    vm.start_turn(Side::Runner);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            .when(Match::targets().once(), Reply::target(dj))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(dj)))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Rig),
            )
            .when(Match::reaction().once(), Reply::take("guest of the evening"))
            .when(Match::targets().once(), Reply::target(chaos))
            .when(Match::reaction(), Reply::take("the guest leaves the game"))
            // Halt once, to read the board while DJ Fenris is still installed.
            .when(Match::action().once(), Reply::Halt)
            .otherwise_click_credit(),
    );
    g.run(&mut vm);
    assert_eq!(vm.st.objects[&chaos].host, Some(dj), "hosted: {}", g.transcript().tail(16));

    // The Runner finishes the turn; the Corp trashes the resource.
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&dj].zone,
        Zone::Discard(Side::Runner),
        "{}",
        g.transcript().tail(20)
    );
    assert_eq!(
        vm.st.objects[&chaos].zone,
        Zone::RemovedFromGame,
        "the hosted identity was removed from the game, not returned to the pile: {}",
        g.transcript().tail(20)
    );
    assert!(!vm.identity_pile(Side::Runner).contains(&chaos));
}

/// CR 1.5.4b: "if an identity card leaves the play area, it must be returned
/// to the pile outside the game" — including when 1.13.13 trashes it because
/// its host left. It does not go to the heap.
#[test]
fn an_identity_leaving_the_play_area_goes_back_to_the_pile() {
    let mut vm = Vm::empty(5506);
    tk::install_identity(&mut vm, card("Andromeda: Dispossessed Ristie"), Side::Runner);
    let dj = tk::install_rig(&mut vm, card_partial("DJ Fenris"));
    let chaos = vm.new_object(card_partial("Chaos Theory: Wünderkind"), Zone::OutsideGame(Side::Runner));
    vm.st.objects.get_mut(&chaos).unwrap().faceup = true;
    tk::host_on(&mut vm, chaos, dj);
    // Setup state, not effect: the Runner is tagged, which is what 5.2.6g
    // requires for the Corp's basic trash-resource action.
    vm.st.runner.tags = 1;
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            .when(Match::targets().once(), Reply::target(dj))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.objects[&dj].zone, Zone::Discard(Side::Runner), "{}", t.tail(16));
    // 1.13.13 trashed the hosted identity when its host changed zones — and
    // 1.5.4b decided where a trashed IDENTITY goes.
    assert_eq!(
        vm.st.objects[&chaos].zone,
        Zone::OutsideGame(Side::Runner),
        "the identity went back to the pile, not to the heap: {}",
        t.tail(16)
    );
    assert!(vm.identity_pile(Side::Runner).contains(&chaos));
}

// ---------------------------------------------------------------------------
// CR 1.15.1b — the cards that name something (`decks/unlisted.rs`)
// ---------------------------------------------------------------------------

/// Ark Lockdown: "Name a card. Remove all copies of that card in the heap
/// from the game." 2.1.4's "copies of" is a name comparison and reaches
/// every copy — and nothing else.
#[test]
fn ark_lockdown_removes_every_copy_of_the_named_card() {
    let mut vm = Vm::empty(101);
    let ark = vm.new_object(card("Ark Lockdown"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ark);
    let a = vm.new_object(card("Sure Gamble"), Zone::Discard(Side::Runner));
    let b = vm.new_object(card("Sure Gamble"), Zone::Discard(Side::Runner));
    let other = vm.new_object(card("Diesel"), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().extend([a, b, other]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(ark))
            .when(Match::name_value().once(), Reply::Name("Sure Gamble"))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(vm.st.objects[&a].zone, Zone::RemovedFromGame, "{}", t.tail(14));
    assert_eq!(vm.st.objects[&b].zone, Zone::RemovedFromGame, "{}", t.tail(14));
    assert_eq!(
        vm.st.objects[&other].zone,
        Zone::Discard(Side::Runner),
        "a card with another name is not a copy: {}",
        t.tail(14)
    );
}

/// Reclamation Order: "Name a card other than Reclamation Order. Reveal any
/// number of copies of the named card from Archives and add them to HQ."
/// CR 10.1.5 makes the exclusion self-referential — no name is written down.
#[test]
fn reclamation_order_returns_copies_of_the_named_card_from_archives() {
    let mut vm = Vm::empty(102);
    let order = vm.new_object(card("Reclamation Order"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(order);
    let a = vm.new_object(card("Hedge Fund"), Zone::Discard(Side::Corp));
    let b = vm.new_object(card("Hedge Fund"), Zone::Discard(Side::Corp));
    let other = vm.new_object(card("BOOM!"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().extend([a, b, other]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(order))
            .when(Match::name_value().once(), Reply::Name("Hedge Fund"))
            .when(Match::targets().once(), Reply::Targets(vec![a, b]))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(vm.st.objects[&a].zone, Zone::Hand(Side::Corp), "{}", t.tail(16));
    assert_eq!(vm.st.objects[&b].zone, Zone::Hand(Side::Corp), "{}", t.tail(16));
    assert_eq!(vm.st.objects[&other].zone, Zone::Discard(Side::Corp));
    // 1.16.10b: the additional [click] joined the play cost, so the whole
    // card cost two of the Corp's three clicks.
    assert_eq!(vm.st.corp.clicks, 1, "{}", t.tail(16));
}

/// Salem's Hospitality: "Choose a card name. The Runner reveals the grip and
/// trashes all cards with the chosen name revealed this way."
#[test]
fn salems_hospitality_trashes_every_copy_in_the_grip() {
    let mut vm = Vm::empty(103);
    let salem = vm.new_object(card("Salem's Hospitality"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(salem);
    let a = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let b = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let other = vm.new_object(card("Diesel"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([a, b, other]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(salem))
            .when(Match::name_value().once(), Reply::Name("Sure Gamble"))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(vm.st.objects[&a].zone, Zone::Discard(Side::Runner), "{}", t.tail(16));
    assert_eq!(vm.st.objects[&b].zone, Zone::Discard(Side::Runner), "{}", t.tail(16));
    assert_eq!(vm.st.objects[&other].zone, Zone::Hand(Side::Runner));
    // 1.21.3: the reveal showed the Corp the whole grip.
    assert!(vm.view_of(Side::Corp).sees(other));
}

/// Azmari EdTech: "When your turn ends, you may name a card type. Gain
/// 2[credit] the first time each turn the Runner plays or installs a card
/// that has the type you last named this way."
///
/// The "first time each turn" is ONE flag over both halves of "plays or
/// installs": two events of the named type in a turn pay once.
#[test]
fn azmari_edtech_names_a_type_and_pays_once_a_turn() {
    let mut vm = Vm::empty(104);
    tk::install_identity(&mut vm, card("Azmari EdTech: Shaping the Future"), Side::Corp);
    let a = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let b = vm.new_object(card("Diesel"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([a, b]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.st.runner.credits = 20;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // 9.6.9: the "you may" is the whole ability.
            .when(Match::reaction().once(), Reply::take("azmari"))
            .when(Match::options().once(), Reply::ChooseNamed("event"))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(a))
            .when(Match::action().once(), Reply::play_card(b))
            .stop_at_action(),
    );

    assert_eq!(vm.st.objects[&a].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.objects[&b].zone, Zone::Discard(Side::Runner));
    assert_eq!(
        vm.st.corp.credits, 5,
        "3 basic credit actions + ONE payout for two events — 9.3.6g's flag \
         is shared by \"plays or installs\": {}",
        t.tail(24)
    );
}

/// Falsified Credentials: "Name a card type. Expose a card in a remote
/// server, then gain 5[credit] if the exposed card has the named card type."
#[test]
fn falsified_credentials_pays_only_when_the_exposed_card_matches() {
    for (named, expected) in [("upgrade", 6u32), ("agenda", 1u32)] {
        let mut vm = Vm::empty(105);
        let fc = vm.new_object(card("Falsified Credentials"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(fc);
        let upgrade = tk::install_root(
            &mut vm,
            PrintedCard::vanilla("Loose Upgrade", Side::Corp, CardType::Upgrade),
            ServerId::Remote(1),
            false,
        );
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 2;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(fc))
                .when(Match::options().once(), Reply::ChooseNamed(named))
                .when(Match::targets().once(), Reply::target(upgrade))
                .stop_at_action(),
        );

        // 1.21.4: exposing is revealing, so the Runner has seen it either way.
        assert!(vm.view_of(Side::Runner).sees(upgrade), "{}", t.tail(16));
        assert_eq!(
            vm.st.runner.credits, expected,
            "named {named}, exposed an upgrade: {}",
            t.tail(16)
        );
    }
}

/// Ibrahim Salem: "As an additional cost to rez Ibrahim Salem, forfeit an
/// agenda. When your turn begins, name a card type. Look at the Runner's grip
/// and trash 1 card in it of the named type."
#[test]
fn ibrahim_salem_names_a_type_and_trashes_one_of_it_from_the_grip() {
    let mut vm = Vm::empty(106);
    tk::install_root(&mut vm, card("Ibrahim Salem"), ServerId::Remote(1), true);
    let event = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
    let hardware = vm.new_object(card("Desperado"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([event, hardware]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().once(), Reply::take("ibrahim salem"))
            .when(Match::options().once(), Reply::ChooseNamed("hardware"))
            .when(Match::targets().once(), Reply::target(hardware))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(vm.st.objects[&hardware].zone, Zone::Discard(Side::Runner), "{}", t.tail(20));
    assert_eq!(
        vm.st.objects[&event].zone,
        Zone::Hand(Side::Runner),
        "only a card of the NAMED type is trashed: {}",
        t.tail(20)
    );
    // 1.21.2: the Corp looked at the grip, so it saw the card it left behind.
    assert!(vm.view_of(Side::Corp).sees(event));
}

/// Wari: "…you may trash Wari to name sentry, code gate or barrier. Expose a
/// piece of ice, then add it to HQ if it has the named subtype."
///
/// The choice outlives the card that made it: 9.10.3c would expire it at the
/// next checkpoint, since paying the [trash] cost put Wari in the heap before
/// the exposure ever happened, so the card states the run as its duration.
#[test]
fn wari_names_a_subtype_and_bounces_matching_ice() {
    for (named, bounced) in [("Barrier", true), ("Sentry", false)] {
        let mut vm = Vm::empty(107);
        let wari = tk::install_rig(&mut vm, card("Wari"));
        let mut barrier = tk::vanilla_ice("Loose Barrier", 1, 1);
        barrier.subtypes = vec!["Barrier"];
        let ice = tk::install_ice(&mut vm, barrier, ServerId::Remote(1), false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Hq))
                .when(Match::reaction().once(), Reply::take("wari"))
                .when(Match::nested_cost().once(), Reply::PayCost(true))
                .when(Match::options().once(), Reply::ChooseNamed(named))
                .when(Match::targets().once(), Reply::target(ice))
                .stop_at_action(),
        );

        assert_eq!(vm.st.objects[&wari].zone, Zone::Discard(Side::Runner), "{}", t.tail(24));
        let expected = if bounced { Zone::Hand(Side::Corp) } else { Zone::Ice(ServerId::Remote(1)) };
        assert_eq!(
            vm.st.objects[&ice].zone, expected,
            "named {named} against a barrier: {}",
            t.tail(24)
        );
        if !bounced {
            // 1.21.4: the ice was exposed either way — but 1.21.6 expires the
            // sighting for the copy that MOVED, so only this branch can look.
            assert!(vm.view_of(Side::Runner).sees(ice), "exposed: {}", t.tail(24));
        }
    }
}

/// Harmony AR Therapy: "Choose up to 5 cards with different names in your
/// heap. Shuffle those cards into your stack. Remove this event from the
/// game." CR 2.1.5 is on the SET: five copies of one card offer one pick.
#[test]
fn harmony_ar_therapy_takes_one_card_per_name() {
    let mut vm = Vm::empty(108);
    let harmony = vm.new_object(card("Harmony AR Therapy"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(harmony);
    let a = vm.new_object(card("Sure Gamble"), Zone::Discard(Side::Runner));
    let b = vm.new_object(card("Sure Gamble"), Zone::Discard(Side::Runner));
    let c = vm.new_object(card("Diesel"), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().extend([a, b, c]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(harmony))
            // Both Sure Gambles are asked for; 2.1.5 keeps one.
            .when(Match::targets().once(), Reply::Targets(vec![a, b, c]))
            .stop_at_action(),
    );

    assert_eq!(vm.st.objects[&a].zone, Zone::Deck(Side::Runner), "{}", t.tail(16));
    assert_eq!(
        vm.st.objects[&b].zone,
        Zone::Discard(Side::Runner),
        "2.1.5: the second copy shares a name, so it cannot be chosen too: {}",
        t.tail(16)
    );
    assert_eq!(vm.st.objects[&c].zone, Zone::Deck(Side::Runner));
    // "Remove this event from the game" — 8.2.2's replaced destination, so
    // step 8.6.7g does not put it in the heap.
    assert_eq!(vm.st.objects[&harmony].zone, Zone::RemovedFromGame, "{}", t.tail(16));
}

/// Asmund Pudlat: "…search your stack for up to 2 virus or weapon cards with
/// different names. Host those cards faceup on this resource." 2.1.5 applies
/// to a search in the same words it applies to a choice.
#[test]
fn asmund_pudlat_finds_two_differently_named_virus_or_weapon_cards() {
    let mut vm = Vm::empty(109);
    let asmund = vm.new_object(card("Asmund Pudlat"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(asmund);
    let mut virus = PrintedCard::vanilla("Loose Virus", Side::Runner, CardType::Program);
    virus.subtypes = vec!["Virus"];
    let v1 = vm.new_object(virus.clone(), Zone::Deck(Side::Runner));
    let v2 = vm.new_object(virus, Zone::Deck(Side::Runner));
    let mut weapon = PrintedCard::vanilla("Loose Weapon", Side::Runner, CardType::Program);
    weapon.subtypes = vec!["Weapon"];
    let w = vm.new_object(weapon, Zone::Deck(Side::Runner));
    let plain = vm.new_object(card("Sure Gamble"), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().extend([v1, v2, w, plain]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asmund)))
            // Both viruses and the weapon are asked for; 2.1.5 keeps one of
            // the two same-named viruses, and 2.16's "or" lets the weapon in.
            .when(Match::targets().once(), Reply::Targets(vec![v1, v2, w]))
            .stop_at_action(),
    );

    assert_eq!(vm.st.objects[&v1].host, Some(asmund), "{}", t.tail(20));
    assert_eq!(vm.st.objects[&w].host, Some(asmund), "2.16's \"or\": {}", t.tail(20));
    assert_eq!(
        vm.st.objects[&v2].host, None,
        "2.1.5: the second Loose Virus shares a name: {}",
        t.tail(20)
    );
    // 1.21.1: hosted FACEUP, so the Corp is entitled to what they are.
    assert!(vm.view_of(Side::Corp).sees(v1), "{}", t.tail(20));
    // 1.13.2a: hosted is not installed.
    assert!(!vm.is_installed(&vm.st.objects[&v1]));
    // Nothing else was found: 8.7.2a's criteria.
    assert_eq!(vm.st.objects[&plain].zone, Zone::Deck(Side::Runner));
}

// ---------------------------------------------------------------------------
// The last six (W21)
// ---------------------------------------------------------------------------

/// Subliminal Messaging: "Gain 1[credit]." / "The first time each turn you
/// play a copy of Subliminal Messaging, gain [click]."
///
/// Two copies are played on purpose. 10.1.5's "a copy of" reaches both, and
/// the ordinal is a stipulation IN the condition — so the second play meets
/// no condition at all, and the Corp gains exactly one [click] however many
/// copies it plays. (9.3.6g's flag would have given a click per copy: it is
/// keyed to the object.)
#[test]
fn subliminal_messaging_gives_one_click_however_many_copies_are_played() {
    let mut vm = Vm::empty(600);
    let s1 = vm.new_object(card("Subliminal Messaging"), Zone::Hand(Side::Corp));
    let s2 = vm.new_object(card("Subliminal Messaging"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().extend([s1, s2]);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(s1))
            .when(Match::action().once(), Reply::play_card(s2))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.credits, 2, "1[credit] per copy played: {}", t.tail(14));
    let clicks_gained = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::ClicksGained { side: Side::Corp, amount: 1 }))
        .count();
    assert_eq!(clicks_gained, 1, "the FIRST copy each turn only: {}", t.tail(14));
    // 3 allotted − 2 spent playing + 1 gained.
    assert_eq!(vm.st.corp.clicks, 2, "{}", t.tail(14));
}

/// Subliminal Messaging: "When your turn begins, if this card is in Archives
/// and the Runner did not initiate any runs during their last turn, you may
/// reveal this card and add it to HQ."
///
/// CR 9.1.8b: the ability STATES the zone it works from, so it is active in
/// Archives — an operation's abilities are otherwise dead the moment it is
/// trashed (8.6.7g). The Runner's turn is played twice over, once with a run
/// and once without, because the requirement is what decides whether the
/// ability is offered at all.
#[test]
fn subliminal_messaging_comes_back_from_archives_only_after_a_quiet_runner_turn() {
    for runner_ran in [false, true] {
        let mut vm = Vm::empty(601);
        let sub = vm.new_object(card("Subliminal Messaging"), Zone::Discard(Side::Corp));
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(sub);
        tk::fill_deck(&mut vm, Side::Corp, 6);
        tk::fill_deck(&mut vm, Side::Runner, 6);

        // A whole Runner turn first, spent either on a run or on credits.
        vm.start_turn(Side::Runner);
        let runner_plan = if runner_ran {
            Plan::runner().when(Match::action().first(), Reply::run(ServerId::Archives))
        } else {
            Plan::runner()
        };
        // The Corp's plan carries the answer through the turn boundary: the
        // "turn begins" window belongs to the turn that starts as the
        // Runner's ends, so it is inside this same run of the driver.
        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::reaction().offering("subliminal"), Reply::take("subliminal"))
                .when(Match::action(), Reply::Halt),
            runner_plan.otherwise_click_credit(),
        );
        assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's turn came round");
        assert_eq!(
            vm.st.objects[&sub].zone,
            if runner_ran { Zone::Discard(Side::Corp) } else { Zone::Hand(Side::Corp) },
            "runner_ran={runner_ran}: {}",
            t.tail(14)
        );
        assert_eq!(
            vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { obj, .. } if *obj == sub)),
            !runner_ran,
            "1.21.3: it is revealed exactly when it comes back: {}",
            t.tail(14)
        );
    }
}

/// Petty Cash: "Play only if you have not finished an action yet this turn."
/// / "Gain 5[credit]. If you played this operation from anywhere except HQ,
/// gain [click]."
///
/// CR 5.2.2a is what "finished" means, so the FIRST action of the turn may
/// be playing it and the second may not. Played out of HQ it gains no
/// [click]: the requirement is about where the play placed the card from.
#[test]
fn petty_cash_is_a_first_action_only_and_gains_no_click_from_hq() {
    for spend_first in [false, true] {
        let mut vm = Vm::empty(602);
        let pc = vm.new_object(card("Petty Cash"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(pc);
        tk::fill_deck(&mut vm, Side::Corp, 6);
        tk::fill_deck(&mut vm, Side::Runner, 6);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Corp);

        let corp = if spend_first {
            Plan::corp().when(Match::action().once(), Reply::credit())
        } else {
            Plan::corp()
        };
        let t = plan::play(&mut vm, corp.stop_at_action(), Plan::runner());
        let offered = t
            .first_window(Kind::Action, Side::Corp)
            .actions()
            .iter()
            .any(|o| matches!(o, jinteki_cr::decision::ActionOption::BasicPlayOperation { card } if *card == pc));
        assert!(offered, "the first action of the turn may be playing it: {}", t.tail(10));
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action().once(), Reply::play_card(pc)).stop_at_action(),
            Plan::runner(),
        );
        if spend_first {
            // 9.1.8c: an action was finished first, so the play is not an
            // option at all and the plan's rule found nothing to take.
            assert_eq!(vm.st.corp.credits, 6, "1 from the credit action only: {}", t.tail(12));
            assert_eq!(vm.st.objects[&pc].zone, Zone::Hand(Side::Corp), "still in HQ");
        } else {
            assert_eq!(vm.st.corp.credits, 7, "5 − 3 play cost + 5: {}", t.tail(12));
            assert_eq!(
                vm.st.corp.clicks, 2,
                "3 allotted − 1 for the action, and NO [click] for a play from HQ: {}",
                t.tail(12)
            );
            assert_eq!(vm.st.objects[&pc].zone, Zone::Discard(Side::Corp));
        }
    }
}

/// Petty Cash: "[click]: Play this operation from Archives. After it
/// resolves, remove it from the game."
///
/// The whole third line, and the second half of the second: played from
/// Archives it gains 5[credit] AND the [click] the sentence promises, and CR
/// 8.6.6d keeps step 8.6.7g from trashing it — it is removed from the game
/// instead. The replay is a turn later because the card says so: playing it
/// out of HQ FINISHES an action, and its own restriction then forbids the
/// second play for the rest of that turn.
///
/// 5.2.1: an ability with [click] in its cost is an ACTION, so the offer sits
/// in the action window — and 9.3.3c keeps it out of that window entirely
/// while the card is still in HQ.
#[test]
fn petty_cash_replays_itself_out_of_archives_and_leaves_the_game() {
    let mut vm = Vm::empty(603);
    let pc = vm.new_object(card("Petty Cash"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(pc);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 9;
    vm.start_turn(Side::Corp);

    // Turn one: play it out of HQ as the first action, then spend the turn.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(pc))
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert!(
        !t.ever_offered("play it again from archives"),
        "9.3.3c: never on offer while the card was in HQ: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.objects[&pc].zone, Zone::Discard(Side::Corp), "{}", t.tail(20));

    // The Runner's turn, then the Corp's next — where the flashback is the
    // first action, so nothing has been finished yet.
    plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's turn came round");
    let before = vm.st.corp.credits;
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::take("play it again from archives"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&pc].zone,
        Zone::RemovedFromGame,
        "8.6.6d: not trashed, removed: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.corp.credits, before - 3 + 5, "the play cost and the gain: {}", t.tail(20));
    let clicks_gained = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::ClicksGained { side: Side::Corp, amount: 1 }))
        .count();
    assert_eq!(clicks_gained, 1, "played from anywhere except HQ: {}", t.tail(20));
    // 3 allotted − 1 for the [click] ability + 1 gained.
    assert_eq!(vm.st.corp.clicks, 3, "{}", t.tail(20));
}

/// Slot Machine: "When the Runner encounters this ice, they put the top card
/// of the stack on the bottom, then you reveal the top 3 cards of the stack."
/// / "[subroutine] The Runner loses 3[credit]." / "…If you revealed 2 or more
/// cards that share a type when this encounter began, gain 3[credit]." /
/// "…If you revealed 3 or more cards that share a type…, place 3 advancement
/// tokens on an installed card."
///
/// One stack per outcome, because the two later subroutines are the same
/// question asked with different numbers: three of a kind pays both, two of a
/// kind pays one, three different types pays neither.
#[test]
fn slot_machine_pays_out_on_the_types_it_revealed() {
    // (the three cards under the top one, largest same-type group, gains)
    let cases: [( [CardType; 3], u32, bool ); 3] = [
        ([CardType::Event, CardType::Event, CardType::Event], 3, true),
        ([CardType::Event, CardType::Event, CardType::Program], 3, false),
        ([CardType::Event, CardType::Program, CardType::Resource], 0, false),
    ];
    for (types, gained, advanced) in cases {
        let mut vm = Vm::empty(604);
        let sm = tk::install_ice(&mut vm, card("Slot Machine"), ServerId::Hq, true);
        // The top card goes to the bottom before the reveal, so it is NOT one
        // of the three the subroutines ask about.
        let bottomed = vm.new_object(
            tk::vanilla_runner_card("Bottomed", CardType::Hardware),
            Zone::Deck(Side::Runner),
        );
        vm.st.deck.get_mut(&Side::Runner).unwrap().push(bottomed);
        for (i, t) in types.iter().enumerate() {
            let name: &'static str = Box::leak(format!("reel-{i}").into_boxed_str());
            let id = vm.new_object(tk::vanilla_runner_card(name, *t), Zone::Deck(Side::Runner));
            vm.st.deck.get_mut(&Side::Runner).unwrap().push(id);
        }
        tk::fill_deck(&mut vm, Side::Runner, 3);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.st.runner.credits = 5;
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::targets(), Reply::target(sm)),
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Hq))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.deck[&Side::Runner].last(),
            Some(&bottomed),
            "1.14.5: the RUNNER put their top card on the bottom: {}",
            t.tail(16)
        );
        assert_eq!(vm.st.runner.credits, 2, "the first subroutine: {}", t.tail(16));
        assert_eq!(vm.st.corp.credits, gained, "{types:?}: {}", t.tail(16));
        assert_eq!(
            vm.st.objects[&sm].counter(CounterKind::Advancement) == 3,
            advanced,
            "{types:?}: three of a kind advances, nothing else does: {}",
            t.tail(16)
        );
    }
}

/// Miss Bones: "Place 12[credit] from the bank on Miss Bones when she is
/// installed." / "Use these credits to trash installed cards."
///
/// CR 1.10.3c is the whole sentence: hosted credits may only be spent as the
/// card's ability allows, and this card allows one class of payment. The
/// Runner's own pool is empty in both halves, so the credits are the only
/// thing that could pay — and 1.15.2c's reading of "installed cards" is what
/// separates them: the asset in a remote is trashable, the operation being
/// accessed in HQ is not.
#[test]
fn miss_bones_pays_to_trash_installed_cards_and_nothing_else() {
    for installed in [true, false] {
        let mut vm = Vm::empty(605);
        let mb = tk::install_rig(&mut vm, card("Miss Bones"));
        {
            let o = vm.st.objects.get_mut(&mb).unwrap();
            o.counters.insert(CounterKind::Credit, 12);
            o.loaded_kinds.insert(CounterKind::Credit);
        }
        let mut loot = PrintedCard::vanilla("Loot", Side::Corp, CardType::Asset);
        loot.trash_cost = Some(3);
        let (server, target) = if installed {
            (ServerId::Remote(1), tk::install_root(&mut vm, loot, ServerId::Remote(1), true))
        } else {
            let id = vm.new_object(loot, Zone::Hand(Side::Corp));
            vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
            (ServerId::Hq, id)
        };
        tk::fill_deck(&mut vm, Side::Corp, 4);
        tk::fill_deck(&mut vm, Side::Runner, 4);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(server))
                .when(Match::of(Kind::MidAccess).once(), Reply::trash_accessed())
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&target].zone == Zone::Discard(Side::Corp),
            installed,
            "installed={installed}: the trash happens only where the card allows the credits: {}",
            t.tail(16)
        );
        assert_eq!(
            vm.st.objects[&mb].counter(CounterKind::Credit),
            if installed { 9 } else { 12 },
            "installed={installed}: 1.10.3a — the credits left the card only for the payment \
             it was allowed to make: {}",
            t.tail(16)
        );
        assert_eq!(vm.st.runner.credits, 0, "nothing came out of the pool");
    }
}

/// Boomerang: "When you install this hardware, choose 1 installed piece of
/// ice. Use this hardware only during encounters with that ice." /
/// "[trash]: Break up to 2 subroutines. When this run ends, if it was
/// successful, you may shuffle 1 copy of Boomerang from your heap into your
/// stack."
///
/// Two pieces of ice protect the same server, and the run passes both: the
/// break ability is offered during the encounter with the CHOSEN ice and
/// during no other (9.3.3c against 9.10.3's remembered object). The second
/// copy in the heap comes back at the end of a successful run — 10.1.5's "a
/// copy of", which is the only kind this one could ever reach, since the copy
/// that broke the subroutines is the one that was trashed.
#[test]
fn boomerang_breaks_only_its_chosen_ice_and_comes_back_from_the_heap() {
    let mut vm = Vm::empty(606);
    let chosen = tk::install_ice(&mut vm, tk::etr_ice("Chosen Wall", 0, 1), ServerId::Hq, true);
    let other = tk::install_ice(&mut vm, tk::etr_ice("Other Wall", 0, 1), ServerId::Rnd, true);
    let boom = vm.new_object(card("Boomerang"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(boom);
    // 10.1.5: a second copy, which is what "1 copy of Boomerang" reaches.
    let spare = vm.new_object(card("Boomerang"), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(spare);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // Install, choosing the HQ ice; then run R&D, where the OTHER ice is.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(boom)))
            .when(Match::targets().once(), Reply::target(chosen))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action(), Reply::Halt),
    );
    assert!(
        !t.ever_offered("break up to 2 subroutines"),
        "9.3.3c: not during an encounter with any other ice: {}",
        t.tail(24)
    );
    assert_eq!(vm.st.objects[&other].zone, Zone::Ice(ServerId::Rnd));

    // Now run HQ, where the chosen ice is.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::paid().offering("break up to 2").once(), Reply::take("break up to 2"))
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("End the run"))
            .when(Match::reaction().offering("shuffle a copy back"), Reply::take("shuffle a copy back"))
            // 10.1.5: BOTH copies in the heap are "a copy of Boomerang" — the
            // one just trashed to pay the cost included — so the Runner picks.
            .when(Match::targets().once(), Reply::target(spare))
            .stop_at_action(),
    );
    assert!(
        t.ever_offered("break up to 2 subroutines"),
        "offered during the encounter with the chosen ice: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&boom].zone,
        Zone::Discard(Side::Runner),
        "[trash] paid: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&spare].zone,
        Zone::Deck(Side::Runner),
        "9.6.13: the delayed conditional shuffled a copy back at the end of the run: {}",
        t.tail(24)
    );
}

/// CR 8.5.16b declares the install destination — and the Runner has none to
/// declare. 8.5.4 puts programs, hardware and resources in the rig and
/// nowhere else, so the declaration has one answer and no second answer is
/// imaginable. It was being asked anyway, which put a prompt reading "Where
/// does it go? / Your rig" and a click in front of every install the Runner
/// ever made.
///
/// The Corp's declaration is untouched, and deliberately: which server, in it
/// or protecting it, or a new remote, is a real choice, and a restriction
/// that leaves one of them standing is a narrowed choice rather than the
/// absence of one.
#[test]
fn a_runner_install_never_asks_where_because_the_rig_is_the_only_answer() {
    let mut vm = Vm::empty(608);
    let desperado = vm.new_object(card("Desperado"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(desperado);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(desperado)))
            .stop_at_action(),
    );
    assert!(
        t.of_kind(Kind::Destination).is_empty(),
        "8.5.4: the rig is the only place it could go, so nobody was asked: {}",
        t.tail(12)
    );
    assert_eq!(
        vm.st.objects[&desperado].zone,
        Zone::Rig,
        "…and it went there regardless: {}",
        t.tail(12)
    );
    assert_eq!(vm.st.runner.credits, 2, "8.5.16d: the 3[c] install cost was paid");
}

/// The same card with nothing to choose. CR 1.15.2b caps an announcement at
/// the eligible targets that exist, so installing Boomerang onto a table with
/// no ice on it announces NOTHING — and an announcement that could only ever
/// be empty is not a choice the Runner makes. Asking for it stopped the game
/// on the live server: "Choose 0 cards", no candidate to draw, no candidate
/// to tap, and no button, because the one that ends a selection early was
/// offered only for an explicit "up to".
///
/// The install itself still completes. 8.5 never depended on the choice, and
/// the result is a piece of hardware that can never be used — no encounter
/// can be with the ice it does not remember — which is the Runner's problem
/// and not the rules engine's.
#[test]
fn boomerang_installed_with_no_ice_on_the_table_asks_nothing_and_stops_nothing() {
    let mut vm = Vm::empty(606);
    let boom = vm.new_object(card("Boomerang"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(boom);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(boom)))
            .stop_at_action(),
    );
    assert!(
        t.windows(Kind::Targets, Side::Runner).is_empty(),
        "1.15.2b: no ice exists, so there was no announcement to make and the \
         Runner was never asked to make one: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.objects[&boom].zone,
        Zone::Rig,
        "8.5: the install finished regardless: {}",
        t.tail(16)
    );
    // The run is the proof that the game did not stop: the driver reached a
    // later action window and spent it.
    assert!(
        t.windows(Kind::Action, Side::Runner).len() >= 2,
        "the turn carried on to the next action: {}",
        t.tail(16)
    );
}

/// Paperclip: "Whenever you encounter a barrier, you may install this program
/// from your heap." / "X[credit]: +X strength. Then, if this program can
/// interface with the barrier you are encountering, break up to X
/// subroutines."
///
/// CR 9.1.8b puts the first ability in the heap — the sentence states the
/// zone it works from — and 9.6.5d puts the interface question after the
/// pump: the Runner announces X = 3 against a strength-3 barrier that
/// Paperclip's printed 1 could not have matched when the ability was offered,
/// and breaks its subroutine anyway.
#[test]
fn paperclip_installs_itself_out_of_the_heap_and_pumps_before_it_breaks() {
    let mut vm = Vm::empty(607);
    let wall = tk::install_ice(&mut vm, tk::etr_ice("Big Wall", 0, 3), ServerId::Hq, true);
    vm.st.objects.get_mut(&wall).unwrap().printed.subtypes = vec!["Barrier"];
    let clip = vm.new_object(card("Paperclip"), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(clip);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 8;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::reaction().offering("out of the heap").once(), Reply::take("out of the heap"))
            .when(Match::destination(), Reply::Destination(jinteki_cr::instr::InstallDest::Rig))
            .when(Match::paid().offering("pump and break").once(), Reply::take("pump and break"))
            .when(Match::declare_x().once(), Reply::DeclareX(3))
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("End the run"))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&clip].zone,
        Zone::Rig,
        "9.1.8b: the ability worked from the heap: {}",
        t.tail(24)
    );
    // 8 − 4 install − 3 announced for X.
    assert_eq!(vm.st.runner.credits, 1, "{}", t.tail(24));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "the subroutine was broken, so the run was not ended: {}",
        t.tail(24)
    );
}

/// Career Fair: "Install 1 resource from your grip, paying 3[credit] less."
///
/// Reported from a live game: the click was spent, the resource was chosen,
/// and the card stayed in the grip. This test is that report.
#[test]
fn career_fair_actually_installs_the_resource_it_chose() {
    let mut vm = Vm::empty(4800);
    let cf = vm.new_object(card("Career Fair"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(cf);
    let hotel = vm.new_object(card("Earthrise Hotel"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(hotel);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(cf))
            .when(Match::targets().once(), Reply::Targets(vec![hotel]))
            .when(Match::targets().once(), Reply::Targets(vec![]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&hotel].zone,
        Zone::Rig,
        "Earthrise Hotel is installed, not left in the grip: {}",
        t.tail(24)
    );
    // Install 4, paying 3 less, floored at 0 by 1.16.2a — so 1 credit.
    assert_eq!(vm.st.runner.credits, 4, "paid 1 (install 4 minus 3): {}", t.tail(24));
}

// ---------------------------------------------------------------------------
// The identity queue (docs/vm/IDENTITY-QUEUE.md) — Runner, Criminal
// ---------------------------------------------------------------------------

/// Gabriel Santiago: "The first time you make a successful run on HQ each
/// turn, gain 2[credit]."
///
/// Two HQ runs and one on Archives, all successful: the ordinal is what is
/// under test, so only the first HQ run may pay, and the run on another
/// central must not pay at all.
#[test]
fn gabriel_santiago_pays_for_the_first_hq_run_each_turn() {
    let mut vm = Vm::empty(6001);
    tk::install_identity(
        &mut vm,
        card("Gabriel Santiago: Consummate Professional"),
        Side::Runner,
    );
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
            .count(),
        3,
        "all three runs were successful: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.credits,
        2,
        "only the first HQ run of the turn paid: {}",
        t.tail(24)
    );
}

/// Los: "The first time the Corp rezzes a piece of ice each turn, gain
/// 2[credit]."
///
/// Two pieces of ice on the attacked server, both rezzed as the Runner
/// approaches them: the first rez pays, the second does not.
#[test]
fn los_pays_for_the_first_ice_rez_each_turn() {
    let mut vm = Vm::empty(6002);
    tk::install_identity(&mut vm, card("Los: Data Hijacker"), Side::Runner);
    let inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner Wall", 1, 1), ServerId::Archives, false);
    let outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer Wall", 1, 1), ServerId::Archives, false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert!(
        vm.st.objects[&inner].faceup && vm.st.objects[&outer].faceup,
        "both pieces of ice were rezzed: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.credits,
        2,
        "only the first rez of the turn paid: {}",
        t.tail(24)
    );
}

/// Liza Talking Thunder: "The first time you make a successful run on a
/// central server each turn, draw 2 cards and take 1 tag."
///
/// Both halves of the sentence, and the ordinal: two successful central runs
/// draw two cards in total and hand over exactly one tag.
#[test]
fn liza_draws_two_and_takes_a_tag_on_the_first_central_run() {
    let mut vm = Vm::empty(6003);
    tk::install_identity(
        &mut vm,
        card("Liza Talking Thunder: Prominent Legislator"),
        Side::Runner,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip starts empty");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "two cards, once — not four: {}",
        t.tail(24)
    );
    assert_eq!(vm.st.runner.tags, 1, "and exactly one tag: {}", t.tail(24));
}

/// Laramy Fisk: "The first time you make a successful run on a central server
/// each turn, you may force the Corp to draw 1 card."
///
/// The "you may" is the whole ability, so 9.6.9 puts the choice where the
/// reaction window puts every declinable pending: the Runner may take the
/// offer or pass it. The proof has two halves — passing leaves HQ alone, and
/// taking it makes the Corp draw — and the offer is made once a turn either
/// way, however many central runs follow.
#[test]
fn laramy_fisk_lets_the_runner_force_one_corp_draw_a_turn() {
    for accept in [false, true] {
        let mut vm = Vm::empty(6004);
        tk::install_identity(&mut vm, card("Laramy Fisk: Savvy Investor"), Side::Runner);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);
        assert!(vm.st.hand[&Side::Corp].is_empty(), "HQ starts empty");

        let mut runner = Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives));
        if accept {
            runner = runner
                .when(Match::reaction().offering("laramy fisk").once(), Reply::take("laramy fisk"));
        }
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            runner.when(Match::action().once(), Reply::run(ServerId::Hq)).stop_at_action(),
        );
        // 9.6.9: the offer was really made, to the Runner, and only on the
        // first central run of the turn — the HQ run that follows gets none.
        let offers: Vec<_> = t
            .of_kind(Kind::Reaction)
            .into_iter()
            .filter(|e| plan::count_labelled(plan::window_options(&e.spec), "laramy fisk") > 0)
            .collect();
        assert_eq!(offers.len(), 1, "offered once, on the first central run: {}", t.tail(24));
        assert_eq!(offers[0].side, Side::Runner, "the ability's controller decides");
        assert_eq!(
            vm.st.hand[&Side::Corp].len(),
            usize::from(accept),
            "the Corp drew exactly when the Runner said so (accept={accept}): {}",
            t.tail(24)
        );
    }
}

/// Leela Patel: "Whenever an agenda is scored or stolen, add 1 unrezzed card
/// to HQ."
///
/// Both halves of the one printed sentence, in one game each, and both halves
/// of "unrezzed": an unrezzed piece of ice is the only card the Runner is
/// offered — the rezzed asset beside it is never a candidate — and the card
/// really lands in HQ.
#[test]
fn leela_patel_bounces_an_unrezzed_card_on_a_score_and_on_a_steal() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(6005);
        tk::install_identity(&mut vm, card("Leela Patel: Trained Pragmatist"), Side::Runner);
        let agenda =
            tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
        vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
        // 8.1.2's pair, side by side: one installed facedown Corp card and
        // one installed faceup one.
        let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 1, 1), ServerId::Archives, false);
        let asset =
            tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(2), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);

        let t = if stolen {
            vm.start_turn(Side::Runner);
            let t = plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .stop_at_action(),
            );
            assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Runner));
            t
        } else {
            vm.start_turn(Side::Corp);
            let t = plan::play(
                &mut vm,
                Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
                Plan::runner(),
            );
            assert_eq!(vm.st.objects[&agenda].zone, Zone::ScoreArea(Side::Corp));
            t
        };

        // 9.1.1a: the ability is the Runner's identity's, so the Runner
        // announces the target — even on the half the CORP's score meets.
        let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
        assert_eq!(announcements.len(), 1, "one target announcement (stolen={stolen}): {}", t.tail(24));
        assert_eq!(announcements[0].side, Side::Runner, "the ability's controller announces");
        assert_eq!(
            announcements[0].candidates(),
            [ice],
            "only the unrezzed card is a candidate — the rezzed asset is not (stolen={stolen}): {}",
            t.tail(24)
        );
        assert_eq!(
            vm.st.objects[&ice].zone,
            Zone::Hand(Side::Corp),
            "the unrezzed card is in HQ (stolen={stolen}): {}",
            t.tail(24)
        );
        assert_eq!(
            vm.st.objects[&asset].zone,
            Zone::Root(ServerId::Remote(2)),
            "the rezzed card stayed where it was (stolen={stolen}): {}",
            t.tail(24)
        );
    }
}

/// Nyusha "Sable" Sintashta: "When your turn begins, identify your mark."
/// "The first time each turn you make a successful run on your mark, gain
///  [click]."
///
/// Both sentences: the turn begins and a central server becomes the mark
/// (10.11.2a), then three successful runs — two on the mark and one on
/// another central — pay exactly one click, on the first run on the mark.
#[test]
fn nyusha_identifies_a_mark_and_pays_the_first_run_on_it() {
    let mut vm = Vm::empty(6006);
    tk::install_identity(
        &mut vm,
        card("Nyusha \"Sable\" Sintashta: Symphonic Prodigy"),
        Side::Runner,
    );
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    assert_eq!(vm.mark(), None, "10.11.2: no mark until a card identifies one");

    // The mark is identified in the turn-begin window, before the first
    // action — so the plan can be written against it.
    let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    let mark = vm.mark().map(|(s, _)| s).expect("the turn-begin ability identified a mark");
    assert_eq!(mark, ServerId::Hq, "seed 6006 designates HQ: {}", t.tail(24));
    let other = ServerId::Rnd;

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(other))
            .when(Match::action().once(), Reply::run(mark))
            .when(Match::action().once(), Reply::run(mark))
            .stop_at_action(),
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
            .count(),
        3,
        "all three runs were successful: {}",
        t.tail(24)
    );
    // 4 allotted clicks (5.6.2a), 3 spent on runs, 1 gained: 2 left. Without
    // the gain there would be 1, and with a gain per run on the mark, 3.
    assert_eq!(
        vm.st.runner.clicks, 2,
        "exactly one click, on the FIRST run on the mark: {}",
        t.tail(24)
    );
}

/// Virtual Intelligence, P.I.: "Once per turn → [click], 1[credit]: Draw 1
/// card and remove 1 tag."
///
/// The cost is paid, both halves of the effect happen, and 9.3.6g's flag is
/// spent by using it: the next action window of the same turn does not offer
/// it again, however many credits and clicks are left. (A paid ability whose
/// cost includes [click] is offered where 5.2.7 puts it — in the action
/// window — because that is the only place a click can be spent.)
#[test]
fn virtual_intelligence_draws_and_removes_a_tag_once_a_turn() {
    let mut vm = Vm::empty(6007);
    tk::install_identity(
        &mut vm,
        card("Virtual Intelligence, P.I.: \"You Can Call Me Vic\""),
        Side::Runner,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 4;
    vm.st.runner.tags = 2;
    vm.start_turn(Side::Runner);
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip starts empty");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        // No `.once()`: the plan would take the ability EVERY time it were
        // offered, so a second draw would mean the flag was not spent.
        Plan::runner().when(Match::action(), Reply::take("vic")).stop_at_action(),
    );
    let offers = t
        .of_kind(Kind::Action)
        .into_iter()
        .filter(|e| Pick::Labeled("vic").find_action(e.actions()).is_some())
        .count();
    assert_eq!(offers, 1, "9.3.6g: offered once, and not again this turn: {}", t.tail(24));
    assert_eq!(vm.st.hand[&Side::Runner].len(), 1, "drew 1 card: {}", t.tail(24));
    assert_eq!(vm.st.runner.tags, 1, "removed 1 tag: {}", t.tail(24));
    assert_eq!(vm.st.runner.credits, 3, "the credit half of the cost: {}", t.tail(24));
    assert_eq!(vm.st.runner.clicks, 3, "the click half of the cost: {}", t.tail(24));
}

/// CR 9.11.3: "usually, each SENTENCE in the text of an ability forms a
/// single instruction." 9.11.4's exceptions are plays/installs/accesses,
/// choose-then-act, nested costs, searches, look/reveal and option choices —
/// none of them splits a sentence because its effects are of different
/// classes.
///
/// This matters because 9.11.3 also says what an instruction BOUNDARY costs:
/// "After each instruction, an ability pauses its resolution to allow
/// priority windows to open… a checkpoint occurs, allowing any appropriate
/// conditional abilities to be marked as pending in a reaction window, then
/// targets are announced for the next instruction. Finally, the next
/// instruction becomes imminent, allowing interrupts relevant to that
/// instruction to resolve." Splitting an "X and Y" sentence therefore invents
/// a checkpoint, a reaction window and a second interrupt window that the
/// card does not have — so a prevention or avoidance effect gets two
/// separate chances where the card gives one, and anything conditional on
/// the first half can act before the second half is imminent.
///
/// Three cards were written the wrong way and are pinned here by shape.
#[test]
fn an_and_sentence_is_one_instruction_not_two() {
    let one_instruction = |name: &str, which: usize| {
        let c = card_partial(name);
        let ins = &c.abilities[which].instructions;
        assert_eq!(
            ins.len(),
            1,
            "{name}: one printed sentence must denote into ONE instruction (9.11.3), got {ins:?}"
        );
        assert!(
            matches!(ins[0], Instruction::Combined(_)),
            "{name}: an 'X and Y' sentence is `Combined`, got {:?}",
            ins[0]
        );
    };
    // "When your turn begins, remove 1 hosted power counter and draw 2 cards."
    one_instruction("Earthrise Hotel", 2);
    // "…draw 2 cards and take 1 tag."
    one_instruction("Liza Talking Thunder: Prominent Legislator", 0);
    // "…gain 1[credit] and flip this identity."
    one_instruction("Nebula Talent Management: Making Stars", 0);
}

// ---------------------------------------------------------------------------
// The identity queue (docs/vm/IDENTITY-QUEUE.md) — Runner, Shaper
// ---------------------------------------------------------------------------

/// Akiko Nisei: "Whenever you breach R&D, you and the Corp secretly spend
/// 0[credit], 1[credit], or 2[credit]. Reveal spent credits. If you and the
/// Corp spent the same number of credits, access 1 additional card."
///
/// The same breach twice over: matched bids access two cards out of R&D,
/// differing bids access one. The bids are also spent either way (10.14.6c),
/// which is what makes the "secretly spend" half more than flavour.
#[test]
fn akiko_nisei_accesses_a_second_card_when_the_bids_match() {
    for matched in [true, false] {
        let mut vm = Vm::empty(6010);
        tk::install_identity(&mut vm, card("Akiko Nisei: Head Case"), Side::Runner);
        let deck = tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 3;
        vm.st.corp.credits = 3;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::psi_bid(), Reply::Bid(1)),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Rnd))
                .when(Match::psi_bid(), Reply::Bid(if matched { 1 } else { 0 }))
                .stop_at_action(),
        );
        let accessed = vm
            .changes
            .log
            .iter()
            .filter(|c| {
                matches!(c, GameChange::CardAccessed { obj } if deck.contains(obj))
            })
            .count();
        assert_eq!(
            accessed,
            if matched { 2 } else { 1 },
            "7.3.5's limit rose only on the matched bid (matched={matched}): {}",
            t.tail(24)
        );
        // 10.14.6c: the bid is spent whatever the outcome.
        assert_eq!(
            vm.st.runner.credits,
            if matched { 2 } else { 3 },
            "the Runner spent their bid (matched={matched}): {}",
            t.tail(24)
        );
        assert_eq!(vm.st.corp.credits, 2, "and the Corp spent theirs (matched={matched})");
    }
}

/// Exile: "Whenever you install a program from your heap, draw 1 card."
///
/// Both stipulations of the one sentence, one at a time: a program installed
/// out of the heap draws (and 4.8.3 keeps that true when a search sets it
/// aside on the way), while a RESOURCE installed out of the same heap does
/// not — the sentence names a type, and that type is part of the condition.
#[test]
fn exile_draws_only_for_a_program_out_of_the_heap() {
    for program in [true, false] {
        let mut vm = Vm::empty(6011);
        tk::install_identity(&mut vm, card("Exile: Streethawk"), Side::Runner);
        let card_in_heap = vm.new_object(
            if program {
                tk::program_cost("Heap-Program", 0)
            } else {
                tk::vanilla_runner_card("Heap-Resource", CardType::Resource)
            },
            Zone::Discard(Side::Runner),
        );
        vm.st.discard.get_mut(&Side::Runner).unwrap().push(card_in_heap);
        tk::install_rig(&mut vm, tk::heap_install_button("Heap-Install"));
        tk::fill_deck(&mut vm, Side::Runner, 3);
        tk::fill_deck(&mut vm, Side::Corp, 3);
        let before = vm.st.hand[&Side::Runner].len();
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::paid().once(), Reply::take("heap install"))
                .when(Match::targets().once(), Reply::Targets(vec![card_in_heap]))
                .stop_at_action(),
        );
        assert_eq!(vm.st.objects[&card_in_heap].zone, Zone::Rig, "installed: {}", t.tail(24));
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            before + usize::from(program),
            "only the program install met the condition (program={program}): {}",
            t.tail(24)
        );
    }
}

/// Exile again, through 4.8.3: a Test Run-class search sets the program aside
/// before installing it, and the install is still reported as coming from the
/// heap — so the type stipulation reads the same card and the Runner draws.
#[test]
fn exile_draws_for_a_program_installed_out_of_a_set_aside() {
    let mut vm = Vm::empty(6012);
    tk::install_identity(&mut vm, card("Exile: Streethawk"), Side::Runner);
    let prog = vm.new_object(tk::program_cost("Heap-Program", 0), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(prog);
    tk::install_rig(&mut vm, tk::test_run_like("TestRun-like", Zone::Discard(Side::Runner)));
    tk::fill_deck(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    let before = vm.st.hand[&Side::Runner].len();
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("test-run")).stop_at_action(),
    );
    assert_eq!(vm.st.objects[&prog].zone, Zone::Rig, "installed: {}", t.tail(24));
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        before + 1,
        "4.8.3: the set-aside is transparent, so the condition was met: {}",
        t.tail(24)
    );
}

/// Rielle "Kit" Peddler: "The first time each turn you encounter a piece of
/// ice, it gains code gate for the remainder of this run."
///
/// Two pieces of ice on the attacked server: the outermost is encountered
/// first, gains the subtype, and KEEPS it after that encounter has ended —
/// the duration the sentence names is the run, not the encounter. The second
/// gains nothing, because the printed ordinal is a stipulation about the
/// occurrence; and when the run ends, so does the grant.
#[test]
fn rielle_kit_peddler_makes_the_first_ice_of_the_turn_a_code_gate() {
    let mut vm = Vm::empty(6013);
    tk::install_identity(&mut vm, card("Rielle \"Kit\" Peddler: Transhuman"), Side::Runner);
    let mut barrier = tk::vanilla_ice("Some Barrier", 0, 1);
    barrier.subtypes = vec!["Barrier"];
    let mut sentry = tk::vanilla_ice("Some Sentry", 0, 1);
    sentry.subtypes = vec!["Sentry"];
    // 6.2.2a: `install_ice` places each piece outermost, so the sentry is the
    // inner one and the barrier is encountered first.
    let inner = tk::install_ice(&mut vm, sentry, ServerId::Archives, true);
    let outer = tk::install_ice(&mut vm, barrier, ServerId::Archives, true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // The grant lives for the run, so it has to be read from inside one:
    // halt at 6.9.4c's jack-out decision, which follows passing the first ice.
    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::of(Kind::JackOut).once(), Reply::Halt)
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    script.run(&mut vm);
    let t = script.transcript();
    assert!(
        vm.has_subtype(outer, "Code Gate"),
        "the first ice encountered gained it, and still has it now the encounter has ended: {}",
        t.tail(28)
    );
    // 2.16.5 counts instances: the grant ADDS, it does not replace.
    assert!(vm.has_subtype(outer, "Barrier"), "and is still a barrier");
    assert!(
        !vm.has_subtype(inner, "Code Gate"),
        "the second piece of ice has not been encountered yet: {}",
        t.tail(28)
    );

    script.run(&mut vm); // resume: encounter the second ice and finish the run
    let t = script.transcript();
    assert!(
        !vm.has_subtype(inner, "Code Gate"),
        "the second encounter of the turn is not the first: {}",
        t.tail(28)
    );
    assert!(
        !vm.has_subtype(outer, "Code Gate"),
        "and the grant died with the run it was made for: {}",
        t.tail(28)
    );
}

/// Tāo Salonga: "Whenever an agenda is scored or stolen, you may swap 2
/// installed pieces of ice."
///
/// Both halves of the one sentence, and 9.1.1a's controller on each: two
/// pieces of ice protecting different servers exchange positions, and the
/// Runner announces both — even on the half the CORP's score meets.
#[test]
fn tao_salonga_swaps_two_pieces_of_ice_on_a_score_and_on_a_steal() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(6014);
        tk::install_identity(&mut vm, card("Tāo Salonga: Telepresence Magician"), Side::Runner);
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Some Agenda", 3, 2),
            ServerId::Remote(1),
            false,
        );
        vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
        let a = tk::install_ice(&mut vm, tk::vanilla_ice("Ice A", 0, 1), ServerId::Hq, false);
        let b = tk::install_ice(&mut vm, tk::vanilla_ice("Ice B", 0, 1), ServerId::Rnd, false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);

        let t = if stolen {
            vm.start_turn(Side::Runner);
            plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .when(Match::reaction().once(), Reply::take("an agenda was stolen"))
                    .when(Match::targets().once(), Reply::Targets(vec![a]))
                    .when(Match::targets().once(), Reply::Targets(vec![b]))
                    .stop_at_action(),
            )
        } else {
            vm.start_turn(Side::Corp);
            plan::play(
                &mut vm,
                Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
                Plan::runner()
                    .when(Match::reaction().once(), Reply::take("an agenda was scored"))
                    .when(Match::targets().once(), Reply::Targets(vec![a]))
                    .when(Match::targets().once(), Reply::Targets(vec![b])),
            )
        };

        assert_eq!(
            vm.st.objects[&a].zone,
            Zone::Ice(ServerId::Rnd),
            "Ice A took Ice B's position (stolen={stolen}): {}",
            t.tail(28)
        );
        assert_eq!(
            vm.st.objects[&b].zone,
            Zone::Ice(ServerId::Hq),
            "and Ice B took Ice A's (stolen={stolen}): {}",
            t.tail(28)
        );
        // 9.1.1a: it is the Runner's identity, so the Runner announces both.
        for e in t.of_kind(Kind::Targets) {
            assert_eq!(e.side, Side::Runner, "the ability's controller announces (stolen={stolen})");
        }
    }
}

/// Cupellation's third sentence, which the same 7.3.5b hole hid: "Whenever
/// you breach HQ, if this program has a hosted Corp card, you may pay
/// 1[credit] and trash this program to access 2 additional cards."
///
/// The grant is made by an ability the breach BEGINNING triggered (11.5.1),
/// two steps before 11.5.3 determines the limit — so it only means anything
/// if the determination adds it rather than overwriting it. Three cards out
/// of HQ is the whole assertion.
#[test]
fn cupellation_digs_two_cards_deeper_out_of_hq() {
    let mut vm = Vm::empty(4602);
    let cup = tk::install_rig(&mut vm, card("Cupellation"));
    let hand = tk::fill_hand(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Runner, 2);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            // Run 1: pocket a card, so the program has a hosted Corp card.
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::of(Kind::MidAccess).once(), Reply::take("pocket the evidence"))
            // Run 2: the breach begins, the deep dig is offered and paid for.
            .when(Match::action().once(), Reply::Take(Pick::Run(ServerId::Hq)))
            .when(Match::reaction().once(), Reply::take("deep dig"))
            .when(Match::of(Kind::NestedCost).once(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&cup].zone, Zone::Discard(Side::Runner), "trashed: {}", t.tail(30));
    let accessed: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CardAccessed { obj } if hand.contains(obj) => Some(*obj),
            _ => None,
        })
        .collect();
    assert_eq!(
        accessed.len(),
        4,
        "1 on the first breach, then 1 + 2 additional on the second: {}",
        t.tail(30)
    );
}

// ---------------------------------------------------------------------------
// The identity queue (docs/vm/IDENTITY-QUEUE.md) — Runner, Anarch
// ---------------------------------------------------------------------------

/// Alice Merchant: "The first time you make a successful run on Archives each
/// turn, the Corp must trash 1 card from HQ."
///
/// Three successful runs — two on Archives and one on another central — cost
/// HQ exactly one card, on the first Archives run. The choice is asserted to
/// be the CORP's: 1.14.5 hands the naming to the player the sentence names,
/// even though 9.1.1a makes the Runner the ability's controller.
#[test]
fn alice_merchant_makes_the_corp_pitch_one_card_on_the_first_archives_run() {
    let mut vm = Vm::empty(6101);
    tk::install_identity(&mut vm, card("Alice Merchant: Clan Agitator"), Side::Runner);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.hand[&Side::Corp].len(),
        2,
        "HQ lost one card, and only one — the second Archives run is not 'the first time each turn', \
         and R&D is not Archives: {}",
        t.tail(24)
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "one card was named, once: {}", t.tail(24));
    assert_eq!(
        announcements[0].side,
        Side::Corp,
        "1.14.5: the sentence says the CORP trashes, so the Corp names the card: {}",
        t.tail(24)
    );
}

/// Edward Kim: "Trash the first operation you access each turn at no cost."
///
/// Three accesses in one turn, in an order that pins both stipulations: an
/// ASSET first (not an operation, so the condition is not met and the ordinal
/// is not spent), then an operation (trashed, with nothing paid), then a
/// second operation (not "the first … each turn", so it survives).
#[test]
fn edward_kim_trashes_the_first_operation_accessed_each_turn_and_no_others() {
    let mut vm = Vm::empty(6102);
    tk::install_identity(&mut vm, card("Edward Kim: Humanity's Hammer"), Side::Runner);
    let asset = vm.new_object(tk::vanilla_asset("Some Asset", 0, 2), Zone::Hand(Side::Corp));
    let first_op = vm.new_object(tk::operation("First Op", 0, vec![]), Zone::Hand(Side::Corp));
    let second_op = vm.new_object(tk::operation("Second Op", 0, vec![]), Zone::Hand(Side::Corp));
    for id in [asset, first_op, second_op] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::install_rig(&mut vm, tk::hq_access_button("Gang-Sign-like"));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    // 7.5.4's basic trash ability must never be what trashes anything here.
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("access-hq"))
            .when(Match::targets().once(), Reply::Targets(vec![asset]))
            .when(Match::paid().once(), Reply::take("access-hq"))
            .when(Match::targets().once(), Reply::Targets(vec![first_op]))
            .when(Match::paid().once(), Reply::take("access-hq"))
            .when(Match::targets().once(), Reply::Targets(vec![second_op]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&first_op].zone,
        Zone::Discard(Side::Corp),
        "the first operation accessed this turn was trashed: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&second_op].zone,
        Zone::Hand(Side::Corp),
        "the SECOND operation is not 'the first … each turn' and stays in HQ: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Hand(Side::Corp),
        "an asset is not an operation, so accessing it neither trashes it nor spends the ordinal: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.runner.credits, 0, "'at no cost' — nothing was paid: {}", t.tail(30));
}

/// Esâ Afontov: "The first time each turn you suffer core damage, you may
/// draw 1 card and sabotage 2. (The Corp trashes 2 cards of their choice from
/// HQ and/or the top of R&D.)"
///
/// Two core damages in one turn: the offer is made once, and taking it draws
/// exactly one card and costs the Corp exactly two cards. The declined half
/// of 9.6.9 is asserted too — passing the offer leaves both decks alone.
#[test]
fn esa_afontov_draws_and_sabotages_on_the_first_core_damage_of_the_turn() {
    for accept in [false, true] {
        let mut vm = Vm::empty(6103);
        tk::install_identity(&mut vm, card("Esâ Afontov: Eco-Insurrectionist"), Side::Runner);
        tk::install_root(&mut vm, tk::core_damage_button("Hurt", 1), ServerId::Remote(1), true);
        tk::fill_hand(&mut vm, Side::Corp, 4);
        tk::fill_hand(&mut vm, Side::Runner, 4);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);
        let corp_cards_before = vm.st.hand[&Side::Corp].len() + vm.st.deck[&Side::Corp].len();
        let stack_before = vm.st.deck[&Side::Runner].len();

        let mut runner = Plan::runner();
        if accept {
            runner = runner.when(
                Match::reaction().offering("esâ afontov").once(),
                Reply::take("esâ afontov"),
            );
        }
        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::paid().once(), Reply::take("do core damage"))
                .when(Match::paid().once(), Reply::take("do core damage"))
                .stop_at_action(),
            runner,
        );
        let offers: Vec<_> = t
            .of_kind(Kind::Reaction)
            .into_iter()
            .filter(|e| plan::count_labelled(plan::window_options(&e.spec), "esâ afontov") > 0)
            .collect();
        assert_eq!(
            offers.len(),
            1,
            "offered on the FIRST core damage of the turn and not the second (accept={accept}): {}",
            t.tail(30)
        );
        assert_eq!(
            stack_before - vm.st.deck[&Side::Runner].len(),
            usize::from(accept),
            "one card drawn exactly when the Runner took the offer (accept={accept}): {}",
            t.tail(30)
        );
        assert_eq!(
            corp_cards_before - (vm.st.hand[&Side::Corp].len() + vm.st.deck[&Side::Corp].len()),
            if accept { 2 } else { 0 },
            "sabotage 2 costs the Corp two cards from HQ and/or the top of R&D, once \
             (accept={accept}): {}",
            t.tail(30)
        );
    }
}

/// MaxX: "When your turn begins, trash the top 2 cards of your stack. Draw 1
/// card."
///
/// Two printed sentences on one condition, so the top THREE cards of the
/// stack all move and each goes where its own sentence sends it: the top two
/// to the heap, the third to the grip. The boundary is the count — a third
/// card must not be trashed, and the drawn card must not be one of the two.
#[test]
fn maxx_trashes_the_top_two_cards_of_the_stack_then_draws_the_third() {
    let mut vm = Vm::empty(6104);
    tk::install_identity(&mut vm, card("MaxX: Maximum Punk Rock"), Side::Runner);
    let stack = tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip starts empty");
    vm.start_turn(Side::Runner);

    let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    assert_eq!(
        vm.st.objects[&stack[0]].zone,
        Zone::Discard(Side::Runner),
        "the top card of the stack was trashed: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&stack[1]].zone,
        Zone::Discard(Side::Runner),
        "and the second one: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&stack[2]].zone,
        Zone::Hand(Side::Runner),
        "the third card was DRAWN, not trashed — the second sentence is its own instruction: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.discard[&Side::Runner].len(),
        2,
        "exactly two cards were trashed: {}",
        t.tail(24)
    );
}

/// Nathaniel "Gnat" Hall: "When your turn begins, gain 1[credit] if you have
/// 2 or fewer cards in your grip."
///
/// The requirement is inside the condition (9.6.5c), so it is the grip AT THE
/// START OF THE TURN that decides. Two games: two cards in the grip pays, and
/// three does not.
#[test]
fn nathaniel_gnat_hall_pays_only_while_the_grip_is_down_to_two() {
    for (grip, expected) in [(2usize, 1u32), (3, 0)] {
        let mut vm = Vm::empty(6105);
        tk::install_identity(&mut vm, card("Nathaniel \"Gnat\" Hall: One-of-a-Kind"), Side::Runner);
        tk::fill_hand(&mut vm, Side::Runner, grip);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
        assert_eq!(
            vm.st.runner.credits, expected,
            "with {grip} cards in the grip the turn-begin ability pays {expected}: {}",
            t.tail(24)
        );
    }
}

// ---------------------------------------------------------------------------
// The identity queue — Runner, Neutral
// ---------------------------------------------------------------------------

/// Nova Initiumia: "Your deck cannot include more than 1 copy of any card."
/// The Catalyst: "Starter game only."
/// The Masque: "Draft format only."
///
/// All three print a deck-construction or format restriction and nothing
/// else. CR 1.4.2 settles deck legality before the game begins, so none of it
/// is an ability — there is no condition to meet and nothing to resolve — and
/// this test is what says so out loud: each identity denotes into no
/// abilities at all, and a turn played under it opens no window naming it.
#[test]
fn a_deckbuilding_restriction_is_not_an_ability_that_ever_fires() {
    for name in [
        "Nova Initiumia: Catalyst & Impetus",
        "The Catalyst: Convention Breaker",
        "The Masque: Cyber General",
        "The Professor: Keeper of Knowledge",
    ] {
        let printed = card(name);
        assert!(
            printed.abilities.is_empty(),
            "{name}: a deck-construction restriction denotes into no ability (EDSL rule of thumb 3)"
        );

        let mut vm = Vm::empty(6106);
        tk::install_identity(&mut vm, printed, Side::Runner);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
        );
        assert!(
            !t.took(&name.to_lowercase()),
            "{name}: nothing of this identity was ever offered or used during a turn: {}",
            t.tail(24)
        );
    }
}

/// Noise: "Whenever you install a virus program, the Corp trashes the top
/// card of R&D."
///
/// Both stipulations, in one game: installing a virus program mills R&D,
/// installing a non-virus program does not, and neither does installing a
/// virus-subtyped card that is not a program.
#[test]
fn noise_mills_rnd_only_for_a_virus_program() {
    let mut vm = Vm::empty(6107);
    tk::install_identity(&mut vm, card("Noise: Hacker Extraordinaire"), Side::Runner);
    let mut virus = tk::vanilla_runner_card("Some Virus", CardType::Program);
    virus.subtypes = vec!["Virus"];
    virus.cost = Some(0);
    let mut plain = tk::vanilla_runner_card("Some Program", CardType::Program);
    plain.cost = Some(0);
    // A virus that is NOT a program — the type stipulation on its own.
    let mut virus_hardware = tk::vanilla_runner_card("Some Virus Rig", CardType::Hardware);
    virus_hardware.subtypes = vec!["Virus"];
    virus_hardware.cost = Some(0);
    let ids: Vec<_> = [virus, plain, virus_hardware]
        .into_iter()
        .map(|c| {
            let id = vm.new_object(c, Zone::Hand(Side::Runner));
            vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
            id
        })
        .collect();
    let rnd = tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut runner = Plan::runner();
    for id in &ids {
        runner = runner.when(Match::action().once(), Reply::Take(Pick::InstallCard(*id)));
    }
    let t = plan::play(&mut vm, Plan::corp(), runner.stop_at_action());
    for id in &ids {
        assert_eq!(vm.st.objects[id].zone, Zone::Rig, "all three were installed: {}", t.tail(30));
    }
    assert_eq!(
        vm.st.deck[&Side::Corp].len(),
        4,
        "exactly one card left R&D — the virus PROGRAM's install, and neither of the others: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&rnd[0]].zone,
        Zone::Discard(Side::Corp),
        "and it was the top card of R&D: {}",
        t.tail(30)
    );
}

// ---------------------------------------------------------------------------
// The identity queue — Runner, Sunny Lebeau
// ---------------------------------------------------------------------------

/// Sunny Lebeau: Security Specialist — Link 2, and a blank text box.
///
/// The whole card is its base link, so the proof is the link: 5.5.2 makes it
/// a characteristic the Runner has while the identity is active, and a trace
/// the Runner would otherwise lose is one they win by 2.
#[test]
fn sunny_lebeau_is_two_link_and_nothing_else() {
    let printed = card("Sunny Lebeau: Security Specialist");
    assert!(
        printed.abilities.iter().all(|a| a.label == "base link"),
        "a blank text box denotes into the base link and nothing else"
    );
    let mut vm = Vm::empty(6108);
    tk::install_identity(&mut vm, printed, Side::Runner);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);
    plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    assert_eq!(vm.runner_link(), 2, "5.5.2: the identity's printed link is the Runner's link");
}

// ---------------------------------------------------------------------------
// The identity queue — Corp, Haas-Bioroid
// ---------------------------------------------------------------------------

/// Haas-Bioroid: Engineering the Future — "The first time you install a card
/// each turn, gain 1[credit]."
///
/// Two installs in one turn pay once. The sentence names no card type, so the
/// test installs two different ones — a piece of ice and an asset — and the
/// FIRST is what pays, whichever it is.
#[test]
fn engineering_the_future_pays_for_the_first_corp_install_of_the_turn() {
    let mut vm = Vm::empty(6109);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Engineering the Future"), Side::Corp);
    let ice = vm.new_object(tk::vanilla_ice("Some Ice", 0, 1), Zone::Hand(Side::Corp));
    let asset = vm.new_object(tk::vanilla_asset("Some Asset", 0, 2), Zone::Hand(Side::Corp));
    for id in [ice, asset] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(ice)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset)))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(
        vm.st.objects[&ice].zone.is_installed() && vm.st.objects[&asset].zone.is_installed(),
        "both cards were installed: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.corp.credits, 1,
        "only the FIRST install of the turn paid — the second is not 'the first time each turn': {}",
        t.tail(30)
    );
}

/// Sportsmetal: "Whenever an agenda is scored or stolen, gain 2[credit] or
/// draw 2 cards."
///
/// Both halves of the printed sentence, one game each, and the option choice
/// in both: the Corp is offered exactly two options and takes a different one
/// each time, so the credits and the cards are each shown to be reachable.
/// 9.1.1a is what puts the choice with the Corp even on the STEAL.
#[test]
fn sportsmetal_offers_the_corp_credits_or_cards_on_a_score_and_on_a_steal() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(6110);
        tk::install_identity(&mut vm, card("Sportsmetal: Go Big or Go Home"), Side::Corp);
        let agenda =
            tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
        vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        // Score => take the credits; steal => take the cards.
        let want_credits = !stolen;
        let pick = if want_credits { "gain 2[credit]" } else { "draw 2 cards" };

        let t = if stolen {
            vm.start_turn(Side::Runner);
            plan::play(
                &mut vm,
                Plan::corp().when(Match::of(Kind::Options).once(), Reply::ChooseNamed(pick)),
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .stop_at_action(),
            )
        } else {
            vm.start_turn(Side::Corp);
            plan::play(
                &mut vm,
                Plan::corp()
                    .when(Match::paid(), Reply::score(agenda))
                    .when(Match::of(Kind::Options).once(), Reply::ChooseNamed(pick))
                    .stop_at_action(),
                Plan::runner(),
            )
        };
        let asked = t.of_kind(Kind::Options);
        assert_eq!(asked.len(), 1, "one option choice was put (stolen={stolen}): {}", t.tail(30));
        assert_eq!(
            asked[0].side,
            Side::Corp,
            "9.1.1a: the Corp's identity, so the Corp chooses — even on a steal (stolen={stolen})"
        );
        if want_credits {
            // The Corp's own turn, so HQ holds the one card 5.6.2b's
            // mandatory draw put there and nothing this ability added.
            assert_eq!(vm.st.corp.credits, 2, "the credits were taken: {}", t.tail(30));
            assert_eq!(vm.st.hand[&Side::Corp].len(), 1, "and not the cards: {}", t.tail(30));
        } else {
            // The Runner's turn, so the Corp draws nothing of its own.
            assert_eq!(vm.st.hand[&Side::Corp].len(), 2, "the cards were taken: {}", t.tail(30));
            assert_eq!(vm.st.corp.credits, 0, "and not the credits: {}", t.tail(30));
        }
    }
}

/// Thule Subsea: "Whenever the Runner steals an agenda, do 1 core damage
/// unless they spend [click] and 2[credit]."
///
/// Both sides of "unless", in two games: a Runner who pays loses a click and
/// two credits and takes nothing, and a Runner who declines keeps them and
/// suffers the core damage — which 10.4.4 also costs them a card and a point
/// of maximum hand size.
#[test]
fn thule_subsea_charges_a_click_and_two_credits_or_does_the_core_damage() {
    for pay in [false, true] {
        let mut vm = Vm::empty(6111);
        tk::install_identity(&mut vm, card("Thule Subsea: Safety Below"), Side::Corp);
        let agenda =
            tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
        tk::fill_hand(&mut vm, Side::Runner, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                .when(Match::of(Kind::NestedCost).once(), Reply::PayCost(pay))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&agenda].zone,
            Zone::ScoreArea(Side::Runner),
            "the agenda was stolen either way (pay={pay}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.runner.credits,
            if pay { 3 } else { 5 },
            "2[credit] of the cost, paid only when the Runner said so (pay={pay}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            if pay { 3 } else { 2 },
            "10.4.4: the core damage trashed a card exactly when the cost was declined (pay={pay}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.max_hand_size(Side::Runner),
            if pay { 5 } else { 4 },
            "and lowered the maximum hand size exactly then too (pay={pay}): {}",
            t.tail(30)
        );
    }
}

// ---------------------------------------------------------------------------
// The identity queue — Corp, Jinteki
// ---------------------------------------------------------------------------

/// Jinteki: Personal Evolution — "Whenever an agenda is scored or stolen, do
/// 1 net damage."
///
/// Both halves of the one printed sentence, one game each. The damage is the
/// CORP's on both, including the half the Runner's own theft meets.
#[test]
fn personal_evolution_does_a_net_damage_on_a_score_and_on_a_steal() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(6112);
        tk::install_identity(&mut vm, card("Jinteki: Personal Evolution"), Side::Corp);
        let agenda =
            tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
        vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
        tk::fill_hand(&mut vm, Side::Runner, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);

        let t = if stolen {
            vm.start_turn(Side::Runner);
            plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .stop_at_action(),
            )
        } else {
            vm.start_turn(Side::Corp);
            plan::play(
                &mut vm,
                Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
                Plan::runner(),
            )
        };
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            2,
            "one net damage, once (stolen={stolen}): {}",
            t.tail(30)
        );
        assert!(
            vm.changes.log.iter().any(
                |c| matches!(c, GameChange::DamageSuffered { kind, .. } if *kind == jinteki_cr::effects::DamageKind::Net)
            ),
            "and it was NET damage (stolen={stolen}): {}",
            t.tail(30)
        );
    }
}

/// Jinteki: Potential Unleashed — "Whenever the Runner takes at least 1 net
/// damage, trash the top card of the stack."
///
/// The kind stipulation is what is under test: two net damage in one
/// occurrence mills one card and not two — the condition is met once per
/// occurrence, not once per point — and a MEAT damage of the same size mills
/// nothing at all.
#[test]
fn potential_unleashed_mills_once_per_net_damage_and_never_for_meat() {
    for (label, meat) in [("net", false), ("meat", true)] {
        let mut vm = Vm::empty(6113);
        tk::install_identity(&mut vm, card("Jinteki: Potential Unleashed"), Side::Corp);
        let button = if meat {
            tk::meat_damage_button("Hurt", 2)
        } else {
            tk::net_damage_button("Hurt", 2)
        };
        tk::install_root(&mut vm, button, ServerId::Remote(1), true);
        tk::fill_hand(&mut vm, Side::Runner, 4);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        let stack = tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let label_of = if meat { "do meat damage" } else { "do net damage" };
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::paid().once(), Reply::take(label_of)).stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            2,
            "the {label} damage landed: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.deck[&Side::Runner].len(),
            if meat { 5 } else { 4 },
            "the stack was milled exactly once, and only for NET damage ({label}): {}",
            t.tail(30)
        );
        if !meat {
            assert_eq!(
                vm.st.objects[&stack[0]].zone,
                Zone::Discard(Side::Runner),
                "and it was the top card of the stack: {}",
                t.tail(30)
            );
        }
    }
}

/// Pālanā Foods: "The first time each turn the Runner draws a card, gain
/// 1[credit]."
///
/// CR 8.4.2 meets a draw condition once PER CARD, so the ordinal is what is
/// under test: the Runner spends two clicks drawing — three cards in all —
/// and the Corp is paid once. The Corp's own mandatory draw does not count,
/// because the sentence names the Runner.
#[test]
fn palana_foods_pays_once_however_many_cards_the_runner_draws() {
    let mut vm = Vm::empty(6114);
    tk::install_identity(&mut vm, card("Pālanā Foods: Sustainable Growth"), Side::Corp);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::Draw))
            .when(Match::action().once(), Reply::Take(Pick::Draw))
            .stop_at_action(),
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "two cards were drawn: {}", t.tail(30));
    assert_eq!(
        vm.st.corp.credits, 1,
        "8.4.2 meets the condition once per card, and 9.6.5c pays only for the first: {}",
        t.tail(30)
    );
}

/// Tennin Institute: "When your turn begins, if the Runner did not make a
/// successful run during their last turn, you may place 1 advancement counter
/// on an installed card."
///
/// The requirement is the card, so both readings are asserted: a SUCCESSFUL
/// run last turn shuts the ability off, and an unsuccessful one does not —
/// "did not make a successful run" is not "made no runs".
#[test]
fn tennin_institute_advances_unless_the_runner_ran_successfully_last_turn() {
    for successful in [false, true] {
        let mut vm = Vm::empty(6115);
        tk::install_identity(&mut vm, card("Tennin Institute: The Secrets Within"), Side::Corp);
        let target =
            tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(1), false);
        // An unsuccessful run needs something to end it: a rezzed ETR piece of
        // ice on the server the Runner will attack.
        tk::install_ice(&mut vm, tk::etr_ice("Wall", 0, 1), ServerId::Archives, true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);

        // A whole Runner turn first: one run, successful or not, then clicks
        // spent on credits until the turn ends. The Corp's plan carries
        // through the turn boundary, because the "when your turn begins"
        // window belongs to the turn that starts as the Runner's ends.
        vm.start_turn(Side::Runner);
        let server = if successful { ServerId::Rnd } else { ServerId::Archives };
        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::reaction().offering("tennin institute"), Reply::take("tennin institute"))
                .when(Match::targets().once(), Reply::Targets(vec![target]))
                .when(Match::action(), Reply::Halt),
            Plan::runner()
                .when(Match::action().first(), Reply::run(server))
                .otherwise_click_credit(),
        );
        assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's turn came round: {}", t.tail(30));
        assert_eq!(
            vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
            successful,
            "the run went as the case intends: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.objects[&target].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
            u32::from(!successful),
            "the counter is placed exactly when the Runner's last turn held no SUCCESSFUL run \
             (successful={successful}): {}",
            t.tail(30)
        );
    }
}

/// Custom Biotics: "You cannot include Jinteki cards in this deck."
/// Ampère: "Your deck cannot include more than 1 copy of any card." /
///         "Your deck may include up to 2 different agenda cards from each
///          Corp faction."
/// The Shadow: "Draft format only." / "You can use agendas from all factions
///             in this deck."
/// The Syndicate: "Starter game only."
///
/// The Corp side of the same reading `a_deckbuilding_restriction_is_not_an_
/// ability_that_ever_fires` makes for the Runner: CR 1.4.2 settles all of it
/// before the game begins, so each of these denotes into no abilities and a
/// turn played under one opens no window naming it.
#[test]
fn a_corp_deckbuilding_restriction_is_not_an_ability_either() {
    for name in [
        "Custom Biotics: Engineered for Success",
        "Ampère: Cybernetics For Anyone",
        "The Shadow: Pulling the Strings",
        "The Syndicate: Profit over Principle",
    ] {
        let printed = card(name);
        assert!(
            printed.abilities.is_empty(),
            "{name}: a deck-construction restriction denotes into no ability (EDSL rule of thumb 3)"
        );
        let mut vm = Vm::empty(6116);
        tk::install_identity(&mut vm, printed, Side::Corp);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.start_turn(Side::Corp);
        let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
        assert!(
            !t.took(&name.to_lowercase()),
            "{name}: nothing of this identity was ever offered or used during a turn: {}",
            t.tail(24)
        );
    }
}

/// Cybernetics Division: "Each player's maximum hand size is reduced by 1."
/// NBN: The World is Yours*: "Your maximum hand size is increased by 1."
///
/// One declaration with two contents, so both are asserted through the same
/// reading: the scope decides WHOSE, and the amount decides which way. Each
/// identity is read against the base of 5 (5.7.2), for both players.
#[test]
fn the_two_hand_size_identities_move_the_limit_in_opposite_directions() {
    for (name, corp, runner) in [
        ("Cybernetics Division: Humanity Upgraded", 4, 4),
        ("NBN: The World is Yours*", 6, 5),
    ] {
        let mut vm = Vm::empty(6117);
        tk::install_identity(&mut vm, card(name), Side::Corp);
        assert_eq!(
            vm.max_hand_size(Side::Corp),
            corp,
            "{name}: the Corp's maximum hand size, from a base of 5"
        );
        assert_eq!(
            vm.max_hand_size(Side::Runner),
            runner,
            "{name}: and the Runner's — 'each player's' reaches them, 'your' does not"
        );
    }
}

/// Cybernetics Division: "Each player's maximum hand size is reduced by 1."
///
/// The declaration is read continuously, so the discard phase obeys it: the
/// Corp ends its turn with five cards in HQ and must discard down to four.
#[test]
fn cybernetics_division_shortens_the_corps_own_discard_phase() {
    let mut vm = Vm::empty(6118);
    tk::install_identity(&mut vm, card("Cybernetics Division: Humanity Upgraded"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::credit()),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn finished: {}", t.tail(24));
    assert_eq!(
        vm.st.hand[&Side::Corp].len(),
        4,
        "5.7.4 discarded down to the reduced maximum, not to 5: {}",
        t.tail(24)
    );
}

/// Haas-Bioroid: Precision Design — "You get +1 maximum hand size." /
/// "Whenever you score an agenda, you may add 1 card from Archives to HQ."
///
/// Both printed lines, and they are different kinds of sentence. The declared
/// half is read off the limit; the conditional half is offered on a score,
/// declinable (9.6.9), and reaches a FACEDOWN card in Archives — the sentence
/// says nothing about which way up a card lies.
#[test]
fn precision_design_raises_the_limit_and_fishes_a_card_out_of_archives() {
    let mut vm = Vm::empty(6119);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Precision Design"), Side::Corp);
    assert_eq!(vm.max_hand_size(Side::Corp), 6, "+1 on 5.7.2's base of 5");
    assert_eq!(vm.max_hand_size(Side::Runner), 5, "'you' is the Corp, and only the Corp");

    let agenda =
        tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
    let buried = vm.new_object(tk::corp_filler("Buried"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid(), Reply::score(agenda))
            .when(Match::reaction().offering("precision design"), Reply::take("precision design"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&buried].zone,
        Zone::Hand(Side::Corp),
        "the card came out of Archives and into HQ: {}",
        t.tail(30)
    );
}

/// Jinteki: Restoring Humanity — "When your discard phase ends, if there is a
/// facedown card in Archives, gain 1[credit]."
///
/// The requirement is what is under test, and 10.3.1a is what makes it
/// meaningful: a card the CORP trashed lies in Archives facedown and a card
/// the RUNNER trashed lies there faceup. Two games, identical but for which
/// way the one card in Archives is lying, and only the facedown one pays.
#[test]
fn restoring_humanity_pays_only_for_a_facedown_card_in_archives() {
    for faceup in [false, true] {
        let mut vm = Vm::empty(6120);
        tk::install_identity(&mut vm, card("Jinteki: Restoring Humanity"), Side::Corp);
        let buried = vm.new_object(tk::corp_filler("Buried"), Zone::Discard(Side::Corp));
        vm.st.objects.get_mut(&buried).unwrap().faceup = faceup;
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action(), Reply::credit()),
            Plan::runner().when(Match::action(), Reply::Halt),
        );
        assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn finished: {}", t.tail(24));
        assert_eq!(
            // 3 clicks on 5.2.6b's basic credit action, plus the identity's.
            vm.st.corp.credits,
            3 + u32::from(!faceup),
            "paid exactly when the card in Archives was FACEDOWN (faceup={faceup}): {}",
            t.tail(24)
        );
    }
}

// ---------------------------------------------------------------------------
// The identity queue — Corp, NBN
// ---------------------------------------------------------------------------

/// NBN: Reality Plus — "The first time each turn the Runner takes a tag, gain
/// 2[credit] or draw 2 cards."
///
/// The ordinal and the option choice together: two tags taken separately in
/// one turn pay once, and the Corp is the one who chooses which way.
#[test]
fn reality_plus_pays_once_a_turn_and_lets_the_corp_pick_how() {
    let mut vm = Vm::empty(6121);
    tk::install_identity(&mut vm, card("NBN: Reality Plus"), Side::Corp);
    tk::install_root(&mut vm, tk::corp_tags_button("Tag Me", 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("tag"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("gain 2[credit]"))
            .when(Match::paid().once(), Reply::take("tag"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.runner.tags, 2, "both tags were handed over: {}", t.tail(30));
    let asked = t.of_kind(Kind::Options);
    assert_eq!(asked.len(), 1, "the choice was put once, on the FIRST tag: {}", t.tail(30));
    assert_eq!(asked[0].side, Side::Corp, "9.1.1a: the Corp's identity, so the Corp chooses");
    assert_eq!(vm.st.corp.credits, 2, "and paid 2, once: {}", t.tail(30));
}

/// Pravdivost Consulting: "The first time each turn the Runner makes a
/// successful run, you may place 1 advancement counter on an installed card
/// you can advance."
///
/// Two successful runs place one counter. "A card you can advance" is the
/// other half: an ordinary asset is never offered, only the agenda beside it.
#[test]
fn pravdivost_advances_once_a_turn_and_only_what_can_be_advanced() {
    let mut vm = Vm::empty(6122);
    tk::install_identity(&mut vm, card("Pravdivost Consulting: Political Solutions"), Side::Corp);
    let agenda =
        tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 5, 3), ServerId::Remote(1), false);
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(2), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::reaction().offering("pravdivost"), Reply::take("pravdivost")),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "offered once, on the first successful run: {}", t.tail(30));
    assert_eq!(
        announcements[0].candidates(),
        [agenda],
        "1.18.3: only a card that CAN be advanced is a candidate — the asset is not: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&agenda].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        1,
        "one counter, from the first run only: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&asset].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        0,
        "and none on the asset: {}",
        t.tail(30)
    );
}

// ---------------------------------------------------------------------------
// The identity queue — Corp, Weyland Consortium
// ---------------------------------------------------------------------------

/// Argus Security: "Whenever the Runner steals an agenda, they must take 1 tag
/// or suffer 2 meat damage."
///
/// "They must" is 1.14.5 putting the choice to the RUNNER — asserted, because
/// it is the only thing separating this card from Sportsmetal's — and 9.12.3's
/// "must" means neither option may be declined. Both options are taken, one
/// game each.
#[test]
fn argus_security_makes_the_runner_choose_a_tag_or_two_meat() {
    for take_tag in [false, true] {
        let mut vm = Vm::empty(6123);
        tk::install_identity(&mut vm, card("Argus Security: Protection Guaranteed"), Side::Corp);
        let agenda =
            tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
        tk::fill_hand(&mut vm, Side::Runner, 4);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let pick = if take_tag { "take 1 tag" } else { "suffer 2 meat damage" };
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                .when(Match::of(Kind::Options).once(), Reply::ChooseNamed(pick))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&agenda].zone,
            Zone::ScoreArea(Side::Runner),
            "the agenda was stolen (take_tag={take_tag}): {}",
            t.tail(30)
        );
        let asked = t.of_kind(Kind::Options);
        assert_eq!(asked.len(), 1, "one choice was put (take_tag={take_tag}): {}", t.tail(30));
        assert_eq!(
            asked[0].side,
            Side::Runner,
            "1.14.5: the sentence says THEY must, so the Runner chooses — not the Corp"
        );
        assert_eq!(
            vm.st.runner.tags,
            u32::from(take_tag),
            "the tag arrived exactly on that branch (take_tag={take_tag}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            if take_tag { 4 } else { 2 },
            "and the 2 meat damage on the other (take_tag={take_tag}): {}",
            t.tail(30)
        );
        if !take_tag {
            assert!(
                vm.changes.log.iter().any(|c| matches!(
                    c,
                    GameChange::DamageSuffered { responsible: Side::Runner, .. }
                )),
                "10.4.1: the sentence directs the Runner to SUFFER the damage, so the \
                 RUNNER and the source are responsible — a 'damage done by the Corp' \
                 reader must not see this occurrence: {}",
                t.tail(30)
            );
        }
    }
}

/// The Outfit: "Whenever you take 1 or more bad publicity, gain 3[credit]."
///
/// "1 or more" is not a threshold: the condition is met once per TAKING, so a
/// card handing over two bad publicity at once pays 3 and not 6.
#[test]
fn the_outfit_pays_three_per_taking_of_bad_publicity_however_many_it_was() {
    let mut vm = Vm::empty(6124);
    tk::install_identity(&mut vm, card("The Outfit: Family Owned and Operated"), Side::Corp);
    tk::install_root(&mut vm, tk::take_bad_pub_button("Scandal", 2), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("take bad publicity")).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.bad_publicity, 2, "two bad publicity arrived at once: {}", t.tail(24));
    assert_eq!(
        vm.st.corp.credits, 3,
        "and paid 3 for the one TAKING, not 3 per point: {}",
        t.tail(24)
    );
}

/// Weyland Consortium: Building a Better World — "Whenever you play a
/// transaction operation, gain 1[credit]."
///
/// Both stipulations: a transaction pays, and an operation without the
/// subtype does not. Two of them are played in the same turn, so the absence
/// of an ordinal is asserted too — every transaction pays, not just the first.
#[test]
fn building_a_better_world_pays_for_every_transaction_and_no_other_operation() {
    let mut vm = Vm::empty(6125);
    tk::install_identity(&mut vm, card("Weyland Consortium: Building a Better World"), Side::Corp);
    let mut make = |name: &'static str, transaction: bool| {
        let mut c = tk::operation(name, 0, vec![]);
        if transaction {
            c.subtypes = vec!["Transaction"];
        }
        let id = vm.new_object(c, Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
        id
    };
    let first = make("First Transaction", true);
    let second = make("Second Transaction", true);
    let plain = make("Plain Operation", false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(first))
            .when(Match::action().once(), Reply::play_card(second))
            .when(Match::action().once(), Reply::play_card(plain))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.corp.credits, 2,
        "1 for each of the two transactions, and nothing for the plain operation: {}",
        t.tail(30)
    );
}

/// Weyland Consortium: Built to Last — "Whenever you advance a card, gain
/// 2[credit] if it had no advancement counters."
///
/// The requirement is read of the card BEFORE the advance, so the first
/// advance of a card pays and the second does not. Two advances of the same
/// card say both halves at once.
#[test]
fn built_to_last_pays_for_the_first_advance_of_a_card_and_not_the_second() {
    let mut vm = Vm::empty(6126);
    tk::install_identity(&mut vm, card("Weyland Consortium: Built to Last"), Side::Corp);
    let agenda =
        tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 5, 3), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::Advance(agenda)))
            .when(Match::action().once(), Reply::Take(Pick::Advance(agenda)))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&agenda].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        2,
        "the card was advanced twice: {}",
        t.tail(30)
    );
    // 5 credits, 2 spent advancing, 2 gained once: 5.
    assert_eq!(
        vm.st.corp.credits, 5,
        "paid 2 for the FIRST advance only — the second found a counter already there: {}",
        t.tail(30)
    );
}

/// Quetzal: "Once per turn → 0[credit]: Break 1 barrier subroutine."
///
/// The subtype restriction and the ordinal, in one run: the identity breaks a
/// subroutine on the barrier and is not offered again this turn — and it is
/// never offered at all during the encounter with the sentry, whose
/// subroutine ends the run.
#[test]
fn quetzal_breaks_one_barrier_subroutine_a_turn_and_never_a_sentry() {
    let mut vm = Vm::empty(6127);
    tk::install_identity(&mut vm, card("Quetzal: Free Spirit"), Side::Runner);
    // 6.2.1: the Runner meets the OUTERMOST piece of ice first, so the
    // sentry goes on innermost and the barrier over it.
    let sentry = tk::install_ice(
        &mut vm,
        tk::subtyped_etr_ice("Some Sentry", "Sentry", 0, 1),
        ServerId::Archives,
        true,
    );
    let barrier = tk::install_ice(
        &mut vm,
        tk::subtyped_etr_ice("Some Barrier", "Barrier", 0, 1),
        ServerId::Archives,
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // No `.once()`: the plan takes the ability every time it is offered, so
    // a second break would mean 9.3.6g's flag was never spent.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::paid(), Reply::take("break 1 barrier subroutine"))
            .stop_at_action(),
    );
    let offers = t
        .of_kind(Kind::Paid)
        .into_iter()
        .filter(|e| plan::count_labelled(plan::window_options(&e.spec), "break 1 barrier") > 0)
        .count();
    assert_eq!(
        offers, 1,
        "offered during the barrier's encounter only, and 9.3.6g spends it there: {}",
        t.tail(40)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::SubroutineBroken { ice, .. } if *ice == barrier)),
        "the barrier's subroutine was broken: {}",
        t.tail(40)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::SubroutineResolved { ice, .. } if *ice == sentry)),
        "and the sentry's resolved, because 9.5.6c never offered the ability there: {}",
        t.tail(40)
    );
}

/// Valencia Estevez: "The Corp starts the game with 1 bad publicity."
/// GRNDL: "You start the game with 10[credit] and 1 bad publicity."
///
/// Both through the real §1.6 setup, not a fixture. Each is a fact about the
/// game's start rather than an ability, so the proof is the state before
/// anyone has done anything — and Valencia's says it about the OTHER player,
/// which is only sayable because 10.6 makes bad publicity always the Corp's.
#[test]
fn the_two_setup_identities_change_the_game_before_it_starts() {
    use jinteki_cr::vm::GameSetup;
    let deck = |n: usize| -> Vec<PrintedCard> { (0..n).map(|_| tk::corp_filler("C-filler")).collect() };
    let rdeck = |n: usize| -> Vec<PrintedCard> {
        (0..n).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect()
    };

    let vm = Vm::new_game(GameSetup {
        seed: 11,
        additional_identities: Default::default(),
        corp_identity: None,
        runner_identity: Some(card("Valencia Estevez: The Angel of Cayambe")),
        corp_deck: deck(20),
        runner_deck: rdeck(20),
        shuffle: true,
        extra_cards: Default::default(),
    });
    assert_eq!(vm.st.corp.bad_publicity, 1, "Valencia hands the Corp its bad publicity at setup");
    assert_eq!(vm.st.corp.credits, 5, "and leaves 1.6.4's five credits alone");
    assert_eq!(vm.st.runner.credits, 5, "on both sides");

    let vm = Vm::new_game(GameSetup {
        seed: 11,
        additional_identities: Default::default(),
        corp_identity: Some(card("GRNDL: Power Unleashed")),
        runner_identity: None,
        corp_deck: deck(20),
        runner_deck: rdeck(20),
        shuffle: true,
        extra_cards: Default::default(),
    });
    assert_eq!(vm.st.corp.credits, 10, "GRNDL starts on ten, not 1.6.4's five");
    assert_eq!(vm.st.corp.bad_publicity, 1, "and with one bad publicity");
    assert_eq!(vm.st.runner.credits, 5, "the Runner is untouched");
}

/// Spark Agency: "The first time each turn you rez an advertisement, the
/// Runner loses 1[credit]."
///
/// Both stipulations: two advertisements rezzed in one turn cost the Runner
/// one credit, and rezzing a card without the subtype costs them nothing.
#[test]
fn spark_agency_taxes_the_first_advertisement_rez_of_the_turn_only() {
    let mut vm = Vm::empty(6128);
    tk::install_identity(&mut vm, card("Spark Agency: Worldswide Reach"), Side::Corp);
    let mut ad = tk::vanilla_asset("Some Advert", 0, 2);
    ad.subtypes = vec!["Advertisement"];
    let first = tk::install_root(&mut vm, ad.clone(), ServerId::Remote(1), false);
    let second = tk::install_root(&mut vm, ad, ServerId::Remote(2), false);
    let plain = tk::install_root(&mut vm, tk::vanilla_asset("Plain Asset", 0, 2), ServerId::Remote(3), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::Rez(first)))
            .when(Match::paid().once(), Reply::Take(Pick::Rez(second)))
            .when(Match::paid().once(), Reply::Take(Pick::Rez(plain)))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(
        vm.st.objects[&first].faceup && vm.st.objects[&second].faceup && vm.st.objects[&plain].faceup,
        "all three were rezzed: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.runner.credits, 4,
        "one credit, from the FIRST advertisement — not the second, and not the plain asset: {}",
        t.tail(30)
    );
}

/// Seidr Laboratories: "The first time each turn the Runner loses or spends
/// [click] during a run, you may add 1 card from Archives to the top of R&D."
///
/// CR 5.2.1 keeps a click SPENT and a click LOST apart, and the card names
/// both — so the test drives the LOSS, which the old condition could not see:
/// a bioroid-class subroutine takes a click during the encounter, and the
/// card in Archives goes to the top of R&D.
#[test]
fn seidr_laboratories_acts_on_a_click_lost_during_a_run_not_only_one_spent() {
    let mut vm = Vm::empty(6129);
    tk::install_identity(&mut vm, card("Seidr Laboratories: Destiny Defined"), Side::Corp);
    // A bioroid-class break ability whose cost is 5.2.1a's LOSE [click] —
    // the occurrence the old condition could not see.
    tk::install_ice(&mut vm, tk::etr_ice("Wall", 0, 1), ServerId::Archives, true);
    tk::install_rig(&mut vm, tk::lose_click_break_program("Eli-like"));
    let buried = vm.new_object(tk::corp_filler("Buried"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
    let rnd = tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("seidr"), Reply::take("seidr"))
            .when(Match::targets().once(), Reply::Targets(vec![buried])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::paid().once(), Reply::take("lose-click"))
            .stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::ClicksLost { side, .. } if *side == Side::Runner)),
        "the subroutine took a click — 5.2.1's LOSS, not a spend: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.deck[&Side::Corp].first().copied(),
        Some(buried),
        "the card from Archives is now the top card of R&D, above what was there: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.deck[&Side::Corp].len(), rnd.len() + 1, "and R&D grew by one: {}", t.tail(40));
}

// ---------------------------------------------------------------------------
// The identity queue — "it" and "that card": CR 1.15.4's back-reference
// ---------------------------------------------------------------------------

/// Titan Transnational: "Whenever you score an agenda, you may place 1 agenda
/// counter on it."
///
/// "It" is the agenda that was scored, and nothing is announced — the
/// condition already fixed which card the sentence is about. The proof is
/// that a SECOND agenda sitting in the score area is left alone, and that no
/// target announcement is ever put to anyone.
#[test]
fn titan_transnational_counters_the_agenda_it_just_scored_and_no_other() {
    let mut vm = Vm::empty(6130);
    tk::install_identity(&mut vm, card("Titan Transnational: Investing In Your Future"), Side::Corp);
    // One already in the score area, so "it" has a wrong answer available.
    let old = vm.new_object(tk::vanilla_agenda("Old Agenda", 3, 2), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(old);
    let scored =
        tk::install_root(&mut vm, tk::vanilla_agenda("New Agenda", 3, 2), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&scored).unwrap().counters.insert(CounterKind::Advancement, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid(), Reply::score(scored))
            .when(Match::reaction().offering("titan"), Reply::take("titan"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(
        t.of_kind(Kind::Targets).is_empty(),
        "nothing was announced — 1.15.4's 'it' is fixed by the condition: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&scored].counters.get(&CounterKind::Agenda).copied().unwrap_or(0),
        1,
        "the agenda just scored carries the counter: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&old].counters.get(&CounterKind::Agenda).copied().unwrap_or(0),
        0,
        "and the one already in the score area does not: {}",
        t.tail(30)
    );
}

/// 419: Amoral Scammer — "The first time the Corp installs a card each turn,
/// you may expose that card unless the Corp pays 1[credit]."
///
/// Three things at once: "that card" is the card the Corp just installed and
/// not some other facedown card on the board; the Corp's 1[credit] stops the
/// exposure; and the ordinal means a second install the same turn offers
/// nothing.
#[test]
fn four_one_nine_exposes_the_first_corp_install_unless_the_corp_pays() {
    for corp_pays in [false, true] {
        let mut vm = Vm::empty(6131);
        tk::install_identity(&mut vm, card("419: Amoral Scammer"), Side::Runner);
        // A decoy: another facedown Corp card, which "that card" must not reach.
        let decoy = tk::install_root(&mut vm, tk::vanilla_asset("Decoy", 0, 2), ServerId::Remote(9), false);
        let first = vm.new_object(tk::vanilla_asset("First Install", 0, 2), Zone::Hand(Side::Corp));
        let second = vm.new_object(tk::vanilla_asset("Second Install", 0, 2), Zone::Hand(Side::Corp));
        for id in [first, second] {
            vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
        }
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::Take(Pick::InstallCard(first)))
                .when(Match::of(Kind::NestedCost).once(), Reply::PayCost(corp_pays))
                .when(Match::action().once(), Reply::Take(Pick::InstallCard(second)))
                .stop_at_action(),
            Plan::runner().when(Match::reaction().offering("419"), Reply::take("419")),
        );
        let exposed: Vec<_> = vm
            .changes
            .log
            .iter()
            .filter_map(|c| match c {
                GameChange::CardExposed { obj } => Some(*obj),
                _ => None,
            })
            .collect();
        if corp_pays {
            assert!(
                exposed.is_empty(),
                "1[credit] from the Corp stopped the exposure altogether: {}",
                t.tail(40)
            );
            assert_eq!(vm.st.corp.credits, 4, "and cost them the credit: {}", t.tail(40));
        } else {
            assert_eq!(
                exposed,
                [first],
                "the card just installed was exposed — not the decoy, and not the second \
                 install, which is no longer 'the first time each turn': {}",
                t.tail(40)
            );
            assert_eq!(vm.st.corp.credits, 5, "the Corp declined and kept its credit: {}", t.tail(40));
        }
        assert!(
            !exposed.contains(&decoy) && !exposed.contains(&second),
            "1.15.4's 'that card' reaches exactly one card (corp_pays={corp_pays}): {}",
            t.tail(40)
        );
    }
}

/// Hayley Kaplan: "The first time you install a card each turn, you may
/// install another card of the same type from your grip (paying its install
/// cost)."
///
/// "Of the same type" is read off the card the condition named, so the grip
/// is offered exactly the cards matching it: installing a PROGRAM offers the
/// other program and never the hardware beside it, and the install really
/// pays its cost.
#[test]
fn hayley_kaplan_offers_only_the_grip_cards_sharing_the_installed_cards_type() {
    let mut vm = Vm::empty(6132);
    tk::install_identity(&mut vm, card("Hayley Kaplan: Universal Scholar"), Side::Runner);
    let mut mk = |name: &'static str, ty: CardType, cost: u32| {
        let mut c = tk::vanilla_runner_card(name, ty);
        c.cost = Some(cost);
        if ty == CardType::Program {
            c.memory_cost = Some(1);
        }
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let first_program = mk("First Program", CardType::Program, 0);
    let second_program = mk("Second Program", CardType::Program, 2);
    let hardware = mk("Some Hardware", CardType::Hardware, 0);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(first_program)))
            .when(Match::reaction().offering("universal scholar"), Reply::take("universal scholar"))
            .stop_at_action(),
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "one card was chosen from the grip: {}", t.tail(40));
    assert_eq!(
        announcements[0].candidates(),
        [second_program],
        "only the grip card of the SAME type is a candidate — the hardware is not: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&second_program].zone,
        Zone::Rig,
        "and it was really installed: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&hardware].zone,
        Zone::Hand(Side::Runner),
        "the hardware stayed in the grip: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.runner.credits, 3, "'paying its install cost' — 2 was paid: {}", t.tail(40));
}

/// Gagarin Deep Space: "As an additional cost to access a card in the root of
/// a remote server, the Runner must pay 1[credit]."
///
/// Both halves of the description, in two runs of the same game: accessing
/// the card in the remote's root costs a credit, and breaching a CENTRAL
/// costs nothing. 1.16.10 puts the payment before the access, so a Runner
/// with no credits accesses nothing.
#[test]
fn gagarin_taxes_a_remote_root_access_and_leaves_the_centrals_alone() {
    let mut vm = Vm::empty(6133);
    tk::install_identity(&mut vm, card("Gagarin Deep Space: Expanding the Horizon"), Side::Corp);
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 99), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::of(Kind::NestedCost), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { .. })),
        "the HQ breach accessed a card: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == asset)),
        "and so did the remote's root, once paid for: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.runner.credits, 0,
        "exactly one credit was paid — for the remote root, not for the HQ breach: {}",
        t.tail(40)
    );
}

/// Poétrï Luxury Brands: "Whenever you score an agenda, look at the top 3
/// cards of R&D. You may install 1 non-agenda card from among them." /
/// "Whenever an agenda is stolen, you may install 1 non-agenda card from HQ."
///
/// Both lines, one game each, and the "non-agenda" word in both: the agenda
/// sitting among the candidates is never offered.
#[test]
fn poetri_installs_a_non_agenda_on_a_score_from_rnd_and_on_a_steal_from_hq() {
    for stolen in [false, true] {
        let mut vm = Vm::empty(6134);
        tk::install_identity(&mut vm, card("Poétrï Luxury Brands: All the Rage"), Side::Corp);
        let scored =
            tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 2), ServerId::Remote(1), false);
        vm.st.objects.get_mut(&scored).unwrap().counters.insert(CounterKind::Advancement, 3);

        // The candidate pool: one asset and one agenda, in whichever zone the
        // line under test names.
        let zone = if stolen { Zone::Hand(Side::Corp) } else { Zone::Deck(Side::Corp) };
        let asset = vm.new_object(tk::vanilla_asset("Installable", 0, 2), zone);
        let decoy = vm.new_object(tk::vanilla_agenda("Not Installable", 3, 2), zone);
        if stolen {
            for id in [asset, decoy] {
                vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
            }
        } else {
            for id in [asset, decoy] {
                vm.st.deck.get_mut(&Side::Corp).unwrap().push(id);
            }
        }
        tk::fill_deck(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;

        let corp = Plan::corp()
            .when(Match::reaction().offering("poétrï"), Reply::take("poétrï"))
            .when(Match::of(Kind::Optional), Reply::Optional(true))
            .when(Match::targets().once(), Reply::Targets(vec![asset]))
            .when(Match::of(Kind::Destination), Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot));
        let t = if stolen {
            vm.start_turn(Side::Runner);
            plan::play(
                &mut vm,
                corp,
                Plan::runner()
                    .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                    .stop_at_action(),
            )
        } else {
            vm.start_turn(Side::Corp);
            plan::play(
                &mut vm,
                corp.when(Match::paid(), Reply::score(scored)).stop_at_action(),
                Plan::runner(),
            )
        };
        let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
        assert_eq!(announcements.len(), 1, "one card was chosen (stolen={stolen}): {}", t.tail(40));
        assert!(
            announcements[0].candidates().contains(&asset),
            "the non-agenda card is a candidate (stolen={stolen}): {}",
            t.tail(40)
        );
        assert!(
            !announcements[0].candidates().contains(&decoy),
            "'non-agenda' keeps the agenda beside it out of the candidates \
             (stolen={stolen}): {}",
            t.tail(40)
        );
        assert!(
            vm.st.objects[&asset].zone.is_installed(),
            "and the card was installed (stolen={stolen}): {}",
            t.tail(40)
        );
    }
}

/// Armand "Geist" Walker: "Whenever you use a [trash] ability, draw 1 card."
///
/// 1.19.4's printed [trash] symbol is not 7.1.5's basic trash ability, and the
/// test is that difference: using a card's own [trash] ability draws, and the
/// Runner paying an accessed card's trash cost does not.
#[test]
fn armand_geist_walker_draws_on_a_trash_symbol_and_not_on_a_basic_trash() {
    for basic in [false, true] {
        let mut vm = Vm::empty(6135);
        tk::install_identity(&mut vm, card("Armand \"Geist\" Walker: Tech Lord"), Side::Runner);
        // The 1.19.4 half: a Runner card whose ability costs [trash].
        tk::install_rig(&mut vm, tk::trash_cost_ability_card("Aesop-like"));
        // The 7.1.5 half: an accessible Corp card with a trash cost.
        let asset = tk::install_root(&mut vm, tk::vanilla_asset("Trashable", 0, 1), ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);
        assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip starts empty");

        let t = if basic {
            plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                    .when(Match::of(Kind::MidAccess).once(), Reply::Take(Pick::BasicTrash))
                    .stop_at_action(),
            )
        } else {
            plan::play(
                &mut vm,
                Plan::corp(),
                Plan::runner().when(Match::paid().once(), Reply::take("trash-cost")).stop_at_action(),
            )
        };
        if basic {
            assert_eq!(
                vm.st.objects[&asset].zone,
                Zone::Discard(Side::Corp),
                "the Runner paid the trash cost with 7.1.5's basic ability: {}",
                t.tail(30)
            );
        }
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            usize::from(!basic),
            "the draw happens for the printed [trash] symbol and NOT for the basic trash \
             ability (basic={basic}): {}",
            t.tail(30)
        );
    }
}

/// Barry "Baz" Wong: "Whenever the Corp rezzes a piece of ice, you may install
/// 1 resource or piece of hardware from your grip."
///
/// "Or" between two card TYPES is one description word, and the test is what
/// it reaches: with a resource, a piece of hardware and a program in the grip,
/// exactly the first two are candidates.
#[test]
fn barry_baz_wong_offers_the_grips_resources_and_hardware_but_not_its_programs() {
    let mut vm = Vm::empty(6136);
    tk::install_identity(&mut vm, card("Barry \"Baz\" Wong: Tri-Maf Veteran"), Side::Runner);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 1), ServerId::Archives, false);
    let mut mk = |name: &'static str, ty: CardType| {
        let mut c = tk::vanilla_runner_card(name, ty);
        c.cost = Some(0);
        if ty == CardType::Program {
            c.memory_cost = Some(1);
        }
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let resource = mk("Some Resource", CardType::Resource);
    let hardware = mk("Some Hardware", CardType::Hardware);
    let program = mk("Some Program", CardType::Program);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::reaction().offering("tri-maf"), Reply::take("tri-maf"))
            .stop_at_action(),
    );
    assert!(vm.st.objects[&ice].faceup, "the ice was rezzed: {}", t.tail(40));
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "one card was chosen from the grip: {}", t.tail(40));
    let candidates = announcements[0].candidates();
    assert!(
        candidates.contains(&resource) && candidates.contains(&hardware),
        "both named types are candidates: {}",
        t.tail(40)
    );
    assert!(
        !candidates.contains(&program),
        "and a program is not — the sentence names two types, not any type: {}",
        t.tail(40)
    );
}

/// Iain Stirling: "When your turn begins, gain 2[credit] if the Corp has more
/// scored agenda points than you."
///
/// A comparison, so all three orderings matter: behind pays, level does not
/// ("MORE than" is strict), and ahead does not either.
#[test]
fn iain_stirling_pays_only_while_the_corp_is_strictly_ahead() {
    for (corp_points, runner_points, expected) in [(2i32, 0i32, 2u32), (2, 2, 0), (0, 2, 0)] {
        let mut vm = Vm::empty(6137);
        tk::install_identity(&mut vm, card("Iain Stirling: Retired Spook"), Side::Runner);
        for (side, points) in [(Side::Corp, corp_points), (Side::Runner, runner_points)] {
            if points > 0 {
                let a = vm.new_object(tk::vanilla_agenda("Scored", 3, points), Zone::ScoreArea(side));
                vm.st.score_area.get_mut(&side).unwrap().push(a);
            }
        }
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
        assert_eq!(
            vm.st.runner.credits, expected,
            "Corp {corp_points} vs Runner {runner_points} pays {expected}: {}",
            t.tail(24)
        );
    }
}

/// Silhouette: "The first time you make a successful run on HQ each turn, you
/// may expose 1 card."
///
/// The card prints no description at all, so the candidates are what CR
/// 1.21.4 leaves: installed cards that are not rezzed. The test puts a rezzed
/// asset beside an unrezzed one and asserts only the unrezzed one is offered
/// — and that the exposure really happens.
#[test]
fn silhouette_may_expose_only_an_installed_unrezzed_card() {
    let mut vm = Vm::empty(6138);
    tk::install_identity(&mut vm, card("Silhouette: Stealth Operative"), Side::Runner);
    let hidden = tk::install_root(&mut vm, tk::vanilla_asset("Hidden", 0, 2), ServerId::Remote(1), false);
    let shown = tk::install_root(&mut vm, tk::vanilla_asset("Shown", 0, 2), ServerId::Remote(2), true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::reaction().offering("silhouette"), Reply::take("silhouette"))
            .stop_at_action(),
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "one card was named, once: {}", t.tail(40));
    assert_eq!(
        announcements[0].candidates(),
        [hidden],
        "1.21.4: only an installed card that is NOT rezzed can be exposed — the rezzed \
         asset is never a candidate: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardExposed { obj } if *obj == hidden)),
        "and the exposure really happened: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardExposed { obj } if *obj == shown)),
        "never the rezzed one: {}",
        t.tail(40)
    );
}

/// Mercury: "Once per turn → When you breach HQ or R&D during a run, if you
/// did not break any subroutines during that run, you may access 1
/// additional card."
///
/// The requirement is the whole of it: the same run, breaching the same
/// server, accesses two cards when nothing was broken and one when a
/// subroutine was. Both stipulations ride on ONE condition, so the printed
/// "Once per turn →" is one flag and not two.
#[test]
fn mercury_accesses_a_second_card_only_when_nothing_was_broken() {
    for broke in [false, true] {
        let mut vm = Vm::empty(6140);
        tk::install_identity(&mut vm, card("Mercury: Chrome Libertador"), Side::Runner);
        let deck = tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        tk::install_ice(&mut vm, tk::three_sub_ice("Some Ice"), ServerId::Rnd, true);
        tk::install_rig(&mut vm, tk::break_button("Breaker"));
        vm.start_turn(Side::Runner);

        let mut runner = Plan::runner().when(Match::action().once(), Reply::run(ServerId::Rnd));
        if broke {
            runner = runner
                .when(Match::paid().once(), Reply::take("break"))
                .when(Match::sub_targets().once(), Reply::SubroutineNamed("bloop 0"));
        }
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            runner
                .when(Match::reaction().offering("chrome libertador"), Reply::take("chrome libertador"))
                .when(Match::optional(), Reply::Optional(true))
                .stop_at_action(),
        );
        let accessed = vm
            .changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CardAccessed { obj } if deck.contains(obj)))
            .count();
        assert_eq!(
            accessed,
            if broke { 1 } else { 2 },
            "the additional access is the requirement's (broke={broke}): {}",
            t.tail(40)
        );
    }
}

/// Mercury again, for 9.3.6g: one condition with two servers in it, so the
/// flag is spent by the first use and the second breach — of the OTHER named
/// server — is never offered.
#[test]
fn mercury_is_one_ability_with_one_once_per_turn_flag() {
    let mut vm = Vm::empty(6141);
    tk::install_identity(&mut vm, card("Mercury: Chrome Libertador"), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_hand(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::reaction().offering("chrome libertador"), Reply::take("chrome libertador"))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(
        t.offers("chrome libertador"),
        1,
        "9.3.6g: one flag for the whole sentence, spent by the R&D breach — the HQ breach \
         of the same turn is not offered: {}",
        t.tail(40)
    );
}

/// MuslihaT: "When your turn begins, look at the top card of your stack. If
/// that card is an icebreaker or a run event, you may reveal it and add it to
/// your grip."
///
/// The disjunction, both ways round and once against a card that is neither:
/// an icebreaker (a subtype alone) and a run event (a type AND a subtype) are
/// each taken, and a plain program is left where it is.
#[test]
fn muslihat_takes_an_icebreaker_or_a_run_event_and_nothing_else() {
    for kind in ["icebreaker", "run event", "neither"] {
        let mut vm = Vm::empty(6142);
        tk::install_identity(&mut vm, card("MuslihaT: Multifarious Marketeer"), Side::Runner);
        let mut top = match kind {
            "run event" => tk::event("Top Card", 0, vec![]),
            _ => tk::program_cost("Top Card", 0),
        };
        top.subtypes = match kind {
            "icebreaker" => vec!["Icebreaker"],
            "run event" => vec!["Run"],
            _ => Vec::new(),
        };
        let top = vm.new_object(top, Zone::Deck(Side::Runner));
        vm.st.deck.get_mut(&Side::Runner).unwrap().push(top);
        tk::fill_deck(&mut vm, Side::Runner, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner().when(Match::optional(), Reply::Optional(true)).stop_at_action(),
        );
        let taken = vm.st.objects[&top].zone == Zone::Hand(Side::Runner);
        assert_eq!(
            taken,
            kind != "neither",
            "the card was taken exactly when it matched the description (kind={kind}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::CardRevealed { obj, .. } if *obj == top)),
            kind != "neither",
            "…and the reveal happened with it (kind={kind}): {}",
            t.tail(30)
        );
        assert!(
            vm.changes.log.iter().any(
                |c| matches!(c, GameChange::CardLookedAt { obj, by } if *obj == top && *by == Side::Runner)
            ),
            "the look is the first sentence and happens whatever the card is (kind={kind}): {}",
            t.tail(30)
        );
    }
}

/// Zahya Sadeghi: "Once per turn → When a run on HQ or R&D ends, you may gain
/// 1[credit] for each time you accessed a card during that run."
///
/// Two accesses in one run of HQ pay two credits — 7.3.6 counts the accesses
/// PERFORMED, so an additional access counts with the ordinary one — and the
/// flag is then spent, so a second run of the same turn pays nothing.
#[test]
fn zahya_sadeghi_pays_one_credit_per_access_of_the_run_once_a_turn() {
    let mut vm = Vm::empty(6143);
    tk::install_identity(&mut vm, card("Zahya Sadeghi: Versatile Smuggler"), Side::Runner);
    tk::install_rig(&mut vm, tk::additional_access_card("Extra", ServerId::Hq, 1));
    tk::fill_hand(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::paid().offering("makers-eye").once(), Reply::take("makers-eye"))
            .when(Match::reaction().offering("versatile smuggler"), Reply::take("versatile smuggler"))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.runner.credits, 2,
        "1 credit for each of the run's two accesses, and nothing for the second run: {}",
        t.tail(40)
    );
    assert_eq!(t.offers("versatile smuggler"), 1, "9.3.6g: offered once: {}", t.tail(40));
}

/// Zahya again: the condition names HQ and R&D, so a run on a remote server
/// ending is not one of the occurrences that meets it.
#[test]
fn zahya_sadeghi_ignores_a_run_on_a_remote_server() {
    let mut vm = Vm::empty(6144);
    tk::install_identity(&mut vm, card("Zahya Sadeghi: Versatile Smuggler"), Side::Runner);
    tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::reaction().offering("versatile smuggler"), Reply::take("versatile smuggler"))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(t.offers("versatile smuggler"), 0, "the remote is not HQ or R&D: {}", t.tail(30));
    assert_eq!(vm.st.runner.credits, 0, "so nothing was gained: {}", t.tail(30));
}

/// Steve Cambridge: "The first time each turn you make a successful run on
/// HQ, you may choose 2 cards in your heap. If you do, the Corp removes 1 of
/// those cards from the game, then you add the other card to your grip."
///
/// One instruction (9.11.4c joins the choosing sentence to the one that
/// follows; its "then" is not among 9.11.4b-g's splits), so every choice is
/// announced before any of it resolves: the Runner's two heap cards, then
/// the Corp's pick — whose candidates are exactly the two announced cards
/// (1.15.4 as a description) and not the rest of the heap. The pick is
/// removed, THE OTHER card lands in the grip, and the third heap card is
/// untouched. A second HQ run the same turn is not offered (9.6.5c), and a
/// decline moves nothing.
#[test]
fn steve_cambridge_hands_one_card_to_the_corp_and_takes_the_other() {
    for accept in [true, false] {
        let mut vm = Vm::empty(6146);
        tk::install_identity(&mut vm, card("Steve Cambridge: Master Grifter"), Side::Runner);
        let heap: Vec<ObjectId> = (0..3)
            .map(|i| {
                let dead = vm.new_object(
                    tk::vanilla_runner_card(
                        ["Dead One", "Dead Two", "Dead Three"][i],
                        CardType::Event,
                    ),
                    Zone::Discard(Side::Runner),
                );
                vm.st.discard.get_mut(&Side::Runner).unwrap().push(dead);
                dead
            })
            .collect();
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let (corp, runner) = if accept {
            (
                Plan::corp().when(Match::targets().once(), Reply::Targets(vec![heap[0]])),
                Plan::runner()
                    .when(Match::action().once(), Reply::run(ServerId::Hq))
                    .when(Match::action().once(), Reply::run(ServerId::Hq))
                    .when(
                        Match::reaction().offering("master grifter"),
                        Reply::take("master grifter"),
                    )
                    .when(Match::targets().once(), Reply::Targets(vec![heap[0], heap[1]]))
                    .stop_at_action(),
            )
        } else {
            // 9.6.9: declining is not taking the offered reaction at all.
            (
                Plan::corp(),
                Plan::runner()
                    .when(Match::action().once(), Reply::run(ServerId::Hq))
                    .stop_at_action(),
            )
        };
        let t = plan::play(&mut vm, corp, runner);

        if accept {
            let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
            assert_eq!(
                announcements.len(),
                2,
                "two announcements — the Runner's pair and the Corp's pick — and no third \
                 on the second HQ run of the turn (9.6.5c): {}",
                t.tail(40)
            );
            assert_eq!(
                announcements[0].candidates().len(),
                3,
                "the whole heap is offered for the Runner's choice: {}",
                t.tail(40)
            );
            assert_eq!(
                announcements[1].candidates(),
                [heap[0], heap[1]],
                "1.15.4: the Corp picks among THOSE cards — the two announced ones, never \
                 the third heap card: {}",
                t.tail(40)
            );
            assert_eq!(
                vm.st.objects[&heap[0]].zone,
                Zone::RemovedFromGame,
                "the Corp's pick is removed from the game: {}",
                t.tail(40)
            );
            assert_eq!(
                vm.st.objects[&heap[1]].zone,
                Zone::Hand(Side::Runner),
                "the OTHER card went to the grip: {}",
                t.tail(40)
            );
            assert!(
                !vm.changes.log.iter().any(|c| matches!(
                    c,
                    GameChange::CardMoved { obj, to: Zone::RemovedFromGame, .. } if *obj == heap[1]
                )),
                "and it went there directly — the removal acted on the Corp's pick alone, \
                 never on the whole announced union: {}",
                t.tail(40)
            );
            assert_eq!(
                vm.st.objects[&heap[2]].zone,
                Zone::Discard(Side::Runner),
                "the card the Runner never chose stayed in the heap: {}",
                t.tail(40)
            );
        } else {
            assert!(
                heap.iter().all(|c| vm.st.objects[c].zone == Zone::Discard(Side::Runner)),
                "a decline moves nothing: {}",
                t.tail(40)
            );
        }
    }
}

/// Steve Cambridge with ONE card in the heap: 1.15.2e announces as many
/// distinct targets as exist, so the choice names one card, the Corp removes
/// it, and "the other card" describes nothing — the grip gains nothing
/// (1.15.3), which is the printed ruling rather than an error.
#[test]
fn steve_cambridge_with_one_heap_card_gives_the_runner_nothing() {
    let mut vm = Vm::empty(6147);
    tk::install_identity(&mut vm, card("Steve Cambridge: Master Grifter"), Side::Runner);
    let only = vm.new_object(
        tk::vanilla_runner_card("Only One", CardType::Event),
        Zone::Discard(Side::Runner),
    );
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(only);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    let grip_before = vm.st.hand[&Side::Runner].len();

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::targets().once(), Reply::Targets(vec![only])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::reaction().offering("master grifter"), Reply::take("master grifter"))
            .when(Match::targets().once(), Reply::Targets(vec![only]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&only].zone,
        Zone::RemovedFromGame,
        "the Corp still removes the one announced card: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        grip_before,
        "and there is no OTHER card to add (1.15.3): {}",
        t.tail(40)
    );
}

/// Captain Padma Isbister: "The first time each turn a run on R&D begins, you
/// may charge 1 of your installed cards. (Add 1 power counter to a card that
/// already has one.)"
///
/// The reminder text is the description: only the card that ALREADY has a
/// power counter is a candidate. And the ordinal is about the occurrence, so
/// the second R&D run of the turn is not offered at all.
#[test]
fn captain_padma_isbister_charges_only_a_card_that_already_has_a_counter() {
    let mut vm = Vm::empty(6145);
    tk::install_identity(
        &mut vm,
        card("Captain Padma Isbister: Intrepid Explorer"),
        Side::Runner,
    );
    let loaded = tk::install_rig(&mut vm, tk::program_cost("Loaded", 0));
    let empty = tk::install_rig(&mut vm, tk::program_cost("Empty", 0));
    tk::place_counters(&mut vm, loaded, CounterKind::Power, 1);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::reaction().offering("intrepid explorer"), Reply::take("intrepid explorer"))
            .stop_at_action(),
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "offered once, on the first R&D run: {}", t.tail(40));
    assert_eq!(
        announcements[0].candidates(),
        [loaded],
        "only the card that already has a power counter can be charged: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.objects[&loaded].counter(CounterKind::Power), 2, "charged: {}", t.tail(40));
    assert_eq!(vm.st.objects[&empty].counter(CounterKind::Power), 0, "and the other untouched");
}

/// Hiram "0mission" Svensson: "Whenever you install or trash a piece of
/// hardware (from any location), look at the top card of R&D."
///
/// Three occurrences in one turn: a piece of hardware installed from the
/// grip, the same one trashed off the rig, and a PROGRAM installed — the
/// first two look, the third does not, because the sentence names a type.
#[test]
fn hiram_svensson_looks_on_installing_and_on_trashing_hardware() {
    let mut vm = Vm::empty(6146);
    tk::install_identity(
        &mut vm,
        card("Hiram \"0mission\" Svensson: Shadow of the Past"),
        Side::Runner,
    );
    let hw = vm.new_object(
        tk::vanilla_runner_card("Some Hardware", CardType::Hardware),
        Zone::Hand(Side::Runner),
    );
    let prog = vm.new_object(tk::program_cost("Some Program", 0), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([hw, prog]);
    tk::install_rig(&mut vm, tk::runner_install_button("Install-Button", 1));
    tk::install_rig(&mut vm, tk::trash_set_button("Trash-Button", vec![hw]));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let looks = |vm: &Vm| {
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CardLookedAt { by, .. } if *by == Side::Runner))
            .count()
    };
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("install-button"))
            .when(Match::targets().once(), Reply::Targets(vec![hw]))
            .when(Match::paid().once(), Reply::take("trash the set"))
            .when(Match::paid().once(), Reply::take("install-button"))
            .when(Match::targets().once(), Reply::Targets(vec![prog]))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&prog].zone, Zone::Rig, "the program was installed too: {}", t.tail(40));
    assert_eq!(
        vm.st.objects[&hw].zone,
        Zone::Discard(Side::Runner),
        "and the hardware was trashed: {}",
        t.tail(40)
    );
    assert_eq!(
        looks(&vm),
        2,
        "the install and the trash each looked; installing a PROGRAM did not: {}",
        t.tail(40)
    );
}

/// The Collective: "The first time you perform the same action three times in
/// a row each turn, gain [click]."
///
/// Four basic credit actions: the third meets the condition and pays a click,
/// the fourth does not, because the printed ordinal is about the occurrence
/// and the turn has already had its one.
#[test]
fn the_collective_pays_a_click_for_three_of_the_same_action_in_a_row() {
    let mut vm = Vm::empty(6147);
    tk::install_identity(&mut vm, card("The Collective: Williams, Wu, et al."), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().times(4), Reply::credit()).stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 4, "four basic credit actions: {}", t.tail(40));
    assert_eq!(
        clicks_gained_by_the_runner(&vm),
        1,
        "the third of the three paid, and the fourth did not — the ordinal is spent: {}",
        t.tail(40)
    );
}

/// The Collective again: "in a row" is the whole of it. A different action in
/// the middle breaks the run, so three identical actions that are not
/// consecutive pay nothing.
#[test]
fn the_collective_pays_nothing_when_the_run_of_actions_is_broken() {
    let mut vm = Vm::empty(6148);
    tk::install_identity(&mut vm, card("The Collective: Williams, Wu, et al."), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::credit())
            .stop_at_action(),
    );
    assert_eq!(
        clicks_gained_by_the_runner(&vm),
        0,
        "four actions and no three of a kind in a row, so nothing was paid: {}",
        t.tail(40)
    );
}

/// Null: Whistleblower: "Once per turn → When you encounter a piece of ice,
/// you may trash 1 card from your grip. If you do, that ice gets –2 strength
/// for the remainder of this run."
///
/// The trash is 1.16.11a's optional COST, so the strength drops only when it
/// is paid — and the cards offered are the grip's, which is what naming the
/// zone does.
#[test]
fn null_whistleblower_trashes_from_the_grip_to_weaken_the_encountered_ice() {
    let mut vm = Vm::empty(6149);
    tk::install_identity(&mut vm, card("Null: Whistleblower"), Side::Runner);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 4), ServerId::Hq, true);
    let hand = tk::fill_hand(&mut vm, Side::Runner, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // The modification lives for the run, so it has to be read from inside
    // one: halt at 6.9.4c's jack-out decision, which follows passing the ice.
    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .when(Match::payment_cards().once(), Reply::Targets(vec![hand[0]]))
            .when(Match::of(Kind::JackOut).once(), Reply::Halt)
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    script.run(&mut vm);
    let t = script.transcript();
    assert_eq!(
        vm.st.objects[&hand[0]].zone,
        Zone::Discard(Side::Runner),
        "the chosen card of the GRIP paid the cost: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.effective_strength(ice),
        Some(2),
        "printed 4, less the 2 the sentence takes off — and still, now the encounter has ended: {}",
        t.tail(40)
    );

    script.run(&mut vm); // resume: finish the run
    let t = script.transcript();
    assert_eq!(
        vm.effective_strength(ice),
        Some(4),
        "the modification died with the run it was made for: {}",
        t.tail(40)
    );
}

/// Null again, from the other side: declining the optional cost costs nothing
/// and takes nothing off the ice — 1.16.11a, and 9.6.9d is what gives the
/// printed "Once per turn →" flag something to be spent by.
#[test]
fn null_whistleblower_takes_nothing_off_the_ice_when_the_cost_is_declined() {
    let mut vm = Vm::empty(6151);
    tk::install_identity(&mut vm, card("Null: Whistleblower"), Side::Runner);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 4), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Runner, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::nested_cost().once(), Reply::PayCost(false))
            .when(Match::of(Kind::JackOut).once(), Reply::Halt)
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    script.run(&mut vm);
    let t = script.transcript();
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "nothing left the grip: {}", t.tail(40));
    assert_eq!(
        vm.effective_strength(ice),
        Some(4),
        "and the ice is as printed: {}",
        t.tail(40)
    );
}

/// The printed "Once per turn →" flag is SPENT by paying: the second
/// encounter of the same turn gets no offer at all. 9.1.6b — a conditional
/// ability is used once its optional component is carried out, and paying
/// the nested cost IS carrying it out, whether or not the ability's own
/// trigger was optional — and 9.3.6g is what that use spends. (Audit
/// 2026-08-05 finding 2: the may_pay path never spent the flag, so Null
/// could weaken every ice it met, every turn.)
#[test]
fn null_whistleblower_once_per_turn_is_spent_by_paying() {
    let mut vm = Vm::empty(6152);
    tk::install_identity(&mut vm, card("Null: Whistleblower"), Side::Runner);
    tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 4), ServerId::Hq, true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 4), ServerId::Hq, true);
    let hand = tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .when(Match::payment_cards(), Reply::Targets(vec![hand[0]]))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    assert_eq!(
        t.of_kind(Kind::NestedCost).len(),
        1,
        "9.3.6g: the flag was spent at the first encounter — the second offers nothing: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "one card paid, once: {}", t.tail(40));
}

/// Ryō "Phoenix" Ōno: "The first time each turn a run becomes successful
/// after a subroutine resolved during that run, gain 1[credit] and the Corp
/// trashes 1 card from HQ."
///
/// The requirement is part of the condition: a successful run with a
/// subroutine resolved pays, and one without it does not — and does not spend
/// the turn's one time either.
#[test]
fn ryo_phoenix_ono_pays_only_after_a_subroutine_resolved_that_run() {
    for resolved in [false, true] {
        let mut vm = Vm::empty(6150);
        tk::install_identity(
            &mut vm,
            card("Ryō \"Phoenix\" Ōno: Out of the Ashes"),
            Side::Runner,
        );
        let server = if resolved { ServerId::Hq } else { ServerId::Rnd };
        tk::install_ice(&mut vm, tk::three_sub_ice("Some Ice"), ServerId::Hq, true);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);
        let hq_before = vm.st.hand[&Side::Corp].len();

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner().when(Match::action().once(), Reply::run(server)).stop_at_action(),
        );
        assert_eq!(
            vm.st.runner.credits,
            u32::from(resolved),
            "the credit is the requirement's (resolved={resolved}): {}",
            t.tail(40)
        );
        assert_eq!(
            hq_before - vm.st.hand[&Side::Corp].len(),
            usize::from(resolved),
            "…and so is the card the Corp pitches (resolved={resolved}): {}",
            t.tail(40)
        );
    }
}

/// The same sentence, from the other end: 9.6.5c's ordinal counts only the
/// occurrences that met the WHOLE condition, so a plain successful run — no
/// subroutine resolved — earlier in the turn was never one of "the times"
/// and spends nothing. The stipulation is read off the declaration's own
/// record: by the time the second run's checkpoint re-asks the condition of
/// the first run's declaration, that run's history window is long closed and
/// the board has nothing left to say about it.
#[test]
fn ryo_phoenix_ono_plain_successful_run_does_not_spend_the_turns_one_time() {
    let mut vm = Vm::empty(6191);
    tk::install_identity(
        &mut vm,
        card("Ryō \"Phoenix\" Ōno: Out of the Ashes"),
        Side::Runner,
    );
    tk::install_ice(&mut vm, tk::three_sub_ice("Some Ice"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);
    let hq_before = vm.st.hand[&Side::Corp].len();

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    let successes: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(successes.len(), 2, "both runs were declared successful: {}", t.tail(50));
    assert_eq!(
        vm.st.runner.credits, 1,
        "the R&D run resolved no subroutine, so the HQ run is still the first time: {}",
        t.tail(50)
    );
    assert_eq!(
        hq_before - vm.st.hand[&Side::Corp].len(),
        1,
        "…and the Corp pitches exactly one card, for the HQ run: {}",
        t.tail(50)
    );
    // The change-log claim that pins WHICH run paid: nothing was gained off
    // the first, subroutine-less success.
    let gained_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CreditsGained { side: Side::Runner, .. }))
        .expect("the identity paid out");
    assert!(
        gained_at > successes[1],
        "the credit follows the SECOND declaration, not the first: {}",
        t.tail(50)
    );
}

/// 1.11.3a: how many clicks an ABILITY gave the Runner in their first turn —
/// read from the change log, so an assertion about it does not depend on
/// where the plan happened to stop. 5.6.2a's allotment is recorded the same
/// way and is the first of them, which is what the `skip(1)` drops.
fn clicks_gained_by_the_runner(vm: &Vm) -> usize {
    vm.changes
        .log
        .iter()
        .take_while(|c| !matches!(c, GameChange::TurnEnded { .. }))
        .filter(|c| matches!(c, GameChange::ClicksGained { side: Side::Runner, .. }))
        .skip(1)
        .count()
}

// ---------------------------------------------------------------------------
// The identity queue — CR 1.16.4a's inherent costs, moved by a declaration
// ---------------------------------------------------------------------------

/// Az McCaffrey: "The first job resource, connection resource, or piece of
/// hardware you install each turn costs 1[credit] less to install."
///
/// Three installs in one turn prove all three halves of the sentence: a plain
/// resource matches no alternative of the description, so it pays in full AND
/// leaves the ordinal unspent; the job resource after it is "the first", so it
/// pays one less; the piece of hardware after that is the second and pays in
/// full. Nothing is ever offered to anyone — the reduction is automatic, not
/// Patchwork's.
#[test]
fn az_mccaffrey_lowers_the_first_matching_install_of_the_turn_only() {
    let mut vm = Vm::empty(6140);
    tk::install_identity(&mut vm, card("Az McCaffrey: Mechanical Prodigy"), Side::Runner);
    let mut mk = |name: &'static str, ty: CardType, subtype: Option<&'static str>| {
        let mut c = tk::vanilla_runner_card(name, ty);
        c.cost = Some(3);
        if let Some(s) = subtype {
            c.subtypes = vec![s];
        }
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let plain = mk("Plain Resource", CardType::Resource, None);
    let job = mk("Job Resource", CardType::Resource, Some("Job"));
    let hardware = mk("Some Hardware", CardType::Hardware, None);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    // 3 for the plain resource, 2 for the first matching install, 3 for the
    // second — exactly, so a wrong number anywhere leaves credits behind or
    // makes an install unaffordable.
    vm.st.runner.credits = 8;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(plain)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(job)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(hardware)))
            .stop_at_action(),
    );
    for c in [plain, job, hardware] {
        assert_eq!(vm.st.objects[&c].zone, Zone::Rig, "{} was installed: {}", vm.st.objects[&c].printed.name, t.tail(40));
    }
    assert_eq!(
        vm.st.runner.credits, 0,
        "3 + 2 + 3: the description's own cards are the only ones lowered, and only the first of them: {}",
        t.tail(40)
    );
}

/// Kate "Mac" McCaffrey: "Lower the install cost of the first program or
/// piece of hardware you install each turn by 1."
///
/// The same declaration about a different description. A resource is outside
/// it and pays in full without spending the ordinal; the program after it is
/// "the first" and pays one less; the hardware after that pays in full.
#[test]
fn kate_mac_mccaffrey_lowers_the_first_program_or_hardware_of_the_turn() {
    let mut vm = Vm::empty(6141);
    tk::install_identity(&mut vm, card("Kate \"Mac\" McCaffrey: Digital Tinker"), Side::Runner);
    let mut mk = |name: &'static str, ty: CardType| {
        let mut c = tk::vanilla_runner_card(name, ty);
        c.cost = Some(3);
        if ty == CardType::Program {
            c.memory_cost = Some(1);
        }
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let resource = mk("Some Resource", CardType::Resource);
    let program = mk("Some Program", CardType::Program);
    let hardware = mk("Some Hardware", CardType::Hardware);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 8;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(resource)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(program)))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(hardware)))
            .stop_at_action(),
    );
    for c in [resource, program, hardware] {
        assert_eq!(vm.st.objects[&c].zone, Zone::Rig, "{} was installed: {}", vm.st.objects[&c].printed.name, t.tail(40));
    }
    assert_eq!(
        vm.st.runner.credits, 0,
        "3 + 2 + 3: the resource is outside the description and the hardware is the second one inside it: {}",
        t.tail(40)
    );
}

/// Reina Roja: "The first piece of ice the Corp rezzes each turn costs
/// 1[credit] more to rez."
///
/// The asset rezzed first is not a piece of ice, so it pays its printed cost
/// and leaves the ordinal unspent; the first piece of ice pays one more; the
/// second pays its printed cost. 8.1.2a is why the two pieces of ice are
/// rezzed on approach and the asset is not: ice is rezzable only in the
/// window 6.9.2b opens for the ice being approached.
#[test]
fn reina_roja_taxes_the_first_ice_rez_of_the_turn_only() {
    let mut vm = Vm::empty(6142);
    tk::install_identity(&mut vm, card("Reina Roja: Freedom Fighter"), Side::Runner);
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 1, 2), ServerId::Remote(1), false);
    let first = tk::install_ice(&mut vm, tk::etr_ice("First Ice", 2, 1), ServerId::Rnd, false);
    let second = tk::install_ice(&mut vm, tk::etr_ice("Second Ice", 2, 1), ServerId::Hq, false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    // 1 for the asset, 3 for the first piece of ice, 2 for the second.
    vm.st.corp.credits = 6;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::Take(Pick::Rez(asset)))
            .when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    for c in [asset, first, second] {
        assert!(vm.st.objects[&c].faceup, "{} was rezzed: {}", vm.st.objects[&c].printed.name, t.tail(40));
    }
    assert_eq!(
        vm.st.corp.credits, 0,
        "1 + 3 + 2: only ice is described, and only the first piece of it: {}",
        t.tail(40)
    );
}

/// Khan: "The first time you pass a piece of ice each turn, you may install
/// an icebreaker from your hand, lowering the install cost by 1."
///
/// The pass is plain: the ice is not Khan's, nothing was broken on it and no
/// subroutine of it resolved, and the sentence is still met. The icebreaker
/// costs 3 and the Runner has 2, so the install proves the reduction as well
/// as the condition — without it there is nothing to pay with.
#[test]
fn khan_installs_an_icebreaker_for_one_less_on_the_turns_first_pass() {
    let mut vm = Vm::empty(6143);
    tk::install_identity(&mut vm, card("Khan: Savvy Skiptracer"), Side::Runner);
    tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 1), ServerId::Archives, true);
    let breaker = {
        let mut c = tk::vanilla_runner_card("Some Breaker", CardType::Program);
        c.cost = Some(3);
        c.memory_cost = Some(1);
        c.subtypes = vec!["Icebreaker"];
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let plain = {
        let mut c = tk::vanilla_runner_card("Some Program", CardType::Program);
        c.cost = Some(0);
        c.memory_cost = Some(1);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::reaction().offering("skiptracer"), Reply::take("skiptracer"))
            .when(Match::targets().once(), Reply::Targets(vec![breaker]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&breaker].zone,
        Zone::Rig,
        "the icebreaker was installed off a plain pass: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.runner.credits, 0,
        "2 credits paid a 3-credit install: the ability lowered it by 1: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&plain].zone,
        Zone::Hand(Side::Runner),
        "and the program that is no icebreaker stayed in the grip: {}",
        t.tail(40)
    );
}

/// René "Loup" Arcemont: "The first time each turn you trash a card you are
/// accessing, gain 1[credit] and draw 1 card."
///
/// Two remotes with a trashable asset each, run one after the other. The
/// first trash pays and draws; the second, in the same turn, does neither —
/// 9.6.5c's ordinal is about the occurrence, so it is spent by the first
/// trash and never met again.
///
/// A trash that is not of the accessed card must NOT meet it, which is what
/// the Corp asset the Runner never reaches is there to prove: it is trashed
/// by the Corp's own ability during the same turn and pays nothing.
#[test]
fn rene_loup_arcemont_pays_for_the_first_accessed_trash_of_the_turn_only() {
    let mut vm = Vm::empty(6144);
    tk::install_identity(&mut vm, card("René \"Loup\" Arcemont: Party Animal"), Side::Runner);
    let first = tk::install_root(&mut vm, tk::vanilla_asset("First Asset", 0, 1), ServerId::Remote(1), true);
    let second = tk::install_root(&mut vm, tk::vanilla_asset("Second Asset", 0, 1), ServerId::Remote(2), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);
    assert!(vm.st.hand[&Side::Runner].is_empty(), "the grip starts empty");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::of(Kind::MidAccess).once(), Reply::Take(Pick::BasicTrash))
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .when(Match::of(Kind::MidAccess).once(), Reply::Take(Pick::BasicTrash))
            .stop_at_action(),
    );
    for c in [first, second] {
        assert_eq!(
            vm.st.objects[&c].zone,
            Zone::Discard(Side::Corp),
            "{} was trashed out of the access: {}",
            vm.st.objects[&c].printed.name,
            t.tail(50)
        );
    }
    // 5 credits, two trash costs of 1 each, one credit back from the ability.
    assert_eq!(
        vm.st.runner.credits, 4,
        "one credit for the FIRST accessed trash and nothing for the second: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        1,
        "and one card drawn, once: {}",
        t.tail(50)
    );
}

/// The other half of René "Loup" Arcemont's condition: a trash that is not of
/// the card being accessed is not one of "the times", so it neither pays nor
/// spends the ordinal.
///
/// The Runner trashes an installed program of their own with a card ability
/// while no access is in progress, then runs and trashes an accessed asset.
/// If the first trash had met the condition it would have taken the credit
/// and the ordinal with it.
#[test]
fn rene_loup_arcemont_ignores_a_trash_outside_an_access() {
    let mut vm = Vm::empty(6145);
    tk::install_identity(&mut vm, card("René \"Loup\" Arcemont: Party Animal"), Side::Runner);
    // A Runner card whose paid ability costs [trash] — a trash of a card with
    // no access anywhere near it.
    tk::install_rig(&mut vm, tk::trash_cost_ability_card("Aesop-like"));
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Trashable", 0, 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("trash-cost"))
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::of(Kind::MidAccess).once(), Reply::Take(Pick::BasicTrash))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Discard(Side::Corp),
        "the accessed asset was trashed: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        1,
        "exactly one draw — from the accessed trash, not from the rig trash: {}",
        t.tail(50)
    );
}

/// Nasir Meidan: "Whenever you encounter a piece of ice after an approach
/// during which that ice was rezzed, lose all credits in your credit pool.
/// Gain credits equal to the rez cost of that ice."
///
/// One run past two pieces of ice. The outer one is rezzed on its approach,
/// so the encounter takes the Runner's whole pool and gives back its rez
/// cost; the inner one is already faceup when it is approached, so its
/// encounter does nothing at all — which is the difference between "was
/// rezzed during that approach" and "is rezzed".
#[test]
fn nasir_meidan_trades_the_pool_for_the_rez_cost_of_ice_rezzed_on_approach() {
    let mut vm = Vm::empty(6146);
    tk::install_identity(&mut vm, card("Nasir Meidan: Cyber Explorer"), Side::Runner);
    // Innermost first: the already-faceup one, which the run reaches second.
    tk::install_ice(&mut vm, tk::vanilla_ice("Already Faceup", 1, 1), ServerId::Archives, true);
    let outer = tk::install_ice(&mut vm, tk::vanilla_ice("Rezzed On Approach", 4, 1), ServerId::Archives, false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 9;
    vm.st.runner.credits = 7;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert!(vm.st.objects[&outer].faceup, "the outer ice was rezzed on its approach: {}", t.tail(50));
    assert_eq!(
        vm.st.runner.credits, 4,
        "7 credits gone, 4 back — the outer ice's printed rez cost — and the faceup \
         inner ice's encounter changed nothing: {}",
        t.tail(50)
    );
}

// ---------------------------------------------------------------------------
// The identity queue — Haas-Bioroid
// ---------------------------------------------------------------------------

/// Asa Group: "The first time each turn you install a card, you may install 1
/// non-agenda card from HQ in the root of or protecting the same server."
///
/// "The same server" is the one the card the occurrence named is in, and
/// 4.6.6b's two halves of it are what the Corp still declares. The proof is
/// that the destination taken by DEFAULT — the first the game offers — is a
/// position protecting the new remote and not one protecting HQ, which is
/// what the offer would start with if the server were the installer's to
/// pick. The agenda in HQ is never a candidate.
#[test]
fn asa_group_installs_a_second_card_into_the_same_server_and_no_other() {
    let mut vm = Vm::empty(6152);
    tk::install_identity(&mut vm, card("Asa Group: Security Through Vigilance"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("First Card", 0, 2), Zone::Hand(Side::Corp));
    let ice = vm.new_object(tk::vanilla_ice("Second Card", 0, 1), Zone::Hand(Side::Corp));
    let decoy = vm.new_object(tk::vanilla_agenda("Not Installable", 3, 2), Zone::Hand(Side::Corp));
    for id in [asset, ice, decoy] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::reaction().offering("vigilance"), Reply::take("vigilance"))
            .when(Match::targets().once(), Reply::Targets(vec![ice]))
            .when(Match::of(Kind::Destination).once(), Reply::Default)
            .stop_at_action(),
        Plan::runner(),
    );
    let Zone::Root(remote) = vm.st.objects[&asset].zone else {
        panic!("the first card made a remote: {}", t.tail(40))
    };
    assert!(matches!(remote, ServerId::Remote(_)), "and it is a remote one: {}", t.tail(40));
    assert_eq!(
        vm.st.objects[&ice].zone,
        Zone::Ice(remote),
        "the second card went to the SAME server, taking the only destination \
         the effect offered — the default would be a position protecting HQ if \
         the server were the installer's to pick: {}",
        t.tail(40)
    );
    let announcements: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(announcements.len(), 1, "one card was chosen: {}", t.tail(40));
    assert!(
        !announcements[0].candidates().contains(&decoy),
        "'non-agenda' keeps the agenda in HQ out of the candidates: {}",
        t.tail(40)
    );
}

/// Cerebral Imaging: "Your maximum hand size is equal to the number of
/// credits in your credit pool."
///
/// The declaration SETS the value rather than moving it (CR 9.12.1a), and it
/// is read continuously: the same board answers differently as the pool
/// changes. The discard phase is the proof that the limit is the credits and
/// not the base of five — three credits leave three cards in HQ.
#[test]
fn cerebral_imaging_makes_the_hand_size_the_credit_pool() {
    let mut vm = Vm::empty(6147);
    tk::install_identity(&mut vm, card("Cerebral Imaging: Infinite Frontiers"), Side::Corp);
    vm.st.corp.credits = 9;
    assert_eq!(vm.max_hand_size(Side::Corp), 9, "nine credits, nine cards — not 5 + 9");
    assert_eq!(vm.max_hand_size(Side::Runner), 5, "'your' is the Corp's, and only the Corp's");
    vm.st.corp.credits = 2;
    assert_eq!(vm.max_hand_size(Side::Corp), 2, "read continuously as the pool changes");

    tk::fill_hand(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::credit()),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn finished: {}", t.tail(30));
    assert_eq!(
        vm.st.corp.credits, 5,
        "2 credits plus three basic actions of 1: the pool the limit is read from"
    );
    assert_eq!(
        vm.st.hand[&Side::Corp].len(),
        5,
        "5.7.4 discarded down to the credit pool, not to the base of five: {}",
        t.tail(30)
    );
}

/// Chronos Protocol: Haas-Bioroid — "Whenever the Runner trashes a card for
/// brain damage, they remove all copies of that card from the game (installed,
/// in the heap, stack, grip, or any other location). Then, they shuffle their
/// stack."
///
/// One core damage against a grip holding a single card, so the randomly
/// trashed card is known. Four more copies of it sit in four different places
/// — the rig, the stack, the heap, and the stack again — and the parenthesis
/// is what reaches all of them: without it 1.15.2c would leave the description
/// meaning the installed cards alone. The trashed card itself is removed too;
/// it is in the heap by the time the ability resolves, which is the first
/// location the parenthesis names.
///
/// The cards that are not copies stay in the stack, and their ORDER does not:
/// the second sentence is a shuffle of its own, and the seed is what makes
/// that assertable.
#[test]
fn chronos_protocol_hb_removes_every_copy_of_the_card_core_damage_trashed() {
    const NAME: &str = "Doppelgänger";
    let mut vm = Vm::empty(6149);
    tk::install_identity(&mut vm, card("Chronos Protocol: Haas-Bioroid"), Side::Corp);
    tk::install_root(&mut vm, tk::core_damage_button("Hurt", 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);

    let copy = |vm: &mut Vm, zone: Zone| vm.new_object(copy_card(NAME), zone);
    let in_grip = copy(&mut vm, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(in_grip);
    let in_heap = copy(&mut vm, Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(in_heap);
    let in_rig = tk::install_rig(&mut vm, copy_card(NAME));
    // A stack of six unrelated cards with two copies buried in it.
    let filler = tk::fill_deck(&mut vm, Side::Runner, 6);
    let in_stack: Vec<ObjectId> = (0..2)
        .map(|_| {
            let id = copy(&mut vm, Zone::Deck(Side::Runner));
            vm.st.deck.get_mut(&Side::Runner).unwrap().push(id);
            id
        })
        .collect();
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("do core damage")).stop_at_action(),
        Plan::runner(),
    );

    for (label, id) in [
        ("the trashed card itself, now in the heap", in_grip),
        ("the copy already in the heap", in_heap),
        ("the installed copy", in_rig),
        ("the first copy in the stack", in_stack[0]),
        ("the second copy in the stack", in_stack[1]),
    ] {
        assert_eq!(
            vm.st.objects[&id].zone,
            Zone::RemovedFromGame,
            "§4.9: {label} was removed from the game: {}",
            t.tail(40)
        );
    }
    let stack = vm.st.deck[&Side::Runner].clone();
    assert_eq!(
        stack.len(),
        6,
        "and only the copies left: a card that is not one is not described: {}",
        t.tail(40)
    );
    assert!(
        filler.iter().all(|f| stack.contains(f)),
        "every non-copy is still in the stack: {}",
        t.tail(40)
    );
    assert_ne!(
        stack, filler,
        "4.2.3: and the second sentence shuffled them out of the order the removal \
         left them in: {}",
        t.tail(40)
    );
}

/// The two halves of the same sentence that decide WHICH occurrence it is met
/// by.
///
/// "Brain damage" is 10.4.2c's older name for core damage and names one of
/// 10.4.2's three types, so a net damage that trashes exactly the same card
/// removes no copies at all. And 10.4.3 trashes the cards of a multi-point
/// damage SIMULTANEOUSLY — one occurrence naming both — so two core damage
/// against a grip of two differently-named cards reaches the copies of BOTH,
/// not just of the first.
#[test]
fn chronos_protocol_hb_reads_the_damage_kind_and_every_card_one_damage_trashed() {
    const A: &str = "Doppelgänger";
    const B: &str = "Zamboni";
    for (label, core, points) in [("net", false, 1), ("core", true, 1), ("core pair", true, 2)] {
        let mut vm = Vm::empty(6150);
        tk::install_identity(&mut vm, card("Chronos Protocol: Haas-Bioroid"), Side::Corp);
        let button = if core {
            tk::core_damage_button("Hurt", points)
        } else {
            tk::net_damage_button("Hurt", points)
        };
        tk::install_root(&mut vm, button, ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);

        // The grip holds one of each, so a 2-point damage trashes both and a
        // 1-point damage trashes whichever the 10.4.3 randomiser picks.
        let grip: Vec<ObjectId> = [A, B]
            .into_iter()
            .map(|n| {
                let id = vm.new_object(copy_card(n), Zone::Hand(Side::Runner));
                vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
                id
            })
            .collect();
        let spares: Vec<ObjectId> =
            [A, B].into_iter().map(|n| tk::install_rig(&mut vm, copy_card(n))).collect();
        tk::fill_deck(&mut vm, Side::Runner, 3);
        vm.start_turn(Side::Corp);

        let take = if core { "do core damage" } else { "do net damage" };
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::paid().once(), Reply::take(take)).stop_at_action(),
            Plan::runner(),
        );

        let removed: Vec<bool> =
            spares.iter().map(|s| vm.st.objects[s].zone == Zone::RemovedFromGame).collect();
        match label {
            "net" => assert_eq!(
                removed,
                vec![false, false],
                "10.4.2: the sentence names brain damage, so a net damage that trashes \
                 the very same cards reaches nothing: {}",
                t.tail(40)
            ),
            "core" => {
                let trashed: Vec<bool> = grip
                    .iter()
                    .map(|g| vm.st.objects[g].zone == Zone::RemovedFromGame)
                    .collect();
                assert_eq!(
                    removed, trashed,
                    "one core damage trashed one of the two, and the copy removed is \
                     the copy of THAT one: {}",
                    t.tail(40)
                );
                assert_eq!(
                    removed.iter().filter(|r| **r).count(),
                    1,
                    "exactly one of them: {}",
                    t.tail(40)
                );
            }
            _ => assert_eq!(
                removed,
                vec![true, true],
                "10.4.3: two core damage trash their cards simultaneously, so ONE \
                 occurrence names both and 1.15.4's \"that card\" is both: {}",
                t.tail(40)
            ),
        }
    }
}

/// Haas-Bioroid: Architects of Tomorrow: "The first time each turn the Runner
/// passes a rezzed piece of bioroid ice, you may rez 1 bioroid card, paying
/// 4[credit] less."
///
/// Two pieces of ice on the way in. The outer one is passed unrezzed and is
/// not a bioroid either, so the condition's description of the ice keeps it
/// out; the inner one is a rezzed bioroid, and passing it offers the rez. The
/// bioroid asset costs 5 and the Corp holds 1, so the install happening at
/// all is the reduction: without it there is nothing to pay with.
#[test]
fn architects_of_tomorrow_rezzes_a_bioroid_for_four_less_on_a_bioroid_pass() {
    let mut vm = Vm::empty(6148);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Architects of Tomorrow"), Side::Corp);
    // Innermost first: the rezzed bioroid, which the run reaches second.
    tk::install_ice(
        &mut vm,
        tk::subtyped_ice("Inner Bioroid", vec!["Bioroid"], 0, 1),
        ServerId::Archives,
        true,
    );
    tk::install_ice(&mut vm, tk::vanilla_ice("Outer Plain", 0, 1), ServerId::Archives, false);
    let asset = {
        let mut c = tk::vanilla_asset("Bioroid Asset", 5, 2);
        c.subtypes = vec!["Bioroid"];
        tk::install_root(&mut vm, c, ServerId::Remote(1), false)
    };
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 1;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("architects"), Reply::take("architects"))
            .when(Match::targets().once(), Reply::Targets(vec![asset])),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert!(
        vm.st.objects[&asset].faceup,
        "the bioroid card was rezzed off the pass of the rezzed bioroid ice: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.corp.credits, 0,
        "1 credit paid a printed rez cost of 5: the ability lowered it by 4: {}",
        t.tail(50)
    );
    let offers = t.of_kind(Kind::Reaction).len();
    assert_eq!(
        offers, 1,
        "one offer, on the bioroid pass — the plain unrezzed ice passed first met nothing: {}",
        t.tail(50)
    );
}

/// The other half of "rezzed": a fact of the pass's own moment. The same ice
/// is passed twice in one turn — unrezzed on the first run, rezzed on the
/// approach of the second — and only the second pass is one of "the times".
/// Reading the board instead of the record would find the first pass rezzed
/// too, count it as the turn's first, and withhold the offer the printed
/// sentence makes.
#[test]
fn architects_of_tomorrow_ice_rezzed_after_a_pass_does_not_rewrite_that_pass() {
    let mut vm = Vm::empty(6192);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Architects of Tomorrow"), Side::Corp);
    let gate = tk::install_ice(
        &mut vm,
        tk::subtyped_ice("Bioroid Gate", vec!["Bioroid"], 0, 1),
        ServerId::Archives,
        false,
    );
    let asset = {
        let mut c = tk::vanilla_asset("Bioroid Asset", 5, 2);
        c.subtypes = vec!["Bioroid"];
        tk::install_root(&mut vm, c, ServerId::Remote(1), false)
    };
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 1;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().approaching_ice().nth(2), Reply::Take(Pick::RezApproachedIce))
            .when(Match::reaction().offering("architects"), Reply::take("architects"))
            .when(Match::targets().once(), Reply::Targets(vec![asset])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert!(
        vm.st.objects[&gate].faceup,
        "the gate was rezzed on the second run's approach: {}",
        t.tail(50)
    );
    let offers = t.of_kind(Kind::Reaction).len();
    assert_eq!(
        offers, 1,
        "one offer, on the second run's pass — the unrezzed pass before it was never one of the times: {}",
        t.tail(50)
    );
    assert!(
        vm.st.objects[&asset].faceup,
        "…and the offer was good: the bioroid asset is rezzed: {}",
        t.tail(50)
    );
    // The change-log claim that pins WHICH pass was the time: the asset's rez
    // follows the second run, not the first pass re-read as rezzed.
    let runs: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::RunBegan { .. }))
        .map(|(i, _)| i)
        .collect();
    let rezzed_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CardRezzed { obj, .. } if *obj == asset))
        .expect("the asset was rezzed");
    assert!(
        rezzed_at > runs[1],
        "the offer came on the second run's pass, not the first's: {}",
        t.tail(50)
    );
}

/// LEO Construction: "Once per turn → Trash 1 rezzed bioroid card in the root
/// of or protecting the attacked server: End the run."
///
/// 4.6.6b puts the root and the ice protecting it both *in* the server, so
/// the card in the root of the attacked remote is payable — and the identical
/// bioroid asset in the OTHER remote is not, which is the whole of "the
/// attacked server". Paying ends the run.
#[test]
fn leo_construction_ends_the_run_for_a_bioroid_in_the_attacked_server_only() {
    let mut vm = Vm::empty(6149);
    tk::install_identity(&mut vm, card("LEO Construction: Labor Solutions"), Side::Corp);
    let bioroid = |vm: &mut Vm, name: &'static str, server| {
        let mut c = tk::vanilla_asset(name, 0, 2);
        c.subtypes = vec!["Bioroid"];
        tk::install_root(vm, c, server, true)
    };
    let attacked = bioroid(&mut vm, "In The Attacked Server", ServerId::Remote(1));
    let elsewhere = bioroid(&mut vm, "In The Other Server", ServerId::Remote(2));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().offering("labor solutions"), Reply::take("labor solutions")),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Remote(1))).stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&attacked].zone,
        Zone::Discard(Side::Corp),
        "the card in the root of the attacked server paid the cost: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&elsewhere].zone,
        Zone::Root(ServerId::Remote(2)),
        "and the identical card in another server was never a candidate: {}",
        t.tail(50)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "the run ended: {}",
        t.tail(50)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "and it ended before the breach: {}",
        t.tail(50)
    );
}

/// The Foundry: "The first time you rez a piece of ice each turn, you may
/// search R&D for another copy of that ice, reveal it, and add it to HQ.
/// Shuffle R&D."
///
/// "Another copy of that ice" is a question about the NAME (2.1.4) asked of
/// the card the condition named (1.15.4): the copy in R&D is found and the
/// differently-named piece of ice beside it is not a candidate. 8.7.3 shuffles
/// R&D as part of the search, which is what the printed "Shuffle R&D" says.
#[test]
fn the_foundry_fetches_a_second_copy_of_the_ice_it_just_rezzed() {
    let mut vm = Vm::empty(6150);
    tk::install_identity(&mut vm, card("The Foundry: Refining the Process"), Side::Corp);
    let rezzed = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 1, 1), ServerId::Archives, false);
    let copy = vm.new_object(tk::vanilla_ice("Some Ice", 1, 1), Zone::Deck(Side::Corp));
    let other = vm.new_object(tk::vanilla_ice("Other Ice", 1, 1), Zone::Deck(Side::Corp));
    for id in [copy, other] {
        vm.st.deck.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce))
            .when(Match::reaction().offering("refining"), Reply::take("refining"))
            .when(Match::targets().once(), Reply::Targets(vec![copy])),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert!(vm.st.objects[&rezzed].faceup, "the ice was rezzed: {}", t.tail(50));
    let finds: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(finds.len(), 1, "one search find was put to the Corp: {}", t.tail(50));
    assert!(finds[0].candidates().contains(&copy), "the copy is a candidate: {}", t.tail(50));
    assert!(
        !finds[0].candidates().contains(&other),
        "'another copy of that ice' keeps the differently-named ice out: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&copy].zone,
        Zone::Hand(Side::Corp),
        "and the copy was revealed and added to HQ: {}",
        t.tail(50)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { obj, .. } if *obj == copy)),
        "the reveal is its own instruction (9.11.4e) and it happened: {}",
        t.tail(50)
    );
}

/// Thunderbolt Armaments: "Whenever you rez a piece of AP or destroyer ice
/// during a run, that ice gets +1 strength and gains “[subroutine] End the
/// run unless the Runner trashes 1 of their installed cards.” after its other
/// subroutines for the remainder of that run."
///
/// One sentence, so one instruction: the strength and the subroutine arrive
/// together. The printed "or" between the two subtypes is a disjunction, so a
/// destroyer that is no AP still meets it; the granted subroutine sits after
/// the printed one and takes the Runner's installed card.
#[test]
fn thunderbolt_armaments_pumps_and_arms_a_destroyer_rezzed_during_a_run() {
    let mut vm = Vm::empty(6151);
    tk::install_identity(&mut vm, card("Thunderbolt Armaments: Peace Through Power"), Side::Corp);
    let ice = tk::install_ice(
        &mut vm,
        tk::subtyped_ice("Some Destroyer", vec!["Destroyer"], 1, 2),
        ServerId::Archives,
        false,
    );
    let rig = tk::install_rig(&mut vm, tk::vanilla_runner_card("Some Program", CardType::Program));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // "For the remainder of that run" lapses when the run ends, so the
    // strength has to be read from INSIDE one: halt at 6.9.4c's jack-out
    // decision, which follows passing the ice.
    let mut script = plan::Script::new(
        Plan::corp().when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .when(Match::payment_cards().once(), Reply::Targets(vec![rig]))
            .when(Match::of(Kind::JackOut).once(), Reply::Halt)
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    script.run(&mut vm);
    let t = script.transcript();
    assert!(vm.st.objects[&ice].faceup, "the destroyer was rezzed during the run: {}", t.tail(60));
    assert_eq!(
        vm.effective_strength(ice),
        Some(3),
        "printed 2, plus the 1 the same sentence gave it: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&rig].zone,
        Zone::Discard(Side::Runner),
        "the granted subroutine took an installed card rather than ending the run: {}",
        t.tail(60)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "and the run is still on, which is what 'the remainder of that run' is read inside: {}",
        t.tail(60)
    );
}

/// Haas-Bioroid: Stronger Together — "All bioroid ice has +1 strength."
///
/// A declaration about the cards a DESCRIPTION reaches, so the test is the
/// description biting: two code gates with the SAME printed strength of 3,
/// one of them bioroid, and one Shibboleth of strength 3 run at each in turn.
/// 9.3.6c offers the interface ability against the plain gate and the
/// subroutine is broken; against the bioroid one the same breaker at the same
/// strength is one short, the ability is never offered, and the subroutine
/// ends the run.
#[test]
fn stronger_together_puts_bioroid_ice_out_of_a_breakers_reach() {
    let mut vm = Vm::empty(6152);
    tk::install_identity(&mut vm, card("Haas-Bioroid: Stronger Together"), Side::Corp);
    let mut plain_card = tk::etr_ice("Plain Gate", 0, 3);
    plain_card.subtypes = vec!["Code Gate"];
    let plain = tk::install_ice(&mut vm, plain_card, ServerId::Rnd, true);
    let mut bioroid_card = tk::etr_ice("Bioroid Gate", 0, 3);
    bioroid_card.subtypes = vec!["Code Gate", "Bioroid"];
    let bioroid = tk::install_ice(&mut vm, bioroid_card, ServerId::Hq, true);
    tk::install_rig(&mut vm, card("Shibboleth"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    // Two credits: enough for the 1[credit] break at EITHER gate, so what
    // decides which run gets through is the strength and nothing else. (The
    // printed pump costs 2 and the plan never reaches for it.)
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);

    assert_eq!(
        vm.effective_strength(bioroid),
        Some(4),
        "printed 3, plus the 1 the identity declares about every bioroid piece of ice"
    );
    assert_eq!(
        vm.effective_strength(plain),
        Some(3),
        "the same printed 3, and the description does not reach it"
    );

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(
                Match::paid().during(StructKind::Encounter).offering("interface: break").once(),
                Reply::take("interface: break"),
            )
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("End the run"))
            .stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(
            |c| matches!(c, GameChange::RunDeclaredUnsuccessful { server } if *server == ServerId::Hq)
        ),
        "the bioroid gate was 1 out of reach, so its subroutine ended the HQ run: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(
            |c| matches!(c, GameChange::RunDeclaredSuccessful { server, .. } if *server == ServerId::Rnd)
        ),
        "and the same breaker, at the same strength, broke the plain gate on R&D: {}",
        t.tail(40)
    );
}

// ---------------------------------------------------------------------------
// The identity queue — NBN and Weyland Consortium
// ---------------------------------------------------------------------------

/// Editorial Division: "The first time each turn you take bad publicity, you
/// may search R&D for 1 non-agenda black ops, gray ops, or liability card and
/// reveal it. (Shuffle R&D after searching it.) Add that card to HQ."
///
/// The printed "or" between the three subtypes is a disjunction, so a gray
/// ops card is found even though it is no black ops; the agenda carrying the
/// same subtype is kept out by "non-agenda", and the plain card by the
/// subtypes. 9.11.4e makes the reveal its own instruction, which is why it is
/// recorded before the move to HQ.
#[test]
fn editorial_division_fetches_a_gray_ops_card_when_bad_publicity_arrives() {
    let mut vm = Vm::empty(6153);
    tk::install_identity(&mut vm, card("Editorial Division: Ad Nihilum"), Side::Corp);
    tk::install_root(&mut vm, tk::take_bad_pub_button("Bad Press", 1), ServerId::Remote(1), true);
    let wanted = {
        let mut c = tk::vanilla_asset("Gray Ops Card", 0, 2);
        c.subtypes = vec!["Gray Ops"];
        vm.new_object(c, Zone::Deck(Side::Corp))
    };
    let agenda = {
        let mut c = tk::vanilla_agenda("Gray Ops Agenda", 3, 2);
        c.subtypes = vec!["Gray Ops"];
        vm.new_object(c, Zone::Deck(Side::Corp))
    };
    let plain = vm.new_object(tk::vanilla_asset("Plain Card", 0, 2), Zone::Deck(Side::Corp));
    for id in [wanted, agenda, plain] {
        vm.st.deck.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::paid().offering_pick(Pick::Labeled("take bad publicity")).once(),
                Reply::take("take bad publicity"),
            )
            .when(Match::reaction().offering("ad nihilum"), Reply::take("ad nihilum"))
            .when(Match::targets().once(), Reply::Targets(vec![wanted]))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.corp.bad_publicity, 1, "the Corp took bad publicity: {}", t.tail(40));
    let finds: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(finds.len(), 1, "one search find was put to the Corp: {}", t.tail(40));
    assert!(finds[0].candidates().contains(&wanted), "the gray ops card is a candidate: {}", t.tail(40));
    assert!(
        !finds[0].candidates().contains(&agenda),
        "'non-agenda' keeps the gray ops AGENDA out: {}",
        t.tail(40)
    );
    assert!(
        !finds[0].candidates().contains(&plain),
        "and the card with none of the three subtypes was never one: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&wanted].zone,
        Zone::Hand(Side::Corp),
        "the found card was revealed and added to HQ: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { obj, .. } if *obj == wanted)),
        "the reveal is its own instruction (9.11.4e): {}",
        t.tail(40)
    );
}

/// Weyland Consortium: Builder of Nations: "The first time each turn an
/// encounter with an advanced piece of ice ends, do 1 meat damage."
///
/// Two pieces of ice on the way in, both encountered. The outer one carries
/// no advancement counter and so meets nothing — which also leaves the
/// ordinal unspent — and the inner, advanced one does the damage. One card
/// leaves the grip, not two.
#[test]
fn builder_of_nations_damages_only_after_an_advanced_ice_encounter() {
    let mut vm = Vm::empty(6154);
    tk::install_identity(&mut vm, card("Weyland Consortium: Builder of Nations"), Side::Corp);
    // Innermost first: the advanced one, which the run reaches second.
    let advanced_ice = tk::install_ice(&mut vm, tk::vanilla_ice("Advanced Ice", 0, 1), ServerId::Archives, true);
    tk::place_counters(&mut vm, advanced_ice, CounterKind::Advancement, 1);
    tk::install_ice(&mut vm, tk::vanilla_ice("Plain Ice", 0, 1), ServerId::Archives, true);
    let grip = tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Archives)).stop_at_action(),
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        grip.len() - 1,
        "one meat damage, from the advanced ice's encounter and no other: {}",
        t.tail(50)
    );
    let meat = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::DamageSuffered { kind, .. } if *kind == jinteki_cr::effects::DamageKind::Meat))
        .count();
    assert_eq!(meat, 1, "exactly one — the plain ice's encounter met nothing: {}", t.tail(50));
}

/// The record's other direction: counters placed after an encounter ended do
/// not make that encounter retroactively "with an advanced piece of ice".
/// The first run's plain ice is advanced by a card ability the moment that
/// run ends; the second run's ice was advanced all along, so its encounter
/// end is still the first time each turn and the damage happens. Reading the
/// board instead of the record would count the first encounter, and the
/// printed damage would never come.
#[test]
fn builder_of_nations_counters_placed_later_do_not_rewrite_an_encounter() {
    let mut vm = Vm::empty(6193);
    tk::install_identity(&mut vm, card("Weyland Consortium: Builder of Nations"), Side::Corp);
    let plain =
        tk::install_ice(&mut vm, tk::vanilla_ice("Plain Ice", 0, 1), ServerId::Archives, true);
    let advanced_ice =
        tk::install_ice(&mut vm, tk::vanilla_ice("Advanced Ice", 0, 1), ServerId::Rnd, true);
    tk::place_counters(&mut vm, advanced_ice, CounterKind::Advancement, 1);
    tk::install_root(
        &mut vm,
        tk::run_ends_advancer("Groundskeeper", ServerId::Archives, plain),
        ServerId::Remote(1),
        true,
    );
    let grip = tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&plain].counter(CounterKind::Advancement),
        1,
        "the groundskeeper advanced the first run's ice when that run ended: {}",
        t.tail(50)
    );
    let meat = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::DamageSuffered { kind, .. } if *kind == jinteki_cr::effects::DamageKind::Meat))
        .count();
    assert_eq!(
        meat, 1,
        "the advanced encounter on R&D is still the first time — its damage happens: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        grip.len() - 1,
        "one card left the grip, no more: {}",
        t.tail(50)
    );
    // The change-log claim that pins WHICH encounter did it: the damage
    // follows the second run, not the plain encounter the counter rewrote.
    let runs: Vec<usize> = vm
        .changes
        .log
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, GameChange::RunBegan { .. }))
        .map(|(i, _)| i)
        .collect();
    let damage_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("the damage happened");
    assert!(
        damage_at > runs[1],
        "the damage follows the R&D run, not the Archives one: {}",
        t.tail(50)
    );
}

// ---------------------------------------------------------------------------
// The identity queue — the draft format's faction partition (CR 2.13)
// ---------------------------------------------------------------------------

/// A supporting board card of a named faction (2.13.3). The identity under
/// test always comes out of its deck module; what a faction partition is
/// drawn OVER is ordinary board furniture, so it is the testkit's — with the
/// one characteristic the sentence reads set explicitly, which is what lets
/// each case below say exactly which groups exist and how big they are.
fn of_faction(mut c: PrintedCard, faction: &'static str) -> PrintedCard {
    c.faction = Some(faction);
    c
}

/// Boris "Syfr" Kovac: "If you have more [criminal] cards installed than any
/// other faction, when your turn begins, remove 1 tag."
///
/// The requirement is the card, so both readings are asserted. A TIE is the
/// case worth having: two Criminal cards against two Anarch ones is not "more
/// than any other faction", and the tag stays.
#[test]
fn boris_syfr_kovac_removes_a_tag_only_while_criminal_is_strictly_ahead() {
    for leading in [false, true] {
        let mut vm = Vm::empty(6180);
        tk::install_identity(&mut vm, card("Boris \"Syfr\" Kovac: Crafty Veteran"), Side::Runner);
        tk::install_rig(
            &mut vm,
            of_faction(tk::vanilla_runner_card("Crim A", CardType::Program), "Criminal"),
        );
        tk::install_rig(
            &mut vm,
            of_faction(tk::vanilla_runner_card("Anarch A", CardType::Program), "Anarch"),
        );
        if leading {
            tk::install_rig(
                &mut vm,
                of_faction(tk::vanilla_runner_card("Crim B", CardType::Hardware), "Criminal"),
            );
        }
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.tags = 2;
        vm.start_turn(Side::Runner);

        let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
        assert_eq!(
            vm.st.runner.tags,
            if leading { 1 } else { 2 },
            "the tag comes off only while the Criminal group is STRICTLY the largest \
             (leading={leading}): {}",
            t.tail(24)
        );
    }
}

/// Jamie "Bzzz" Micken: "If you have more [shaper] cards installed than any
/// other faction, when you install a card the first time each turn, draw 1
/// card."
///
/// Both of 9.6.5c's stipulations at once. The Runner installs twice in a turn:
/// while Shaper leads, the FIRST install draws and the second does not; while
/// it is tied, neither does — and the ordinal is not banked for later, because
/// the condition was never met at all.
#[test]
fn jamie_bzzz_micken_draws_on_the_first_install_only_while_shaper_leads() {
    for leading in [false, true] {
        let mut vm = Vm::empty(6181);
        tk::install_identity(&mut vm, card("Jamie \"Bzzz\" Micken: Techno Savant"), Side::Runner);
        tk::install_rig(
            &mut vm,
            of_faction(tk::vanilla_runner_card("Shaper A", CardType::Program), "Shaper"),
        );
        if !leading {
            tk::install_rig(
                &mut vm,
                of_faction(tk::vanilla_runner_card("Crim A", CardType::Program), "Criminal"),
            );
        }
        // Hardware: installable with the basic action (5.2.7d) and costing no
        // [mu], so the two installs are the only thing the case turns on.
        let first = vm
            .new_object(tk::vanilla_runner_card("First", CardType::Hardware), Zone::Hand(Side::Runner));
        let second = vm
            .new_object(tk::vanilla_runner_card("Second", CardType::Hardware), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().extend([first, second]);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::Take(Pick::InstallCard(first)))
                .when(Match::action().once(), Reply::Take(Pick::InstallCard(second)))
                .stop_at_action(),
        );
        let drawn = vm.st.hand[&Side::Runner].len();
        assert_eq!(
            drawn,
            usize::from(leading),
            "two installs draw exactly once, and only while Shaper is strictly ahead \
             (leading={leading}): {}",
            t.tail(30)
        );
    }
}

/// Strategic Innovations: "If you have more [haas-bioroid] cards rezzed than
/// any other faction, when the Runner's turn ends, shuffle 1 card in Archives
/// into R&D."
///
/// The partition is drawn over the REZZED cards, so an unrezzed Haas-Bioroid
/// card is in no group at all: the two cases differ only in whether the second
/// Haas-Bioroid card is faceup, which is what decides a tie against NBN.
#[test]
fn strategic_innovations_recycles_archives_only_while_hb_leads_the_rezzed_cards() {
    for leading in [false, true] {
        let mut vm = Vm::empty(6182);
        tk::install_identity(
            &mut vm,
            card("Strategic Innovations: Future Forward"),
            Side::Corp,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("HB One", 0, 2), "Haas-Bioroid"),
            ServerId::Remote(1),
            true,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("NBN One", 0, 2), "NBN"),
            ServerId::Remote(2),
            true,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("HB Two", 0, 2), "Haas-Bioroid"),
            ServerId::Remote(3),
            leading,
        );
        let buried = vm.new_object(tk::corp_filler("Buried"), Zone::Discard(Side::Corp));
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        let deck_before = vm.st.deck[&Side::Corp].len();
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::targets().once(), Reply::Targets(vec![buried]))
                .when(Match::action(), Reply::Halt),
            Plan::runner().otherwise_click_credit(),
        );
        assert_eq!(vm.st.turn_side, Side::Corp, "the Runner's turn ended: {}", t.tail(30));
        assert_eq!(
            vm.st.discard[&Side::Corp].len(),
            usize::from(!leading),
            "Archives emptied exactly when Haas-Bioroid was strictly ahead among the REZZED \
             cards (leading={leading}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.deck[&Side::Corp].len(),
            // 5.5.2: the Corp's turn opens with the mandatory draw, which the
            // plan runs through on its way to the Corp's action window.
            deck_before - 1 + usize::from(leading),
            "and the card went into R&D (leading={leading}): {}",
            t.tail(30)
        );
    }
}

/// Fringe Applications: "If you have more [weyland-consortium] cards rezzed
/// than any other faction, when the Runner's turn begins, place an
/// advancement token on a piece of ice."
///
/// The Corp's identity, read at the start of the RUNNER's turn — 9.1.7 keeps
/// it active across the turn boundary — and the counter is placed by the Corp,
/// who announces which piece of ice.
#[test]
fn fringe_applications_advances_an_ice_as_the_runners_turn_opens() {
    for leading in [false, true] {
        let mut vm = Vm::empty(6183);
        tk::install_identity(&mut vm, card("Fringe Applications: Tomorrow, Today"), Side::Corp);
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("NBN One", 0, 2), "NBN"),
            ServerId::Remote(1),
            true,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("Weyland One", 0, 2), "Weyland Consortium"),
            ServerId::Remote(2),
            true,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("Weyland Two", 0, 2), "Weyland Consortium"),
            ServerId::Remote(3),
            leading,
        );
        let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 1), ServerId::Hq, false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::targets().once(), Reply::Targets(vec![ice])),
            Plan::runner().stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&ice].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
            u32::from(leading),
            "the token is placed only while Weyland Consortium is strictly ahead \
             (leading={leading}): {}",
            t.tail(24)
        );
    }
}

/// Information Dynamics: "If you have more [nbn] cards rezzed than any other
/// faction, whenever an agenda is scored or stolen, give the runner 1 tag."
///
/// One sentence, two occurrences: 1.17.3a's score and 1.17.3b's steal are two
/// conditional abilities, and each asks the faction question for itself. Both
/// halves are driven, and the not-leading case shuts both off.
#[test]
fn information_dynamics_tags_on_a_score_and_on_a_steal_while_nbn_leads() {
    for stolen in [false, true] {
        for leading in [false, true] {
            let mut vm = Vm::empty(6184);
            tk::install_identity(
                &mut vm,
                card("Information Dynamics: All You Need To Know"),
                Side::Corp,
            );
            tk::install_root(
                &mut vm,
                of_faction(tk::vanilla_asset("NBN One", 0, 2), "NBN"),
                ServerId::Remote(2),
                true,
            );
            tk::install_root(
                &mut vm,
                of_faction(tk::vanilla_asset("Jinteki One", 0, 2), "Jinteki"),
                ServerId::Remote(3),
                true,
            );
            tk::install_root(
                &mut vm,
                of_faction(tk::vanilla_asset("NBN Two", 0, 2), "NBN"),
                ServerId::Remote(4),
                leading,
            );
            let agenda = tk::install_root(
                &mut vm,
                tk::vanilla_agenda("Some Agenda", 3, 2),
                ServerId::Remote(1),
                false,
            );
            vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
            tk::fill_deck(&mut vm, Side::Corp, 5);
            tk::fill_deck(&mut vm, Side::Runner, 5);

            let t = if stolen {
                vm.start_turn(Side::Runner);
                plan::play(
                    &mut vm,
                    Plan::corp(),
                    Plan::runner()
                        .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                        .stop_at_action(),
                )
            } else {
                vm.start_turn(Side::Corp);
                plan::play(
                    &mut vm,
                    Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
                    Plan::runner(),
                )
            };
            assert_eq!(
                vm.st.runner.tags,
                u32::from(leading),
                "the tag lands exactly when NBN is strictly ahead among the rezzed cards \
                 (stolen={stolen}, leading={leading}): {}",
                t.tail(30)
            );
        }
    }
}

/// Synthetic Systems: "If you have more [jinteki] cards rezzed than any other
/// faction, when your turn begins, you may swap 2 pieces of installed ice."
///
/// The declinable half is the Corp's, and 8.8.2 is what keeps the second
/// announcement off the card the first one took.
#[test]
fn synthetic_systems_swaps_two_ice_while_jinteki_leads_the_rezzed_cards() {
    for leading in [false, true] {
        let mut vm = Vm::empty(6185);
        tk::install_identity(
            &mut vm,
            card("Synthetic Systems: The World Re-imagined"),
            Side::Corp,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("Jinteki One", 0, 2), "Jinteki"),
            ServerId::Remote(1),
            true,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("Weyland One", 0, 2), "Weyland Consortium"),
            ServerId::Remote(2),
            true,
        );
        tk::install_root(
            &mut vm,
            of_faction(tk::vanilla_asset("Jinteki Two", 0, 2), "Jinteki"),
            ServerId::Remote(3),
            leading,
        );
        let a = tk::install_ice(&mut vm, tk::vanilla_ice("Ice A", 0, 1), ServerId::Hq, false);
        let b = tk::install_ice(&mut vm, tk::vanilla_ice("Ice B", 0, 1), ServerId::Rnd, false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(
                    Match::reaction().offering("the world re-imagined"),
                    Reply::take("the world re-imagined"),
                )
                .when(Match::targets().once(), Reply::Targets(vec![a]))
                .when(Match::targets().once(), Reply::Targets(vec![b]))
                .stop_at_action(),
            Plan::runner(),
        );
        let (want_a, want_b) = if leading {
            (Zone::Ice(ServerId::Rnd), Zone::Ice(ServerId::Hq))
        } else {
            (Zone::Ice(ServerId::Hq), Zone::Ice(ServerId::Rnd))
        };
        assert_eq!(
            vm.st.objects[&a].zone, want_a,
            "the swap happens only while Jinteki is strictly ahead (leading={leading}): {}",
            t.tail(30)
        );
        assert_eq!(vm.st.objects[&b].zone, want_b, "and the other half of it: {}", t.tail(30));
    }
}

// ---------------------------------------------------------------------------
// The identity queue — Jinteki and NBN, the rest of the batch
// ---------------------------------------------------------------------------

/// PT Untaian: "When your discard phase ends, if there are 3 or fewer cards in
/// HQ, you may pay 1[credit] to place 1 advancement counter on an unrezzed
/// card you can advance."
///
/// The requirement is asked AFTER the discard, which is the whole point of a
/// discard-phase-end condition: a Corp holding six cards discards to five and
/// is still over the line, and one holding three is under it. The description
/// is the other half — the facedown asset beside the agenda is unrezzed but
/// cannot be advanced, so only the agenda is ever a candidate.
#[test]
fn pt_untaian_pays_a_credit_for_a_counter_only_with_a_small_hq() {
    for hand in [2usize, 5usize] {
        let mut vm = Vm::empty(6186);
        tk::install_identity(&mut vm, card("PT Untaian: Life's Building Blocks"), Side::Corp);
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Some Agenda", 5, 3),
            ServerId::Remote(1),
            false,
        );
        let asset =
            tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(2), false);
        tk::fill_hand(&mut vm, Side::Corp, hand);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::nested_cost(), Reply::PayCost(true))
                .when(Match::targets().once(), Reply::Targets(vec![agenda]))
                .when(Match::action(), Reply::credit()),
            Plan::runner().when(Match::action(), Reply::Halt),
        );
        let small = hand == 2;
        assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn finished: {}", t.tail(40));
        assert_eq!(
            vm.st.objects[&agenda].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
            u32::from(small),
            "the counter is placed only with 3 or fewer cards left in HQ (hand={hand}): {}",
            t.tail(40)
        );
        assert_eq!(
            // 3 clicks on 5.2.6b's basic credit action, less the 1 this pays.
            vm.st.corp.credits,
            3 - u32::from(small),
            "and the credit is spent exactly when the counter is placed (hand={hand}): {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&asset].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
            0,
            "the unrezzed ASSET can never be advanced, so it is never a candidate: {}",
            t.tail(40)
        );
    }
}

/// New Angeles Sol: "Whenever an agenda is scored or stolen, you may play 1
/// current from HQ or Archives (paying its play cost)."
///
/// The "or" between the two zones is one description, so the same ability
/// reaches a current in either — both are driven. Targeted Marketing is the
/// current: it stays in the play area once played (8.6.6c), which is what
/// makes the play observable at all.
#[test]
fn new_angeles_sol_plays_a_current_out_of_hq_or_out_of_archives() {
    for from_archives in [false, true] {
        let mut vm = Vm::empty(6187);
        tk::install_identity(&mut vm, card("New Angeles Sol: Your News"), Side::Corp);
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Some Agenda", 3, 2),
            ServerId::Remote(1),
            false,
        );
        vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
        let current = if from_archives {
            let c = vm.new_object(card("Targeted Marketing"), Zone::Discard(Side::Corp));
            vm.st.discard.get_mut(&Side::Corp).unwrap().push(c);
            c
        } else {
            let c = vm.new_object(card("Targeted Marketing"), Zone::Hand(Side::Corp));
            vm.st.hand.get_mut(&Side::Corp).unwrap().push(c);
            c
        };
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::paid(), Reply::score(agenda))
                .when(
                    Match::reaction().offering("an agenda was scored"),
                    Reply::take("an agenda was scored"),
                )
                .when(Match::targets().once(), Reply::Targets(vec![current]))
                .when(Match::name_value().once(), Reply::Name("Sure Gamble"))
                .stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(
            vm.st.objects[&current].zone,
            Zone::PlayArea(Side::Corp),
            "the current was played and 8.6.6c kept it there (from_archives={from_archives}): {}",
            t.tail(40)
        );
    }
}

// ---------------------------------------------------------------------------
// The identity queue — Weyland, Jinteki and NBN, the next batch
// ---------------------------------------------------------------------------

/// Jemison Astronautics: "Whenever you forfeit an agenda, place X advancement
/// counters on 1 installed card. X is equal to the agenda point value of the
/// forfeited agenda plus 1."
///
/// 24/7 News Cycle pays the forfeit as an additional play cost, so the agenda
/// is in the removed-from-game zone before this ability ever resolves — and X
/// is still its printed value, which is the whole reason the quantity reads
/// the card and not the score area. Both a 2-point agenda (3 counters) and a
/// 0-point one (1 counter) are driven, because "plus 1" is part of the
/// definition and not a floor.
#[test]
fn jemison_places_the_forfeited_agendas_points_plus_one() {
    for points in [0i32, 2i32] {
        let mut vm = Vm::empty(6188);
        tk::install_identity(
            &mut vm,
            card("Jemison Astronautics: Sacrifice. Audacity. Success."),
            Side::Corp,
        );
        let spare = vm.new_object(
            tk::vanilla_agenda("Spare Initiative", 3, points),
            Zone::ScoreArea(Side::Corp),
        );
        // A second agenda so the operation has a "when scored" ability to
        // resolve and the forfeit is a real choice between two cards.
        let headline = vm.new_object(card("Tomorrow's Headline"), Zone::ScoreArea(Side::Corp));
        vm.st.score_area.get_mut(&Side::Corp).unwrap().extend([spare, headline]);
        let cycle = vm.new_object(card("24/7 News Cycle"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(cycle);
        let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 1), ServerId::Hq, false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::play_card(cycle))
                .when(Match::payment_cards(), Reply::Targets(vec![spare]))
                .when(
                    Match::reaction().offering("sacrifice, audacity, success"),
                    Reply::take("sacrifice, audacity, success"),
                )
                .when(Match::targets(), Reply::target(ice))
                .stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(
            vm.st.objects[&spare].zone,
            Zone::RemovedFromGame,
            "8.2.5: the forfeit happened (points={points}): {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&ice].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
            (points + 1) as u32,
            "X is the forfeited agenda's printed points plus 1 (points={points}): {}",
            t.tail(40)
        );
    }
}

/// Near-Earth Hub: "The first time each turn you create a remote server, draw
/// 1 card."
///
/// The condition is 4.6.8d's server coming into existence, not the install.
/// Three installs are driven: one into a remote that ALREADY exists, which
/// creates nothing and draws nothing; one that makes a new remote and draws;
/// and a third that makes another new remote, by which time the printed
/// ordinal is spent.
#[test]
fn near_earth_hub_draws_only_when_a_remote_is_actually_created() {
    let mut vm = Vm::empty(6189);
    tk::install_identity(&mut vm, card("Near-Earth Hub: Broadcast Center"), Side::Corp);
    // The remote that already exists, so the first install creates nothing.
    tk::install_root(&mut vm, tk::vanilla_asset("Sitting Asset", 0, 2), ServerId::Remote(1), false);
    let into_existing = vm.new_object(tk::vanilla_ice("Some Ice", 0, 1), Zone::Hand(Side::Corp));
    let makes_one = vm.new_object(tk::vanilla_asset("Second Asset", 0, 2), Zone::Hand(Side::Corp));
    let makes_another = vm.new_object(tk::vanilla_asset("Third Asset", 0, 2), Zone::Hand(Side::Corp));
    for id in [into_existing, makes_one, makes_another] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(into_existing)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1))),
            )
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(makes_one)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(makes_another)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&into_existing].zone,
        Zone::Ice(ServerId::Remote(1)),
        "the first install went into the remote that already existed: {}",
        t.tail(50)
    );
    assert_ne!(
        vm.st.objects[&makes_one].zone,
        vm.st.objects[&makes_another].zone,
        "and the other two each made a remote of their own: {}",
        t.tail(50)
    );
    // The Corp's FIRST turn only: the play runs on into the next one, and
    // "each turn" is what the ordinal is counted over.
    let this_turn = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TurnEnded { side: Side::Corp }))
        .unwrap_or(vm.changes.log.len());
    let draws = vm.changes.log[..this_turn]
        .iter()
        .filter(|c| matches!(c, GameChange::CardDrawn { side: Side::Corp, .. }))
        .count();
    assert_eq!(
        draws,
        // 5.6.1's mandatory draw, plus this identity's one and only one.
        2,
        "nothing for the install into an existing remote, one for the remote \
         that was created, and the ordinal is spent before the second: {}",
        t.tail(50)
    );
}

/// Haarpsichord Studios: "The Runner cannot steal more than one agenda each
/// turn."
///
/// Two agendas sit in two remotes. The Runner runs both, and 1.2.2's absolute
/// "cannot" means the second one is accessed and simply not stolen — it stays
/// where it is, and the access carries on. The control is the same board
/// without the identity, where both are stolen.
#[test]
fn haarpsichord_lets_the_runner_steal_only_the_first_agenda_of_the_turn() {
    for with_identity in [false, true] {
        let mut vm = Vm::empty(6190);
        if with_identity {
            tk::install_identity(
                &mut vm,
                card("Haarpsichord Studios: Entertainment Unleashed"),
                Side::Corp,
            );
        }
        let one =
            tk::install_root(&mut vm, tk::vanilla_agenda("First", 3, 2), ServerId::Remote(1), false);
        let two = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Second", 3, 2),
            ServerId::Remote(2),
            false,
        );
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&one].zone,
            Zone::ScoreArea(Side::Runner),
            "the first agenda is always stolen (with_identity={with_identity}): {}",
            t.tail(50)
        );
        let want =
            if with_identity { Zone::Root(ServerId::Remote(2)) } else { Zone::ScoreArea(Side::Runner) };
        assert_eq!(
            vm.st.objects[&two].zone, want,
            "and the second only without the identity (with_identity={with_identity}): {}",
            t.tail(50)
        );
    }
}

/// Harishchandra Ent.: "While the Runner is tagged, they play with the grip
/// revealed."
///
/// 4.3.2 is the only reason a grip is hidden, so lifting it for that hand is
/// the whole effect — and 9.3.7a's "while" means it comes back the moment the
/// tag does. Asserted through the Corp's VIEW, which is where information the
/// Corp is entitled to lives, and the Runner's view of HQ is checked too: the
/// declaration names one hand and reaches no other.
#[test]
fn harishchandra_opens_the_grip_exactly_while_the_runner_is_tagged() {
    let mut vm = Vm::empty(6191);
    tk::install_identity(&mut vm, card("Harishchandra Ent.: Where You're the Star"), Side::Corp);
    let grip = tk::fill_hand(&mut vm, Side::Runner, 2);
    let hq = tk::fill_hand(&mut vm, Side::Corp, 2);

    assert!(
        !vm.view_of(Side::Corp).sees(grip[0]),
        "4.3.2: with no tag the grip is hidden from the Corp"
    );
    vm.st.runner.tags = 1;
    assert!(
        vm.view_of(Side::Corp).sees(grip[0]) && vm.view_of(Side::Corp).sees(grip[1]),
        "while the Runner is tagged the whole grip is open to the Corp"
    );
    assert!(
        !vm.view_of(Side::Runner).sees(hq[0]),
        "and HQ is untouched — the declaration names one hand"
    );
    vm.st.runner.tags = 0;
    assert!(
        !vm.view_of(Side::Corp).sees(grip[0]),
        "9.3.7a: the declaration stops applying the moment the last tag goes"
    );
}

/// Industrial Genomics: "The trash cost of each card is increased by 1 for
/// each facedown card in Archives."
///
/// The quantity is calculated (9.12.2), so the same asset costs 2, 3 or 4 to
/// trash depending on what is in Archives at the moment the cost is read —
/// and only FACEDOWN cards count, which is 10.3.1a's distinction between a
/// card the Corp trashed and one the Runner did.
#[test]
fn industrial_genomics_raises_the_trash_cost_once_per_facedown_archives_card() {
    for facedown in [0usize, 2usize] {
        let mut vm = Vm::empty(6192);
        tk::install_identity(&mut vm, card("Industrial Genomics: Growing Solutions"), Side::Corp);
        let asset =
            tk::install_root(&mut vm, tk::vanilla_asset("Some Asset", 0, 2), ServerId::Remote(1), true);
        for i in 0..facedown {
            let c = vm.new_object(
                tk::vanilla_asset(if i == 0 { "Buried One" } else { "Buried Two" }, 0, 2),
                Zone::Discard(Side::Corp),
            );
            vm.st.objects.get_mut(&c).unwrap().faceup = false;
            vm.st.discard.get_mut(&Side::Corp).unwrap().push(c);
        }
        // A FACEUP card in Archives is not what the sentence describes.
        let seen = vm.new_object(tk::vanilla_asset("Seen One", 0, 2), Zone::Discard(Side::Corp));
        vm.st.objects.get_mut(&seen).unwrap().faceup = true;
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(seen);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        // Exactly the printed cost plus the facedown cards: enough to trash
        // with the identity, and nothing spare.
        vm.st.runner.credits = 2 + facedown as u32;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                .when(Match::mid_access(), Reply::Take(Pick::BasicTrash))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&asset].zone,
            Zone::Discard(Side::Corp),
            "the Runner could still afford the raised cost (facedown={facedown}): {}",
            t.tail(50)
        );
        assert_eq!(
            vm.st.runner.credits, 0,
            "and paid the printed 2 plus 1 for each facedown card in Archives \
             (facedown={facedown}): {}",
            t.tail(50)
        );
    }
}

/// Jinteki: Replicating Perfection: "The Runner cannot run on remote servers.
/// Ignore this ability until the end of the turn whenever the Runner runs on
/// a central server."
///
/// 6.3.2a puts the prohibition on ANNOUNCING the server, so what changes is
/// which run actions are OFFERED — which the plan reads directly: "run the
/// remote" applies exactly where that run is on offer, and falls through to
/// "run Archives" where it is not. So the order of the two runs is the whole
/// assertion. Without the identity the Runner takes the remote first; with
/// it, the remote is closed until the central run has lifted the ability, and
/// then open for the rest of the turn.
#[test]
fn replicating_perfection_closes_the_remotes_until_a_central_is_run() {
    for with_identity in [false, true] {
        let mut vm = Vm::empty(6193);
        if with_identity {
            tk::install_identity(&mut vm, card("Jinteki: Replicating Perfection"), Side::Corp);
        }
        tk::install_root(
            &mut vm,
            tk::vanilla_asset("Some Asset", 0, 2),
            ServerId::Remote(1),
            true,
        );
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                .when(Match::action().once(), Reply::run(ServerId::Archives))
                .stop_at_action(),
        );
        let order: Vec<ServerId> = vm
            .changes
            .log
            .iter()
            .filter_map(|c| match c {
                GameChange::RunBegan { server } => Some(*server),
                _ => None,
            })
            .collect();
        let want = if with_identity {
            vec![ServerId::Archives, ServerId::Remote(1)]
        } else {
            vec![ServerId::Remote(1), ServerId::Archives]
        };
        assert_eq!(
            order, want,
            "the remote is closed until a central has been run \
             (with_identity={with_identity}): {}",
            t.tail(50)
        );
    }
}

/// The Zwicky Group: "The first time each turn you gain credits through an
/// ability on an agenda or operation, you may draw 1 card."
///
/// 9.1.4 makes the description a question about the ability's SOURCE. Hedge
/// Fund is an operation and pays; the Corp's own basic credit action came
/// through no card at all and does not — which is the whole distinction the
/// words "through an ability on" draw. The second Hedge Fund of the turn is
/// the ordinal's other half.
#[test]
fn the_zwicky_group_draws_only_for_the_first_gain_off_a_card() {
    let mut vm = Vm::empty(6194);
    tk::install_identity(&mut vm, card("The Zwicky Group: Invisible Hands"), Side::Corp);
    let first = vm.new_object(card("Hedge Fund"), Zone::Hand(Side::Corp));
    let second = vm.new_object(card("Hedge Fund"), Zone::Hand(Side::Corp));
    for id in [first, second] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 10;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // The basic credit action first: it gains a credit and must not
            // spend the ordinal.
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::play_card(first))
            .when(Match::action().once(), Reply::play_card(second))
            .when(
                Match::reaction().offering("invisible hands"),
                Reply::take("invisible hands"),
            )
            .stop_at_action(),
        Plan::runner(),
    );
    let this_turn = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TurnEnded { side: Side::Corp }))
        .unwrap_or(vm.changes.log.len());
    let draws = vm.changes.log[..this_turn]
        .iter()
        .filter(|c| matches!(c, GameChange::CardDrawn { side: Side::Corp, .. }))
        .count();
    assert_eq!(
        draws,
        // 5.6.1's mandatory draw, plus this identity's one and only one.
        2,
        "the basic credit action came through no card and the second operation \
         is past the ordinal: {}",
        t.tail(60)
    );
}

/// Hyoubu Institute: "The first time each turn you reveal a card, gain
/// 1[credit]. [click]: Reveal 1 card from the grip at random or the top card
/// of the stack."
///
/// Both halves of the option choice are driven, and the paid ability feeds
/// the conditional one — which is the card as printed. The reveal is the
/// CORP's even though the card shown is the Runner's, so the credit arrives;
/// and the second use of the same turn gets nothing, which is the ordinal.
/// 1.21.3 also means the card the Corp saw stays visible to both players.
#[test]
fn hyoubu_institute_pays_for_the_first_reveal_of_the_turn_only() {
    for from_stack in [false, true] {
        let mut vm = Vm::empty(6195);
        tk::install_identity(&mut vm, card("Hyoubu Institute: Absolute Clarity"), Side::Corp);
        tk::fill_hand(&mut vm, Side::Runner, 3);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let option = if from_stack { "top card of the stack" } else { "grip at random" };
        let t = plan::play(
            &mut vm,
            Plan::corp()
                // 5.2.4: a [click] ability is an ACTION, so it is offered in
                // the action window and not in a paid one.
                .when(Match::action().times(2), Reply::take("reveal a card"))
                .when(Match::options(), Reply::ChooseNamed(option))
                .when(
                    Match::reaction().offering("absolute clarity"),
                    Reply::take("absolute clarity"),
                )
                .stop_at_action(),
            Plan::runner(),
        );
        let this_turn = vm
            .changes
            .log
            .iter()
            .position(|c| matches!(c, GameChange::TurnEnded { side: Side::Corp }))
            .unwrap_or(vm.changes.log.len());
        let reveals: Vec<jinteki_cr::object::ObjectId> = vm.changes.log[..this_turn]
            .iter()
            .filter_map(|c| match c {
                GameChange::CardRevealed { obj, by: Side::Corp } => Some(*obj),
                _ => None,
            })
            .collect();
        assert_eq!(
            reveals.len(),
            2,
            "the ability was used twice and revealed a card each time \
             (from_stack={from_stack}): {}",
            t.tail(50)
        );
        let want_zone =
            if from_stack { Zone::Deck(Side::Runner) } else { Zone::Hand(Side::Runner) };
        assert_eq!(
            vm.st.objects[&reveals[0]].zone, want_zone,
            "and out of the half the Corp chose (from_stack={from_stack}): {}",
            t.tail(50)
        );
        assert!(
            vm.view_of(Side::Corp).sees(reveals[0]),
            "1.21.3: a revealed card is shown to all players (from_stack={from_stack}): {}",
            t.tail(50)
        );
        assert_eq!(
            vm.st.corp.credits, 1,
            "paid for the first reveal of the turn and no other \
             (from_stack={from_stack}): {}",
            t.tail(50)
        );
    }
}

// ---------------------------------------------------------------------------
// The identity queue — the tail of Weyland and NBN
// ---------------------------------------------------------------------------

/// SSO Industries: "When your turn ends, you may choose a piece of ice with no
/// advancement tokens on it. If you do, place 1 advancement token on that
/// piece of ice for each agenda point on all installed faceup agendas."
///
/// Three agendas sit on the board and only one of them is what the sentence
/// describes: a FACEUP INSTALLED one, worth 2. The facedown installed agenda
/// beside it and the faceup one in the score area are each excluded by one
/// half of the description, so the answer is 2 and not 5 or 8. The other
/// variant advances the only piece of ice first, which takes it out of the
/// candidates entirely — "with no advancement tokens on it" is a description,
/// so an already-advanced ice is not a legal target and nothing happens.
#[test]
fn sso_industries_advances_ice_once_per_faceup_installed_agenda_point() {
    for advanced in [false, true] {
        let mut vm = Vm::empty(6196);
        tk::install_identity(&mut vm, card("SSO Industries: Fueling Innovation"), Side::Corp);
        let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Some Ice", 0, 1), ServerId::Hq, false);
        if advanced {
            vm.st.objects.get_mut(&ice).unwrap().counters.insert(CounterKind::Advancement, 1);
        }
        tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Faceup Initiative", 3, 2),
            ServerId::Remote(1),
            true,
        );
        // 8.1.2's usual state for an installed Corp card — not described.
        tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Facedown Initiative", 3, 3),
            ServerId::Remote(2),
            false,
        );
        // Faceup, but in a score area rather than installed.
        let scored = vm.new_object(
            tk::vanilla_agenda("Scored Initiative", 3, 3),
            Zone::ScoreArea(Side::Corp),
        );
        vm.st.objects.get_mut(&scored).unwrap().faceup = true;
        vm.st.score_area.get_mut(&Side::Corp).unwrap().push(scored);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(
                    Match::reaction().offering("fueling innovation"),
                    Reply::take("fueling innovation"),
                )
                .when(Match::targets().once(), Reply::target(ice))
                .otherwise_click_credit(),
            Plan::runner().when(Match::action(), Reply::Halt),
        );
        assert_eq!(vm.st.turn_side, Side::Runner, "the Corp's turn ended: {}", t.tail(30));
        assert_eq!(
            vm.st.objects[&ice].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
            if advanced { 1 } else { 2 },
            "one token per agenda point on the faceup INSTALLED agenda, and only for an ice \
             with none on it already (advanced={advanced}): {}",
            t.tail(30)
        );
    }
}

/// NBN: Controlling the Message: "The first time the Runner trashes an
/// installed Corp card each turn, you may trace[4]. If successful, give the
/// Runner 1 tag (cannot be avoided)."
///
/// The Runner runs two remotes and trashes the asset in each. Only the first
/// trace happens — the second trash is past 9.6.5c's ordinal — and the tag it
/// gives cannot be taken away: a Decoy-class avoider sits in the rig with its
/// [trash] ability ready, and 9.9.3a never finds it relevant, because 9.4.5
/// leaves the restriction on the value. The Decoy is still there afterwards,
/// which is the observable half of "was never offered".
#[test]
fn controlling_the_message_traces_once_a_turn_for_a_tag_that_cannot_be_avoided() {
    let mut vm = Vm::empty(6197);
    tk::install_identity(&mut vm, card("NBN: Controlling the Message"), Side::Corp);
    let one =
        tk::install_root(&mut vm, tk::vanilla_asset("First Asset", 0, 1), ServerId::Remote(1), true);
    let two = tk::install_root(
        &mut vm,
        tk::vanilla_asset("Second Asset", 0, 1),
        ServerId::Remote(2),
        true,
    );
    let decoy = tk::install_rig(&mut vm, tk::decoy_like("Decoy"));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::reaction().offering("controlling the message"),
                Reply::take("controlling the message"),
            )
            .when(Match::trace_spend(), Reply::Spend(0))
            .stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .when(Match::mid_access(), Reply::Take(Pick::BasicTrash))
            .when(Match::trace_spend(), Reply::Spend(0))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&one].zone,
        Zone::Discard(Side::Corp),
        "both assets were trashed: {}",
        t.tail(60)
    );
    assert_eq!(vm.st.objects[&two].zone, Zone::Discard(Side::Corp), "{}", t.tail(60));
    let traces = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::TraceInitiated { .. }))
        .count();
    assert_eq!(traces, 1, "the ordinal is spent by the first trash of the turn: {}", t.tail(60));
    assert_eq!(vm.st.runner.tags, 1, "trace 4 beat 0 link: {}", t.tail(60));
    assert_eq!(
        vm.st.objects[&decoy].zone,
        Zone::Rig,
        "9.4.5: the restriction rode the value, so the avoider was never relevant: {}",
        t.tail(60)
    );
}

/// GameNET: "Whenever a Corp card ability causes the Runner to spend or lose
/// at least 1[credit] during a run, gain 1[credit]."
///
/// Both halves of "spend or lose" are driven against the same identity: Gold
/// Farmer's subroutines make the Runner PAY 3[credit] each to keep the run
/// alive, and a Whitespace-class subroutine makes them LOSE 3[credit]
/// outright. Each occurrence pays the Corp exactly once however many credits
/// moved — the sentence's "at least 1" is about the occurrence and not a
/// threshold — so Gold Farmer, which prints the same subroutine twice, pays
/// twice and the single loss pays once.
///
/// The control is in the same play: the Runner opens the turn by paying
/// 5[credit] to play Sure Gamble, which is a payment of their own outside any
/// run, and the identity stays quiet for it.
#[test]
fn gamenet_pays_once_for_each_corp_caused_spend_or_loss_during_a_run() {
    for lose in [false, true] {
        let mut vm = Vm::empty(6198);
        tk::install_identity(&mut vm, card("GameNET: Where Dreams are Real"), Side::Corp);
        let ice = if lose {
            tk::install_ice(&mut vm, tk::whitespace_like("Whitespace", 3), ServerId::Hq, true)
        } else {
            tk::install_ice(&mut vm, card("Gold Farmer"), ServerId::Hq, true)
        };
        let gamble = vm.new_object(card("Sure Gamble"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(gamble);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 5;
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(gamble))
                .when(Match::action().once(), Reply::run(ServerId::Hq))
                // Gold Farmer's nested cost: pay the 3 rather than let the
                // run end, which is the spending half of the sentence.
                .when(Match::of(Kind::NestedCost), Reply::PayCost(true))
                .stop_at_action(),
        );
        assert!(
            vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterBegan { ice: i, .. } if *i == ice)),
            "the run reached the ice (lose={lose}): {}",
            t.tail(60)
        );
        assert_eq!(
            vm.st.corp.credits,
            // One per occurrence: Gold Farmer prints its nested cost twice.
            if lose { 1 } else { 2 },
            "one credit for each occurrence the Corp's card caused, and nothing for the \
             Runner's own play cost outside the run (lose={lose}): {}",
            t.tail(60)
        );
    }
}

/// Synapse Global: "The first time each turn a tag is removed, you may reveal
/// and install 1 card from HQ, ignoring all costs. [click], remove 1 tag: Gain
/// 2[credit]."
///
/// The two printed lines are driven together, because the card is built that
/// way: the paid ability's own cost removes the tag that meets the conditional
/// ability's condition (1.16.10b records a payment where conditions can see
/// it). With no tag to remove the ability cannot be used at all (1.16.1b), so
/// the Corp's credits and HQ are both untouched — that is the other variant.
///
/// The install is "ignoring all costs" (1.16.5c), and the destination is a
/// server that already has a piece of ice — so 8.5.11a's 1[credit] per piece
/// of ice already protecting it would otherwise be paid. The Corp's credits
/// after the ability are exactly the 2 it gained, which is what says the cost
/// was ignored rather than merely affordable.
#[test]
fn synapse_global_turns_a_removed_tag_into_a_free_install() {
    for tagged in [false, true] {
        let mut vm = Vm::empty(6199);
        tk::install_identity(&mut vm, card("Synapse Global: Faster than Thought"), Side::Corp);
        tk::install_ice(&mut vm, tk::vanilla_ice("Sitting Ice", 0, 1), ServerId::Hq, false);
        let from_hq = vm.new_object(tk::vanilla_ice("New Ice", 0, 1), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(from_hq);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.st.runner.tags = u32::from(tagged);
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::take("remove a tag"))
                .when(
                    Match::reaction().offering("faster than thought"),
                    Reply::take("faster than thought"),
                )
                .when(Match::targets().once(), Reply::target(from_hq))
                .when(
                    Match::of(Kind::Destination).once(),
                    Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
                )
                .stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(
            vm.st.runner.tags,
            0,
            "the tag paid for the ability (tagged={tagged}): {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.corp.credits,
            if tagged { 2 } else { 0 },
            "2 gained, and 8.5.11a's install cost ignored (tagged={tagged}): {}",
            t.tail(40)
        );
        let want = if tagged { Zone::Ice(ServerId::Hq) } else { Zone::Hand(Side::Corp) };
        assert_eq!(
            vm.st.objects[&from_hq].zone, want,
            "the card from HQ was installed exactly when a tag came off (tagged={tagged}): {}",
            t.tail(40)
        );
        if tagged {
            assert!(
                vm.changes
                    .log
                    .iter()
                    .any(|c| matches!(c, GameChange::CardRevealed { obj, by: Side::Corp } if *obj == from_hq)),
                "9.11.4e: the card was revealed before it was installed: {}",
                t.tail(40)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The identity queue — CR 1.10.3c's restricted credits, and what a payment
// is FOR
// ---------------------------------------------------------------------------

/// Ele "Smoke" Scovak: Cynosure of the Net — "1[recurring-credit] / Use this
/// credit to pay for using icebreakers."
///
/// The Runner's pool is empty in both halves, so the recurring credit is the
/// only thing that could pay — and 9.1.6a is what tells the two apart: paying
/// a trigger cost is USING the card the ability is on, so the icebreaker's
/// interface ability is payable and the identical ability on a program that
/// is not an icebreaker is not offered at all.
#[test]
fn smoke_pays_for_using_icebreakers_and_nothing_else() {
    for icebreaker in [true, false] {
        let mut vm = Vm::empty(6180);
        let smoke =
            tk::install_identity(&mut vm, card("Ele \"Smoke\" Scovak: Cynosure of the Net"), Side::Runner);
        // 3.9.5g/h: the breaker has to be big enough and of the right kind
        // for the interface ability to be offered, so the ice is strength 0.
        let ice = tk::install_ice(
            &mut vm,
            tk::subtyped_etr_ice("Some Sentry", "Sentry", 0, 0),
            ServerId::Archives,
            true,
        );
        let mut breaker = tk::vanilla_runner_card("Some Breaker", CardType::Program);
        breaker.subtypes = if icebreaker { vec!["Icebreaker", "Killer"] } else { vec!["Killer"] };
        breaker.strength = Some(1);
        breaker.abilities = vec![jinteki_cr::ability::AbilityDef::paid(
            jinteki_cr::ability::Cost::credits(1),
            vec![Instruction::BreakSubroutines {
                subs: jinteki_cr::instr::SubroutineSpec::Chosen {
                    count: jinteki_cr::instr::Quantity::c(1),
                    up_to: false,
                },
            }],
        )
        .with_flag(jinteki_cr::ability::AbilityFlag::Interface)
        .with_timing(jinteki_cr::ability::TimingRestriction::EncounterOnly {
            required_subtype: Some("Sentry"),
            required_choice: None, required_self: false,
        })
        .labeled("interface: break 1 sentry subroutine")];
        tk::install_rig(&mut vm, breaker);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Archives))
                .when(Match::paid(), Reply::take("interface: break 1 sentry subroutine"))
                .stop_at_action(),
        );
        let broke = vm
            .changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::SubroutineBroken { ice: i, .. } if *i == ice));
        assert_eq!(
            broke, icebreaker,
            "icebreaker={icebreaker}: 1.10.3c allows the credit only for using an \
             icebreaker: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.objects[&smoke].counter(CounterKind::Credit),
            if icebreaker { 0 } else { 1 },
            "icebreaker={icebreaker}: 1.10.3a — the credit left the card only for the \
             payment it was allowed to make: {}",
            t.tail(30)
        );
        assert_eq!(vm.st.runner.credits, 0, "nothing came out of the pool");
    }
}

/// Whizzard: Master Gamer — "3[recurring-credit] / Use these credits to trash
/// cards."
///
/// The same shape as Miss Bones with the other reading of the description.
/// Miss Bones prints "installed cards" and 1.15.2c gives her that for free;
/// Whizzard prints "cards", so the card in HQ being accessed — which is not
/// installed — is trashable too. The pool is empty, so the recurring credits
/// are the only thing that could pay either way.
#[test]
fn whizzard_pays_to_trash_a_card_anywhere_not_only_an_installed_one() {
    for installed in [true, false] {
        let mut vm = Vm::empty(6181);
        let wz = tk::install_identity(&mut vm, card("Whizzard: Master Gamer"), Side::Runner);
        let mut loot = PrintedCard::vanilla("Loot", Side::Corp, CardType::Asset);
        loot.trash_cost = Some(3);
        let (server, target) = if installed {
            (ServerId::Remote(1), tk::install_root(&mut vm, loot, ServerId::Remote(1), true))
        } else {
            let id = vm.new_object(loot, Zone::Hand(Side::Corp));
            vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
            (ServerId::Hq, id)
        };
        tk::fill_deck(&mut vm, Side::Corp, 4);
        tk::fill_deck(&mut vm, Side::Runner, 4);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(server))
                .when(Match::of(Kind::MidAccess).once(), Reply::trash_accessed())
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&target].zone,
            Zone::Discard(Side::Corp),
            "installed={installed}: \"cards\" reaches the card wherever it is: {}",
            t.tail(16)
        );
        assert_eq!(
            vm.st.objects[&wz].counter(CounterKind::Credit),
            0,
            "installed={installed}: all three credits paid the 3[credit] trash cost: {}",
            t.tail(16)
        );
        assert_eq!(vm.st.runner.credits, 0, "nothing came out of the pool");
    }
}

/// NBN: Making News — "2[recurring-credit] / Use these credits during trace
/// attempts."
///
/// The Corp's pool is empty, so the two hosted credits are the only thing
/// that could raise the trace strength — and 10.8.6c is where the card allows
/// them, so they do.
#[test]
fn making_news_pays_at_a_traces_spend_step() {
    let mut vm = Vm::empty(6182);
    let mn = tk::install_identity(&mut vm, card("NBN: Making News"), Side::Corp);
    let ash = tk::install_root(&mut vm, tk::ash_like("Some Ash"), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::trace_spend(), Reply::Spend(2)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::trace_spend(), Reply::Spend(0))
            .stop_at_action(),
    );
    let _ = ash;
    assert_eq!(
        vm.st.objects[&mn].counter(CounterKind::Credit),
        0,
        "10.8.6c: both credits were spendable at the trace's spend step: {}",
        t.tail(30)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CostPaid { side: Side::Corp, credits: 2, .. })),
        "and the trace strength rose by exactly what was spent: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.corp.credits, 0, "nothing came out of the pool");
}

/// The other half of the same sentence: a rez is not a trace attempt, so the
/// same two credits cannot pay for one. The control is the credit pool — two
/// credits in the POOL rez the ice, two on the identity do not, with nothing
/// else changed.
#[test]
fn making_news_credits_cannot_rez_a_piece_of_ice() {
    for in_pool in [true, false] {
        let mut vm = Vm::empty(6183);
        let mn = tk::install_identity(&mut vm, card("NBN: Making News"), Side::Corp);
        let wall = tk::install_ice(&mut vm, tk::etr_ice("Some Wall", 2, 1), ServerId::Rnd, false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = if in_pool { 2 } else { 0 };
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::paid(), Reply::Take(Pick::RezApproachedIce)),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Rnd))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&wall].faceup, in_pool,
            "in_pool={in_pool}: 1.10.3c — the hosted credits are for trace attempts \
             and a rez is not one: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.objects[&mn].counter(CounterKind::Credit),
            2,
            "in_pool={in_pool}: the identity's credits never moved: {}",
            t.tail(30)
        );
    }
}

/// Lat: Ethical Freelancer — "When your discard phase ends, if you have the
/// same number of cards in your grip as the Corp has in HQ, you may draw 1
/// card."
///
/// Two calculated quantities compared against each other: one game where the
/// grip and HQ match at the end of the Runner's discard phase and one where
/// they do not, with nothing else changed.
#[test]
fn lat_draws_only_when_the_grip_and_hq_are_the_same_size() {
    for matching in [true, false] {
        let mut vm = Vm::empty(6183);
        tk::install_identity(&mut vm, card("Lat: Ethical Freelancer"), Side::Runner);
        tk::fill_hand(&mut vm, Side::Runner, 3);
        tk::fill_hand(&mut vm, Side::Corp, if matching { 3 } else { 4 });
        tk::fill_deck(&mut vm, Side::Runner, 5);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action(), Reply::Halt),
            Plan::runner()
                .when(Match::action(), Reply::credit())
                .when(Match::reaction(), Reply::take("ethical freelancer")),
        );
        assert_eq!(vm.st.turn_side, Side::Corp, "the Runner's turn finished: {}", t.tail(24));
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            3 + usize::from(matching),
            "matching={matching}: 9.6.5c's requirement is the whole of it: {}",
            t.tail(24)
        );
    }
}

/// A piece of ice whose only subroutine gives the Runner 1 tag — a tag that
/// lands DURING a run, at a moment nothing else about the run depends on.
fn tag_sub_ice(name: &'static str) -> PrintedCard {
    let mut c = tk::vanilla_ice(name, 0, 0);
    c.abilities = vec![jinteki_cr::ability::AbilityDef::subroutine(vec![
        Instruction::GainTags { amount: 1, avoidable: true },
    ])
    .labeled("[sub] give the Runner 1 tag")];
    c
}

/// Jesminder Sareen: Girl Behind the Curtain — "[interrupt] → The first time
/// each run you would take 1 or more tags, prevent 1 tag."
///
/// The span is the whole of what is under test, so the two halves differ in
/// nothing but how the two tags are spread over runs. Two tags in ONE run
/// leaves one: the second is not the first time. One tag in each of TWO runs
/// leaves none — which is exactly what a turn-scoped ordinal would get wrong.
#[test]
fn jesminder_prevents_the_first_tag_of_every_run() {
    for one_run in [true, false] {
        let mut vm = Vm::empty(6184);
        tk::install_identity(
            &mut vm,
            card("Jesminder Sareen: Girl Behind the Curtain"),
            Side::Runner,
        );
        tk::install_ice(&mut vm, tag_sub_ice("Tagger A"), ServerId::Archives, true);
        if one_run {
            tk::install_ice(&mut vm, tag_sub_ice("Tagger B"), ServerId::Archives, true);
        }
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let runs = if one_run { 1 } else { 2 };
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().times(runs), Reply::run(ServerId::Archives))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.runner.tags,
            usize::from(one_run) as u32,
            "one_run={one_run}: 9.6.5c's ordinal is counted over the RUN: {}",
            t.tail(40)
        );
    }
}

/// Nero Severn: "Once per turn → When you encounter a sentry, you may jack
/// out."
///
/// Both halves of the subtype stipulation, one game each: the encounter with
/// a sentry offers the choice and the run ends before the breach, and the
/// encounter with a barrier offers nothing at all.
#[test]
fn nero_severn_jacks_out_of_an_encounter_with_a_sentry_and_not_with_a_barrier() {
    for sentry in [false, true] {
        let mut vm = Vm::empty(6165);
        tk::install_identity(&mut vm, card("Nero Severn: Information Broker"), Side::Runner);
        let subtype = if sentry { "Sentry" } else { "Barrier" };
        tk::install_ice(
            &mut vm,
            tk::subtyped_ice("Some Ice", vec![subtype], 0, 1),
            ServerId::Hq,
            true,
        );
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Hq))
                .when(Match::reaction().offering("information broker"), Reply::take("information broker"))
                .when(Match::of(Kind::Optional), Reply::Optional(true))
                .when(Match::of(Kind::JackOut), Reply::JackOut(false))
                .stop_at_action(),
        );
        assert_eq!(
            vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
            !sentry,
            "sentry={sentry}: jacking out ends the run before the breach, and \
             the encounter with a barrier never offers it: {}",
            t.tail(40)
        );
        assert!(
            vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
            "sentry={sentry}: either way the run is over by the end of the turn: {}",
            t.tail(40)
        );
    }
}

/// Wyvern: "You must maintain the order of your heap. / Whenever you trash a
/// Corp card, if you have more [anarch] cards installed than any other
/// faction, shuffle the top card of your heap into your stack."
///
/// Both of the last line's parts, one game each — and the middle line is what
/// the assertion about WHICH card moved is checking: the top of the heap is
/// the card most recently trashed, which is a question a pile CR 4.4.2 leaves
/// unordered could not answer.
#[test]
fn wyvern_shuffles_the_top_of_the_heap_back_only_while_anarch_leads() {
    for leading in [false, true] {
        let mut vm = Vm::empty(6166);
        tk::install_identity(&mut vm, card("Wyvern: Chemically Enhanced"), Side::Runner);
        tk::install_rig(
            &mut vm,
            of_faction(tk::vanilla_runner_card("Anarch A", CardType::Program), "Anarch"),
        );
        if !leading {
            tk::install_rig(
                &mut vm,
                of_faction(tk::vanilla_runner_card("Crim A", CardType::Program), "Criminal"),
            );
        }
        // The heap, in pile order: `deeper` went in first, so `top` is the top.
        let deeper = vm.new_object(tk::runner_filler("Deeper"), Zone::Discard(Side::Runner));
        vm.st.discard.get_mut(&Side::Runner).unwrap().push(deeper);
        let top = vm.new_object(tk::runner_filler("Top of Heap"), Zone::Discard(Side::Runner));
        vm.st.discard.get_mut(&Side::Runner).unwrap().push(top);
        // The Corp card the Runner trashes: 7.1.5's basic trash ability.
        let asset =
            tk::install_root(&mut vm, tk::vanilla_asset("Trashable", 0, 1), ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                .when(Match::of(Kind::MidAccess).once(), Reply::Take(Pick::BasicTrash))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&asset].zone,
            Zone::Discard(Side::Corp),
            "leading={leading}: the Runner trashed a Corp card: {}",
            t.tail(40)
        );
        // 1.12.3 makes a card entering the deck a NEW object, so the old one
        // is gone from the heap rather than sitting in the stack.
        assert_eq!(
            vm.st.discard[&Side::Runner].contains(&top),
            !leading,
            "leading={leading}: the TOP card of the heap left it, and only while \
             the Anarch group is strictly the largest: {}",
            t.tail(40)
        );
        assert!(
            vm.st.discard[&Side::Runner].contains(&deeper),
            "leading={leading}: the card under it never moves: {}",
            t.tail(40)
        );
    }
}

/// Harmony Medtech: "Each player needs 1 fewer agenda point to win the game."
///
/// The comparison 1.17.2 makes, and nothing else. The same six agenda points
/// on the same board win the game with this identity out and do not without
/// it — and the SCORE is six either way, which is the point of modifying the
/// requirement rather than the total: every ability that reads a score goes on
/// reading the real one.
#[test]
fn harmony_medtech_wins_the_game_at_six_agenda_points() {
    for with_identity in [false, true] {
        let mut vm = Vm::empty(6180);
        if with_identity {
            tk::install_identity(
                &mut vm,
                card("Harmony Medtech: Biomedical Pioneer"),
                Side::Corp,
            );
        }
        tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Banked A", 3, 2), Side::Corp);
        tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Banked B", 3, 2), Side::Corp);
        let third = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Third Agenda", 3, 2),
            ServerId::Remote(1),
            false,
        );
        vm.st.objects.get_mut(&third).unwrap().counters.insert(CounterKind::Advancement, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::paid(), Reply::score(third)).stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(
            vm.score(Side::Corp),
            6,
            "with_identity={with_identity}: the score itself is untouched: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.game_over.is_some(),
            with_identity,
            "with_identity={with_identity}: 10.3.1c compares against the number this \
             declaration moved: {}",
            t.tail(30)
        );
        // "Each player": the Corp's own card lowers the Runner's requirement too.
        assert_eq!(
            vm.agenda_points_to_win(Side::Runner),
            if with_identity { 6 } else { 7 },
            "with_identity={with_identity}: the sentence names both players"
        );
    }
}

/// Issuaq Adaptics: "Whenever you score an agenda that you did not install or
/// advance this turn, place 1 power counter on this identity. / For each
/// hosted power counter, you need 1 less agenda point to win the game."
///
/// The description on the condition is what the two games differ by: the same
/// 1/1 agenda scored out of the same remote, once with the counter already on
/// it from an earlier turn and once advanced with 5.2.6f's basic action this
/// turn. Only the first places a counter, and only the first lowers what the
/// Corp needs.
#[test]
fn issuaq_adaptics_counts_agendas_it_neither_installed_nor_advanced_this_turn() {
    for advanced_this_turn in [false, true] {
        let mut vm = Vm::empty(6181);
        let ident =
            tk::install_identity(&mut vm, card("Issuaq Adaptics: Sustaining Diversity"), Side::Corp);
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Quiet Agenda", 1, 1),
            ServerId::Remote(1),
            false,
        );
        if !advanced_this_turn {
            // Placed rather than advanced, and before the turn began — so the
            // history the condition reads holds neither an install nor an
            // advance of this card.
            vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 1);
        }
        vm.st.corp.credits = 5;
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let mut corp = Plan::corp();
        if advanced_this_turn {
            corp = corp.when(Match::action().once(), Reply::Take(Pick::Advance(agenda)));
        }
        let t = plan::play(
            &mut vm,
            corp.when(Match::paid(), Reply::score(agenda)).stop_at_action(),
            Plan::runner(),
        );
        assert_eq!(
            vm.st.objects[&agenda].zone,
            Zone::ScoreArea(Side::Corp),
            "advanced={advanced_this_turn}: the agenda was scored either way: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&ident].counters.get(&CounterKind::Power).copied().unwrap_or(0),
            u32::from(!advanced_this_turn),
            "advanced={advanced_this_turn}: only the agenda the Corp neither installed \
             nor advanced this turn places a counter: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.agenda_points_to_win(Side::Corp),
            if advanced_this_turn { 7 } else { 6 },
            "advanced={advanced_this_turn}: and the requirement follows the counters"
        );
    }
}

/// Nisei Division: "Whenever you and the Runner reveal secretly spent credits,
/// gain 1[credit]."
///
/// The psi game comes from the other side of the table — Akiko Nisei's breach
/// of R&D — which is what makes the assertion about the REVEAL rather than
/// about anything this identity did. The Corp is paid whatever the bids were,
/// including the game where it bid nothing at all: 10.14.6c reveals before
/// 10.14.4a spends, and it is the reveal the sentence names.
#[test]
fn nisei_division_is_paid_for_the_reveal_whatever_the_bids_were() {
    for corp_bid in [0, 2] {
        let mut vm = Vm::empty(6182);
        tk::install_identity(&mut vm, card("Nisei Division: The Next Generation"), Side::Corp);
        tk::install_identity(&mut vm, card("Akiko Nisei: Head Case"), Side::Runner);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 3;
        vm.st.runner.credits = 3;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::psi_bid(), Reply::Bid(corp_bid)),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Rnd))
                .when(Match::psi_bid(), Reply::Bid(1))
                .stop_at_action(),
        );
        assert!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::SecretlySpentCreditsRevealed { .. })),
            "corp_bid={corp_bid}: 10.14.6c's reveal happened: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.corp.credits,
            3 - corp_bid + 1,
            "corp_bid={corp_bid}: the bid was spent and the identity paid 1 for the \
             reveal: {}",
            t.tail(30)
        );
    }
}

/// Epiphany Analytica: "The first time each turn the Runner steals or trashes
/// a Corp card, place 1 power counter on this identity. / [click], hosted
/// power counter: Look at the top 3 cards of R&D. You may install 1 of those
/// cards."
///
/// The ordinal is shared, which is the whole reason the printed "or" is one
/// condition. The Runner trashes an asset AND steals an agenda in the same
/// turn, and exactly one counter arrives — two abilities, each with their own
/// "first time each turn", would have placed two.
#[test]
fn epiphany_analytica_places_one_counter_for_the_first_steal_or_trash_of_the_turn() {
    let mut vm = Vm::empty(6183);
    let ident =
        tk::install_identity(&mut vm, card("Epiphany Analytica: Nations Undivided"), Side::Corp);
    let asset =
        tk::install_root(&mut vm, tk::vanilla_asset("Trashable", 0, 1), ServerId::Remote(1), true);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Stealable", 3, 2),
        ServerId::Remote(2),
        false,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::of(Kind::MidAccess).once(), Reply::Take(Pick::BasicTrash))
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Discard(Side::Corp),
        "the Runner trashed a Corp card: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Runner),
        "and stole an agenda in the same turn: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&ident].counters.get(&CounterKind::Power).copied().unwrap_or(0),
        1,
        "one ordinal for the whole condition, so one counter: {}",
        t.tail(60)
    );
}

/// Epiphany Analytica's second line: "[click], hosted power counter: Look at
/// the top 3 cards of R&D. You may install 1 of those cards."
///
/// 1.9.2's cost comes off the source, so an EMPTY identity cannot use the
/// ability at all — 5.2.4 offers a [click] ability as an action, and an action
/// whose cost is unpayable is not among them. With a counter on it the Corp
/// spends the counter and installs one of the three cards it looked at.
#[test]
fn epiphany_analytica_spends_a_counter_to_install_off_the_top_of_rnd() {
    for stocked in [false, true] {
        let mut vm = Vm::empty(6184);
        let ident = tk::install_identity(
            &mut vm,
            card("Epiphany Analytica: Nations Undivided"),
            Side::Corp,
        );
        if stocked {
            vm.st.objects.get_mut(&ident).unwrap().counters.insert(CounterKind::Power, 1);
        }
        // The top of R&D is installable Corp cards; the mandatory draw takes
        // the topmost, so three of these are what the ability looks at.
        let deck: Vec<_> = (0..5)
            .map(|_| vm.new_object(tk::vanilla_asset("Deck Asset", 0, 1), Zone::Deck(Side::Corp)))
            .collect();
        vm.st.deck.get_mut(&Side::Corp).unwrap().extend(deck.iter().copied());
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::take("top 3 cards"))
                .when(Match::optional(), Reply::Optional(true))
                .when(
                    Match::destination(),
                    Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
                )
                .stop_at_action(),
            Plan::runner(),
        );
        let offered = t
            .of_kind(Kind::Action)
            .into_iter()
            .filter(|e| {
                plan::action_options(&e.spec).iter().any(|o| {
                    matches!(o, jinteki_cr::decision::ActionOption::CardAction { label, .. }
                        if label.contains("top 3 cards"))
                })
            })
            .count();
        assert_eq!(
            offered > 0,
            stocked,
            "stocked={stocked}: 1.9.2's counter cost is what makes the empty identity \
             unusable rather than free: {}",
            t.tail(40)
        );
        let installed =
            deck.iter().filter(|c| matches!(vm.st.objects[c].zone, Zone::Root(_))).count();
        assert_eq!(
            installed,
            usize::from(stocked),
            "stocked={stocked}: one of the three cards it looked at was installed: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&ident].counters.get(&CounterKind::Power).copied().unwrap_or(0),
            0,
            "stocked={stocked}: the counter was spent: {}",
            t.tail(40)
        );
    }
}

/// Arissana Rocha Nahu: "Once per turn → 0[credit]: Install 1 program from
/// your grip (paying its install cost). Use this ability only during a run.
/// When that run ends, trash that program if it is not a trojan."
///
/// Two games with the same program install, one where the program is a trojan
/// and one where it is not. The delayed conditional 9.6.13 creates is what
/// finds the card again when the run ends — "that program" is the target the
/// same ability announced, bound when the delayed ability was created, since
/// the frame that announced it is long gone by then.
#[test]
fn arissana_trashes_the_program_she_installed_unless_it_is_a_trojan() {
    for trojan in [false, true] {
        let mut vm = Vm::empty(6185);
        tk::install_identity(&mut vm, card("Arissana Rocha Nahu: Street Artist"), Side::Runner);
        let mut prog = tk::vanilla_runner_card("Some Program", CardType::Program);
        prog.cost = Some(1);
        if trojan {
            prog.subtypes = vec!["Trojan"];
        }
        let program = vm.new_object(prog, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(program);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Archives))
                .when(Match::paid().offering("street artist"), Reply::take("street artist"))
                .when(Match::targets().once(), Reply::target(program))
                .stop_at_action(),
        );
        assert!(
            vm.changes.log.iter().any(|c| matches!(
                c,
                GameChange::CardInstalled { obj, .. } if *obj == program
            )),
            "trojan={trojan}: the program was installed during the run: {}",
            t.tail(50)
        );
        assert_eq!(
            vm.st.objects[&program].zone,
            if trojan { Zone::Rig } else { Zone::Discard(Side::Runner) },
            "trojan={trojan}: the run ending trashes the program it is not a trojan: {}",
            t.tail(50)
        );
    }
}

/// Arissana's middle sentence: "Use this ability only during a run."
///
/// 9.3.3c's limit on WHEN, asked where it bites — the action phase's own paid
/// windows, before any run has begun. The ability is simply not among the
/// options there, which is what keeps the delayed conditional from ever being
/// created outside a run (9.6.13d).
#[test]
fn arissana_is_not_offered_outside_a_run() {
    let mut vm = Vm::empty(6186);
    tk::install_identity(&mut vm, card("Arissana Rocha Nahu: Street Artist"), Side::Runner);
    let mut prog = tk::vanilla_runner_card("Some Program", CardType::Program);
    prog.cost = Some(1);
    let program = vm.new_object(prog, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(program);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::credit()).stop_at_action(),
    );
    let offers = t
        .of_kind(Kind::Paid)
        .into_iter()
        .filter(|e| plan::count_labelled(plan::window_options(&e.spec), "street artist") > 0)
        .count();
    assert_eq!(offers, 0, "no run in progress, so the ability is never offered: {}", t.tail(40));
    assert_eq!(
        vm.st.objects[&program].zone,
        Zone::Hand(Side::Runner),
        "and the program stayed in the grip: {}",
        t.tail(40)
    );
}

/// Apex: "When your turn begins, you may install 1 card from your grip
/// facedown."
///
/// The whole of the sentence is in the last word. What goes into the rig is
/// an EVENT — a card 8.5.1 never installs — because 8.1.4a leaves a facedown
/// installed Runner card with no card type to be judged by; and nothing is
/// paid for it, because 8.5.11a gives a facedown Runner card no install cost
/// however expensive the card is printed. The description names no type for
/// the same reason, so the grip is offered whole.
#[test]
fn apex_installs_any_card_of_the_grip_facedown_and_pays_nothing_for_it() {
    let mut vm = Vm::empty(6187);
    tk::install_identity(&mut vm, card("Apex: Invasive Predator"), Side::Runner);
    let mut ev = tk::vanilla_runner_card("Some Event", CardType::Event);
    ev.cost = Some(4);
    let event = vm.new_object(ev, Zone::Hand(Side::Runner));
    let mut prog = tk::vanilla_runner_card("Some Program", CardType::Program);
    prog.cost = Some(3);
    prog.memory_cost = Some(5);
    let program = vm.new_object(prog, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([event, program]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().offering("invasive predator"), Reply::take("invasive predator"))
            .when(Match::targets().once(), Reply::target(event))
            .stop_at_action(),
    );
    let announced = t.of_kind(Kind::Targets);
    assert_eq!(announced.len(), 1, "one card was chosen from the grip: {}", t.tail(30));
    let candidates = announced[0].candidates();
    assert!(
        candidates.contains(&event) && candidates.contains(&program),
        "the description names no card type, so an event is as installable as a program: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&event].zone,
        Zone::Rig,
        "the event was installed: {}",
        t.tail(30)
    );
    assert!(
        !vm.st.objects[&event].faceup,
        "8.5.16a placed it with the status the effect stipulated: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.runner.credits, 0,
        "8.5.11a: a facedown Runner card has no install cost, so its printed 4 was never asked for: {}",
        t.tail(30)
    );
}

/// The same sentence, with the memory limit watching (CR 8.1.4a).
///
/// A program installed facedown has no card type and no memory cost, so the
/// 1.20.2 limit of 4 does not see it — installing a 5[mu] program facedown
/// must not trash anything.
#[test]
fn apexs_facedown_program_costs_no_memory() {
    let mut vm = Vm::empty(6188);
    tk::install_identity(&mut vm, card("Apex: Invasive Predator"), Side::Runner);
    let mut prog = tk::vanilla_runner_card("Some Program", CardType::Program);
    prog.cost = Some(0);
    prog.memory_cost = Some(5);
    let program = vm.new_object(prog, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(program);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().offering("invasive predator"), Reply::take("invasive predator"))
            .when(Match::targets().once(), Reply::target(program))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&program].zone,
        Zone::Rig,
        "5[mu] over a limit of 4, and it stayed installed — a facedown card is not a program: {}",
        t.tail(30)
    );
}

/// Apex: "You cannot install non-virtual resources."
///
/// CR 1.2.2: the "cannot" takes precedence over the ability that directs the
/// install, so the basic action (5.2.7d) is not offered for a resource the
/// sentence describes — and a **virtual** one, which it does not describe, is
/// offered as normal.
#[test]
fn apex_cannot_install_a_resource_that_is_not_virtual() {
    let mut vm = Vm::empty(6189);
    tk::install_identity(&mut vm, card("Apex: Invasive Predator"), Side::Runner);
    let mut plain = tk::vanilla_runner_card("Some Resource", CardType::Resource);
    plain.cost = Some(0);
    let plain = vm.new_object(plain, Zone::Hand(Side::Runner));
    let mut virt = tk::vanilla_runner_card("Some Virtual Resource", CardType::Resource);
    virt.cost = Some(0);
    virt.subtypes = vec!["Virtual"];
    let virt = vm.new_object(virt, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([plain, virt]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().offering("invasive predator"), Reply::Pass)
            .stop_at_action(),
    );
    let actions = t.of_kind(Kind::Action);
    assert!(!actions.is_empty(), "the action window was reached: {}", t.tail(30));
    let offered: Vec<_> = actions[0]
        .actions()
        .iter()
        .filter_map(|a| match a {
            jinteki_cr::decision::ActionOption::BasicInstall { card } => Some(*card),
            _ => None,
        })
        .collect();
    assert!(
        offered.contains(&virt),
        "a virtual resource is not what the sentence forbids: {}",
        t.tail(30)
    );
    assert!(
        !offered.contains(&plain),
        "and a non-virtual one cannot be installed at all: {}",
        t.tail(30)
    );
}

/// Where Apex's two sentences meet: the facedown install takes ANY card.
/// What it produces is 8.1.4a's blank object — no name, no type, no
/// subtypes — so "you cannot install non-virtual resources" has nothing to
/// read there (1.15.3), while 5.2.7d's faceup basic action still refuses
/// the same resource. `Vm::install_prohibited` asks with the declared face
/// in hand.
#[test]
fn apexs_facedown_install_takes_a_non_virtual_resource() {
    let mut vm = Vm::empty(6190);
    tk::install_identity(&mut vm, card("Apex: Invasive Predator"), Side::Runner);
    let mut plain = tk::vanilla_runner_card("Some Resource", CardType::Resource);
    plain.cost = Some(0);
    let plain = vm.new_object(plain, Zone::Hand(Side::Runner));
    let mut virt = tk::vanilla_runner_card("Some Virtual Resource", CardType::Resource);
    virt.cost = Some(0);
    virt.subtypes = vec!["Virtual"];
    let virt = vm.new_object(virt, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().extend([plain, virt]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().offering("invasive predator"), Reply::take("invasive predator"))
            .when(Match::targets().once(), Reply::target(plain))
            .stop_at_action(),
    );
    let announced = t.of_kind(Kind::Targets);
    assert_eq!(announced.len(), 1, "the identity announced its card: {}", t.tail(30));
    let candidates = announced[0].candidates();
    assert!(
        candidates.contains(&plain),
        "8.1.4a: what a facedown install produces has no type for the 'cannot' to read: {}",
        t.tail(30)
    );
    assert!(candidates.contains(&virt), "the virtual one too, as before: {}", t.tail(30));
    assert_eq!(vm.st.objects[&plain].zone, Zone::Rig, "installed: {}", t.tail(30));
    assert!(!vm.st.objects[&plain].faceup, "facedown: {}", t.tail(30));
}

/// Magdalene Keino-Chemutai: "Whenever you discard cards to reach your
/// maximum hand size, you may install 1 program or piece of hardware from
/// among those cards."
///
/// The discard step moves every card at once, so it is ONE occurrence naming
/// all of them — which is what "those cards" needs. The program here is the
/// SECOND card discarded and the first is an event, so a reading that kept
/// only the card the occurrence named first would offer nothing at all.
///
/// The printed "you may" is the other half: declining leaves the program
/// where the discard put it.
#[test]
fn magdalene_installs_a_program_from_anywhere_among_the_cards_she_just_discarded() {
    for take in [true, false] {
        let mut vm = Vm::empty(6191);
        tk::install_identity(
            &mut vm,
            card("Magdalene Keino-Chemutai: Cryptarchitect"),
            Side::Runner,
        );
        let mut mk = |name: &'static str, ty: CardType, cost: u32| {
            let mut c = tk::vanilla_runner_card(name, ty);
            c.cost = Some(cost);
            if ty == CardType::Program {
                c.memory_cost = Some(1);
            }
            let id = vm.new_object(c, Zone::Hand(Side::Runner));
            vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
            id
        };
        let event = mk("Some Event", CardType::Event, 0);
        let program = mk("Some Program", CardType::Program, 2);
        // Five fillers beside them: a grip of seven against a maximum hand
        // size of five is 5.7.4's two-card discard.
        tk::fill_hand(&mut vm, Side::Runner, 5);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);

        let mut runner = Plan::runner()
            .when(Match::action(), Reply::credit())
            .when(Match::discard(), Reply::Discard(vec![event, program]));
        if take {
            runner = runner
                .when(Match::reaction().offering("cryptarchitect"), Reply::take("cryptarchitect"));
        }
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::action(), Reply::Halt),
            runner,
        );
        assert_eq!(vm.st.turn_side, Side::Corp, "the Runner's turn finished: {}", t.tail(40));
        assert_eq!(
            vm.st.objects[&event].zone,
            Zone::Discard(Side::Runner),
            "take={take}: the event was discarded and stays discarded: {}",
            t.tail(40)
        );
        if take {
            let announced = t.of_kind(Kind::Targets);
            assert_eq!(announced.len(), 1, "the identity announced its card: {}", t.tail(40));
            assert_eq!(
                announced[0].candidates(),
                [program],
                "only the discarded PROGRAM is described — not the event beside it, \
                 and not a card the discard never touched: {}",
                t.tail(40)
            );
            assert_eq!(
                vm.st.objects[&program].zone,
                Zone::Rig,
                "and it was really installed out of the heap: {}",
                t.tail(40)
            );
            assert_eq!(
                vm.st.runner.credits,
                5 + 4 - 2,
                "8.5.11: the install paid its own cost: {}",
                t.tail(40)
            );
        } else {
            assert_eq!(
                vm.st.objects[&program].zone,
                Zone::Discard(Side::Runner),
                "declined: the program stays in the heap: {}",
                t.tail(40)
            );
            assert_eq!(vm.st.runner.credits, 5 + 4, "and nothing was paid: {}", t.tail(40));
        }
    }
}

/// Kabonesa Wu: "[click]: Search your stack for a non-virus program and
/// install it, lowering its install cost by 1[credit], then shuffle your
/// stack. If that program is still installed when your turn ends, remove it
/// from the game."
///
/// The whole of the second sentence's difficulty is which card "that program"
/// is. It is not a target — 8.7.4's find is not 1.15.2's announcement, so the
/// ability announced nothing at all — and the frame that installed it is gone
/// by the time the turn ends. The delayed conditional carries the install
/// across the gap, and this test is what proves it names the RIGHT card: a
/// second program, installed the same turn with the basic action (5.2.6d),
/// sits in the rig beside it and must survive.
#[test]
fn kabonesa_wu_removes_the_program_her_search_installed_and_no_other() {
    let mut vm = Vm::empty(6201);
    tk::install_identity(&mut vm, card("Kabonesa Wu: Netspace Thrillseeker"), Side::Runner);

    // The stack: one non-virus program (cost 2) and one virus program, which
    // the criteria must leave alone.
    let mut wanted = tk::vanilla_runner_card("Stack Program", CardType::Program);
    wanted.cost = Some(2);
    let found = vm.new_object(wanted, Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(found);
    let mut virus = tk::vanilla_runner_card("Stack Virus", CardType::Program);
    virus.cost = Some(1);
    virus.subtypes = vec!["Virus"];
    let unwanted = vm.new_object(virus, Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(unwanted);
    tk::fill_deck(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);

    // And one in the grip, installed by the basic action: the card the
    // identity must NOT reach.
    let mut other = tk::vanilla_runner_card("Grip Program", CardType::Program);
    other.cost = Some(1);
    let beside = vm.new_object(other, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(beside);

    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::take("kabonesa wu"))
            .when(Match::targets().once(), Reply::target(found))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(beside)))
            .when(Match::action(), Reply::credit()),
    );

    let offered = t.of_kind(Kind::Targets);
    assert_eq!(
        offered[0].candidates(),
        [found],
        "8.7.2a: the virus in the same stack is not a card the search may find: {}",
        t.tail(50)
    );
    assert_eq!(vm.st.turn_side, Side::Corp, "the Runner's turn finished: {}", t.tail(50));
    assert_eq!(
        vm.st.objects[&found].zone,
        Zone::RemovedFromGame,
        "the program the ability installed is gone when the turn ends: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&beside].zone,
        Zone::Rig,
        "and the one the basic action installed is not \"that program\": {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&unwanted].zone,
        Zone::Deck(Side::Runner),
        "the virus was never found: {}",
        t.tail(50)
    );
    // 5 credits, 4 clicks: the ability (1 of 2 after 1.16.6's reduction), the
    // basic install (1), then two basic credit actions.
    assert_eq!(
        vm.st.runner.credits,
        5 - 1 - 1 + 2,
        "1.16.6 lowered the install cost by 1 and the basic install paid its own: {}",
        t.tail(50)
    );
}

/// The other half of Kabonesa Wu's second sentence: "**if** that program is
/// still installed".
///
/// 8.7.2e lets a criteria search of a deck fail to find, so a stack with no
/// non-virus program in it leaves the ability with nothing to install — and
/// 8.7.4 resumes the resolution anyway. The delayed conditional is still
/// created and still meets its condition when the turn ends; what it must not
/// do is reach the virus it could not find, or anything else.
#[test]
fn kabonesa_wu_removes_nothing_when_her_search_installed_nothing() {
    let mut vm = Vm::empty(6202);
    tk::install_identity(&mut vm, card("Kabonesa Wu: Netspace Thrillseeker"), Side::Runner);

    let mut virus = tk::vanilla_runner_card("Stack Virus", CardType::Program);
    virus.cost = Some(1);
    virus.subtypes = vec!["Virus"];
    let unwanted = vm.new_object(virus, Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(unwanted);
    tk::fill_deck(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);

    let mut other = tk::vanilla_runner_card("Grip Program", CardType::Program);
    other.cost = Some(1);
    let beside = vm.new_object(other, Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(beside);

    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::take("kabonesa wu"))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(beside)))
            .when(Match::action(), Reply::credit()),
    );

    assert!(
        t.of_kind(Kind::Targets).is_empty(),
        "8.7.2e: nothing in the stack matches, so nothing is even offered: {}",
        t.tail(50)
    );
    assert_eq!(vm.st.turn_side, Side::Corp, "the Runner's turn finished: {}", t.tail(50));
    assert_eq!(
        vm.st.objects[&beside].zone,
        Zone::Rig,
        "the program the identity never installed survives the turn ending: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.objects[&unwanted].zone,
        Zone::Deck(Side::Runner),
        "and the virus it could not find is still in the stack: {}",
        t.tail(50)
    );
}

/// Mti Mwekundu: "Once per turn → When the Runner approaches a server, you may
/// install 1 piece of ice from HQ in the innermost position protecting that
/// server, ignoring all costs. The Runner moves to that ice and approaches it.
/// If this is not the first time they have approached ice this run, they may
/// jack out."
///
/// The main line: a server with one piece of ice protecting it. The Runner
/// passes it, reaches 6.9.4g, and the identity puts a second piece of ice
/// INSIDE the first — 6.2.2b's end of the sequence, not 8.5.2d's — and sends
/// them back to approach it. Having already approached ice this run, they are
/// offered the jack-out 6.1.5a would have given them for the pass they never
/// made.
#[test]
fn mti_mwekundu_installs_ice_innermost_and_sends_the_runner_back_to_approach_it() {
    let mut vm = Vm::empty(6203);
    tk::install_identity(&mut vm, card("Mti Mwekundu: Life Improved"), Side::Corp);
    let printed = tk::install_ice(&mut vm, tk::vanilla_ice("Outer Ice", 0, 1), ServerId::Hq, false);
    let ambush = vm.new_object(tk::vanilla_ice("Ambush Ice", 3, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ambush);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::reaction().offering("life improved"), Reply::take("life improved"))
            .when(Match::targets().once(), Reply::Targets(vec![ambush])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            // 6.9.4c, after passing the printed ice: the run's own offer.
            .when(Match::of(Kind::JackOut).once(), Reply::JackOut(false))
            // The identity's third sentence — halt on it, so the board is
            // read from inside the ability that made it.
            .when(Match::optional().once(), Reply::Halt)
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    script.run(&mut vm);
    let t = script.transcript();

    let (server, pos) = vm
        .position_of_ice(ambush)
        .unwrap_or_else(|| panic!("the ice from HQ is protecting a server: {}", t.tail(60)));
    assert_eq!(server, ServerId::Hq, "the attacked server, not one the Corp picked: {}", t.tail(60));
    assert_eq!(
        vm.positions_inward_of(ServerId::Hq, pos),
        Some(0),
        "6.2.2b: the INNERMOST position, inward of the ice already there: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.positions_inward_of(ServerId::Hq, vm.position_of_ice(printed).unwrap().1),
        Some(1),
        "and the printed ice is now the outer of the two: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.corp.credits, 5,
        "1.16.5c: ignoring all costs pays neither the install cost nor 8.5.11a's \
         1[credit] for the ice already protecting HQ: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.run_ctx().and_then(|r| r.position),
        Some(pos),
        "6.2.8a: the Runner moved to that ice: {}",
        t.tail(60)
    );
    let offer = t.last().expect("the plan halted on a decision");
    assert_eq!(offer.side, Side::Runner, "'they may jack out' is put to the RUNNER: {}", t.tail(60));
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::IceApproached { ice } if *ice == ambush)),
        "and it is offered BEFORE the approach the move sent them to, which is why \
         'not the first time' counts only the approaches already made: {}",
        t.tail(60)
    );
}

/// The other half of Mti Mwekundu's third sentence: 6.1.5b's case.
///
/// A server with no ice protecting it is approached at once, so the ice this
/// identity installs is the FIRST the Runner approaches this run — and the
/// sentence withholds the jack-out exactly there. The run still offers its
/// own, 6.1.5b's, before the approach and 6.9.4c's after the pass; what must
/// not appear is the ability's optional one in between.
#[test]
fn mti_mwekundu_offers_no_jack_out_when_the_server_had_no_ice() {
    let mut vm = Vm::empty(6204);
    tk::install_identity(&mut vm, card("Mti Mwekundu: Life Improved"), Side::Corp);
    let ambush = vm.new_object(tk::vanilla_ice("Ambush Ice", 3, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ambush);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp()
            .when(Match::reaction().offering("life improved"), Reply::take("life improved"))
            .when(Match::targets().once(), Reply::Targets(vec![ambush])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );
    script.run(&mut vm);
    let t = script.transcript();

    assert_eq!(
        vm.position_of_ice(ambush).map(|(s, _)| s),
        Some(ServerId::Hq),
        "the ice went in, on a server that had none: {}",
        t.tail(60)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::IceApproached { ice } if *ice == ambush)),
        "and the Runner was sent back to approach it: {}",
        t.tail(60)
    );
    assert!(
        t.of_kind(Kind::Optional).is_empty(),
        "but the identity offered them nothing to decline — this ice IS the first \
         they have approached this run, which is 6.1.5b's case: {}",
        t.tail(60)
    );
}

/// And what the offer is FOR: a Runner who takes it ends the run there, on the
/// inside of a piece of ice they never chose to face. 6.1.5 says jacking out
/// "follows the usual process for ending the run", so the run ends before the
/// Success Phase and HQ is never breached.
#[test]
fn mti_mwekundu_lets_the_runner_jack_out_of_the_ice_it_installed() {
    let mut vm = Vm::empty(6205);
    tk::install_identity(&mut vm, card("Mti Mwekundu: Life Improved"), Side::Corp);
    tk::install_ice(&mut vm, tk::vanilla_ice("Outer Ice", 0, 1), ServerId::Hq, false);
    let ambush = vm.new_object(tk::vanilla_ice("Ambush Ice", 3, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ambush);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::reaction().offering("life improved"), Reply::take("life improved"))
            .when(Match::targets().once(), Reply::Targets(vec![ambush])),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::of(Kind::JackOut).once(), Reply::JackOut(false))
            .when(Match::optional().once(), Reply::Optional(true))
            .when(Match::action(), Reply::credit()),
    );

    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "the run ended: {}",
        t.tail(60)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "and it ended before the Success Phase, so HQ was never breached: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.position_of_ice(ambush).map(|(s, _)| s),
        Some(ServerId::Hq),
        "while the ice the identity installed is still protecting HQ, innermost, \
         for the next run to find: {}",
        t.tail(60)
    );
}


/// Weyland Consortium: Because We Built It — "1[recurring-credit] / Use this
/// credit to advance ice."
///
/// Four plays over two variables: whether the card being advanced is a piece
/// of ice, and whether the Corp has a credit of their own. Both cards carry the
/// same "you can advance this card" declaration and are rezzed, so 1.18.3
/// offers both to 5.2.6f — what differs is only whether the identity's credit
/// may pay for the action.
///
/// With an empty pool the action exists only against the ice: 1.16.1b withholds
/// an action whose cost cannot be paid, and 1.10.3c allows this credit for
/// nothing else. Give the Corp a credit of their own and the asset can be
/// advanced after all, out of the pool, with the identity's credit untouched —
/// which is what says the first half was about the PAYMENT and not about which
/// cards may be advanced. The fourth play is 1.10.3c's division itself: with
/// both locations allowed and only one credit owed, the Corp is asked which to
/// spend, and spends the identity's.
#[test]
fn because_we_built_it_pays_to_advance_ice_and_nothing_else() {
    for (is_ice, pool) in [(true, 0), (false, 0), (false, 1), (true, 1)] {
        let mut vm = Vm::empty(6206);
        let bwbi = tk::install_identity(
            &mut vm,
            card("Weyland Consortium: Because We Built It"),
            Side::Corp,
        );
        // 1.18.3: a card can be advanced if it is an agenda or if an active
        // ability says so — the same declaration on both, so the only thing
        // that differs between the halves is the card's TYPE.
        let advanceable = |name: &'static str, ty: CardType| {
            let mut c = PrintedCard::vanilla(name, Side::Corp, ty);
            c.abilities = vec![jinteki_cr::ability::AbilityDef::static_ability(vec![
                jinteki_cr::ability::StaticDecl::CanBeAdvancedSelf,
            ])
            .labeled("you can advance this card")];
            c
        };
        let target = if is_ice {
            tk::install_ice(
                &mut vm,
                advanceable("Advanceable Ice", CardType::Ice),
                ServerId::Hq,
                true,
            )
        } else {
            tk::install_root(
                &mut vm,
                advanceable("Advanceable Asset", CardType::Asset),
                ServerId::Remote(0),
                true,
            )
        };
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = pool;
        vm.start_turn(Side::Corp);

        let takeable = is_ice || pool > 0;
        let mut corp = Plan::corp();
        if takeable {
            corp = corp
                .when(Match::action().once(), Reply::Take(Pick::Advance(target)))
                // 1.10.3c: spend the identity's credit and keep the pool's,
                // which is only ever asked when both are allowed.
                .when(Match::division().once(), Reply::Division(vec![0, 1]));
        }
        let t = plan::play(&mut vm, corp.stop_at_action(), Plan::runner());

        let offered = t.entries.iter().any(|e| {
            e.actions().iter().any(|a| matches!(
                a,
                jinteki_cr::decision::ActionOption::BasicAdvance { card } if *card == target
            ))
        });
        assert_eq!(
            offered, takeable,
            "is_ice={is_ice} pool={pool}: 1.10.3c allows the credit only for advancing \
             ice, and 1.16.1b withholds an action nothing can pay for: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.objects[&target].counter(CounterKind::Advancement),
            u32::from(takeable),
            "is_ice={is_ice} pool={pool}: the counter is on the card exactly when the \
             action was takeable: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.objects[&bwbi].counter(CounterKind::Credit),
            u32::from(!is_ice),
            "is_ice={is_ice} pool={pool}: 1.10.3a — the credit left the identity only \
             for the payment it was allowed to make: {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.corp.credits,
            if is_ice { pool } else { 0 },
            "is_ice={is_ice} pool={pool}: and the pool paid only where the identity \
             could not: {}",
            t.tail(30)
        );
        assert_eq!(
            !t.of_kind(Kind::Division).is_empty(),
            is_ice && pool > 0,
            "is_ice={is_ice} pool={pool}: 1.10.3c's division is a real choice only \
             when more than one location is allowed and not all of it is owed: {}",
            t.tail(30)
        );
    }
}

/// Chronos Protocol: Selective Mind-mapping — "For the first net damage the
/// Runner suffers each turn, you may look at the Runner's grip and select the
/// card that is trashed."
///
/// The Corp presses the same button twice in one turn. The FIRST net damage
/// offers the choice and trashes the card the Corp names; the second offers
/// nothing and takes 10.4.2a's random card, which is the printed ordinal being
/// read from the change log rather than from a once-per-turn flag a static
/// ability would never spend (9.4.1).
#[test]
fn chronos_protocol_selects_the_card_the_first_net_damage_trashes() {
    let mut vm = Vm::empty(6207);
    tk::install_identity(&mut vm, card("Chronos Protocol: Selective Mind-mapping"), Side::Corp);
    tk::install_root(&mut vm, tk::net_damage_button("Hurt", 1), ServerId::Remote(1), true);
    let grip = tk::fill_hand(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    let victim = grip[2];
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().times(2), Reply::take("do net damage"))
            .when(Match::optional(), Reply::Optional(true))
            .when(Match::targets(), Reply::Targets(vec![victim]))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&victim].zone,
        Zone::Discard(Side::Runner),
        "10.4.3a: the card the Corp selected is the one that was trashed: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "both net damages landed — the declaration changes which card, never how \
         many: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::Optional).len(),
        1,
        "and the choice was offered exactly once: the SECOND net damage of the turn \
         is not the first one, so the declaration does not reach it: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::Targets).len(),
        1,
        "so the grip was named only for the damage the declaration reached: {}",
        t.tail(40)
    );
}

/// The two halves the same sentence states about WHICH damage it reaches, and
/// about how it is offered.
///
/// "Net damage" names one of 10.4.2's three types, so a meat damage of the same
/// size is left random and the Corp is asked nothing at all. And the printed
/// "you may" governs the looking as well as the selecting: a Corp who declines
/// is never shown the grip, so no target announcement happens — while the
/// damage itself lands either way.
#[test]
fn chronos_protocol_ignores_meat_damage_and_names_no_grip_when_it_is_declined() {
    for (label, meat, take) in [("meat", true, true), ("declined", false, false)] {
        let mut vm = Vm::empty(6208);
        tk::install_identity(&mut vm, card("Chronos Protocol: Selective Mind-mapping"), Side::Corp);
        let button = if meat {
            tk::meat_damage_button("Hurt", 1)
        } else {
            tk::net_damage_button("Hurt", 1)
        };
        tk::install_root(&mut vm, button, ServerId::Remote(1), true);
        tk::fill_hand(&mut vm, Side::Runner, 4);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        let label_of = if meat { "do meat damage" } else { "do net damage" };
        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::paid().once(), Reply::take(label_of))
                .when(Match::optional(), Reply::Optional(take))
                .stop_at_action(),
            Plan::runner(),
        );

        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            3,
            "the {label} damage landed: {}",
            t.tail(40)
        );
        assert_eq!(
            t.of_kind(Kind::Optional).len(),
            if meat { 0 } else { 1 },
            "the declaration names net damage, so only that one is offered ({label}): {}",
            t.tail(40)
        );
        assert!(
            t.of_kind(Kind::Targets).is_empty(),
            "and the grip is never named — not for a damage the declaration does not \
             reach, and not for one the Corp declined to use it on ({label}): {}",
            t.tail(40)
        );
    }
}

/// Trashes the Corp made out of R&D — the occurrence Nuvem SA's second line
/// names. Counted from the log rather than off the deck's length, which
/// 5.6.1's mandatory draw also moves.
fn corp_trashes_from_rnd(vm: &Vm) -> usize {
    vm.changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardTrashed { by, was_zone, .. }
                             if *by == Side::Corp && *was_zone == Zone::Deck(Side::Corp)))
        .count()
}

/// Nuvem SA: Law of the Land — "Whenever you finish resolving an operation or
/// an action on an expendable card, look at the top card of R&D. You may trash
/// that card. / The first time you trash a card from R&D during each of your
/// turns, gain 2[credit]."
///
/// The play half of the first line and the whole of the second, in one Corp
/// turn spent on three operations. Every one of them reaches 8.6.7h and every
/// one of them is looked at for — the looking is not what the printed "you
/// may" governs — but only the FIRST trash is paid for, and the third is
/// declined and takes no card at all.
///
/// The look count is the other assertion: playing an operation with 5.2.6e's
/// basic action also completes an ACTION, and the sentence's second half
/// describes a card the action is an ability of. 9.1.3 gives a basic action no
/// card, so three operations are three looks and not six.
#[test]
fn nuvem_looks_at_rnd_for_each_operation_and_pays_for_the_first_trash() {
    let mut vm = Vm::empty(6208);
    tk::install_identity(&mut vm, card("Nuvem SA: Law of the Land"), Side::Corp);
    let ops: Vec<_> = ["Op One", "Op Two", "Op Three"]
        .into_iter()
        .map(|n| {
            let id = vm.new_object(tk::operation(n, 0, vec![]), Zone::Hand(Side::Corp));
            vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
            id
        })
        .collect();
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::PlayCard(ops[0])))
            .when(Match::optional().once(), Reply::Optional(true))
            .when(Match::action().once(), Reply::Take(Pick::PlayCard(ops[1])))
            .when(Match::optional().once(), Reply::Optional(true))
            .when(Match::action().once(), Reply::Take(Pick::PlayCard(ops[2])))
            .when(Match::optional().once(), Reply::Optional(false))
            .stop_at_action(),
        Plan::runner(),
    );

    let looks = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardLookedAt { by, .. } if *by == Side::Corp))
        .count();
    assert_eq!(
        looks, 3,
        "8.6.7h once per operation, and 5.2.2d's basic action is no card's ability: {}",
        t.tail(60)
    );
    assert_eq!(
        corp_trashes_from_rnd(&vm),
        2,
        "two of the three looked-at cards were trashed and the third was declined: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.corp.credits, 2,
        "only the FIRST trash from R&D this turn paid — the second is not 'the first time': {}",
        t.tail(60)
    );
}

/// Nuvem SA: Law of the Land, the action half — "…or an action on an
/// expendable card".
///
/// The same [click] ability on the same asset, twice, with only the printed
/// subtype differing: 5.2.2d's moment is reached either way and the sentence
/// reaches only the described card. The basic credit action taken afterwards
/// is the control on the other side — it completes an action too, and 9.1.3
/// leaves no card for the description to be about.
#[test]
fn nuvem_reads_an_action_only_on_a_card_the_sentence_describes() {
    for expendable in [true, false] {
        let mut vm = Vm::empty(6209);
        tk::install_identity(&mut vm, card("Nuvem SA: Law of the Land"), Side::Corp);
        let mut button = PrintedCard::vanilla("Paper Shredder", Side::Corp, CardType::Asset);
        button.subtypes = if expendable { vec!["Expendable"] } else { Vec::new() };
        button.abilities = vec![jinteki_cr::ability::AbilityDef::paid(
            jinteki_cr::ability::Cost {
                clicks: 1,
                ..jinteki_cr::ability::Cost::default()
            },
            vec![Instruction::GainCredits(Side::Corp, jinteki_cr::instr::Quantity::c(1))],
        )
        .labeled("shredder: [click] gain 1")];
        tk::install_root(&mut vm, button, ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::Take(Pick::Labeled("shredder")))
                .when(Match::optional().once(), Reply::Optional(true))
                .when(Match::action().once(), Reply::credit())
                .stop_at_action(),
            Plan::runner(),
        );

        let looks = vm
            .changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CardLookedAt { by, .. } if *by == Side::Corp))
            .count();
        assert_eq!(
            looks,
            usize::from(expendable),
            "expendable={expendable}: 5.2.4's action is an ability OF a card, and only \
             the described one is named — and never a basic action: {}",
            t.tail(50)
        );
        assert_eq!(
            corp_trashes_from_rnd(&vm),
            usize::from(expendable),
            "expendable={expendable}: R&D lost a card exactly when the ability reached it: {}",
            t.tail(50)
        );
        assert_eq!(
            vm.st.corp.credits,
            if expendable { 4 } else { 2 },
            "expendable={expendable}: the ability's 1 and the basic action's 1, plus the \
             identity's 2 for the first trash from R&D: {}",
            t.tail(50)
        );
    }
}

/// Nuvem SA: Law of the Land, the second line's span — "during each of **your**
/// turns".
///
/// The same Corp card trashes the same top card of R&D, once when the Corp's
/// turn begins and once when the Runner's does. 9.2.1's active player is the
/// whole difference: the ordinal counts inside whichever turn is being played,
/// so without the stipulation the Runner's turn would carry a "first time"
/// of its own and pay for it.
#[test]
fn nuvem_pays_for_a_trash_from_rnd_only_during_a_corp_turn() {
    for corp_turn in [true, false] {
        let mut vm = Vm::empty(6210);
        tk::install_identity(&mut vm, card("Nuvem SA: Law of the Land"), Side::Corp);
        let whose = if corp_turn { Side::Corp } else { Side::Runner };
        let mut miller = PrintedCard::vanilla("Auto Miller", Side::Corp, CardType::Asset);
        miller.abilities = vec![jinteki_cr::ability::AbilityDef::conditional(
            jinteki_cr::ability::TriggerCond::turn_begins(whose),
            vec![Instruction::TrashCards(jinteki_cr::instr::TargetSpec::TopOfDeck {
                side: Side::Corp,
                count: jinteki_cr::instr::Quantity::c(1),
            })],
            false,
        )
        .labeled("auto miller: trash the top card of R&D")];
        tk::install_root(&mut vm, miller, ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(whose);

        let t = plan::play(
            &mut vm,
            Plan::corp().stop_at_action(),
            Plan::runner().stop_at_action(),
        );

        assert_eq!(
            corp_trashes_from_rnd(&vm),
            1,
            "corp_turn={corp_turn}: the card left R&D either way — the trash is the same: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.corp.credits,
            if corp_turn { 2 } else { 0 },
            "corp_turn={corp_turn}: 9.2.1 — the sentence is about the Corp's own turns: {}",
            t.tail(40)
        );
    }
}

// ---------------------------------------------------------------------------
// The identity queue — CR 9.11.3's one sentence, with a back-reference
// across its "and"
// ---------------------------------------------------------------------------

/// Blue Sun: "When your turn begins, you may add 1 rezzed card to HQ and
/// gain credits equal to its rez cost."
///
/// One sentence, one instruction (9.11.3): the card is announced before any
/// of the sentence resolves (1.15.2), which is what lets the second half's
/// "its" (1.15.4) read the card the first half chose. Two rezzed cards with
/// different printed rez costs are the assertion that the gain reads the
/// CHOSEN one; the unrezzed asset is the assertion that "rezzed" (8.1.2)
/// bites at the announcement; and the printed number is 1.16.4a's inherent
/// cost, not a record of a payment — the ice was rezzed by the testkit for
/// nothing and its cost is still what the sentence pays. Declining moves
/// nothing and gains nothing.
#[test]
fn blue_sun_adds_a_rezzed_card_to_hq_and_gains_its_printed_rez_cost() {
    for accept in [true, false] {
        let mut vm = Vm::empty(25123);
        tk::install_identity(&mut vm, card("Blue Sun: Powering the Future"), Side::Corp);
        let cheap =
            tk::install_root(&mut vm, tk::vanilla_asset("Cheap Asset", 2, 2), ServerId::Remote(1), true);
        let dear = tk::install_ice(&mut vm, tk::etr_ice("Dear Wall", 5, 1), ServerId::Hq, true);
        let hidden =
            tk::install_root(&mut vm, tk::vanilla_asset("Hidden Asset", 3, 2), ServerId::Remote(2), false);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let corp = if accept {
            Plan::corp()
                .when(
                    Match::reaction().offering("powering the future"),
                    Reply::take("powering the future"),
                )
                .when(Match::targets().once(), Reply::Targets(vec![dear]))
                .stop_at_action()
        } else {
            // 9.6.9: declining is not taking the offered reaction at all.
            Plan::corp().stop_at_action()
        };
        let t = plan::play(&mut vm, corp, Plan::runner());

        if accept {
            let announce = t
                .entries
                .iter()
                .find(|e| e.kind() == Kind::Targets)
                .unwrap_or_else(|| panic!("no target announcement: {}", t.tail(16)));
            assert!(
                announce.candidates().contains(&cheap) && announce.candidates().contains(&dear),
                "both rezzed cards are candidates: {:?}",
                announce.candidates()
            );
            assert!(
                !announce.candidates().contains(&hidden),
                "8.1.2: an unrezzed card is not a rezzed one: {:?}",
                announce.candidates()
            );
            assert_eq!(
                vm.st.objects[&dear].zone,
                Zone::Hand(Side::Corp),
                "the chosen card was added to HQ: {}",
                t.tail(16)
            );
            assert_eq!(
                vm.st.objects[&cheap].zone,
                Zone::Root(ServerId::Remote(1)),
                "the card the Corp did not choose stayed where it was: {}",
                t.tail(16)
            );
            assert_eq!(
                vm.st.corp.credits,
                5,
                "1.15.4/1.16.4a: the gain is the CHOSEN card's printed rez cost: {}",
                t.tail(16)
            );
        } else {
            assert_eq!(
                vm.st.objects[&dear].zone,
                Zone::Ice(ServerId::Hq),
                "nothing moved on a decline: {}",
                t.tail(16)
            );
            assert_eq!(vm.st.corp.credits, 0, "and nothing was gained: {}", t.tail(16));
        }
    }
}

// ---------------------------------------------------------------------------
// The identity queue — CR 9.9's "would be declared successful"
// ---------------------------------------------------------------------------

/// Omar Keung: "Once per turn → [click]: Run Archives. If that run would be
/// declared successful, change the attacked server to HQ or R&D for the
/// remainder of that run."
///
/// The run is announced against Archives and succeeds against HQ. Three
/// separate claims of the printed sentence, asserted together because only
/// their conjunction distinguishes this card from a run on HQ:
///
/// - the declaration follows the change, so 6.9.5a records the success on HQ
///   and never on Archives — an ability reacting AFTER the declaration would
///   record it the other way round;
/// - 6.9.5b breaches the server as it now stands, so the card accessed comes
///   out of HQ;
/// - 6.1.2d changes the attacked server WITHOUT moving the Runner, so the
///   rezzed ice protecting HQ is never approached and never encountered.
///
/// The ice is the control that a plain `Reply::run(ServerId::Hq)` could not
/// pass: it ends the run.
#[test]
fn omar_keung_succeeds_on_hq_without_ever_meeting_the_ice_protecting_it() {
    let mut vm = Vm::empty(6220);
    tk::install_identity(&mut vm, card("Omar Keung: Conspiracy Theorist"), Side::Runner);
    let gate = tk::install_ice(&mut vm, tk::etr_ice("Gate", 0, 3), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            // No `.once()` on the action rule: taking it again would mean
            // 9.3.6g's flag was never spent.
            .when(Match::action(), Reply::take("run archives"))
            .when(Match::interrupt(), Reply::take("Omar"))
            .when(Match::options().once(), Reply::ChooseNamed("HQ"))
            .stop_at_action(),
    );

    let declared: Vec<ServerId> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::RunDeclaredSuccessful { server, .. } => Some(*server),
            _ => None,
        })
        .collect();
    assert_eq!(
        declared,
        vec![ServerId::Hq],
        "6.9.5a read the attacked server the interrupt had already changed: {}",
        t.tail(40)
    );
    let breached: Vec<ServerId> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::BreachBegan { server } => Some(*server),
            _ => None,
        })
        .collect();
    assert_eq!(breached, vec![ServerId::Hq], "6.9.5b breached HQ: {}", t.tail(40));
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == gate)),
        "6.1.2d: the attacked server changed without the Runner moving, so the ice \
         protecting HQ was never met: {}",
        t.tail(40)
    );
    let offers = t
        .of_kind(Kind::Action)
        .into_iter()
        .filter(|e| Pick::Labeled("run archives").find_action(e.actions()).is_some())
        .count();
    assert_eq!(offers, 1, "9.3.6g: the ability is offered once a turn: {}", t.tail(40));
}

/// The same identity against a Crisium-class upgrade in the root of Archives:
/// "runs on this server cannot be declared successful."
///
/// 9.9.2 is the whole test. The expected effects of an imminent instruction
/// are what a static ability modifies, and a run that cannot be declared
/// successful has no such effect to expect — so the interrupt is not relevant
/// (9.9.3), is never offered, and the attacked server stays Archives. An
/// implementation that read the prohibition only when the declaration
/// RESOLVED would offer the choice, take the [click], and change the server
/// for nothing.
#[test]
fn omar_keung_is_not_offered_when_the_run_cannot_be_declared_successful() {
    let mut vm = Vm::empty(6221);
    tk::install_identity(&mut vm, card("Omar Keung: Conspiracy Theorist"), Side::Runner);
    tk::install_root(&mut vm, tk::crisium_like("Crisium-like"), ServerId::Archives, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::take("run archives"))
            .when(Match::interrupt(), Reply::Forbid)
            .when(Match::options(), Reply::Forbid)
            .stop_at_action(),
    );

    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the Crisium-class static held: {}",
        t.tail(40)
    );
    let breached: Vec<ServerId> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::BreachBegan { server } => Some(*server),
            _ => None,
        })
        .collect();
    assert_eq!(
        breached,
        vec![ServerId::Archives],
        "6.9.5b still breaches — the declaration is what the static forbids, not the \
         breach — and the server it breaches is the one the run announced: {}",
        t.tail(40)
    );
}

/// Topan: Ormas Leader — "Once per turn → [click]: Install 1 card from your
/// grip, paying 2[credit] less. When you install that card, suffer 1 meat
/// damage."
///
/// The ability installs a 3-cost program for 1[credit], and the damage lands
/// AFTER the install is complete — the condition is met by the install the
/// ability performed, pended at the checkpoint that processes it, so the log
/// shows the install first and exactly one meat damage after it. 9.3.6g's
/// flag is spent by the use: the action is offered once this turn.
///
/// "1 card" is untyped, so the announcement is where 8.5.3 bites: the event
/// in the grip is never a candidate, because an event is not a card that can
/// be installed and so not a valid target (1.15.3).
#[test]
fn topan_installs_for_two_less_and_the_damage_follows_that_install() {
    let mut vm = Vm::empty(6227);
    tk::install_identity(&mut vm, card("Topan: Ormas Leader"), Side::Runner);
    let program = {
        let mut c = tk::vanilla_runner_card("Some Program", CardType::Program);
        c.cost = Some(3);
        c.memory_cost = Some(1);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    let event = {
        let mut c = tk::vanilla_runner_card("Some Event", CardType::Event);
        c.cost = Some(0);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            // No `.once()` on the action rule: a second offer would be taken
            // again, and the offer count below would catch the unspent flag.
            .when(Match::action(), Reply::take("ormas leader"))
            .when(Match::targets().once(), Reply::Targets(vec![program]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&program].zone,
        Zone::Rig,
        "the ability installed the program: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.runner.credits, 0,
        "1 credit paid a 3-credit install — 1.16.6 lowered it by 2: {}",
        t.tail(40)
    );
    let meat = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::DamageSuffered { kind, .. } if *kind == jinteki_cr::effects::DamageKind::Meat))
        .count();
    assert_eq!(meat, 1, "the second sentence did exactly one meat damage: {}", t.tail(40));
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        3,
        "five cards in the grip: the install took the program, the damage one more: {}",
        t.tail(40)
    );
    let announce = t
        .entries
        .iter()
        .find(|e| e.kind() == Kind::Targets)
        .unwrap_or_else(|| panic!("no target announcement: {}", t.tail(16)));
    assert!(
        announce.candidates().contains(&program),
        "the program is a candidate: {:?}",
        announce.candidates()
    );
    assert!(
        !announce.candidates().contains(&event),
        "8.5.3: an event is never installed, so it is not one: {:?}",
        announce.candidates()
    );
    let installed_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CardInstalled { obj, .. } if *obj == program))
        .expect("the install was recorded");
    let damaged_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("the damage was recorded");
    assert!(
        installed_at < damaged_at,
        "8.5.16f before the conditional it meets — install first, damage after: {}",
        t.tail(40)
    );
    let offers = t
        .of_kind(Kind::Action)
        .into_iter()
        .filter(|e| Pick::Labeled("ormas leader").find_action(e.actions()).is_some())
        .count();
    assert_eq!(offers, 1, "9.3.6g: the ability is offered once a turn: {}", t.tail(40));
}

/// The other half of the new occurrence fact: the basic action's install
/// (5.2.7d) is the PLAYER's, not this ability's, so "when you install that
/// card" is met by nothing and no damage happens.
#[test]
fn topans_damage_does_not_follow_the_basic_actions_install() {
    let mut vm = Vm::empty(6228);
    tk::install_identity(&mut vm, card("Topan: Ormas Leader"), Side::Runner);
    let program = {
        let mut c = tk::vanilla_runner_card("Some Program", CardType::Program);
        c.cost = Some(0);
        c.memory_cost = Some(1);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::fill_hand(&mut vm, Side::Runner, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(program)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&program].zone,
        Zone::Rig,
        "the basic action installed the program: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::DamageSuffered { .. })),
        "and the identity's condition was met by none of it: {}",
        t.tail(40)
    );
}

/// Every `WindowOption` this transcript ever put in front of a player. The
/// prohibition Saraswati creates is a "cannot", so what it changes is what is
/// OFFERED (1.2.2), and that is where it has to be read.
fn offered_options(t: &plan::Transcript) -> Vec<jinteki_cr::decision::WindowOption> {
    t.entries.iter().flat_map(|e| e.options().iter().cloned()).collect()
}

/// Saraswati Mnemonics: "[click], 1[credit]: Install 1 card from HQ in the
/// root of a remote server, then place 1 advancement counter on it. You
/// cannot score or rez that card until your next turn begins."
///
/// The install half and the rez half of the prohibition. The card is an
/// upgrade, so 8.5.16b's declaration has real alternatives to be narrowed
/// from — every central root and a position protecting each server — and only
/// the remote roots survive. The upgrade then costs 0 to rez and still cannot
/// be, all the way to the end of the Corp's turn, while the asset sharing its
/// root is offered throughout: the difference between them is the effect and
/// not the window.
#[test]
fn saraswati_mnemonics_installs_into_a_remote_root_and_holds_the_rez_off() {
    let mut vm = Vm::empty(6211);
    tk::install_identity(&mut vm, card("Saraswati Mnemonics: Endless Exploration"), Side::Corp);
    // An existing remote, so "a remote server" has something to name besides
    // the one 8.5.2a would create.
    let neighbour =
        tk::install_root(&mut vm, tk::vanilla_asset("Neighbour", 0, 2), ServerId::Remote(1), false);
    let upgrade = vm.new_object(tk::vanilla_upgrade("Held Upgrade", 0), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(upgrade);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().offering("endless exploration").once(), Reply::take("endless exploration"))
            .when(Match::targets().once(), Reply::Targets(vec![upgrade]))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Root(ServerId::Remote(1))),
            )
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    let offered_dests: Vec<jinteki_cr::instr::InstallDest> = t
        .of_kind(Kind::Destination)
        .first()
        .map(|e| match &e.spec {
            jinteki_cr::decision::DecisionSpec::DeclareInstallDestination { options } => {
                options.clone()
            }
            other => panic!("a destination declaration, not {other:?}"),
        })
        .unwrap_or_else(|| panic!("the installer declared a destination: {}", t.tail(40)));
    assert!(
        offered_dests.iter().all(|d| matches!(
            d,
            jinteki_cr::instr::InstallDest::Root(ServerId::Remote(_))
                | jinteki_cr::instr::InstallDest::NewRemoteRoot
        )),
        "4.6.8 + 4.6.6b: only the roots of remote servers are on offer, though an \
         upgrade could otherwise occupy every central root and protect any server: \
         {offered_dests:?}"
    );
    assert!(
        offered_dests.contains(&jinteki_cr::instr::InstallDest::NewRemoteRoot),
        "8.5.2a's brand-new remote is one of them: {offered_dests:?}"
    );

    assert_eq!(
        vm.st.objects[&upgrade].zone,
        Zone::Root(ServerId::Remote(1)),
        "the card from HQ went to the root the Corp declared: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&upgrade].counter(CounterKind::Advancement),
        1,
        "…and the same instruction placed the advancement counter on it: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.corp.credits, 6,
        "5 - 1 for the trigger cost, then two basic credit actions: the [click] and \
         the 1[credit] are the whole cost, and an upgrade has no install cost: {}",
        t.tail(40)
    );

    let options = offered_options(&t);
    assert!(
        !options.iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Rez { card } if *card == upgrade
        )),
        "1.2.2: the (R) option is never offered for that card, though its rez cost \
         is 0: {}",
        t.tail(40)
    );
    assert!(
        options.iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Rez { card } if *card == neighbour
        )),
        "and the asset in the same root IS offered, so it is the prohibition and not \
         the window: {}",
        t.tail(40)
    );

    // The middle of the duration, which is what makes it longer than "this
    // turn": the Runner runs the server the upgrade is in, the Corp gets the
    // (R) windows that run opens, and the upgrade is still not among what
    // they are offered.
    let t2 = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.turn_side, Side::Runner, "still the Runner's turn: {}", t2.tail(40));
    let during_the_run = offered_options(&t2);
    assert!(
        during_the_run.iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Rez { card } if *card == neighbour
        )),
        "the run opened (R) windows on that server: {}",
        t2.tail(40)
    );
    assert!(
        !during_the_run.iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Rez { card } if *card == upgrade
        )),
        "…and the prohibition reaches through the whole of the OPPONENT's turn, \
         which is what makes it longer than 'this turn': {}",
        t2.tail(40)
    );

    // The far end: the Corp's next turn begins and the effect is gone before
    // anything in that turn happens.
    let t3 = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's next turn came round: {}", t3.tail(40));
    assert!(
        offered_options(&t3).iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Rez { card } if *card == upgrade
        )),
        "…and the (R) option is back the moment that turn begins: {}",
        t3.tail(40)
    );
}

/// The score half of the same sentence, and the reason the counter matters:
/// a 1/1 agenda installed by this ability meets its advancement requirement
/// the instant the ability places the counter, and still cannot be scored
/// this turn.
#[test]
fn saraswati_mnemonics_withholds_the_score_until_her_next_turn() {
    let mut vm = Vm::empty(6212);
    tk::install_identity(&mut vm, card("Saraswati Mnemonics: Endless Exploration"), Side::Corp);
    let agenda = vm.new_object(tk::vanilla_agenda("Quick Agenda", 1, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().offering("endless exploration").once(), Reply::take("endless exploration"))
            .when(Match::targets().once(), Reply::Targets(vec![agenda]))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    let Zone::Root(remote) = vm.st.objects[&agenda].zone else {
        panic!("the agenda is in a server root: {}", t.tail(40))
    };
    assert!(matches!(remote, ServerId::Remote(_)), "a remote one: {}", t.tail(40));
    assert_eq!(
        vm.st.objects[&agenda].counter(CounterKind::Advancement),
        1,
        "1.18.2: the counter is PLACED, and it meets the 1-advancement requirement: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CardAdvanced { .. })),
        "…placed, not advanced, so nothing that reads an advancement is met: {}",
        t.tail(40)
    );
    assert!(
        !offered_options(&t).iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Score { card } if *card == agenda
        )),
        "1.2.2: the (S) option is withheld for the rest of the turn even though \
         1.17.3 would otherwise offer it: {}",
        t.tail(40)
    );

    let t2 = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's next turn came round: {}", t2.tail(40));
    assert!(
        offered_options(&t2).iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Score { card } if *card == agenda
        )),
        "…and 'until your next turn begins' has run out by the first window of it: {}",
        t2.tail(40)
    );
}

/// A Teia: "Limit 2 remote servers. The first time each turn you install a
/// card in the root of or protecting a remote server, you may install 1 card
/// from HQ in the root of or protecting another remote server, ignoring all
/// costs. You cannot score the second card this turn."
///
/// Both halves of the condition and both halves of the destination, in one
/// turn. The Corp's first install protects HQ — a central, so 4.6.8 puts it
/// outside the sentence and it neither fires the ability nor spends 9.6.5c's
/// ordinal. The second protects a remote and does both.
///
/// The destination is then the whole point: the second card is a piece of ice,
/// which could legally protect every server on the board, and exactly one
/// position is offered — the OTHER remote. The same remote is out because it
/// is not "another"; the three centrals are out because they are not remote;
/// and 8.5.2a's brand-new remote is out because the first printed line has
/// already been spent, which is what makes the two lines one card.
#[test]
fn a_teia_installs_into_another_remote_and_never_the_same_one() {
    let mut vm = Vm::empty(6222);
    tk::install_identity(&mut vm, card("A Teia: IP Recovery"), Side::Corp);
    // Two remotes, which is the limit — so nothing below can create a third.
    tk::install_root(&mut vm, tk::vanilla_asset("Neighbour A", 0, 2), ServerId::Remote(1), false);
    tk::install_root(&mut vm, tk::vanilla_asset("Neighbour B", 0, 2), ServerId::Remote(2), false);
    // One piece of ice already protecting the far remote, so 8.5.11a has a
    // credit to charge for the second install and "ignoring all costs" has
    // something to waive.
    tk::install_ice(&mut vm, tk::vanilla_ice("Guard", 0, 1), ServerId::Remote(2), false);
    let central_ice = vm.new_object(tk::vanilla_ice("Central Ice", 0, 1), Zone::Hand(Side::Corp));
    let remote_ice = vm.new_object(tk::vanilla_ice("Remote Ice", 0, 1), Zone::Hand(Side::Corp));
    let second = vm.new_object(tk::vanilla_ice("Second Card", 0, 1), Zone::Hand(Side::Corp));
    for id in [central_ice, remote_ice, second] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // (1) protecting HQ: an install, and not one the sentence reaches.
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(central_ice)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
            )
            // (2) protecting a remote: the first time each turn it happens.
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(remote_ice)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1))),
            )
            .when(Match::reaction().offering("ip recovery"), Reply::take("ip recovery"))
            .when(Match::targets().once(), Reply::Targets(vec![second]))
            .when(Match::of(Kind::Destination).once(), Reply::Default)
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    assert_eq!(
        vm.st.objects[&central_ice].zone,
        Zone::Ice(ServerId::Hq),
        "the first install went to a central: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&remote_ice].zone,
        Zone::Ice(ServerId::Remote(1)),
        "the second install protects a remote: {}",
        t.tail(60)
    );

    // 4.6.8: the central install is an install like any other, and the
    // ability is offered exactly once all turn — never for it.
    let offered_at: Vec<usize> = t
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.options().iter().any(|o| matches!(
                o,
                jinteki_cr::decision::WindowOption::TriggerInstance { label, .. }
                    if label.contains("ip recovery")
            ))
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        offered_at.len(),
        1,
        "one offer: the install protecting HQ is not one the sentence reaches, so it \
         neither fires the ability nor spends 9.6.5c's ordinal on it: {}",
        t.tail(60)
    );

    let dests: Vec<Vec<jinteki_cr::instr::InstallDest>> = t
        .of_kind(Kind::Destination)
        .into_iter()
        .map(|e| match &e.spec {
            jinteki_cr::decision::DecisionSpec::DeclareInstallDestination { options } => {
                options.clone()
            }
            other => panic!("a destination declaration, not {other:?}"),
        })
        .collect();
    // Three declarations in the order the installs happened: the central one,
    // the remote one, and the ability's. Had the central install fired the
    // ability, the restricted declaration would be the SECOND of the three.
    assert_eq!(dests.len(), 3, "three installs, three declarations: {}", t.tail(60));
    assert_eq!(
        dests[2],
        vec![jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(2))],
        "1.15.4 inverted: the ONE position on offer is the other remote's. The same \
         remote is not 'another'; the three centrals a piece of ice could otherwise \
         protect are not remote; and 4.6.8f's limit of 2 has already ruled out \
         8.5.2a's new one, which the first declaration shows was on offer: {}",
        t.tail(60)
    );
    assert!(
        dests[0].contains(&jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
        "…and the unrestricted declaration really did offer the centrals: {dests:?}"
    );
    assert_eq!(
        vm.st.objects[&second].zone,
        Zone::Ice(ServerId::Remote(2)),
        "the card from HQ went there: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.corp.credits, 6,
        "5 credits, one basic credit action, and nothing paid: 8.5.11a would have \
         charged 1[credit] for the piece of ice already protecting that remote, and \
         1.16.5c waives it: {}",
        t.tail(60)
    );
}

/// A Teia again, for the sentence the first test's ice cannot be asked about:
/// "You cannot score the second card this turn."
///
/// The card installed is an agenda whose advancement requirement is already
/// met, so 1.17.3 would offer the (S) option at the very next window. 1.2.2
/// withholds it for the rest of the turn — and, the span being "this turn"
/// rather than Saraswati's longer one, hands it back at the start of the
/// Corp's next.
#[test]
fn a_teia_withholds_the_score_of_the_second_card() {
    let mut vm = Vm::empty(6223);
    tk::install_identity(&mut vm, card("A Teia: IP Recovery"), Side::Corp);
    tk::install_root(&mut vm, tk::vanilla_asset("Neighbour A", 0, 2), ServerId::Remote(1), false);
    // The far remote exists because a piece of ice protects it (4.6.8d), so
    // its root is free for an agenda.
    tk::install_ice(&mut vm, tk::vanilla_ice("Guard", 0, 1), ServerId::Remote(2), false);
    let trigger = vm.new_object(tk::vanilla_ice("Trigger Ice", 0, 1), Zone::Hand(Side::Corp));
    let agenda = vm.new_object(tk::vanilla_agenda("Free Agenda", 0, 1), Zone::Hand(Side::Corp));
    for id in [trigger, agenda] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(trigger)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1))),
            )
            .when(Match::reaction().offering("ip recovery"), Reply::take("ip recovery"))
            .when(Match::targets().once(), Reply::Targets(vec![agenda]))
            .when(Match::of(Kind::Destination).once(), Reply::Default)
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::Root(ServerId::Remote(2)),
        "the agenda went to the root of the other remote: {}",
        t.tail(60)
    );
    assert!(
        !offered_options(&t).iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Score { card } if *card == agenda
        )),
        "1.2.2: the (S) option is withheld for the rest of the turn, though the \
         agenda needs no advancement at all: {}",
        t.tail(60)
    );

    let t2 = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's next turn came round: {}", t2.tail(40));
    assert!(
        offered_options(&t2).iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Score { card } if *card == agenda
        )),
        "…and 'this turn' ran out with the turn it named: {}",
        t2.tail(40)
    );
}

/// A Teia, third: 9.6.5c's ordinal is answered of the install as it HAPPENED.
///
/// "The first time each turn you install a card in the root of or protecting a
/// remote server" makes the ordinal's scan ask the whole condition again of
/// every earlier change this turn — and where the card went is a fact about
/// the moment of the install, not about where the card is when the scan runs.
/// Here the Corp installs an agenda into a remote root, declines the offer,
/// scores it, and installs again: the agenda is in the score area by then and
/// in no server at all, and the first install has still spent the turn's one
/// offer.
#[test]
fn a_teia_spends_the_turns_offer_on_an_install_whose_card_has_since_moved() {
    let mut vm = Vm::empty(6224);
    tk::install_identity(&mut vm, card("A Teia: IP Recovery"), Side::Corp);
    tk::install_root(&mut vm, tk::vanilla_asset("Neighbour A", 0, 2), ServerId::Remote(1), false);
    // 4.6.8d: the far remote exists because a piece of ice protects it, so its
    // root is free for the agenda.
    tk::install_ice(&mut vm, tk::vanilla_ice("Guard", 0, 1), ServerId::Remote(2), false);
    let agenda = vm.new_object(tk::vanilla_agenda("Free Agenda", 0, 1), Zone::Hand(Side::Corp));
    let later = vm.new_object(tk::vanilla_ice("Later Ice", 0, 1), Zone::Hand(Side::Corp));
    for id in [agenda, later] {
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(agenda)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Root(ServerId::Remote(2))),
            )
            // 9.6.9: declining resolves nothing, and spends no ordinal either
            // — the ordinal is stipulated on the CONDITION, which occurred.
            .when(Match::reaction().offering("ip recovery").once(), Reply::Pass)
            .when(Match::paid().offering_pick(Pick::Score(agenda)).once(), Reply::score(agenda))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(later)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1))),
            )
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Corp),
        "the agenda was installed into a remote root and then scored out of it: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&later].zone,
        Zone::Ice(ServerId::Remote(1)),
        "…and the second install protects a remote, so it meets the condition too: {}",
        t.tail(60)
    );
    let offers = t
        .entries
        .iter()
        .filter(|e| {
            e.options().iter().any(|o| matches!(
                o,
                jinteki_cr::decision::WindowOption::TriggerInstance { label, .. }
                    if label.contains("ip recovery")
            ))
        })
        .count();
    assert_eq!(
        offers, 1,
        "one offer all turn: the scored agenda is in no server now, and the install \
         that put it in one still counts as the first time — the record says where \
         it went: {}",
        t.tail(60)
    );
}

/// AU Co.: "Whenever you do damage or trash 1 or more cards from HQ, place 1
/// power counter on this identity."
///
/// Four occurrences, two counters. A 2-point net damage is one aggregated
/// occurrence (10.4.3), so one counter — and its own trashes leave the
/// RUNNER's grip, so the trash half never sees them. A Corp trash of TWO
/// cards out of HQ is 9.12.2a's plural: one event, one counter. The Runner
/// choosing to SUFFER damage is 10.4.1's other branch — the Runner is
/// responsible, so the Corp never "did" it — and a Runner card trashing an
/// HQ card is not "you trash" either: no counter for those.
#[test]
fn au_co_counts_its_own_damage_and_hq_trash_events_once_each() {
    let mut vm = Vm::empty(6225);
    let ident =
        tk::install_identity(&mut vm, card("AU Co.: The Gold Standard in Clones"), Side::Corp);
    tk::install_root(&mut vm, tk::net_damage_button("Hurt", 2), ServerId::Remote(1), true);
    let hq = tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Purge", vec![hq[0], hq[1]]),
        ServerId::Remote(2),
        true,
    );
    tk::install_rig(
        &mut vm,
        tk::suffer_damage_button("Ouch", jinteki_cr::effects::DamageKind::Net, 1),
    );
    tk::install_rig(&mut vm, tk::trash_set_button("Swipe", vec![hq[2]]));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("do net damage"))
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("suffer damage"))
            .when(Match::paid().once(), Reply::take("trash the set"))
            .stop_at_action(),
    );
    assert!(
        [hq[0], hq[1], hq[2]]
            .iter()
            .all(|c| vm.st.objects[c].zone == Zone::Discard(Side::Corp)),
        "all three HQ cards were trashed — two by the Corp's event, one by the Runner's: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "both damages happened — 2 net from the Corp, 1 the Runner suffered: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&ident].counters.get(&CounterKind::Power).copied().unwrap_or(0),
        2,
        "one counter for the damage the Corp DID (however many points), one for the \
         HQ trash EVENT (however many cards) — and none for the suffered damage or \
         the Runner's trash: {}",
        t.tail(60)
    );
}

/// AU Co.'s second line: "When your turn begins, you may remove 2 hosted
/// power counters to look at the top 3 cards of R&D. Trash 1 of those cards
/// and add the rest to HQ."
///
/// With 2 hosted counters the offer is made: paying looks at the top 3 of
/// R&D, the Corp's one announced choice is trashed, and "the rest" — the
/// looked-at cards the choice did not take — go to HQ with nobody asked
/// (1.15.2e leaves no choice over a count equal to the description). With
/// only 1 counter the 1.9.2 cost is unpayable and nothing is offered at all;
/// declining spends and moves nothing.
#[test]
fn au_co_spends_two_counters_at_turn_start_to_filter_the_top_of_rnd() {
    for (counters, accept) in [(2u32, true), (2, false), (1, true)] {
        let mut vm = Vm::empty(6226);
        let ident =
            tk::install_identity(&mut vm, card("AU Co.: The Gold Standard in Clones"), Side::Corp);
        vm.st.objects.get_mut(&ident).unwrap().counters.insert(CounterKind::Power, counters);
        let deck: Vec<ObjectId> = (0..5)
            .map(|_| vm.new_object(tk::vanilla_asset("Deck Asset", 0, 1), Zone::Deck(Side::Corp)))
            .collect();
        vm.st.deck.get_mut(&Side::Corp).unwrap().extend(deck.iter().copied());
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.start_turn(Side::Corp);

        // The turn-begins window opens before the mandatory draw, so the top
        // 3 of R&D are deck[0..3]; the trash takes deck[1] and "the rest" is
        // the other two.
        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::nested_cost(), Reply::PayCost(accept))
                .when(Match::targets().once(), Reply::Targets(vec![deck[1]]))
                .when(Match::targets().once(), Reply::Targets(vec![deck[0], deck[2]]))
                .stop_at_action(),
            Plan::runner(),
        );
        let spent = counters == 2 && accept;
        assert_eq!(
            vm.st.objects[&ident].counters.get(&CounterKind::Power).copied().unwrap_or(0),
            if spent { 0 } else { counters },
            "counters={counters} accept={accept}: 1.9.2's cost comes off the identity, \
             and an unpayable one is never offered: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&deck[1]].zone,
            if spent { Zone::Discard(Side::Corp) } else { Zone::Deck(Side::Corp) },
            "counters={counters} accept={accept}: the announced choice is the card \
             trashed: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.deck[&Side::Corp].len(),
            if spent { 1 } else { 4 },
            "counters={counters} accept={accept}: the mandatory draw takes one card, \
             and the ability takes the next three exactly when it was paid for: {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.hand[&Side::Corp].len(),
            if spent { 3 } else { 1 },
            "counters={counters} accept={accept}: the drawn card, plus 'the rest' — \
             the two looked-at cards the trash did not take: {}",
            t.tail(40)
        );
        let anns: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
        if spent {
            assert_eq!(
                anns.len(),
                2,
                "two announcements — the trash's choice of 1, then 'the rest': {}",
                t.tail(40)
            );
            assert_eq!(
                anns[0].candidates().len(),
                3,
                "the trash chooses among exactly the three looked-at cards: {}",
                t.tail(40)
            );
            assert_eq!(
                anns[1].candidates(),
                [deck[0], deck[2]],
                "'the rest' describes the looked-at cards the trash did not take: {}",
                t.tail(40)
            );
            assert!(
                matches!(
                    anns[1].spec,
                    jinteki_cr::decision::DecisionSpec::ChooseTargets {
                        count: 2,
                        up_to: false,
                        min: 2,
                        ..
                    }
                ),
                "…and its count is the description's own: 1.15.2e leaves nothing to \
                 reach for, so adding fewer than all of them is not an answer: {}",
                t.tail(40)
            );
        } else {
            assert!(
                anns.is_empty(),
                "counters={counters} accept={accept}: nothing was announced — the \
                 sentence's descriptions describe nothing unpaid-for: {}",
                t.tail(40)
            );
        }
    }
}

/// Earth Station, front face: "As an additional cost to run HQ, the Runner
/// must pay 1[credit]." — the toll is charged on HQ and on HQ only: a run on
/// Archives asks for nothing.
#[test]
fn earth_station_taxes_the_run_on_hq_and_only_hq() {
    let mut vm = Vm::empty(6300);
    tk::install_identity(&mut vm, card("Earth Station: SEA Headquarters"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            // 1.16.10a: pay the 1[credit] and the run is made.
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::RunBegan { .. })).count(),
        2,
        "both runs happened: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.runner.credits,
        4,
        "1[credit] for HQ and nothing for Archives — the declaration reaches \
         only the server the sentence names: {}",
        t.tail(20)
    );
}

/// Earth Station, front face, the other half of 1.16.10a: the Runner may
/// decline the additional cost, and then the action is not taken at all —
/// no run, and the [click] not spent either (1.16.4c's shape).
#[test]
fn earth_station_declined_toll_costs_neither_the_run_nor_the_click() {
    let mut vm = Vm::empty(6301);
    tk::install_identity(&mut vm, card("Earth Station: SEA Headquarters"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::nested_cost().once(), Reply::PayCost(false))
            .when(Match::action(), Reply::Halt),
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunBegan { .. })),
        "declined, so no run began: {}",
        t.tail(12)
    );
    assert_eq!(
        vm.st.runner.clicks, vm.st.runner.allotted_clicks,
        "…and the [click] was not spent: {}",
        t.tail(12)
    );
    assert_eq!(vm.st.runner.credits, 5, "…and no credit either");
}

/// Earth Station, front face: "Limit 1 remote server." (4.6.8f) — with one
/// remote up, a new remote is not a destination the Corp may declare, and an
/// install that names no other destination identifies none at all (8.5.14).
#[test]
fn earth_station_limits_the_corp_to_one_remote() {
    let mut vm = Vm::empty(6302);
    tk::install_identity(&mut vm, card("Earth Station: SEA Headquarters"), Side::Corp);
    assert!(vm.can_create_new_remote(), "no remote exists yet");
    tk::install_root(&mut vm, tk::vanilla_asset("First Asset", 0, 3), ServerId::Remote(1), false);
    assert!(
        !vm.can_create_new_remote(),
        "4.6.8f: one remote already exists, so the limit forbids creating another"
    );

    // The install effect still runs; its destination just cannot be
    // identified, so the card stays in HQ.
    let hand = vm.new_object(tk::vanilla_asset("Second Asset", 0, 3), Zone::Hand(Side::Corp));
    tk::install_root(&mut vm, tk::adt_button("Installer", hand), ServerId::Hq, true);
    vm.start_turn(Side::Corp);
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().first(), Reply::take("adt")).stop_at_action(),
        Plan::runner().stopping_at_the_rest(),
    );
    assert!(t.ever_offered("adt"), "the installing ability was used: {}", t.tail(12));
    assert_eq!(
        vm.st.objects[&hand].zone,
        Zone::Hand(Side::Corp),
        "8.5.14: no destination could be identified, so no installation took place"
    );
    assert_eq!(vm.remote_servers().len(), 1, "and no second remote server exists");
}

/// Earth Station, both faces: "[click]: Flip this identity." turns the back
/// face up; the back's "As an additional cost to run a remote server, the
/// Runner must pay 6[credit]" reaches 4.6.8's whole class and no central;
/// and "When the Runner makes a successful run on HQ, flip this identity."
/// turns the front face home again.
#[test]
fn earth_station_flips_for_a_click_and_the_back_taxes_remotes_until_hq_flips_it_home() {
    let mut vm = Vm::empty(6303);
    let id = tk::install_identity(&mut vm, card("Earth Station: SEA Headquarters"), Side::Corp);
    // The one remote the limit allows; its occupant is expensive to trash so
    // the access default is to pass.
    tk::install_root(&mut vm, tk::vanilla_asset("Orbital Asset", 0, 9), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 7;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::take("flip"))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            // The back face's toll on the remote: 6[credit], paid.
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            // HQ under the back face: no toll is asked at all — the next
            // decision after the run action is the action window again.
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action(), Reply::Halt),
    );
    assert!(
        vm.st.objects[&id].flipped.is_none(),
        "flipped up for the [click], home again on the successful HQ run: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        2,
        "exactly two flips: the paid ability's and the HQ run's: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.runner.credits,
        1,
        "7 − 6 for the remote, 0 for HQ — the back face's declaration names \
         the class of remotes and nothing central: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::RunBegan { .. })).count(),
        2,
        "both runs were made: {}",
        t.tail(30)
    );
}

// ---------------------------------------------------------------------------
// BANGUN: When Disaster Strikes
// ---------------------------------------------------------------------------

/// BANGUN, first sentence: "You may install agendas faceup." — the permission
/// puts the face to the INSTALLER as a Decision at step 8.5.16a, where 8.5.2
/// otherwise settles a Corp card facedown with nobody asked. Answered FACEUP:
/// the installed agenda sits faceup — and its abilities stay INACTIVE, which
/// is the printed parenthetical "(This does not make their abilities
/// active.)" restating 8.1.1 (a faceup agenda is neither rezzed nor unrezzed)
/// and 3.2.3 (an agenda is inactive while installed, however it faces).
#[test]
fn bangun_asks_the_installer_the_face_and_a_faceup_agenda_stays_inactive() {
    let mut vm = Vm::empty(6140);
    tk::install_identity(&mut vm, card("BANGUN: When Disaster Strikes"), Side::Corp);
    // An agenda carrying a paid ability ("+1 to imminent meat damage") that a
    // wrongly-ACTIVE faceup install would put into every paid window.
    let agenda = vm.new_object(tk::cleaners_like("Loud Agenda"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(agenda)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::install_face().once(), Reply::Optional(true))
            .stop_at_action(),
        Plan::runner(),
    );
    let asked = t.of_kind(Kind::InstallFace);
    assert_eq!(asked.len(), 1, "8.5.16a: the face was asked exactly once: {}", t.tail(20));
    assert_eq!(
        asked[0].side,
        Side::Corp,
        "…and put to the INSTALLER — the Corp installs its own cards (8.5.1): {}",
        t.tail(20)
    );
    let o = &vm.st.objects[&agenda];
    assert!(o.zone.is_installed(), "the install completed: {}", t.tail(20));
    assert!(o.faceup, "the declared status held: the agenda sits FACEUP: {}", t.tail(20));
    // 8.1.1 / 3.2.3: faceup, installed — and still inactive. `card_active` is
    // 1.8.3's own surface, and the paid window never offered the ability.
    assert!(
        !jinteki_cr::object::card_active(o),
        "3.2.3: an installed agenda is inactive however it faces: {}",
        t.tail(20)
    );
    assert!(
        t.entries.iter().all(|e| plan::count_labelled(e.options(), "cleaners") == 0),
        "no window ever offered the faceup agenda's ability: {}",
        t.tail(20)
    );
}

/// BANGUN, the permission DECLINED: the answer is 8.5.2's default and the
/// agenda goes in facedown — on which the second sentence's "faceup
/// installed" describes nothing: the Runner's access does no damage, gives no
/// tag, and the steal is the whole of what happens.
#[test]
fn bangun_declined_installs_facedown_and_that_access_triggers_nothing() {
    let mut vm = Vm::empty(6141);
    tk::install_identity(&mut vm, card("BANGUN: When Disaster Strikes"), Side::Corp);
    let agenda = vm.new_object(tk::vanilla_agenda("Quiet Agenda", 3, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::fill_hand(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(agenda)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::install_face().once(), Reply::Optional(false))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(100)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Runner),
        "the steal happened as on any facedown agenda: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.runner.tags, 0, "no tag — the card was not faceup: {}", t.tail(30));
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        4,
        "and no meat damage either: {}",
        t.tail(30)
    );
}

/// BANGUN, second sentence: "Whenever the Runner accesses a faceup installed
/// agenda, do 2 meat damage and give the Runner 1 tag." The damage is
/// 10.4.1's Corp-does branch and the tag is a tag; both land AFTER the access
/// in the log. The faceup agenda's own ability stays out of every window even
/// as the damage resolves — the Corp's plan asks for it and is never offered
/// it.
#[test]
fn bangun_faceup_agenda_access_costs_two_meat_and_a_tag() {
    let mut vm = Vm::empty(6142);
    tk::install_identity(&mut vm, card("BANGUN: When Disaster Strikes"), Side::Corp);
    let agenda = vm.new_object(tk::cleaners_like("Loud Agenda"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::fill_hand(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(agenda)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::install_face().once(), Reply::Optional(true))
            .always_uses("cleaners")
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(100)))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.tags, 1, "the tag landed: {}", t.tail(30));
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "2 meat damage — and not 3: the faceup agenda's '+1 meat' interrupt is \
         INACTIVE even while damage it would love is imminent: {}",
        t.tail(30)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::DamageSuffered { responsible: Side::Corp, amount: 2, .. }
        )),
        "10.4.1: 'do 2 meat damage' is the Corp-does branch: {}",
        t.tail(30)
    );
    let access = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == agenda))
        .expect("the agenda was accessed");
    let damage = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("the damage was suffered");
    let tag = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagsTaken { .. }))
        .expect("the tag was taken");
    assert!(
        access < damage && access < tag,
        "the sentence's effects follow the access that met it: {}",
        t.tail(30)
    );
    assert!(
        t.entries.iter().all(|e| plan::count_labelled(e.options(), "cleaners") == 0),
        "the plan asked for the agenda's ability at every window and was never \
         offered it: {}",
        t.tail(30)
    );
}

/// Hoshiko, front face: "When your turn ends, if you accessed a card this
/// turn, gain 2[credit] and flip this identity." — one sentence, so the gain
/// and the flip land together at the turn's end, and 9.6.5c reads the
/// requirement AT that occurrence: an R&D access earlier in the turn is what
/// makes it true.
#[test]
fn hoshiko_gains_2_and_flips_together_when_an_access_turn_ends() {
    let mut vm = Vm::empty(6400);
    let id = tk::install_identity(&mut vm, card("Hoshiko Shiro: Untold Protagonist"), Side::Runner);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Rnd)),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "flipped exactly once, at the turn's end: {}",
        t.tail(16)
    );
    assert!(vm.st.objects[&id].flipped.is_some(), "the back face is up");
    // 5 + 3 (remaining basic credits after the run's click) + 2 (Hoshiko).
    assert_eq!(
        vm.st.runner.credits,
        10,
        "the gain 2 arrived with the flip — one instruction, not two: {}",
        t.tail(16)
    );
}

/// Hoshiko, front face, the requirement FALSE: a turn spent clicking for
/// credits ends with no access in its history, so nothing fires — no gain,
/// no flip.
#[test]
fn hoshiko_stays_put_when_a_turn_without_access_ends() {
    let mut vm = Vm::empty(6401);
    let id = tk::install_identity(&mut vm, card("Hoshiko Shiro: Untold Protagonist"), Side::Runner);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::IdentityFlipped { .. })),
        "no access this turn, so no flip: {}",
        t.tail(16)
    );
    assert!(vm.st.objects[&id].flipped.is_none(), "the front face is still up");
    assert_eq!(vm.st.runner.credits, 9, "5 + 4 basic credits and nothing from Hoshiko");
}

/// Mahou Shoujo (the back face): "When your turn begins, draw 1 card and
/// lose 1[credit]." — mandatory, and one sentence: the draw and the loss
/// arrive together at the turn's begin.
#[test]
fn hoshiko_back_draws_and_loses_1_when_the_turn_begins() {
    let mut vm = Vm::empty(6402);
    let id = tk::install_identity(&mut vm, card("Hoshiko Shiro: Untold Protagonist"), Side::Runner);
    // Setup state: the identity begins on its back face (as after an access
    // turn) — placement, not effect-by-fiat.
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 1, "drew 1 at turn begin: {}", t.tail(12));
    assert_eq!(vm.st.runner.credits, 4, "…and lost 1[credit] with it: {}", t.tail(12));
}

/// Mahou Shoujo at 0[credit]: CR 1.10.3b — a forced loss moves as many
/// credits as the pool holds and no more, so the Runner at 0 loses nothing
/// and the mandatory draw still happens.
#[test]
fn hoshiko_back_at_zero_credits_still_draws_and_loses_nothing() {
    let mut vm = Vm::empty(6403);
    let id = tk::install_identity(&mut vm, card("Hoshiko Shiro: Untold Protagonist"), Side::Runner);
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        1,
        "the draw is not hostage to the loss: {}",
        t.tail(12)
    );
    assert_eq!(vm.st.runner.credits, 0, "1.10.3b: a pool of 0 loses 0");
}

/// Hoshiko, the whole round trip: an access turn ends and she flips out
/// (gaining 2); the next turn opens with Mahou Shoujo's draw-and-lose, is
/// spent NOT accessing, and its end flips her home — "if you did not access
/// any cards this turn" is the front's question with the answer the back
/// wants.
#[test]
fn hoshiko_round_trip_flips_out_on_access_and_home_on_none() {
    let mut vm = Vm::empty(6404);
    let id = tk::install_identity(&mut vm, card("Hoshiko Shiro: Untold Protagonist"), Side::Runner);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        // The Corp's whole turn between the Runner's two: three basic
        // credits, then a halt when the Runner's second turn has ended.
        Plan::corp()
            .when(Match::action().times(3), Reply::credit())
            .when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action(), Reply::credit()),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        2,
        "flipped out at the first turn's end and home at the second's: {}",
        t.tail(30)
    );
    assert!(vm.st.objects[&id].flipped.is_none(), "the front face is up again");
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        1,
        "Mahou Shoujo's morning drew exactly once — the front has no such line: {}",
        t.tail(30)
    );
}

/// BANGUN asks nothing about a non-agenda install: the permission's criteria
/// describe agendas, so an asset's face is settled by 8.5.2 as ever — no
/// decision exists, and the asset goes in facedown.
#[test]
fn bangun_puts_no_face_question_to_a_non_agenda_install() {
    let mut vm = Vm::empty(6143);
    tk::install_identity(&mut vm, card("BANGUN: When Disaster Strikes"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("Some Asset", 0, 3), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(asset);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(
        t.of_kind(Kind::InstallFace).is_empty(),
        "the permission describes agendas and an asset is not one — 8.5.2 \
         settles the face with nobody asked: {}",
        t.tail(20)
    );
    let o = &vm.st.objects[&asset];
    assert!(o.zone.is_installed(), "the install completed: {}", t.tail(20));
    assert!(!o.faceup, "…facedown, as 8.5.2 says: {}", t.tail(20));
}

/// Stealing is access-driven and status-independent: the FACEUP agenda the
/// Runner accesses is still stolen (7.3.4), and the steal completes AFTER the
/// damage and the tag — BANGUN's instances resolve off the access before the
/// steal step reaches the card.
#[test]
fn bangun_faceup_agenda_is_still_stolen_after_the_damage_and_tag() {
    let mut vm = Vm::empty(6144);
    tk::install_identity(&mut vm, card("BANGUN: When Disaster Strikes"), Side::Corp);
    let agenda = vm.new_object(tk::vanilla_agenda("Prize", 3, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::fill_hand(&mut vm, Side::Runner, 4);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(agenda)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::install_face().once(), Reply::Optional(true))
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(100)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Runner),
        "the faceup agenda was stolen: {}",
        t.tail(30)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::AgendaStolen { obj, points: 2 } if *obj == agenda
        )),
        "and scored its printed points: {}",
        t.tail(30)
    );
    let damage = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("the damage was suffered");
    let tag = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::TagsTaken { .. }))
        .expect("the tag was taken");
    let stolen = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::AgendaStolen { obj, .. } if *obj == agenda))
        .expect("the agenda was stolen");
    assert!(
        damage < stolen && tag < stolen,
        "the steal completed after the damage and the tag: {}",
        t.tail(30)
    );
}

/// Acme Consulting: "The Runner is considered to have 1 additional tag (even
/// if they have 0) during encounters with the outermost piece of ice
/// protecting any server."
///
/// The qualifying half: with 0 real tags, an encounter with the OUTERMOST of
/// a two-ice server makes every modified-count reader see 1. The probe asset
/// is both readers at once — its "whenever an encounter begins" condition
/// requires the Runner be tagged (`RunnerTagsAtLeast`), and its effect pays
/// the Corp 1[credit] per `Quantity::RunnerTags` — so the outer encounter
/// pays exactly 1 and the inner encounter of the SAME run pays nothing. The
/// real count never moves: no tag was taken, so 5.2.6e's remove-tag action
/// (which reads the real count — a tag nobody has cannot be removed) never
/// has anything to remove. That action's availability gate is private to the
/// VM, so the assertion is on what it reads: the real count, still 0.
#[test]
fn acme_counts_one_considered_tag_during_the_outermost_encounter_only() {
    let mut vm = Vm::empty(6227);
    tk::install_identity(&mut vm, card("Acme Consulting: The Truth You Need"), Side::Corp);
    let _probe =
        tk::install_root(&mut vm, tk::considered_tag_probe("Probe", 1), ServerId::Remote(2), true);
    let _inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 0), ServerId::Remote(1), true);
    let _outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 0), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    let corp0 = vm.st.corp.credits;

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.corp.credits,
        corp0 + 1,
        "the outer encounter fired the tagged-gated probe for exactly 1 (the considered \
         count), and the inner encounter of the same run for nothing: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.runner.tags, 0,
        "no tag was ever taken — the declaration modifies the NUMBER a reader sees, so \
         5.2.6e's remove-tag action (a real-count reader) never had anything to remove: {}",
        t.tail(50)
    );
}

/// Acme Consulting, the inner half: an encounter with a piece of ice that is
/// NOT the outermost of its server counts nothing. The outer ice stays
/// unrezzed — an unrezzed piece of ice still occupies the outermost position
/// (6.2.1 positions do not care about rez status), so the encountered inner
/// ice is not "the outermost piece of ice protecting its server" and the
/// probe's tagged requirement reads 0.
#[test]
fn acme_counts_nothing_during_an_inner_encounter() {
    let mut vm = Vm::empty(6228);
    tk::install_identity(&mut vm, card("Acme Consulting: The Truth You Need"), Side::Corp);
    let _probe =
        tk::install_root(&mut vm, tk::considered_tag_probe("Probe", 1), ServerId::Remote(2), true);
    let _inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 0), ServerId::Remote(1), true);
    let _outer =
        tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 0), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    let corp0 = vm.st.corp.credits;

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.corp.credits, corp0,
        "the only encounter was with the inner ice, which is not the outermost of its \
         server, so no reader ever saw a tag: {}",
        t.tail(50)
    );
    assert_eq!(vm.st.runner.tags, 0, "and the real count never moved: {}", t.tail(50));
}

/// Acme Consulting, outside any encounter: the declaration's 9.3.7a stated
/// condition ("during encounters with…") does not hold, so the considered
/// count IS the real count — 0 with no tags, and exactly the real number
/// with some, because the modification lapses rather than lingers.
#[test]
fn acme_counts_nothing_outside_an_encounter() {
    let mut vm = Vm::empty(6229);
    tk::install_identity(&mut vm, card("Acme Consulting: The Truth You Need"), Side::Corp);
    let _outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 0), ServerId::Remote(1), true);

    assert_eq!(
        vm.considered_runner_tags(),
        0,
        "outermost ice exists, but no encounter is in progress, so the stated condition \
         does not hold and nothing is added"
    );
    vm.st.runner.tags = 2;
    assert_eq!(
        vm.considered_runner_tags(),
        2,
        "with real tags and no encounter the considered count is exactly the real one"
    );
}

/// Acme Consulting on top of a real tag: during the qualifying encounter the
/// modified-count readers see 2 (the probe requires at least 2 and pays
/// `Quantity::RunnerTags`, so it fires for exactly 2[credit]) — and removing
/// a tag as a COST still works on the real one: the shedder's "remove 1 tag"
/// takes the real tag, leaving 0, while the considered count during the rest
/// of the encounter is still 1. A cost can never take the considered tag:
/// "(even if they have 0)" is the printed way of saying it is not there.
#[test]
fn acme_stacks_on_a_real_tag_and_a_cost_still_removes_only_the_real_one() {
    let mut vm = Vm::empty(6230);
    tk::install_identity(&mut vm, card("Acme Consulting: The Truth You Need"), Side::Corp);
    let _probe =
        tk::install_root(&mut vm, tk::considered_tag_probe("Probe", 2), ServerId::Remote(2), true);
    let _inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 0), ServerId::Remote(1), true);
    let _outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 0), ServerId::Remote(1), true);
    let _shed = tk::install_rig(&mut vm, tk::tag_shedder("Shed"));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.start_turn(Side::Runner);
    let corp0 = vm.st.corp.credits;
    let runner0 = vm.st.runner.credits;

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(
                Match::paid().during(StructKind::Encounter).once(),
                Reply::take("tag shedder"),
            )
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.corp.credits,
        corp0 + 2,
        "during the outer encounter the readers saw 1 real + 1 considered = 2 — the probe's \
         at-least-2 requirement was met and Quantity::RunnerTags paid 2: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.runner.tags, 0,
        "the shedder's remove-a-tag cost took the REAL tag — the considered one cannot be \
         removed, so nothing is left: {}",
        t.tail(50)
    );
    assert_eq!(
        vm.st.runner.credits,
        runner0 + 1,
        "and the shedder's effect paid out, proving the cost was payable with 1 real tag \
         even though no reader of the considered count is involved in paying it: {}",
        t.tail(50)
    );
}

// ---------------------------------------------------------------------------
// Ob Superheavy Logistics: Extract. Export. Excel.
// ---------------------------------------------------------------------------

/// Ob Superheavy Logistics: "Once per turn → When you trash a rezzed card,
/// except during installation, you may search R&D for 1 card with a printed
/// rez cost exactly 1[credit] less than the trashed card's printed rez cost.
/// Install and rez the card you found, ignoring credit costs."
///
/// The happy path, with the Corp at 0[credit] the whole way: the Corp trashes
/// its own rezzed 3-cost asset, the offer comes, the search finds exactly the
/// 2-cost asset — not the same-cost asset beside it, and not the operation
/// whose printed cost is also 2, because an operation has no rez cost to
/// stand in the relation — and the found card lands installed AND rezzed with
/// the credit pool untouched, which only "ignoring credit costs" allows.
#[test]
fn ob_finds_installs_and_rezzes_a_card_exactly_one_credit_cheaper() {
    let mut vm = Vm::empty(6230);
    tk::install_identity(
        &mut vm,
        card("Ob Superheavy Logistics: Extract. Export. Excel."),
        Side::Corp,
    );
    let pricey =
        tk::install_root(&mut vm, tk::vanilla_asset("Pricey Asset", 3, 2), ServerId::Remote(1), true);
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Trigger Button", vec![pricey]),
        ServerId::Remote(2),
        true,
    );
    // The filler goes on TOP (drawn first is pushed first), so the Corp's
    // mandatory draw never takes a probe card.
    tk::fill_deck(&mut vm, Side::Corp, 3);
    let bargain = vm.new_object(tk::vanilla_asset("Bargain Asset", 2, 2), Zone::Deck(Side::Corp));
    let same_cost = vm.new_object(tk::vanilla_asset("Same Cost Asset", 3, 2), Zone::Deck(Side::Corp));
    let mut op = PrintedCard::vanilla("Cheap Operation", Side::Corp, CardType::Operation);
    op.cost = Some(2);
    let op = vm.new_object(op, Zone::Deck(Side::Corp));
    for id in [bargain, same_cost, op] {
        vm.st.deck.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .when(
                Match::reaction().offering("extract export excel"),
                Reply::take("extract export excel"),
            )
            .when(Match::targets().once(), Reply::Targets(vec![bargain]))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .stop_at_action(),
        Plan::runner(),
    );

    // The trash-moment facts were recorded with the trash itself.
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardTrashed { obj, was_rezzed: true, during_install: false, .. }
                if *obj == pricey
        )),
        "8.1.2: the record kept that the trashed card WAS rezzed, outside any install: {}",
        t.tail(40)
    );
    // 8.7.2a: the relational criterion picked out exactly the 1-cheaper card.
    let finds: Vec<_> = t.of_kind(Kind::Targets).into_iter().collect();
    assert_eq!(finds.len(), 1, "one search find was put to the Corp: {}", t.tail(40));
    assert!(finds[0].candidates().contains(&bargain), "the 2-cost asset is a candidate: {}", t.tail(40));
    assert!(
        !finds[0].candidates().contains(&same_cost),
        "'exactly 1[credit] less' keeps the same-cost asset out: {}",
        t.tail(40)
    );
    assert!(
        !finds[0].candidates().contains(&op),
        "an operation has no rez cost, so its printed cost of 2 stands in no relation: {}",
        t.tail(40)
    );
    // 8.7.3: searching R&D shuffled it.
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::DeckShuffled { side: Side::Corp })),
        "the search reshuffled R&D: {}",
        t.tail(40)
    );
    // The found card landed installed AND rezzed…
    assert!(
        matches!(vm.st.objects[&bargain].zone, Zone::Root(ServerId::Remote(_))),
        "the found card was installed into the declared root: {}",
        t.tail(40)
    );
    assert!(vm.st.objects[&bargain].faceup, "…and rezzed: {}", t.tail(40));
    // …with the Corp's credits unchanged: the 2[credit] rez cost (and any
    // install cost) was a credit cost, and the card ignores those.
    assert_eq!(
        vm.st.corp.credits, 0,
        "'ignoring credit costs': nothing was paid from a pool that had nothing: {}",
        t.tail(40)
    );
}

/// The condition's first stipulation: the trashed card must have been REZZED
/// at the moment of the trash. The Corp trashes its own UNREZZED installed
/// asset — `installed_only` would have been met, but 8.1.2's "rezzed" is
/// stricter, so no offer comes and R&D is never searched.
#[test]
fn ob_makes_no_offer_when_the_trashed_card_was_unrezzed() {
    let mut vm = Vm::empty(6231);
    tk::install_identity(
        &mut vm,
        card("Ob Superheavy Logistics: Extract. Export. Excel."),
        Side::Corp,
    );
    let hidden =
        tk::install_root(&mut vm, tk::vanilla_asset("Hidden Asset", 3, 2), ServerId::Remote(1), false);
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Trigger Button", vec![hidden]),
        ServerId::Remote(2),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 4);
    let bargain = vm.new_object(tk::vanilla_asset("Bargain Asset", 2, 2), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(bargain);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&hidden].zone,
        Zone::Discard(Side::Corp),
        "the unrezzed asset was trashed: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardTrashed { obj, was_rezzed: false, .. } if *obj == hidden
        )),
        "8.1.2: the record kept that it was NOT rezzed: {}",
        t.tail(40)
    );
    assert!(
        !t.ever_offered("extract export excel"),
        "no offer for an unrezzed trash: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&bargain].zone,
        Zone::Deck(Side::Corp),
        "and R&D was never searched: {}",
        t.tail(40)
    );
}

/// The condition's second stipulation: "except during installation" is
/// 8.5.11a's like-card trash, the one the install procedure itself performs.
/// The Corp installs an asset into a root that already holds a REZZED asset —
/// the old one is trashed by step 8.5.16c, `was_rezzed` is true and would
/// otherwise qualify, and the record's `during_install` is what keeps the
/// offer from coming.
#[test]
fn ob_makes_no_offer_for_the_like_card_trash_of_an_install() {
    let mut vm = Vm::empty(6232);
    tk::install_identity(
        &mut vm,
        card("Ob Superheavy Logistics: Extract. Export. Excel."),
        Side::Corp,
    );
    let old =
        tk::install_root(&mut vm, tk::vanilla_asset("Old Asset", 3, 2), ServerId::Remote(1), true);
    let newcomer = vm.new_object(tk::vanilla_asset("New Asset", 0, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(newcomer);
    // The installing ability fixes the destination, so step 8.5.16c has a
    // like card to trash when the new asset is placed.
    tk::install_root(
        &mut vm,
        tk::corp_install_button(
            "Slotter",
            newcomer,
            jinteki_cr::instr::InstallDest::Root(ServerId::Remote(1)),
        ),
        ServerId::Remote(2),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 4);
    let bargain = vm.new_object(tk::vanilla_asset("Bargain Asset", 2, 2), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(bargain);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-install: fixed card"))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&old].zone,
        Zone::Discard(Side::Corp),
        "8.5.16c trashed the like card: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&newcomer].zone,
        Zone::Root(ServerId::Remote(1)),
        "and the new asset took the root: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardTrashed { obj, was_rezzed: true, during_install: true, .. }
                if *obj == old
        )),
        "the record kept both facts: rezzed, and during an installation: {}",
        t.tail(40)
    );
    assert!(
        !t.ever_offered("extract export excel"),
        "'except during installation' kept the offer from coming: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&bargain].zone,
        Zone::Deck(Side::Corp),
        "and R&D was never searched: {}",
        t.tail(40)
    );
}

/// "Once per turn →" is 9.3.6g's flag, spent by using the ability. One
/// instruction trashes TWO rezzed 3-cost assets — 9.6.4b meets the singular
/// condition once per card, so two occurrences are pending — and only one
/// offer comes: taking the first spends the flag and the second occurrence
/// finds it spent. The second 2-cost card stays in R&D.
#[test]
fn ob_offers_only_once_per_turn_however_many_rezzed_trashes_qualify() {
    let mut vm = Vm::empty(6233);
    tk::install_identity(
        &mut vm,
        card("Ob Superheavy Logistics: Extract. Export. Excel."),
        Side::Corp,
    );
    let first =
        tk::install_root(&mut vm, tk::vanilla_asset("First Asset", 3, 2), ServerId::Remote(1), true);
    let second =
        tk::install_root(&mut vm, tk::vanilla_asset("Second Asset", 3, 2), ServerId::Remote(2), true);
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Trigger Button", vec![first, second]),
        ServerId::Remote(3),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 3);
    let bargain_a = vm.new_object(tk::vanilla_asset("Bargain A", 2, 2), Zone::Deck(Side::Corp));
    let bargain_b = vm.new_object(tk::vanilla_asset("Bargain B", 2, 2), Zone::Deck(Side::Corp));
    for id in [bargain_a, bargain_b] {
        vm.st.deck.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .when(
                Match::reaction().offering("extract export excel"),
                Reply::take("extract export excel"),
            )
            .when(Match::targets().once(), Reply::Targets(vec![bargain_a]))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .stop_at_action(),
        Plan::runner(),
    );

    assert!(
        vm.st.objects[&first].zone == Zone::Discard(Side::Corp)
            && vm.st.objects[&second].zone == Zone::Discard(Side::Corp),
        "both rezzed assets were trashed: {}",
        t.tail(40)
    );
    assert_eq!(
        t.offers("extract export excel"),
        1,
        "9.3.6g: the flag was spent by the first use, so the second qualifying \
         trash brought no second offer: {}",
        t.tail(40)
    );
    assert!(
        matches!(vm.st.objects[&bargain_a].zone, Zone::Root(ServerId::Remote(_)))
            && vm.st.objects[&bargain_a].faceup,
        "the one found card was installed and rezzed: {}",
        t.tail(40)
    );
    assert!(
        !matches!(vm.st.objects[&bargain_b].zone, Zone::Root(_)),
        "and the other 2-cost card was never searched out and installed — the 8.7.3 \
         shuffle may have put it anywhere in R&D, and the mandatory draw may then \
         have drawn it, but no second search reached it: {}",
        t.tail(40)
    );
}

/// 8.7.2e lets a criteria search of a deck fail — and with NO card of the
/// right printed rez cost in R&D there is nothing to put to the Corp at all:
/// the search completes empty, 8.7.3 still shuffles R&D, and nothing is
/// installed. The 2-cost OPERATION in R&D is the boundary: its printed cost
/// is the right number, but an operation has no rez cost to compare.
#[test]
fn ob_search_completes_empty_when_no_rez_cost_matches() {
    let mut vm = Vm::empty(6234);
    tk::install_identity(
        &mut vm,
        card("Ob Superheavy Logistics: Extract. Export. Excel."),
        Side::Corp,
    );
    let pricey =
        tk::install_root(&mut vm, tk::vanilla_asset("Pricey Asset", 3, 2), ServerId::Remote(1), true);
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Trigger Button", vec![pricey]),
        ServerId::Remote(2),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 3);
    let too_cheap = vm.new_object(tk::vanilla_asset("Too Cheap Asset", 1, 2), Zone::Deck(Side::Corp));
    let same_cost = vm.new_object(tk::vanilla_asset("Same Cost Asset", 3, 2), Zone::Deck(Side::Corp));
    let mut op = PrintedCard::vanilla("Cheap Operation", Side::Corp, CardType::Operation);
    op.cost = Some(2);
    let op = vm.new_object(op, Zone::Deck(Side::Corp));
    for id in [too_cheap, same_cost, op] {
        vm.st.deck.get_mut(&Side::Corp).unwrap().push(id);
    }
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .when(
                Match::reaction().offering("extract export excel"),
                Reply::take("extract export excel"),
            )
            .stop_at_action(),
        Plan::runner(),
    );

    assert!(t.took("extract export excel"), "the offer came and was taken: {}", t.tail(40));
    assert!(
        t.of_kind(Kind::Targets).is_empty(),
        "no find was put to the Corp — no card stands in the relation: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::DeckShuffled { side: Side::Corp })),
        "8.7.3: R&D was still shuffled: {}",
        t.tail(40)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardInstalled { .. })),
        "nothing was installed at all — the 8.7.3 shuffle may have moved the R&D \
         cards around, but none of them reached the play area: {}",
        t.tail(40)
    );
    for (id, name) in [(too_cheap, "1-cost asset"), (same_cost, "3-cost asset"), (op, "operation")] {
        assert!(
            !matches!(vm.st.objects[&id].zone, Zone::Root(_)),
            "the {name} was not searched out of R&D: {}",
            t.tail(40)
        );
    }
}

// ---------------------------------------------------------------------------
// Skorpios Defense Systems: Persuasive Power
// ---------------------------------------------------------------------------

/// Skorpios Defense Systems: "[interrupt] → Whenever 1 or more Runner cards
/// would be trashed (from any location), set those cards aside instead of
/// adding them to the heap. You can look at those cards. You may remove 1 of
/// them from the game. Then, add all of those cards that are still set aside
/// to the heap. …"
///
/// The single-card case, end to end: a Corp effect trashes one installed
/// Runner resource, the replacement holds it out of the heap, and the Corp's
/// one allowed removal takes it — so the heap never receives it, while 8.2.2
/// keeps the trash an occurrence of trashing (the record is there for any
/// condition that asks).
#[test]
fn skorpios_sets_a_trashed_runner_card_aside_and_removes_it_from_the_game() {
    let mut vm = Vm::empty(6230);
    tk::install_identity(&mut vm, card("Skorpios Defense Systems: Persuasive Power"), Side::Corp);
    let r1 = tk::install_rig(
        &mut vm,
        tk::vanilla_runner_card("Sable Dossier", CardType::Resource),
    );
    tk::install_root(&mut vm, tk::corp_trash_button("Zap", vec![r1]), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .when(Match::targets().once(), Reply::Targets(vec![r1]))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&r1].zone,
        Zone::RemovedFromGame,
        "§4.9: the Corp removed the set-aside card from the game: {}",
        t.tail(40)
    );
    assert!(
        vm.st.discard[&Side::Runner].is_empty(),
        "…so the heap never received it — the movement's replacement held: {}",
        t.tail(40)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CardTrashed { obj, .. } if *obj == r1)),
        "8.2.2: the replaced trash is still an occurrence of trashing, and it is \
         recorded as one: {}",
        t.tail(40)
    );
    assert!(
        vm.st.objects.values().all(|o| o.zone != Zone::SetAside),
        "4.8: the set-aside zone is a temporary holding space, and the ability \
         finished with it: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::Targets).len(),
        1,
        "one interception, one removal announcement: {}",
        t.tail(40)
    );
}

/// "Whenever **1 or more** Runner cards would be trashed …, set **those
/// cards** aside instead …" — a 2-point net damage trashes its cards
/// simultaneously (10.4.3), so the sentence is met ONCE by the pair: one
/// group set aside, one removal offered over it, and the survivor lands in
/// the heap when the ability finishes. And while the group is set aside it is
/// FACEUP (4.8.6 — the ability says nothing about facedown), so both players
/// see it: "You can look at those cards" costs the Runner nothing they were
/// keeping, every card here being bound for the open heap (4.4.4).
#[test]
fn skorpios_intercepts_a_multi_card_damage_trash_once_as_one_group() {
    let mut vm = Vm::empty(6231);
    tk::install_identity(&mut vm, card("Skorpios Defense Systems: Persuasive Power"), Side::Corp);
    tk::install_root(&mut vm, tk::net_damage_button("Hurt", 2), ServerId::Remote(1), true);
    let a = vm.new_object(copy_card("Doppelgänger"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(a);
    let b = vm.new_object(copy_card("Zamboni"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(b);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    // Halt at the removal announcement: the group is set aside right now.
    let t0 = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("do net damage"))
            .when(Match::targets(), Reply::Halt),
        Plan::runner(),
    );
    for (label, id) in [("the first", a), ("the second", b)] {
        assert_eq!(
            vm.st.objects[&id].zone,
            Zone::SetAside,
            "{label} card of the pair is set aside while the ability resolves: {}",
            t0.tail(40)
        );
        for side in [Side::Corp, Side::Runner] {
            assert!(
                vm.identity_visible_to(id, side),
                "4.8.6: the group is faceup, so {side:?} sees {label} card: {}",
                t0.tail(40)
            );
        }
    }

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets().once(), Reply::Targets(vec![a]))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&a].zone,
        Zone::RemovedFromGame,
        "the Corp removed 1 of them — one card, out of the whole group: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&b].zone,
        Zone::Discard(Side::Runner),
        "…then all of those cards that are still set aside go to the heap: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::Targets).len(),
        1,
        "10.4.3 trashes the pair simultaneously, so the sentence is met once and \
         the removal is announced once — not once per card: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::DamageSuffered { cards, .. }
                if cards.contains(&a) && cards.contains(&b)
        )),
        "8.2.2: the damage still trashed both cards, in one occurrence: {}",
        t.tail(40)
    );
    assert!(
        vm.st.objects.values().all(|o| o.zone != Zone::SetAside),
        "and the set-aside zone is empty again: {}",
        t.tail(40)
    );
}

/// "You **may** remove 1 of them from the game." — 1.15.2e's "up to" makes
/// zero choosable, and a Corp who declines removes nothing: the whole group
/// completes the movement into the heap together.
#[test]
fn skorpios_declining_the_removal_sends_the_whole_group_to_the_heap() {
    let mut vm = Vm::empty(6232);
    tk::install_identity(&mut vm, card("Skorpios Defense Systems: Persuasive Power"), Side::Corp);
    tk::install_root(&mut vm, tk::net_damage_button("Hurt", 2), ServerId::Remote(1), true);
    let a = vm.new_object(copy_card("Doppelgänger"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(a);
    let b = vm.new_object(copy_card("Zamboni"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(b);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("do net damage"))
            .when(Match::targets().once(), Reply::Targets(Vec::new()))
            .stop_at_action(),
        Plan::runner(),
    );

    for (label, id) in [("the first", a), ("the second", b)] {
        assert_eq!(
            vm.st.objects[&id].zone,
            Zone::Discard(Side::Runner),
            "{label} card reached the heap — nothing was removed: {}",
            t.tail(40)
        );
    }
    assert!(
        vm.st.objects.values().all(|o| o.zone != Zone::RemovedFromGame),
        "declining is choosing none, so no card left the game: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::Targets).len(),
        1,
        "the choice was still offered — declining spends nothing: {}",
        t.tail(40)
    );
}

/// "Ignore this ability if you have already removed a card from the game with
/// it this turn." — spent by the REMOVAL: after the Corp removes a card, a
/// second trash the same turn passes by untouched, straight to the heap with
/// nothing set aside and nothing asked; the turn ending hands the ability
/// back.
#[test]
fn skorpios_is_ignored_for_the_rest_of_the_turn_once_it_removed_a_card() {
    let mut vm = Vm::empty(6233);
    tk::install_identity(&mut vm, card("Skorpios Defense Systems: Persuasive Power"), Side::Corp);
    let r1 = tk::install_rig(
        &mut vm,
        tk::vanilla_runner_card("Sable Dossier", CardType::Resource),
    );
    tk::install_root(&mut vm, tk::corp_trash_button("Zap", vec![r1]), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::net_damage_button("Hurt", 1), ServerId::Remote(2), true);
    let grip = tk::fill_hand(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    // Corp turn 1: trash the resource and remove it; then the damage trash
    // the same turn is ignored — its card goes straight to the heap.
    let t1 = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .when(Match::targets().once(), Reply::Targets(vec![r1]))
            .when(Match::paid().once(), Reply::take("do net damage"))
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    assert_eq!(
        vm.st.objects[&r1].zone,
        Zone::RemovedFromGame,
        "the first trash of the turn was intercepted and its card removed: {}",
        t1.tail(40)
    );
    assert_eq!(
        vm.st.discard[&Side::Runner].len(),
        1,
        "the second trash of the same turn passed by untouched — one grip card, \
         straight to the heap: {}",
        t1.tail(40)
    );
    assert_eq!(
        t1.of_kind(Kind::Targets).len(),
        1,
        "…and nothing was asked about it: one removal announcement all turn: {}",
        t1.tail(40)
    );
    assert!(
        vm.st.objects.values().all(|o| o.zone != Zone::SetAside),
        "nothing stayed set aside either — an ignored ability sets nothing aside: {}",
        t1.tail(40)
    );

    // The Runner's turn passes; the Corp's next turn begins, and the ability
    // is back: the same damage is intercepted and a second removal made.
    let t2 = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("do net damage"))
            .when(Match::targets().once(), Reply::Targets(grip.clone()))
            .stop_at_action(),
        Plan::runner().otherwise_click_credit(),
    );

    assert_eq!(vm.st.turn_side, Side::Corp, "the Corp's next turn came round: {}", t2.tail(40));
    let removed_fillers =
        grip.iter().filter(|g| vm.st.objects[g].zone == Zone::RemovedFromGame).count();
    assert_eq!(
        removed_fillers, 1,
        "a new turn, a new removal — the ignore ran out with the turn: {}",
        t2.tail(40)
    );
    assert_eq!(
        vm.st.discard[&Side::Runner].len(),
        1,
        "…and the heap still holds only the card the IGNORED trash sent there: {}",
        t2.tail(40)
    );
    assert_eq!(
        t2.of_kind(Kind::Targets).len(),
        1,
        "one interception this turn, one announcement: {}",
        t2.tail(40)
    );
}

/// "Whenever 1 or more **Runner** cards would be trashed…" — a Corp card's
/// trash is not described, so it is not replaced: the asset goes to Archives
/// the ordinary way, with nothing set aside and nothing asked.
#[test]
fn skorpios_leaves_a_corp_card_trash_untouched() {
    let mut vm = Vm::empty(6234);
    tk::install_identity(&mut vm, card("Skorpios Defense Systems: Persuasive Power"), Side::Corp);
    let asset = tk::install_root(
        &mut vm,
        tk::vanilla_asset("Warm Reception", 0, 3),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Dump", vec![asset]),
        ServerId::Remote(2),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-trash: trash the set"))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Discard(Side::Corp),
        "1.19: the Corp card was trashed to Archives, with no replacement in the \
         way: {}",
        t.tail(40)
    );
    assert_eq!(
        t.of_kind(Kind::Targets).len(),
        0,
        "no group, no removal announcement: {}",
        t.tail(40)
    );
    assert!(
        vm.st.objects.values().all(|o| o.zone != Zone::SetAside
            && o.zone != Zone::RemovedFromGame),
        "nothing was set aside and nothing left the game: {}",
        t.tail(40)
    );
}

// ---------------------------------------------------------------------------
// Freedom Khumalo: Crypto-Anarchist
// ---------------------------------------------------------------------------

/// Freedom Khumalo: "Access, once per turn → Any X virus counters: Trash the
/// non-agenda card you are accessing. X must be equal to that card's rez or
/// play cost." — the whole sentence on a 2-cost asset, with the virus
/// counters spread over TWO different cards.
///
/// "Any" is 1.10.3c's division said about counters: with 2 hosted on each of
/// two cards and X determined at 2, which cards the 2 come from is a real
/// choice, put to the Runner exactly as a credit payment's division is — and
/// the answer 1+1 takes one counter off EACH card.
#[test]
fn freedom_khumalo_pays_x_across_two_cards_and_the_asset_falls() {
    let mut vm = Vm::empty(6400);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    let asset =
        tk::install_root(&mut vm, tk::vanilla_asset("Two-Cost Asset", 2, 9), ServerId::Remote(1), false);
    let host_a = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host A", CardType::Program));
    let host_b = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host B", CardType::Program));
    tk::place_counters(&mut vm, host_a, CounterKind::Virus, 2);
    tk::place_counters(&mut vm, host_b, CounterKind::Virus, 2);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::take("crypto-anarchist"))
            // 1.10.3c: one counter from each host.
            .when(Match::division().once(), Reply::Division(vec![1, 1]))
            .when(Match::action(), Reply::Halt),
    );
    // The division was asked as a COUNTER division, for exactly X = 2.
    let asked = t.entries.iter().find_map(|e| match &e.spec {
        jinteki_cr::decision::DecisionSpec::DivideCounterPayment { total, kind, locations } => {
            Some((*total, *kind, locations.clone()))
        }
        _ => None,
    });
    let (total, kind, locations) = asked.expect("the counter division was put to the Runner");
    assert_eq!(total, 2, "X is the accessed card's printed rez cost");
    assert_eq!(kind, CounterKind::Virus, "the cost names virus counters and no other kind");
    assert_eq!(
        locations,
        vec![(host_a, 2), (host_b, 2)],
        "every card of the Runner's hosting virus counters is a location"
    );
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Discard(Side::Corp),
        "the accessed asset was trashed: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.objects[&host_a].counter(CounterKind::Virus),
        1,
        "one virus counter came off Host A"
    );
    assert_eq!(
        vm.st.objects[&host_b].counter(CounterKind::Virus),
        1,
        "…and one off Host B"
    );
}

/// Freedom Khumalo: "X must be equal to that card's rez or play cost." —
/// 1.16.2c's announcement with an EQUALITY restriction is not a choice under
/// a ceiling: X is determined, so no DeclareX decision is ever put to the
/// Runner, and a division answer trying to spend 3 on a 2-cost card is
/// clamped to exactly 2. And where the determined X exceeds what the
/// Runner's cards host between them, the cost is unpayable (1.16.1b) and the
/// ability is not offered at all.
#[test]
fn freedom_khumalo_x_is_determined_not_chosen() {
    // Scene one: 4 counters available (3 + 1), a 2-cost asset, a greedy
    // division answer of [3, 0] — exactly 2 are spent, not 3.
    let mut vm = Vm::empty(6401);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    let asset =
        tk::install_root(&mut vm, tk::vanilla_asset("Two-Cost Asset", 2, 9), ServerId::Remote(1), false);
    let host_a = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host A", CardType::Program));
    let host_b = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host B", CardType::Program));
    tk::place_counters(&mut vm, host_a, CounterKind::Virus, 3);
    tk::place_counters(&mut vm, host_b, CounterKind::Virus, 1);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::take("crypto-anarchist"))
            .when(Match::division().once(), Reply::Division(vec![3, 0]))
            .when(Match::action(), Reply::Halt),
    );
    assert!(
        !t.entries.iter().any(|e| Kind::of(&e.spec) == Kind::DeclareX),
        "X is determined by the equality, so no announcement decision exists: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.objects[&asset].zone, Zone::Discard(Side::Corp), "trashed for exactly 2");
    assert_eq!(
        vm.st.objects[&host_a].counter(CounterKind::Virus)
            + vm.st.objects[&host_b].counter(CounterKind::Virus),
        2,
        "exactly 2 of the 4 counters were spent — a division answer cannot pay 3: {}",
        t.tail(20)
    );

    // Scene two: a 5-cost asset over 3 available counters — X is determined
    // at 5, the Runner cannot produce it, and the ability is never offered.
    let mut vm = Vm::empty(6402);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    let pricey =
        tk::install_root(&mut vm, tk::vanilla_asset("Five-Cost Asset", 5, 9), ServerId::Remote(1), false);
    let host = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host", CardType::Program));
    tk::place_counters(&mut vm, host, CounterKind::Virus, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::action(), Reply::Halt),
    );
    assert!(
        !t.ever_offered("crypto-anarchist"),
        "1.16.1b: exactly-5 cannot be produced from 3, so the cost is unpayable: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.objects[&pricey].zone, Zone::Root(ServerId::Remote(1)), "the asset stands");
    assert_eq!(vm.st.objects[&host].counter(CounterKind::Virus), 3, "nothing was spent");
}

/// Freedom Khumalo on a 0-cost card: X is determined at 0, and CR 1.16.1d
/// pays a cost of zero by announcing it — the payment is the announcement,
/// so the trash simply happens, with no virus counter anywhere on the board.
#[test]
fn freedom_khumalo_trashes_a_zero_cost_card_for_free() {
    let mut vm = Vm::empty(6403);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    let asset =
        tk::install_root(&mut vm, tk::vanilla_asset("Zero-Cost Asset", 0, 9), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::take("crypto-anarchist"))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.st.objects[&asset].zone,
        Zone::Discard(Side::Corp),
        "X = 0: the zero cost is paid by announcing it and the trash happens: {}",
        t.tail(20)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CounterRemoved { kind: CounterKind::Virus, .. }
        )),
        "no virus counter moved anywhere: {}",
        t.tail(20)
    );
}

/// Freedom Khumalo: "the **non-agenda** card you are accessing" — during the
/// access of an agenda the stipulation describes nothing, so the ability is
/// not offered at all (1.15.3): never "offered for X = 0", even though an
/// agenda has no rez or play cost to determine X with.
#[test]
fn freedom_khumalo_is_not_offered_during_an_agenda_access() {
    let mut vm = Vm::empty(6404);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    tk::install_root(&mut vm, tk::vanilla_agenda("Some Agenda", 3, 1), ServerId::Remote(1), false);
    let host = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host", CardType::Program));
    tk::place_counters(&mut vm, host, CounterKind::Virus, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::action(), Reply::Halt),
    );
    assert!(
        !t.ever_offered("crypto-anarchist"),
        "an agenda access is not described by the sentence: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.objects[&host].counter(CounterKind::Virus), 3, "nothing was spent");
}

/// Freedom Khumalo: "once per turn" — 9.3.6g's flag, spent by USE. After the
/// first access of the turn spends it, the second access the same turn does
/// not offer the ability at all, whatever the counters could still pay.
#[test]
fn freedom_khumalo_once_per_turn_second_access_gets_no_offer() {
    let mut vm = Vm::empty(6405);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    tk::install_root(&mut vm, tk::vanilla_asset("First Target", 1, 9), ServerId::Remote(1), false);
    let second =
        tk::install_root(&mut vm, tk::vanilla_asset("Second Target", 1, 9), ServerId::Remote(2), false);
    let host = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host", CardType::Program));
    tk::place_counters(&mut vm, host, CounterKind::Virus, 4);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::take("crypto-anarchist"))
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::RunBegan { .. })).count(),
        2,
        "both runs happened: {}",
        t.tail(30)
    );
    assert_eq!(
        plan::count_labelled(&offered_options(&t), "crypto-anarchist"),
        1,
        "9.3.6g: used on the first access, gone for the second: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&second].zone,
        Zone::Root(ServerId::Remote(2)),
        "the second asset stands"
    );
    assert_eq!(vm.st.objects[&host].counter(CounterKind::Virus), 3, "only the first X = 1 was spent");
}

/// Freedom Khumalo: the once-per-turn flag is spent by USE, not by an offer
/// declined — CR 9.1.6a: "a paid ability is considered used once the trigger
/// cost has been paid", and 9.3.6g's flag is spent by that use. An access
/// where the Runner passes the mid-access window pays nothing, so the next
/// access the same turn offers the ability again.
#[test]
fn freedom_khumalo_declined_offer_returns_the_same_turn() {
    let mut vm = Vm::empty(6406);
    tk::install_identity(&mut vm, card("Freedom Khumalo: Crypto-Anarchist"), Side::Runner);
    tk::install_root(&mut vm, tk::vanilla_asset("First Target", 1, 9), ServerId::Remote(1), false);
    let second =
        tk::install_root(&mut vm, tk::vanilla_asset("Second Target", 1, 9), ServerId::Remote(2), false);
    let host = tk::install_rig(&mut vm, tk::vanilla_runner_card("Virus Host", CardType::Program));
    tk::place_counters(&mut vm, host, CounterKind::Virus, 4);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            // Offered, and declined: 9.1.6a — no trigger cost paid, no use.
            .when(Match::mid_access().once(), Reply::Pass)
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .when(Match::mid_access().once(), Reply::take("crypto-anarchist"))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::RunBegan { .. })).count(),
        2,
        "both runs happened: {}",
        t.tail(30)
    );
    assert_eq!(
        plan::count_labelled(&offered_options(&t), "crypto-anarchist"),
        2,
        "declining spends nothing, so the second access offers it again: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.objects[&second].zone,
        Zone::Discard(Side::Corp),
        "…and using it there trashes the second asset: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.objects[&host].counter(CounterKind::Virus), 3, "one counter for X = 1");
}

/// Dewi, front face: "Whenever you make a successful run, if your [mu] is
/// full, you may flip this identity and gain 1[credit]." — with 4 of 4[mu]
/// filled the offer is made, and accepting flips AND gains as one sentence.
#[test]
fn dewi_flips_and_gains_when_mu_is_full() {
    let mut vm = Vm::empty(6410);
    let id =
        tk::install_identity(&mut vm, card("Dewi Subrotoputri: Pedagogical Dhalang"), Side::Runner);
    let mut prog = tk::vanilla_runner_card("Fat Program", CardType::Program);
    prog.memory_cost = Some(4);
    tk::install_rig(&mut vm, prog);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::reaction().once(), Reply::take("dewi"))
            .stop_at_action(),
    );
    assert!(vm.st.objects[&id].flipped.is_some(), "flipped to Shadow Guide: {}", t.tail(16));
    assert_eq!(vm.st.runner.credits, 6, "…and gained 1 with the flip: {}", t.tail(16));
}

/// Dewi, front face, the requirement FALSE: with unused [mu] the run
/// succeeds and nothing is offered at all — 9.6.5c's requirement is part of
/// the condition, so the ability never pends.
#[test]
fn dewi_makes_no_offer_while_mu_is_not_full() {
    let mut vm = Vm::empty(6411);
    let id =
        tk::install_identity(&mut vm, card("Dewi Subrotoputri: Pedagogical Dhalang"), Side::Runner);
    let mut prog = tk::vanilla_runner_card("Slim Program", CardType::Program);
    prog.memory_cost = Some(3);
    tk::install_rig(&mut vm, prog);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert!(!t.ever_offered("dewi"), "1 unused [mu] is not full: {}", t.tail(16));
    assert!(vm.st.objects[&id].flipped.is_none(), "the front face is still up");
    assert_eq!(vm.st.runner.credits, 5, "and no credit arrived");
}

/// Shadow Guide (the back face): "Whenever you make a successful run, if you
/// have at least 1 unused [mu], you may flip this identity and draw 1 card."
/// — the front's question with the threshold at the other end, drawing where
/// the front paid.
#[test]
fn dewi_back_flips_home_and_draws_with_unused_mu() {
    let mut vm = Vm::empty(6412);
    let id =
        tk::install_identity(&mut vm, card("Dewi Subrotoputri: Pedagogical Dhalang"), Side::Runner);
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
    let mut prog = tk::vanilla_runner_card("Slim Program", CardType::Program);
    prog.memory_cost = Some(3);
    tk::install_rig(&mut vm, prog);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::reaction().once(), Reply::take("shadow guide"))
            .stop_at_action(),
    );
    assert!(vm.st.objects[&id].flipped.is_none(), "flipped home to the front face: {}", t.tail(16));
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        1,
        "…and drew 1 with the flip — the back's own line: {}",
        t.tail(16)
    );
    assert_eq!(vm.st.runner.credits, 5, "the back draws; the front's gain is not its line");
}

/// SYNC, front face: "The Runner pays 1[credit] more when spending a [click]
/// to remove a tag (not through a card ability)." — 5.2.7g's basic action
/// costs 3, and exactly 3 is enough.
#[test]
fn sync_front_taxes_the_basic_remove_tag() {
    let mut vm = Vm::empty(6420);
    tk::install_identity(&mut vm, card("SYNC: Everything, Everywhere"), Side::Corp);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::RemoveTag))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.runner.tags, 0, "the tag came off: {}", t.tail(12));
    assert_eq!(vm.st.runner.credits, 0, "…for 3[credit]: the printed 2 plus SYNC's 1");
}

/// SYNC, front face, the other edge of the tax: with 2[credit] the modified
/// cost is out of reach, so 5.2.7g is not offered at all (1.16.1) — the plan
/// falls through to its halt and the tag stays.
#[test]
fn sync_front_tax_puts_the_action_out_of_a_2_credit_reach() {
    let mut vm = Vm::empty(6421);
    tk::install_identity(&mut vm, card("SYNC: Everything, Everywhere"), Side::Corp);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::RemoveTag))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(vm.st.runner.tags, 1, "2[credit] no longer buys the action: {}", t.tail(12));
    assert_eq!(vm.st.runner.credits, 2, "nothing was paid");
}

/// SYNC, back face: "You may pay 2[credit] fewer when spending a [click] to
/// trash a resource (not through a card ability)." — 5.2.6g's 2[credit]
/// floors at 0 (1.16.2a), so a Corp with 1[credit] can take the action and
/// pays nothing.
#[test]
fn sync_back_discounts_the_basic_trash_resource_to_zero() {
    let mut vm = Vm::empty(6422);
    let id = tk::install_identity(&mut vm, card("SYNC: Everything, Everywhere"), Side::Corp);
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
    let res =
        tk::install_rig(&mut vm, tk::vanilla_runner_card("Doomed Resource", CardType::Resource));
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.tags = 1;
    vm.st.corp.credits = 1;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            .when(Match::action(), Reply::Halt),
        Plan::runner().stopping_at_the_rest(),
    );
    assert_eq!(
        vm.st.objects[&res].zone,
        Zone::Discard(Side::Runner),
        "the resource went to the heap: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.corp.credits,
        1,
        "1[credit] was enough because the discounted cost is 0 — below the \
         printed 2 the action would have refused: {}",
        t.tail(16)
    );
}

/// SYNC, both faces: "[click]: Flip this identity." on each side, and each
/// side's static is its OWN printed sentence — after the Corp's click-flip
/// the front's tag tax is gone (the basic remove-tag is 2 again), and the
/// back's trash discount has taken over; the second click turns it home.
#[test]
fn sync_flips_for_a_click_and_each_face_keeps_its_own_static() {
    let mut vm = Vm::empty(6423);
    let id = tk::install_identity(&mut vm, card("SYNC: Everything, Everywhere"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::take("sync: flip"))
            .when(Match::action().times(2), Reply::credit())
            .when(Match::action().once(), Reply::take("sync: flip home"))
            .when(Match::action(), Reply::Halt),
        Plan::runner()
            .when(Match::action().once(), Reply::Take(Pick::RemoveTag))
            .when(Match::action(), Reply::credit()),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        2,
        "one flip out, one flip home — a [click] each: {}",
        t.tail(30)
    );
    assert!(vm.st.objects[&id].flipped.is_none(), "the front face is up again");
    assert_eq!(vm.st.runner.tags, 0, "with the back showing, the tag tax was gone: {}", t.tail(30));
    // 2 − 2 (the printed price, untaxed with the back up) + 3 basic credits.
    assert_eq!(
        vm.st.runner.credits,
        3,
        "…so 2[credit] bought the basic action at its printed price: {}",
        t.tail(30)
    );
}


// ---------------------------------------------------------------------------
// Méliès U: Only the Brightest — one front, three secretly-chosen backs
// ---------------------------------------------------------------------------

/// Méliès U, front face: "When your discard phase ends, secretly set your
/// identity to any copy of Méliès U: Only the Brightest." — the choice is put
/// to the CORP, among the three printed backs, and the Runner learns nothing
/// but that it happened.
///
/// What "the Runner learns nothing" is, mechanically: the sealed answer lives
/// in kernel-private state (the psi-bid grain — `psi_first_bid` is the
/// precedent), never in the change log, which CR 10.2.1 makes open to both
/// players — the one record of the set (`IdentityFaceSecretlySet`) carries
/// the side and nothing else. A back face is not even an object, so no
/// `View` (zones of `CardView`s, maintained choices, credit pools) has a slot
/// that could carry it; the assertions below pin the record's silence and the
/// decision's addressee, which are the two surfaces the kernel exposes.
#[test]
fn melies_secretly_sets_a_face_at_discard_phase_end_and_the_record_stays_silent() {
    let mut vm = Vm::empty(6400);
    let id = tk::install_identity(&mut vm, card("Méliès U: Only the Brightest"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(
            Match::of(Kind::Options).once(),
            Reply::ChooseNamed("Subsurface Labs"),
        ),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    let asked = t.of_kind(Kind::Options);
    assert_eq!(asked.len(), 1, "one secret set, at the discard phase's end: {}", t.tail(20));
    assert_eq!(asked[0].side, Side::Corp, "9.1.1a: the Corp's identity, so the Corp seals");
    assert_eq!(
        plan::choices(&asked[0].spec),
        [
            "Tenure Floors: Méliès U",
            "Subsurface Labs: Méliès U",
            "Disposal Grounds: Méliès U"
        ],
        "the options are exactly the three printed backs, in face order"
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(
            c,
            GameChange::IdentityFaceSecretlySet { side: Side::Corp }
        )).count(),
        1,
        "THAT the set happened is open information: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.objects[&id].flipped, None, "nothing turned over — the set is not a flip");
    // 10.2.2a: both views show the same thing — the identity's front. The
    // sealed face has no object id, so neither view has anywhere to say it.
    assert!(vm.view_of(Side::Runner).sees(id), "the identity itself is open to the Runner");
    assert!(vm.view_of(Side::Corp).sees(id), "…and to the Corp — the views do not differ");
}

/// Méliès U: the front's "When the Runner makes a successful run on a central
/// server, flip this identity." turns up the SEALED back — and that back's
/// own "When you flip this identity to this side during a run on R&D, look
/// at the top card of R&D. You may trash that card. If you do, add 1 card
/// from Archives to HQ." fires, because the run that flipped it is on the
/// server the back names.
#[test]
fn melies_flips_to_the_sealed_back_and_the_back_speaks_on_its_server() {
    let mut vm = Vm::empty(6401);
    let id = tk::install_identity(&mut vm, card("Méliès U: Only the Brightest"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    let deck = tk::fill_deck(&mut vm, Side::Corp, 5);
    // The Corp's mandatory draw takes deck[0] at the turn's start, so the
    // top card of R&D when the run happens is deck[1].
    let top = deck[1];
    tk::fill_deck(&mut vm, Side::Runner, 5);
    let buried = vm.new_object(tk::corp_filler("Archived Paper"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("Subsurface Labs"))
            // 1.16.11a: pay the optional cost — trash the looked-at card.
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            // "Add 1 card from Archives to HQ": the just-trashed card is in
            // Archives too and is honestly on offer; this Corp reaches for
            // the one that was buried all along.
            .when(Match::of(Kind::Targets).once(), Reply::target(buried)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.st.objects[&id].flipped,
        Some(1),
        "the run on R&D turned up the SEALED copy — Subsurface Labs, faces[1]: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "one flip — the reveal: {}",
        t.tail(40)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardLookedAt { obj, by: Side::Corp } if *obj == top
        )),
        "the back's look: the top card of R&D, shown to the Corp alone: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&top].zone,
        Zone::Discard(Side::Corp),
        "the looked-at card was trashed for the optional cost: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&buried].zone,
        Zone::Hand(Side::Corp),
        "…and 'if you do': 1 card from Archives went to HQ: {}",
        t.tail(40)
    );
}

/// Méliès U, front face, the other two sentences — and the honest edge of
/// the N-faces word. "When the Runner's action phase ends, gain 1[credit]."
/// pays the Corp at the end of the RUNNER's action phase; and a successful
/// central run before any discard phase has sealed a face flips NOTHING:
/// with three backs and no set, no face is determined, and 9.11.2 does as
/// much as it can — which is nothing at all. (In a real game the Corp's
/// first discard phase precedes every Runner run, so the edge is unreachable
/// from setup; the kernel still refuses to invent a face.)
#[test]
fn melies_gains_at_runner_action_phase_end_and_an_unsealed_flip_does_nothing() {
    let mut vm = Vm::empty(6402);
    let id = tk::install_identity(&mut vm, card("Méliès U: Only the Brightest"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::action(), Reply::Halt),
        Plan::runner().when(Match::action().once(), Reply::run(ServerId::Hq)),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::RunDeclaredSuccessful { server: ServerId::Hq, .. }
        )),
        "the central run WAS successful: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        0,
        "…and flipped nothing — three backs, none sealed, no face to turn up: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.objects[&id].flipped, None, "still the front");
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::ActionPhaseEnded { side: Side::Runner }
        )),
        "the Runner's action phase ended: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.corp.credits,
        1,
        "…and the tuition arrived: 1[credit] at ITS end, not the Corp's own: {}",
        t.tail(30)
    );
}

/// Méliès U: the set happens again at every Corp discard phase's end, and
/// the LAST seal is the one the flip reveals — re-setting replaces the
/// pending face.
#[test]
fn melies_resets_the_sealed_face_on_a_later_discard_phase() {
    let mut vm = Vm::empty(6403);
    let id = tk::install_identity(&mut vm, card("Méliès U: Only the Brightest"), Side::Corp);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // A decision an earlier rule claims never counts towards a later
            // one, so the second seal's rule wants `once()`, not `nth(2)`.
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("Tenure Floors"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("Disposal Grounds")),
        Plan::runner()
            // The Runner's first turn spends itself on credits; the second
            // opens with the run on Archives.
            .when(Match::action().times(4), Reply::credit())
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .when(Match::action().once(), Reply::Halt),
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(
            c,
            GameChange::IdentityFaceSecretlySet { side: Side::Corp }
        )).count(),
        2,
        "two Corp discard phases, two seals: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.st.objects[&id].flipped,
        Some(2),
        "the run on Archives revealed the SECOND seal — Disposal Grounds, \
         faces[2] — not the replaced first: {}",
        t.tail(40)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "one flip: {}",
        t.tail(40)
    );
}


// ---------------------------------------------------------------------------
// C1: the game-start window (CR 1.6.2 / 1.6.1a / 1.6.7a)
// ---------------------------------------------------------------------------

/// A NEXT Design game, built through the real §1.6 setup: the corp deck's
/// first five cards (the starting hand under `shuffle: false`) hold `ice`
/// pieces of ice, the rest is filler.
fn next_design_game(ice: usize, deck_size: usize) -> Vm {
    use jinteki_cr::vm::GameSetup;
    let mut corp_deck: Vec<PrintedCard> = Vec::new();
    for i in 0..deck_size {
        if i < ice {
            corp_deck.push(tk::vanilla_ice(["Ice-A", "Ice-B", "Ice-C"][i], 1, 3));
        } else {
            corp_deck.push(tk::corp_filler("C-filler"));
        }
    }
    let runner_deck: Vec<PrintedCard> =
        (0..8).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    Vm::new_game(GameSetup {
        corp_identity: Some(card("NEXT Design: Guarding the Net")),
        runner_identity: None,
        corp_deck,
        runner_deck,
        shuffle: false,
        seed: 6401,
        additional_identities: Default::default(),
        extra_cards: Default::default(),
    })
}

/// NEXT Design: "Before taking your first turn, you may install up to 3
/// pieces of ice, with no more than a single piece of ice per server. Draw
/// until you have 5 cards in HQ." — all three installed, one per server, the
/// hand refilled to exactly 5, and every bit of it before the Corp's first
/// turn formally begins (1.6.7a: "and thus before the game starts").
#[test]
fn next_design_installs_three_ice_and_draws_back_to_five_before_the_first_turn() {
    let mut vm = next_design_game(3, 12);
    let ice: Vec<ObjectId> = vm.st.hand[&Side::Corp]
        .iter()
        .copied()
        .filter(|c| vm.st.objects[c].printed.card_type == CardType::Ice)
        .collect();
    assert_eq!(ice.len(), 3, "the opening hand holds the three ice");

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets().once(), Reply::Targets(vec![ice[0]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
            )
            .when(Match::targets().once(), Reply::Targets(vec![ice[1]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Rnd)),
            )
            .when(Match::targets().once(), Reply::Targets(vec![ice[2]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(
                    ServerId::Archives,
                )),
            )
            .stop_at_action(),
        Plan::runner(),
    );
    let protecting = |s: ServerId| -> Vec<ObjectId> {
        vm.st.ice[&s].iter().filter_map(|p| p.ice).collect()
    };
    assert_eq!(protecting(ServerId::Hq), vec![ice[0]], "one ice protecting HQ: {}", t.tail(20));
    assert_eq!(protecting(ServerId::Rnd), vec![ice[1]], "one protecting R&D");
    assert_eq!(protecting(ServerId::Archives), vec![ice[2]], "one protecting Archives");
    // 1.6.7a: the whole ability resolves immediately BEFORE the first turn,
    // "and thus before the game starts" — read off the log, which puts every
    // install and every one of its draws before GameBegan, and GameBegan
    // before the turn's formal beginning (5.1.4a). "Draw until you have 5
    // cards in HQ" landed at exactly 5: the hand held 5 − 3 installed = 2,
    // and exactly 3 pre-game draws follow. (The hand then visible holds 6 —
    // the first turn's own 5.3 mandatory draw, which is not this ability's.)
    let log = &vm.changes.log;
    let last_install = log
        .iter()
        .rposition(|c| matches!(c, GameChange::CardInstalled { .. }))
        .expect("three installs were recorded");
    let began = log
        .iter()
        .position(|c| matches!(c, GameChange::GameBegan))
        .expect("the game began");
    let first_turn = log
        .iter()
        .position(|c| matches!(c, GameChange::TurnBegan { side: Side::Corp }))
        .expect("the Corp's first turn began");
    let pre_game_draws = log[..began]
        .iter()
        .filter(|c| matches!(c, GameChange::CardDrawn { side: Side::Corp, .. }))
        .count();
    assert_eq!(
        pre_game_draws, 3,
        "draw-until-5 drew exactly 3 before the game began: {}",
        t.tail(30)
    );
    assert!(
        last_install < began && began < first_turn,
        "1.6.7a: installs, then the game begins, then the first turn: {}",
        t.tail(30)
    );
}

/// The per-server stipulation: after the first ice protects HQ, the second
/// destination declaration no longer offers HQ at all — "no more than a
/// single piece of ice per server" is enforced by the offer, not by trust.
#[test]
fn next_design_offers_each_server_only_once() {
    let mut vm = next_design_game(2, 12);
    let ice: Vec<ObjectId> = vm.st.hand[&Side::Corp]
        .iter()
        .copied()
        .filter(|c| vm.st.objects[c].printed.card_type == CardType::Ice)
        .collect();

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets().once(), Reply::Targets(vec![ice[0]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
            )
            .when(Match::targets().once(), Reply::Targets(vec![ice[1]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Rnd)),
            )
            .when(Match::targets(), Reply::Targets(Vec::new()))
            .stop_at_action(),
        Plan::runner(),
    );
    let second = t.nth_window(Kind::Destination, Side::Corp, 2);
    let jinteki_cr::decision::DecisionSpec::DeclareInstallDestination { options } = &second.spec
    else {
        panic!("a destination declaration");
    };
    assert!(
        !options.contains(&jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
        "HQ already took this effect's single piece of ice: {options:?}"
    );
    assert!(
        options.contains(&jinteki_cr::instr::InstallDest::Protecting(ServerId::Rnd)),
        "the untouched centrals are still offered: {options:?}"
    );
}

/// "You may install **up to** 3" — declining the first pick installs none,
/// and the draw half still resolves: it is one ability, and the draw is
/// mandatory. A full hand draws nothing ("until you have 5" of a hand of 5).
#[test]
fn next_design_declines_every_install_and_the_full_hand_draws_nothing() {
    let mut vm = next_design_game(3, 12);
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets().once(), Reply::Targets(Vec::new()))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(
        vm.st.ice.values().all(|v| v.iter().all(|p| p.ice.is_none())),
        "nothing installed anywhere: {}",
        t.tail(20)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardInstalled { .. })),
        "and no install was recorded"
    );
    // The hand already had 5, so "draw until you have 5" drew nothing: not
    // one Corp draw precedes GameBegan (the one that follows it is the first
    // turn's own 5.3 mandatory draw).
    let began = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::GameBegan))
        .expect("the game began");
    assert_eq!(
        vm.changes.log[..began]
            .iter()
            .filter(|c| matches!(c, GameChange::CardDrawn { side: Side::Corp, .. }))
            .count(),
        0,
        "a full hand draws nothing: {}",
        t.tail(20)
    );
}

/// "Draw until you have 5 cards in HQ" against a short R&D: the draw takes
/// what remains and stops. It is not 5.3's mandatory draw, so 1.7.2c's
/// flatline-by-decking does not fire — the game goes on.
#[test]
fn next_design_draws_fewer_when_rnd_is_short() {
    // 5 in hand (3 ice + 2 filler), exactly 1 left in R&D.
    let mut vm = next_design_game(3, 6);
    let ice: Vec<ObjectId> = vm.st.hand[&Side::Corp]
        .iter()
        .copied()
        .filter(|c| vm.st.objects[c].printed.card_type == CardType::Ice)
        .collect();
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets().once(), Reply::Targets(vec![ice[0]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
            )
            .when(Match::targets().once(), Reply::Targets(vec![ice[1]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Rnd)),
            )
            .when(Match::targets().once(), Reply::Targets(vec![ice[2]]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(
                    ServerId::Archives,
                )),
            )
            .stop_at_action(),
        Plan::runner(),
    );
    let log = &vm.changes.log;
    let began = log
        .iter()
        .position(|c| matches!(c, GameChange::GameBegan))
        .expect("the game began");
    assert_eq!(
        log[..began]
            .iter()
            .filter(|c| matches!(c, GameChange::CardDrawn { side: Side::Corp, .. }))
            .count(),
        1,
        "R&D held 1, so the draw-until-5 took 1 and stopped: {}",
        t.tail(20)
    );
    assert!(vm.st.deck[&Side::Corp].is_empty(), "R&D gave everything it had");
    // NEXT's draw is NOT 1.7.2c's "required to draw": the game went on — its
    // first turn formally began on an empty R&D. The 5.3 mandatory draw that
    // follows IS required, meets the empty deck, and ends the game; the
    // contrast is the ruling.
    assert!(
        log.iter().any(|c| matches!(c, GameChange::TurnBegan { side: Side::Corp })),
        "the first turn began after the shortfall: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.game_over,
        Some(jinteki_cr::decision::GameResult::RndEmpty),
        "then the MANDATORY draw met the empty R&D (1.7.2c)"
    );
}

/// An Ayla game through the real §1.6 setup. `new_game` returns before the
/// starting hands: her ability resolves first.
fn ayla_game() -> Vm {
    use jinteki_cr::vm::GameSetup;
    let runner_deck: Vec<PrintedCard> =
        (0..20).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    let corp_deck: Vec<PrintedCard> = (0..8).map(|_| tk::corp_filler("C-filler")).collect();
    Vm::new_game(GameSetup {
        runner_identity: Some(card("Ayla \"Bios\" Rahim: Simulant Specialist")),
        corp_deck,
        runner_deck,
        corp_identity: None,
        shuffle: false,
        seed: 6402,
        additional_identities: Default::default(),
        extra_cards: Default::default(),
    })
}

/// Ayla: "Before drawing your starting hand, set aside the top 6 cards of
/// your stack facedown. … Shuffle 2 of those cards into your stack." — six
/// set aside before any hand exists, two chosen back in, four left in the
/// group stamped with the identity, and only THEN the starting hand.
#[test]
fn ayla_sets_aside_six_and_shuffles_two_back_before_the_starting_hand() {
    let mut vm = ayla_game();
    // 1.6.1a: the ability resolves BEFORE the 1.6.6 draw — the hands are not
    // drawn yet when new_game returns.
    assert!(vm.st.hand[&Side::Runner].is_empty(), "no starting hand before the ability");
    let top6: Vec<ObjectId> = vm.st.deck[&Side::Runner][0..6].to_vec();
    let ayla = vm.identity_of(Side::Runner).unwrap();

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner().when(Match::targets().once(), Reply::Targets(vec![top6[0], top6[1]])),
    );
    let set_aside: Vec<ObjectId> = vm
        .st
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::SetAside && o.set_aside_group.is_some_and(|g| g.with == Some(ayla))
        })
        .map(|o| o.id)
        .collect();
    assert_eq!(set_aside.len(), 4, "6 set aside − 2 shuffled back: {}", t.tail(20));
    for c in &top6[2..] {
        assert!(set_aside.contains(c), "the unchosen four stay set aside");
        assert!(
            !vm.st.objects[c].faceup,
            "facedown — though the group is Ayla's, so she may look at any time"
        );
    }
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "then the starting hand: {}", t.tail(20));
    // 20 − 6 set aside + 2 shuffled back − 5 drawn.
    assert_eq!(vm.st.deck[&Side::Runner].len(), 11, "the stack after all of §1.6");
}

/// Ayla's second line: "[click]: Add 1 card set aside with this identity to
/// your grip." — the set-aside group outlives setup, and a click on the
/// Runner's first turn moves one of the four to the grip.
#[test]
fn ayla_spends_a_click_to_add_a_set_aside_card_to_the_grip() {
    let mut vm = ayla_game();
    let top6: Vec<ObjectId> = vm.st.deck[&Side::Runner][0..6].to_vec();

    let t = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner()
            .when(Match::targets().once(), Reply::Targets(vec![top6[0], top6[1]]))
            .when(Match::action().once(), Reply::take("add 1 set-aside card"))
            .when(Match::targets().once(), Reply::Targets(vec![top6[2]]))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&top6[2]].zone,
        Zone::Hand(Side::Runner),
        "the chosen set-aside card is in the grip: {}",
        t.tail(25)
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 6, "5 drawn + 1 retrieved");
    assert_eq!(vm.st.runner.clicks, 3, "4 allotted − 1 spent on the identity");
    let ayla = vm.identity_of(Side::Runner).unwrap();
    let still_aside = vm
        .st
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::SetAside && o.set_aside_group.is_some_and(|g| g.with == Some(ayla))
        })
        .count();
    assert_eq!(still_aside, 3, "three remain for later clicks");
}

/// The mulligan interaction, per the CR alone: 1.6.6a's mulligan "shuffles
/// their starting hand back into their deck, then draws a new starting hand"
/// — the set-aside cards are in neither the hand nor the stack, so the
/// redraw neither returns them nor sets aside more. One resolution, before
/// the first draw; the second hand is drawn from the 16-card stack the
/// ability left behind.
#[test]
fn ayla_mulligan_redraws_around_an_untouched_set_aside() {
    let mut vm = ayla_game();
    let top6: Vec<ObjectId> = vm.st.deck[&Side::Runner][0..6].to_vec();

    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::targets().once(), Reply::Targets(vec![top6[0], top6[1]]))
            .when(Match::mulligan(), Reply::Mulligan),
    );
    let ayla = vm.identity_of(Side::Runner).unwrap();
    let set_aside: Vec<ObjectId> = vm
        .st
        .objects
        .values()
        .filter(|o| {
            o.zone == Zone::SetAside && o.set_aside_group.is_some_and(|g| g.with == Some(ayla))
        })
        .map(|o| o.id)
        .collect();
    assert_eq!(
        set_aside.len(),
        4,
        "still exactly four — the mulligan set aside none: {}",
        t.tail(20)
    );
    for c in &top6[2..] {
        assert!(set_aside.contains(c), "and they are the same four objects");
    }
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "the second starting hand");
    assert_eq!(
        vm.st.deck[&Side::Runner].len(),
        11,
        "16 − 5: the redraw came from the stack the ability left"
    );
}

/// Adam: "You start the game with 3 different directive cards installed
/// (these cards are not considered part of your deck)." — the three
/// directives are installed and ACTIVE from the game's first moment (Safety
/// First's max-hand-size reduction is already in force), the stack was never
/// touched by them, and they are in no deck-derived zone at all.
#[test]
fn adam_starts_the_game_with_three_directives_installed_and_active() {
    use jinteki_cr::vm::GameSetup;
    let runner_deck: Vec<PrintedCard> =
        (0..20).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    let corp_deck: Vec<PrintedCard> = (0..8).map(|_| tk::corp_filler("C-filler")).collect();
    let mut vm = Vm::new_game(GameSetup {
        runner_identity: Some(card_partial("Adam: Compulsive Hacker")),
        corp_deck,
        runner_deck,
        extra_cards: [(
            Side::Runner,
            vec![
                card_partial("Always Be Running"),
                card_partial("Neutralize All Threats"),
                card_partial("Safety First"),
            ],
        )]
        .into_iter()
        .collect(),
        corp_identity: None,
        shuffle: false,
        seed: 6403,
        additional_identities: Default::default(),
    });
    let directives: Vec<ObjectId> = vm
        .st
        .objects
        .values()
        .filter(|o| o.printed.subtypes.contains(&"Directive"))
        .map(|o| o.id)
        .collect();
    assert_eq!(directives.len(), 3, "exactly the three brought cards exist");
    for d in &directives {
        let o = &vm.st.objects[d];
        assert_eq!(o.zone, Zone::Rig, "installed in the play area (1.5.3b)");
        assert!(o.faceup, "4.6.4c: Runner cards are installed faceup");
    }
    assert_eq!(vm.st.deck[&Side::Runner].len(), 15, "20 − 5 drawn; no directive among them");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "an ordinary starting hand");
    assert!(
        vm.st.hand[&Side::Runner]
            .iter()
            .chain(vm.st.deck[&Side::Runner].iter())
            .all(|c| !directives.contains(c)),
        "1.5.3: not considered part of the deck"
    );
    // Active from the first moment: Safety First's "Your maximum hand size
    // is reduced by 2" is a static declaration of an installed, active card.
    assert_eq!(vm.max_hand_size(Side::Runner), 3, "5 − 2 (Safety First already in force)");

    // And the game plays on normally from there.
    let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
    assert!(vm.game_over.is_none(), "{}", t.tail(10));
}

// ---------------------------------------------------------------------------
// Sebastião Souza Pessoa: Activist Organizer (`identities/runner_anarch.rs`)
// ---------------------------------------------------------------------------

/// Sebastião, first sentence: "Whenever you take 1 or more tags, if you had
/// no tags, you may install 1 connection resource from your grip, paying
/// 2[credit] less." At 0 tags the taking meets the condition — the
/// "had"-requirement reads the occurrence's record, not the pool that
/// already counts the new tag — and the install arrives at −2.
#[test]
fn sebastiao_installs_a_connection_at_a_discount_when_the_first_tags_land() {
    let mut vm = Vm::empty(6430);
    tk::install_identity(
        &mut vm,
        card("Sebastião Souza Pessoa: Activist Organizer"),
        Side::Runner,
    );
    let conn = {
        let mut c = tk::vanilla_runner_card("Union Contact", CardType::Resource);
        c.subtypes = vec!["Connection"];
        c.cost = Some(3);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::install_rig(&mut vm, tk::take_tag_button("TagMe"));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("take 1 tag"))
            .when(Match::reaction().once(), Reply::take("organize while clean"))
            .when(Match::optional().once(), Reply::Optional(true))
            .when(Match::targets().once(), Reply::target(conn))
            .when(
                Match::destination(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Rig),
            )
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.tags, 1, "the button landed the tag: {}", t.tail(20));
    assert_eq!(
        vm.st.objects[&conn].zone,
        Zone::Rig,
        "the connection came out of the grip: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.runner.credits,
        2,
        "1[credit] paid a 3[credit] install — 1.16.6 lowered it by 2: {}",
        t.tail(20)
    );
}

/// Sebastião, first sentence's requirement: a Runner who already HAD a tag
/// takes another — the occurrence's record says `had: 1`, so the ability is
/// never offered at all.
#[test]
fn sebastiao_stays_quiet_when_the_runner_already_had_a_tag() {
    let mut vm = Vm::empty(6434);
    tk::install_identity(
        &mut vm,
        card("Sebastião Souza Pessoa: Activist Organizer"),
        Side::Runner,
    );
    let conn = {
        let mut c = tk::vanilla_runner_card("Union Contact", CardType::Resource);
        c.subtypes = vec!["Connection"];
        c.cost = Some(3);
        let id = vm.new_object(c, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(id);
        id
    };
    tk::install_rig(&mut vm, tk::take_tag_button("TagMe"));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 3;
    // Setup state, not effect: the Runner is already tagged, so the coming
    // taking is not one they had no tags before.
    vm.st.runner.tags = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("take 1 tag"))
            .when(
                Match::reaction().offering("organize while clean"),
                Reply::Forbid,
            )
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.tags, 2, "the button landed the second tag: {}", t.tail(20));
    assert_eq!(
        vm.st.objects[&conn].zone,
        Zone::Hand(Side::Runner),
        "no install was ever offered — the requirement read `had: 1`: {}",
        t.tail(20)
    );
    assert_eq!(vm.st.runner.credits, 3, "nothing was paid: {}", t.tail(20));
}

/// CR 5.2.6g reshaped by 1.15.2: the basic trash-resource action announces
/// WHICH resource before any of its costs are paid. Halted at the
/// announcement, the click and the credits are untouched; resumed, the costs
/// land and the announced resource is trashed — no Sebastião anywhere, so
/// this is the plain action's own order.
#[test]
fn the_trash_resource_action_announces_its_target_before_any_cost_is_paid() {
    let mut vm = Vm::empty(6431);
    let res = tk::install_rig(
        &mut vm,
        tk::vanilla_runner_card("Doomed Resource", CardType::Resource),
    );
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            .when(Match::action().once(), Reply::Halt)
            .when(Match::targets().nth(1), Reply::Halt)
            .when(Match::targets().once(), Reply::target(res)),
        Plan::runner(),
    );
    g.run(&mut vm);
    // Halted AT the target announcement: the action is initiated, and
    // 1.15.2 has put the announcement in front of the payment — so neither
    // the click nor the 2[credit] has moved yet.
    assert_eq!(
        g.transcript().entries.last().map(|e| e.kind()),
        Some(Kind::Targets),
        "halted at the announcement: {}",
        g.transcript().tail(8)
    );
    assert_eq!(
        vm.st.corp.credits,
        5,
        "no credit paid before the announcement: {}",
        g.transcript().tail(8)
    );
    assert_eq!(
        vm.st.corp.clicks,
        3,
        "no click spent before the announcement: {}",
        g.transcript().tail(8)
    );

    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&res].zone,
        Zone::Discard(Side::Runner),
        "the announced resource was trashed: {}",
        g.transcript().tail(12)
    );
    assert_eq!(
        vm.st.corp.credits,
        3,
        "the 2[credit] were paid after the announcement, as before it moved: {}",
        g.transcript().tail(12)
    );
    assert_eq!(vm.st.corp.clicks, 2, "…and the click: {}", g.transcript().tail(12));
}

/// Sebastião, second sentence: trashing a CONNECTION with the basic action
/// carries the 1.16.10 additional cost — after announcing the connection,
/// the Corp must also trash 1 card from HQ, and which card is the Corp's
/// choice (1.14.5).
#[test]
fn sebastiaos_connection_tax_makes_the_corp_trash_a_chosen_card_from_hq() {
    let mut vm = Vm::empty(6432);
    tk::install_identity(
        &mut vm,
        card("Sebastião Souza Pessoa: Activist Organizer"),
        Side::Runner,
    );
    let conn = {
        let mut c = tk::vanilla_runner_card("Union Contact", CardType::Resource);
        c.subtypes = vec!["Connection"];
        tk::install_rig(&mut vm, c)
    };
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);
    // The turn's mandatory draw made HQ four cards; the Corp will choose
    // this one to pay Sebastião's tax with.
    let hq_pick = vm.st.hand[&Side::Corp][0];

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            .when(Match::targets().once(), Reply::target(conn))
            .when(Match::payment_cards().once(), Reply::target(hq_pick))
            .when(Match::action().once(), Reply::Halt),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&conn].zone,
        Zone::Discard(Side::Runner),
        "the connection was trashed: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.objects[&hq_pick].zone,
        Zone::Discard(Side::Corp),
        "the Corp's CHOSEN card paid the additional cost into Archives: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.hand[&Side::Corp].len(),
        3,
        "HQ is one card down (four after the mandatory draw): {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.corp.credits,
        3,
        "the regular 2[credit] was still paid alongside: {}",
        t.tail(16)
    );
}

/// Sebastião, second sentence under 1.16.1b: with an empty HQ the combined
/// cost of trashing a connection cannot be paid, so a connection cannot even
/// be announced — the action is not offered while the connection is the only
/// resource, and once a plain resource exists the action returns with the
/// connection missing from its candidates.
#[test]
fn an_empty_hq_shields_connections_from_the_basic_trash_action() {
    let mut vm = Vm::empty(6433);
    tk::install_identity(
        &mut vm,
        card("Sebastião Souza Pessoa: Activist Organizer"),
        Side::Runner,
    );
    let conn = {
        let mut c = tk::vanilla_runner_card("Union Contact", CardType::Resource);
        c.subtypes = vec!["Connection"];
        tk::install_rig(&mut vm, c)
    };
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.tags = 1;
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);
    // The card the mandatory draw is about to put in HQ: the Corp's first
    // action plays it away, and HQ is empty when the second window opens.
    let drawn = vm.st.deck[&Side::Corp][0];

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp()
            .when(Match::action().nth(1), Reply::Take(Pick::PlayCard(drawn)))
            .when(Match::action().nth(1), Reply::Halt)
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::Take(Pick::TrashResource))
            // The plain resource does not exist when this plan is written;
            // the default answer takes the first (and only) candidate.
            .when(Match::targets().once(), Reply::Default),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    g.run(&mut vm);
    // HQ is empty and the connection is the only resource; its combined
    // cost is unpayable, so the ACTION is not offered at all (1.16.1b).
    let offer = g
        .transcript()
        .entries
        .iter()
        .rev()
        .find(|e| e.kind() == Kind::Action)
        .expect("an action window was reached");
    let jinteki_cr::decision::DecisionSpec::TakeAction { options } = &offer.spec else {
        panic!("not an action window: {:?}", offer.spec)
    };
    assert!(
        !options.contains(&jinteki_cr::decision::ActionOption::BasicTrashResource),
        "no resource's whole cost is payable, so there is no action: {options:?}"
    );
    assert!(vm.st.hand[&Side::Corp].is_empty(), "HQ really is empty: {}", g.transcript().tail(8));

    // A plain resource arrives; the action returns — offered for IT, with
    // the connection still not a candidate. (The halted window is stale, so
    // the plan spends it on a credit and trashes from the next one.)
    let plain = tk::install_rig(
        &mut vm,
        tk::vanilla_runner_card("Plain Resource", CardType::Resource),
    );
    g.run(&mut vm);
    let announce = g
        .transcript()
        .entries
        .iter()
        .find(|e| e.kind() == Kind::Targets)
        .expect("the action announced a target");
    assert_eq!(
        announce.candidates(),
        &[plain],
        "the connection is not among the candidates — announcing it would \
         announce a cost that cannot be paid: {}",
        g.transcript().tail(12)
    );
    assert_eq!(
        vm.st.objects[&plain].zone,
        Zone::Discard(Side::Runner),
        "the plain resource was still trashable: {}",
        g.transcript().tail(12)
    );
    assert_eq!(
        vm.st.objects[&conn].zone,
        Zone::Rig,
        "the connection survives behind its unpayable tax: {}",
        g.transcript().tail(12)
    );
}


// ---------------------------------------------------------------------------
// Jinteki Biotech: Life Imagined — the pre-first-turn switch, sealed; three
// backs, one triple-click reveal
// ---------------------------------------------------------------------------

/// A Jinteki Biotech game through the real §1.6 setup. Under `shuffle: false`
/// the corp deck's first five cards are the starting hand.
fn biotech_game(seed: u64, corp_deck: Vec<PrintedCard>) -> Vm {
    use jinteki_cr::vm::GameSetup;
    let runner_deck: Vec<PrintedCard> =
        (0..8).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    Vm::new_game(GameSetup {
        corp_identity: Some(card("Jinteki Biotech: Life Imagined")),
        runner_identity: None,
        corp_deck,
        runner_deck,
        shuffle: false,
        seed,
        additional_identities: Default::default(),
        extra_cards: Default::default(),
    })
}

/// Jinteki Biotech: "Before taking your first turn, you may switch this
/// identity with any copy of Jinteki Biotech." — the switch is offered to the
/// CORP in 1.6.7a's window, as the secret choice among the three printed
/// backs (which copy lies on the table is what the physically identical
/// fronts hide); this Corp seals The Tank. Then
/// "[click][click][click]: Flip this identity." reveals it, and The Tank's
/// "When you flip this identity, shuffle all cards in Archives into R&D."
/// empties Archives into the deck.
#[test]
fn biotech_seals_the_tank_before_the_first_turn_and_the_flip_shuffles_archives_into_rnd() {
    let corp_deck: Vec<PrintedCard> = (0..12).map(|_| tk::corp_filler("C-filler")).collect();
    let mut vm = biotech_game(6404, corp_deck);
    // Stage Archives: three cards buried before the game formally begins.
    let buried: Vec<ObjectId> = ["Vat-A", "Vat-B", "Vat-C"]
        .into_iter()
        .map(|n| {
            let o = vm.new_object(tk::corp_filler(n), Zone::Discard(Side::Corp));
            vm.st.discard.get_mut(&Side::Corp).unwrap().push(o);
            o
        })
        .collect();
    let id = vm.identity_of(Side::Corp).expect("the identity is in the play area");

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("The Tank"))
            .when(Match::action().once(), Reply::take("jinteki biotech: flip"))
            .stop_at_action(),
        Plan::runner().stop_at_action(),
    );
    let asked = t.of_kind(Kind::Options);
    assert_eq!(asked.len(), 1, "one switch, before the first turn: {}", t.tail(30));
    assert_eq!(asked[0].side, Side::Corp, "put to the Corp — it is their identity");
    assert_eq!(
        plan::choices(&asked[0].spec),
        [
            "The Brewery: Jinteki Biotech",
            "The Tank: Jinteki Biotech",
            "The Greenhouse: Jinteki Biotech"
        ],
        "the options are exactly the three copies, in face order"
    );
    assert_eq!(
        vm.st.objects[&id].flipped,
        Some(1),
        "the flip revealed the sealed copy — The Tank, faces[1]: {}",
        t.tail(30)
    );
    // The Tank's effect: Archives is empty and every buried card is in R&D.
    for b in &buried {
        assert_eq!(
            vm.st.objects[b].zone,
            Zone::Deck(Side::Corp),
            "shuffled into R&D: {}",
            t.tail(30)
        );
    }
    // The shuffle emptied Archives; the ONE card there now is the discard
    // phase's own (5.5.4 — the mandatory draw made the hand 6, the flip took
    // every click, so one card went down AFTER the shuffle), and it is none
    // of the buried three.
    assert_eq!(vm.st.discard[&Side::Corp].len(), 1, "only the discard phase's later card");
    assert!(
        !vm.st.discard[&Side::Corp].iter().any(|c| buried.contains(c)),
        "none of the buried three came back"
    );
    // 12 − 5 (starting hand) − 1 (the first turn's mandatory draw) + 3 in.
    assert_eq!(vm.st.deck[&Side::Corp].len(), 9, "R&D count after the shuffle-in");
    // The order of the record: the seal before the game began (1.6.7a), the
    // flip during the first turn, the shuffle after the flip that caused it.
    let log = &vm.changes.log;
    let sealed = log
        .iter()
        .position(|c| matches!(c, GameChange::IdentityFaceSecretlySet { side: Side::Corp }))
        .expect("the seal is recorded — THAT it happened is open");
    let began = log.iter().position(|c| matches!(c, GameChange::GameBegan)).expect("game began");
    let flip = log
        .iter()
        .position(|c| matches!(c, GameChange::IdentityFlipped { .. }))
        .expect("the flip");
    let shuffle = log
        .iter()
        .position(|c| matches!(c, GameChange::DeckShuffled { side: Side::Corp }))
        .expect("the shuffle");
    assert!(
        sealed < began && began < flip && flip < shuffle,
        "seal ({sealed}) < game began ({began}) < flip ({flip}) < shuffle ({shuffle}): {}",
        t.tail(40)
    );
    assert_eq!(vm.st.corp.clicks, 0, "the flip took the whole action phase");
}

/// Declining the switch: the physical card always has SOME back — the copy
/// the Corp brought, The Brewery (`faces[0]`, the first face the card data
/// lists). Choosing it IS the decline: same table state as never having
/// switched, so the kernel asks once, not twice. The triple-click flip then
/// reveals The Brewery, whose own sentence resolves.
#[test]
fn biotech_declining_the_switch_keeps_the_default_copy_and_the_flip_reveals_the_brewery() {
    let corp_deck: Vec<PrintedCard> = (0..12).map(|_| tk::corp_filler("C-filler")).collect();
    let mut vm = biotech_game(6405, corp_deck);
    let id = vm.identity_of(Side::Corp).expect("the identity");

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // The decline: keep the default copy — faces[0], The Brewery.
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("The Brewery"))
            .when(Match::action().once(), Reply::take("jinteki biotech: flip"))
            .stop_at_action(),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&id].flipped,
        Some(0),
        "the default copy turned up — The Brewery, faces[0]: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::IdentityFlipped { .. })).count(),
        1,
        "one flip: {}",
        t.tail(30)
    );
    // …and its effect happened: 2 net damage, two cards off the grip.
    assert_eq!(vm.st.hand[&Side::Runner].len(), 3, "5 drawn − 2 net damage: {}", t.tail(30));
}

/// The Greenhouse: "When you flip this identity, place 4 advancement
/// counters on 1 installed card that you can advance." — sealed before the
/// first turn, revealed on the second (turn one spent a click installing the
/// agenda the counters want), and the placement is 1.18.2's bare placing:
/// four counters arrive, no advance happens.
#[test]
fn biotech_greenhouse_places_four_advancement_counters_on_an_advanceable_card() {
    let mut corp_deck: Vec<PrintedCard> = vec![tk::vanilla_agenda("Vat Complex", 3, 1)];
    corp_deck.extend((0..11).map(|_| tk::corp_filler("C-filler")));
    let mut vm = biotech_game(6406, corp_deck);
    let id = vm.identity_of(Side::Corp).expect("the identity");
    let agenda = vm.st.hand[&Side::Corp]
        .iter()
        .copied()
        .find(|c| vm.st.objects[c].printed.card_type == CardType::Agenda)
        .expect("the agenda is in the starting hand");

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("The Greenhouse"))
            // Turn 1: install the agenda, then two clicks on credits.
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(agenda)))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::action().times(2), Reply::credit())
            // Turn 2: the whole action phase on the flip.
            .when(Match::action().once(), Reply::take("jinteki biotech: flip"))
            .when(Match::targets().once(), Reply::target(agenda))
            .stop_at_action(),
        Plan::runner().when(Match::action().times(4), Reply::credit()).stop_at_action(),
    );
    assert_eq!(vm.st.objects[&id].flipped, Some(2), "The Greenhouse — faces[2]: {}", t.tail(40));
    assert_eq!(
        vm.st.objects[&agenda].counters.get(&CounterKind::Advancement).copied().unwrap_or(0),
        4,
        "4 advancement counters placed on the advanceable card: {}",
        t.tail(40)
    );
    // 1.18.2: placed, not advanced — no advance was recorded.
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAdvanced { .. })),
        "bare placement is not an advance: {}",
        t.tail(40)
    );
}

/// The Brewery's sentence in full: "When you flip this identity, do 2 net
/// damage." — one flip, one damage occurrence of 2, the Corp responsible
/// (10.4.2), two cards from the grip to the heap.
#[test]
fn biotech_brewery_does_two_net_damage_on_the_flip() {
    let corp_deck: Vec<PrintedCard> = (0..12).map(|_| tk::corp_filler("C-filler")).collect();
    let mut vm = biotech_game(6407, corp_deck);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("The Brewery"))
            .when(Match::action().once(), Reply::take("jinteki biotech: flip"))
            .stop_at_action(),
        Plan::runner().stop_at_action(),
    );
    let hits: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::DamageSuffered { kind, amount, cards, responsible }
                if *kind == jinteki_cr::effects::DamageKind::Net =>
            {
                Some((*amount, cards.len(), *responsible))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        hits,
        vec![(2, 2, Side::Corp)],
        "one occurrence of 2 net damage, 2 cards trashed, the Corp responsible: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.discard[&Side::Runner].len(), 2, "both in the heap");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 3, "5 − 2 in the grip");
}

/// The flip costs exactly [click][click][click]: a full action phase pays it
/// (3 → 0, three ClickSpent records), and at 2 clicks the ability is not
/// offered at all — 1.16.10's cost gate removes it from the window, so the
/// plan's take-rule never fires and nothing flips.
#[test]
fn biotech_flip_costs_exactly_three_clicks() {
    // Affordable: the whole action phase.
    let corp_deck: Vec<PrintedCard> = (0..12).map(|_| tk::corp_filler("C-filler")).collect();
    let mut vm = biotech_game(6408, corp_deck);
    let id = vm.identity_of(Side::Corp).expect("the identity");
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("The Tank"))
            .when(Match::action().once(), Reply::take("jinteki biotech: flip"))
            .stop_at_action(),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(vm.st.objects[&id].flipped, Some(1), "flipped at 3 clicks: {}", t.tail(30));
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::ClickSpent { side: Side::Corp }))
            .count(),
        3,
        "three clicks spent — the whole allotment, all on the flip: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.corp.clicks, 0, "3 → 0");

    // Unaffordable: after one click on a credit, 2 remain and the ability is
    // no longer offered — the take-rule's implicit guard never matches.
    let corp_deck: Vec<PrintedCard> = (0..12).map(|_| tk::corp_filler("C-filler")).collect();
    let mut vm2 = biotech_game(6409, corp_deck);
    let id2 = vm2.identity_of(Side::Corp).expect("the identity");
    let t2 = plan::play(
        &mut vm2,
        Plan::corp()
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("The Tank"))
            .when(Match::action().nth(1), Reply::credit())
            .when(Match::action(), Reply::take("jinteki biotech: flip"))
            .stop_at_action(),
        Plan::runner().stop_at_action(),
    );
    assert_eq!(vm2.st.objects[&id2].flipped, None, "2 clicks cannot pay 3: {}", t2.tail(30));
    assert!(
        !vm2.changes.log.iter().any(|c| matches!(c, GameChange::IdentityFlipped { .. })),
        "no flip was ever recorded: {}",
        t2.tail(30)
    );
    assert_eq!(vm2.st.corp.clicks, 2, "the driver halted with the two clicks unspent");
}


// ---------------------------------------------------------------------------
// MirrorMorph: Endless Iteration — a fourth action, a [click] cheaper
// ---------------------------------------------------------------------------

/// MirrorMorph: "If the first, second, and third actions you take on your
/// turn are each different from one another, when the third action completes,
/// you may gain 1[credit] or take another different action, paying [click]
/// less." Three different actions — the basic credit, draw and install — and
/// when the third completes the offer comes. The Corp takes a FOURTH
/// different action, the basic play, paying no [click] for it: three clicks
/// allotted, three spent, four actions on the record — each initiated and
/// completed, so anything counting the turn's actions sees four.
#[test]
fn mirrormorph_hands_out_a_fourth_different_action_for_no_click() {
    let mut vm = Vm::empty(6500);
    tk::install_identity(&mut vm, card("MirrorMorph: Endless Iteration"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("Third-Asset", 0, 2), Zone::Hand(Side::Corp));
    let op = vm.new_object(tk::operation("Fourth-Op", 0, vec![]), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().extend([asset, op]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset)))
            .when(Match::reaction().offering("mirrormorph").once(), Reply::take("mirrormorph"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("take another"))
            // The offer is a TakeAction decision like the window's own —
            // the fourth Kind::Action decision this plan sees.
            .when(Match::action().once(), Reply::play_card(op)),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert!(t.ever_offered("mirrormorph"), "the third completion brought the offer: {}", t.tail(20));
    assert_eq!(
        vm.st.objects[&op].zone,
        Zone::Discard(Side::Corp),
        "the fourth action happened — the operation was played: {}",
        t.tail(20)
    );
    // The click ledger: three allotted, three spent — the fourth action's
    // 1[click] was reduced to 0 (1.16.2a) and spent nothing.
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::ClickSpent { side: Side::Corp })).count(),
        3,
        "three clicks paid for four actions: {}",
        t.tail(20)
    );
    // All four initiations are on the record, each a different 5.2.5
    // identity, in the order taken.
    let taken: Vec<ActionIdentity> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::ActionTaken { side: Side::Corp, action } => Some(*action),
            _ => None,
        })
        .collect();
    assert_eq!(
        taken,
        vec![
            ActionIdentity::Basic(BasicAction::Credit),
            ActionIdentity::Basic(BasicAction::Draw),
            ActionIdentity::Basic(BasicAction::Install),
            ActionIdentity::Basic(BasicAction::PlayOperation),
        ],
        "four ActionTaken records, all different: {}",
        t.tail(20)
    );
    // …and all four completed (5.2.2a): the offered action finished inside
    // the third completion's checkpoint, before the turn moved on.
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::ActionCompleted { side: Side::Corp, .. })).count(),
        4,
        "four completions — the fourth is an action like any other: {}",
        t.tail(20)
    );
}

/// MirrorMorph, the other half of the choice: "you may gain 1[credit] or
/// take another different action" — choosing the credit lands 1[credit] and
/// no fourth action is taken.
#[test]
fn mirrormorph_gains_one_credit_when_that_half_is_chosen() {
    let mut vm = Vm::empty(6501);
    tk::install_identity(&mut vm, card("MirrorMorph: Endless Iteration"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("Third-Asset", 0, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(asset);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset)))
            .when(Match::reaction().offering("mirrormorph").once(), Reply::take("mirrormorph"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("gain 1")),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.st.corp.credits,
        2,
        "1 from the basic credit action + 1 from the identity: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::ActionTaken { side: Side::Corp, .. })).count(),
        3,
        "the credit was chosen, so no fourth action: {}",
        t.tail(20)
    );
}

/// MirrorMorph is silent when the first three actions are NOT all different:
/// credit, credit, draw — the first two share 5.2.5a's identity, so the
/// condition is never met and no offer comes.
#[test]
fn mirrormorph_stays_silent_when_an_action_repeats() {
    let mut vm = Vm::empty(6502);
    tk::install_identity(&mut vm, card("MirrorMorph: Endless Iteration"), Side::Corp);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().times(2), Reply::credit())
            .when(Match::action().once(), Reply::draw()),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert!(
        !t.ever_offered("mirrormorph"),
        "5.2.5a: two basic credit actions are the same action: {}",
        t.tail(20)
    );
}

/// MirrorMorph is a "may": declining the pending ability at the reaction
/// window resolves nothing — no credit, no fourth action.
#[test]
fn mirrormorph_declined_entirely_does_nothing() {
    let mut vm = Vm::empty(6503);
    tk::install_identity(&mut vm, card("MirrorMorph: Endless Iteration"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("Third-Asset", 0, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(asset);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    // No reaction rule: the neutral fallback passes on the optional pending.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset))),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert!(t.ever_offered("mirrormorph"), "the offer WAS there to decline: {}", t.tail(20));
    assert_eq!(vm.st.corp.credits, 1, "only the basic credit action paid: {}", t.tail(20));
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::ActionTaken { side: Side::Corp, .. })).count(),
        3,
        "declined: three actions stay three: {}",
        t.tail(20)
    );
}

/// "Take another DIFFERENT action": the offer's option list is the action
/// window's, minus the three 5.2.5 identities already taken. With credit,
/// draw and install spent, the only different action this board affords is
/// the basic play — and the three taken kinds are absent from the offer.
#[test]
fn mirrormorph_offers_only_actions_different_from_the_three_taken() {
    let mut vm = Vm::empty(6504);
    tk::install_identity(&mut vm, card("MirrorMorph: Endless Iteration"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("Third-Asset", 0, 2), Zone::Hand(Side::Corp));
    let op = vm.new_object(tk::operation("Fourth-Op", 0, vec![]), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().extend([asset, op]);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(asset)))
            .when(Match::reaction().offering("mirrormorph").once(), Reply::take("mirrormorph"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("take another"))
            // Halt ON the offer itself, so its option list can be asserted.
            .when(Match::action().once(), Reply::Halt),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    let offer = t.of_kind(Kind::Action).into_iter().last().expect("the offer decision");
    assert!(offer.answer.is_none(), "halted on the offer: {}", t.tail(10));
    let DecisionSpec::TakeAction { options } = &offer.spec else {
        panic!("the offer is a TakeAction decision: {:?}", offer.spec)
    };
    // Only the basic play remains: credit, draw and install are the taken
    // identities (two plays of different cards are still ONE identity, so
    // every playable card is one option each); purge needs 3[click] and
    // only 1 is discounted; no other action is afforded by this board.
    assert!(
        options.iter().all(|o| matches!(o, ActionOption::BasicPlayOperation { .. })),
        "the three taken kinds are absent from the offer: {:?}\n{}",
        options,
        t.tail(10)
    );
    assert!(
        options.contains(&ActionOption::BasicPlayOperation { card: op }),
        "…and the operation in hand is on it: {:?}\n{}",
        options,
        t.tail(10)
    );
}

/// MirrorMorph works fresh each turn: 5.2.5b counts the actions of THIS
/// turn, so a second turn of three different actions brings the offer again.
#[test]
fn mirrormorph_comes_back_the_next_turn() {
    let mut vm = Vm::empty(6505);
    tk::install_identity(&mut vm, card("MirrorMorph: Endless Iteration"), Side::Corp);
    let a1 = vm.new_object(tk::vanilla_asset("Turn-1-Asset", 0, 2), Zone::Hand(Side::Corp));
    let a2 = vm.new_object(tk::vanilla_asset("Turn-2-Asset", 0, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().extend([a1, a2]);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(a1)))
            .when(Match::reaction().offering("mirrormorph").once(), Reply::take("mirrormorph"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("gain 1"))
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::draw())
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(a2)))
            .when(Match::reaction().offering("mirrormorph").once(), Reply::take("mirrormorph"))
            .when(Match::of(Kind::Options).once(), Reply::ChooseNamed("gain 1")),
        Plan::runner()
            .when(Match::action().times(4), Reply::credit())
            .when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        t.offers("mirrormorph"),
        2,
        "three different actions each Corp turn, one offer each: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.corp.credits,
        4,
        "each turn: 1 from the credit action + 1 from the identity: {}",
        t.tail(30)
    );
}

#[test]
fn ag_infusion_trashes_the_approached_ice_and_moves_the_runner_to_the_chosen_remote() {
    let mut vm = Vm::empty(6430);
    tk::install_identity(&mut vm, card("AgInfusion: New Miracles for a New World"), Side::Corp);
    let gate = tk::install_ice(&mut vm, tk::vanilla_ice("Gate", 0, 1), ServerId::Hq, false);
    let wall = tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp()
            .when(
                Match::paid().approaching_ice().offering("new miracles"),
                Reply::take("new miracles"),
            )
            .when(Match::choose_server(), Reply::Server(ServerId::Remote(1))),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            // 6.9.4c on the remote, after passing its encountered outermost
            // ice — halt with the redirected run still in progress.
            .when(Match::of(Kind::JackOut), Reply::Halt),
    );
    script.run(&mut vm);
    let t = script.transcript();

    let choices: Vec<_> = t.entries.iter().filter(|e| e.kind() == Kind::ChooseServer).collect();
    assert_eq!(choices.len(), 1, "one server choice was put to the Corp: {}", t.tail(60));
    assert_eq!(choices[0].side, Side::Corp, "1.14.5: the ability's controller chooses");
    assert_eq!(
        choices[0].spec,
        jinteki_cr::decision::DecisionSpec::ChooseServer {
            options: vec![ServerId::Rnd, ServerId::Archives, ServerId::Remote(1)],
        },
        "every server there IS except the attacked one — the remote created \
         this game included, HQ excluded: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&gate].zone,
        Zone::Discard(Side::Corp),
        "1.16.10: the approached ice was trashed as the trigger cost: {}",
        t.tail(60)
    );
    let approached = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::IceApproached { ice } if *ice == wall));
    let encountered = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == wall));
    assert!(
        approached.is_some(),
        "6.2.8b: the move is an approach of the remote's outermost ice: {}",
        t.tail(60)
    );
    assert!(
        encountered.is_some(),
        "…and 6.4.4 turns the approach of a REZZED ice into the encounter: {}",
        t.tail(60)
    );
    assert!(approached < encountered, "approached first, encountered from there");
    let r = vm.run_ctx().expect("halted at 6.9.4c with the run in progress");
    assert_eq!(r.server, ServerId::Remote(1), "6.2.8b: that server became the attacked server");
    assert_eq!(
        r.position,
        Some(vm.position_of_ice(wall).unwrap().1),
        "…and the Runner stands in its outermost position: {}",
        t.tail(60)
    );
}

/// AgInfusion, the chosen server protected by NO ice: 6.2.8b's other half.
/// The Runner ceases to have a position and the current timing step becomes
/// the Movement Phase — no approach, no encounter, no pass (the run did not
/// reach the phase from an approach or an encounter) — and the run continues
/// toward the server itself (6.9.4g).
#[test]
fn ag_infusion_sends_the_runner_straight_to_the_server_when_the_chosen_one_has_no_ice() {
    let mut vm = Vm::empty(6431);
    tk::install_identity(&mut vm, card("AgInfusion: New Miracles for a New World"), Side::Corp);
    let gate = tk::install_ice(&mut vm, tk::vanilla_ice("Gate", 0, 1), ServerId::Hq, false);
    // 4.6.8d: a root card alone is enough for the remote to EXIST — and to
    // be offered.
    tk::install_root(&mut vm, tk::vanilla_asset("Vault", 0, 3), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::paid().approaching_ice().offering("new miracles"),
                Reply::take("new miracles"),
            )
            .when(Match::choose_server(), Reply::Server(ServerId::Remote(1))),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );

    assert_eq!(
        vm.st.objects[&gate].zone,
        Zone::Discard(Side::Corp),
        "the cost was still paid: {}",
        t.tail(60)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterBegan { .. })),
        "no ice there, so nothing was encountered anywhere this run: {}",
        t.tail(60)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::ServerApproached { server: ServerId::Remote(1) })),
        "6.2.8b + 6.9.4g: with no position the run continued to the server itself: {}",
        t.tail(60)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::RunDeclaredSuccessful { server: ServerId::Remote(1), .. }
        )),
        "…and was declared successful THERE, not on HQ: {}",
        t.tail(60)
    );
}

/// AgInfusion, the chosen server's outermost UNREZZED: the documented
/// reading. "The Runner moves to the outermost position of that server" is
/// 6.2.8b, and 6.2.8b makes the move an APPROACH — never a direct or forced
/// encounter (6.5.9a is stated for abilities that encounter "without first
/// changing their position", and this sentence moves). So the unrezzed
/// outermost is approached, the Corp may rez it in the 6.9.2b window the
/// approach opens (6.4.1/6.4.3), and only then does 6.4.4 deliver the
/// printed "encounters any ice there".
#[test]
fn ag_infusion_approaches_an_unrezzed_outermost_and_the_corp_may_rez_it_into_the_encounter() {
    let mut vm = Vm::empty(6432);
    tk::install_identity(&mut vm, card("AgInfusion: New Miracles for a New World"), Side::Corp);
    tk::install_ice(&mut vm, tk::vanilla_ice("Gate", 0, 1), ServerId::Hq, false);
    let wall = tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 2, 1), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::paid().approaching_ice().offering("new miracles"),
                Reply::take("new miracles"),
            )
            .when(Match::choose_server(), Reply::Server(ServerId::Remote(1)))
            // The second approach window of the run — the one 6.2.8b's move
            // opened on the remote — is where 6.4.3 lets the Corp rez the
            // approached ice.
            .when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );

    let approached = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::IceApproached { ice } if *ice == wall));
    let encountered = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == wall));
    assert!(
        approached.is_some(),
        "6.2.8b: the unrezzed outermost is APPROACHED, not force-encountered: {}",
        t.tail(60)
    );
    assert!(
        vm.st.objects[&wall].faceup,
        "6.4.1/6.4.3: the approach opened the rez window and the Corp used it: {}",
        t.tail(60)
    );
    assert_eq!(vm.st.corp.credits, 3, "…paying Wall's printed 2[credit] rez cost");
    assert!(
        encountered.is_some() && approached < encountered,
        "6.4.4: rezzed during the approach, encountered after it — the printed \
         'encounters any ice there' arrives through the run's own structure: {}",
        t.tail(60)
    );
}

/// AgInfusion's arrow: 9.3.6g's once-per-turn flag is spent by using the
/// ability, so the second approach of an unrezzed piece of ice in the same
/// turn offers nothing.
#[test]
fn ag_infusion_is_offered_once_per_turn() {
    let mut vm = Vm::empty(6433);
    tk::install_identity(&mut vm, card("AgInfusion: New Miracles for a New World"), Side::Corp);
    let inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 1), ServerId::Hq, false);
    let outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 1), ServerId::Hq, false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::paid().approaching_ice().offering("new miracles"),
                Reply::take("new miracles"),
            )
            .when(Match::choose_server(), Reply::Server(ServerId::Archives)),
        Plan::runner()
            .when(Match::action().times(2), Reply::run(ServerId::Hq))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );

    let offers = t
        .entries
        .iter()
        .filter(|e| e.kind() == Kind::Paid)
        .filter(|e| {
            e.options().iter().any(|o| matches!(
                o,
                jinteki_cr::decision::WindowOption::TriggerPaid { label, .. }
                    if label.contains("new miracles")
            ))
        })
        .count();
    assert_eq!(
        offers, 1,
        "the flag was spent by the first use — the second run's approach of \
         Inner, the same turn, offered nothing: {}",
        t.tail(60)
    );
    // Each run met the outermost ice of its moment: Outer paid the first
    // use's cost; Inner — outermost once Outer left — was approached on the
    // second run and simply passed.
    assert_eq!(
        vm.st.objects[&outer].zone,
        Zone::Discard(Side::Corp),
        "the first approach's ice went to the cost: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&inner].zone,
        Zone::Ice(ServerId::Hq),
        "the second approach's ice stayed exactly where it was: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunBegan { .. }))
            .count(),
        2,
        "two runs were made: {}",
        t.tail(60)
    );
}

/// AgInfusion is not offered at all while the approached ice is REZZED:
/// 9.5.6b's timing restriction reads the sentence's own stipulation — "the
/// **unrezzed** piece of ice the Runner is approaching" — and 1.16.1b agrees
/// from the cost's side, whose description reaches no card.
#[test]
fn ag_infusion_is_not_offered_when_the_approached_ice_is_rezzed() {
    let mut vm = Vm::empty(6434);
    tk::install_identity(&mut vm, card("AgInfusion: New Miracles for a New World"), Side::Corp);
    let gate = tk::install_ice(&mut vm, tk::vanilla_ice("Gate", 0, 1), ServerId::Hq, true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::of(Kind::JackOut), Reply::JackOut(false))
            .stop_at_action(),
    );

    assert!(
        !t.entries.iter().filter(|e| e.kind() == Kind::Paid).any(|e| {
            e.options().iter().any(|o| matches!(
                o,
                jinteki_cr::decision::WindowOption::TriggerPaid { label, .. }
                    if label.contains("new miracles")
            ))
        }),
        "no window of the run offered the ability: {}",
        t.tail(60)
    );
    assert_eq!(
        vm.st.objects[&gate].zone,
        Zone::Ice(ServerId::Hq),
        "and nothing was trashed: {}",
        t.tail(60)
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == gate)),
        "the rezzed ice was encountered as the run's own structure provides: {}",
        t.tail(60)
    );
}


// ---------------------------------------------------------------------------
// Cyber Bureau: Keeping the Peace (both faces)
// ---------------------------------------------------------------------------

/// A Cyber Bureau game, built through the real §1.6 setup: the corp deck's
/// first ten cards (the starting hand under `shuffle: false`) hold three
/// 8-to-rez assets and two 2-to-rez ice; the rest is operation filler, which
/// no install can choose.
fn cyber_bureau_game() -> Vm {
    use jinteki_cr::vm::GameSetup;
    let mut corp_deck: Vec<PrintedCard> = vec![
        tk::vanilla_asset("Asset-8a", 8, 1),
        tk::vanilla_asset("Asset-8b", 8, 1),
        tk::vanilla_asset("Asset-8c", 8, 1),
        tk::vanilla_ice("Ice-2a", 2, 3),
        tk::vanilla_ice("Ice-2b", 2, 3),
    ];
    for _ in 0..10 {
        corp_deck.push(tk::corp_filler("C-filler"));
    }
    let runner_deck: Vec<PrintedCard> =
        (0..8).map(|_| tk::vanilla_runner_card("R-filler", CardType::Resource)).collect();
    Vm::new_game(GameSetup {
        corp_identity: Some(card("Cyber Bureau: Keeping the Peace")),
        runner_identity: None,
        corp_deck,
        runner_deck,
        shuffle: false,
        seed: 6410,
        additional_identities: Default::default(),
        extra_cards: Default::default(),
    })
}

/// Cyber Bureau, the whole opening: the starting hand is 10 (1.6.6 draws the
/// printed number), the pre-first-turn window installs five cards without the
/// credit pool moving (1.16.5c), the Corp rezzes three 8-cost assets out of
/// the one 20[credit] pool — shares declared 8, 8, 4 (1.16.2f's division,
/// clamped to what remains), so 24[credit] of rez costs are paid with
/// 4[credit] — and the identity flips before the first turn formally begins.
#[test]
fn cyber_bureau_opens_with_ten_installs_five_free_rezzes_three_from_the_pool_and_flips() {
    let mut vm = cyber_bureau_game();
    assert_eq!(vm.st.hand[&Side::Corp].len(), 10, "1.6.6 drew the printed 10");
    let id = vm.identity_of(Side::Corp).expect("the Corp identity");
    let by_name = |vm: &Vm, n: &str| -> ObjectId {
        vm.st.hand[&Side::Corp]
            .iter()
            .copied()
            .find(|c| vm.st.objects[c].printed.name == n)
            .expect("in the opening hand")
    };
    let a = by_name(&vm, "Asset-8a");
    let b = by_name(&vm, "Asset-8b");
    let c = by_name(&vm, "Asset-8c");
    let i1 = by_name(&vm, "Ice-2a");
    let i2 = by_name(&vm, "Ice-2b");

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // Five installs, one at a time (8.5.5), all cost-ignored.
            .when(Match::targets().once(), Reply::Targets(vec![a]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::targets().once(), Reply::Targets(vec![b]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::targets().once(), Reply::Targets(vec![c]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::NewRemoteRoot),
            )
            .when(Match::targets().once(), Reply::Targets(vec![i1]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Hq)),
            )
            .when(Match::targets().once(), Reply::Targets(vec![i2]))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Rnd)),
            )
            // Three rezzes out of the pool: shares 8, 8, then 8 clamped to
            // the 4 that remain (1.16.2f's nonnegative numbers, summing to
            // no more than the modifier).
            .when(Match::targets().once(), Reply::Targets(vec![a]))
            .when(Match::cost_division().once(), Reply::Divide(8))
            .when(Match::targets().once(), Reply::Targets(vec![b]))
            .when(Match::cost_division().once(), Reply::Divide(8))
            .when(Match::targets().once(), Reply::Targets(vec![c]))
            .when(Match::cost_division().once(), Reply::Divide(8))
            // Decline whatever the pool can still afford after that.
            .when(Match::targets(), Reply::Targets(Vec::new()))
            .stop_at_action(),
        Plan::runner(),
    );

    let log = &vm.changes.log;
    let began = log
        .iter()
        .position(|c| matches!(c, GameChange::GameBegan))
        .expect("the game began");
    assert_eq!(
        log[..began]
            .iter()
            .filter(|c| matches!(c, GameChange::CardInstalled { .. }))
            .count(),
        5,
        "all five installs land before the game begins (1.6.7a): {}",
        t.tail(60)
    );
    // 1.16.5c: the installs moved no credits; the three rezzes paid 8−8,
    // 8−8 and 8−4 — so the whole opening paid exactly 4 of the Corp's 5.
    let paid: u32 = log[..began]
        .iter()
        .filter_map(|c| match c {
            GameChange::CostPaid { side: Side::Corp, credits, .. } => Some(*credits),
            _ => None,
        })
        .sum();
    assert_eq!(paid, 4, "24[credit] of rez costs, 20 lowered, 4 paid: {}", t.tail(60));
    assert_eq!(vm.st.corp.credits, 1, "5 − 4: the installs cost nothing");
    let rezzed: Vec<ObjectId> = log[..began]
        .iter()
        .filter_map(|c| match c {
            GameChange::CardRezzed { obj, .. } => Some(*obj),
            _ => None,
        })
        .collect();
    assert_eq!(rezzed, vec![a, b, c], "the three chosen assets rezzed, in order");
    assert!(!vm.st.objects[&i1].faceup, "the ice was never rezzed");
    // "Flip this identity." — mandatory, and inside the window: the flip is
    // recorded before GameBegan, and the back face shows from then on.
    let flip = log
        .iter()
        .position(|c| matches!(c, GameChange::IdentityFlipped { side: Side::Corp, .. }))
        .expect("the identity flipped");
    assert!(flip < began, "flipped before the game began: {}", t.tail(60));
    assert_eq!(vm.st.objects[&id].flipped, Some(0), "Detective's Bureau faces up");
}

/// "Install up to 5" — declining the very first pick installs none and
/// rezzes nothing, and "Flip this identity." still happens: the flip is a
/// mandatory third instruction of the same ability, not a reward for using
/// the first two. The game then proceeds into an ordinary first turn.
#[test]
fn cyber_bureau_declined_installs_still_flip_the_identity() {
    let mut vm = cyber_bureau_game();
    let id = vm.identity_of(Side::Corp).expect("the Corp identity");
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets(), Reply::Targets(Vec::new()))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardInstalled { .. })),
        "nothing installed: {}",
        t.tail(30)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRezzed { .. })),
        "and nothing rezzed"
    );
    assert_eq!(vm.st.corp.credits, 5, "no cost was paid by anything");
    assert_eq!(vm.st.objects[&id].flipped, Some(0), "the flip is mandatory");
    let log = &vm.changes.log;
    let flip = log
        .iter()
        .position(|c| matches!(c, GameChange::IdentityFlipped { side: Side::Corp, .. }))
        .expect("the identity flipped");
    let first_turn = log
        .iter()
        .position(|c| matches!(c, GameChange::TurnBegan { side: Side::Corp }))
        .expect("the first turn began");
    assert!(flip < first_turn, "flipped before the first turn: {}", t.tail(30));
}

/// Detective's Bureau: "The first time the Runner initiates a run each turn,
/// force the Runner to lose 1[credit] for each agenda point in his or her
/// score area, then you gain 1[credit] for each credit lost." — 3 points
/// scored, so the first run costs 3 and pays the Corp 3; the second run the
/// same turn meets nothing (9.6.5c's ordinal).
#[test]
fn detectives_bureau_tolls_only_the_first_run_each_turn() {
    let mut vm = Vm::empty(6411);
    let id = tk::install_identity(&mut vm, card("Cyber Bureau: Keeping the Peace"), Side::Corp);
    // Setup state: the identity begins on its back face (as the mandatory
    // pre-first-turn flip leaves it) — placement, not effect-by-fiat.
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
    tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Stolen-3", 3, 3), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().times(2), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    let losses: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CreditsLost { side: Side::Runner, amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(losses, vec![3], "one toll, on the FIRST run only: {}", t.tail(40));
    assert_eq!(vm.st.runner.credits, 2, "5 − 3, and the second run cost nothing");
    let gains: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CreditsGained { side: Side::Corp, amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(gains, vec![3], "…and the Corp was paid it once: {}", t.tail(40));
    assert_eq!(vm.st.corp.credits, 3, "the Corp pool holds exactly the toll it was paid");
}

/// The toll against a pool of 1: CR 1.10.3b — a forced loss takes as many
/// credits as the pool holds and no more, and "for each credit lost" is the
/// amount ACTUALLY lost (the recorded loss), not the 3 the score area
/// computes — so the Corp gains exactly 1.
#[test]
fn detectives_bureau_toll_is_capped_by_the_runners_pool() {
    let mut vm = Vm::empty(6412);
    let id = tk::install_identity(&mut vm, card("Cyber Bureau: Keeping the Peace"), Side::Corp);
    vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
    tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Stolen-3", 3, 3), Side::Runner);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 1;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 0, "1.10.3b: a pool of 1 loses 1: {}", t.tail(40));
    let gains: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CreditsGained { side: Side::Corp, amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(
        gains,
        vec![1],
        "the gain reads the recorded loss, not the computed 3: {}",
        t.tail(40)
    );
    assert_eq!(vm.st.corp.credits, 1, "the Corp gained only the 1 credit that really moved");
}

/// "[click]: Gain 3[credit] or draw 3 cards." — both halves of 9.11.4g's
/// choice, each for one click.
#[test]
fn detectives_bureau_click_gains_3_or_draws_3() {
    for draw_branch in [false, true] {
        let mut vm = Vm::empty(6413);
        let id =
            tk::install_identity(&mut vm, card("Cyber Bureau: Keeping the Peace"), Side::Corp);
        vm.st.objects.get_mut(&id).unwrap().flipped = Some(0);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let option = if draw_branch { "draw 3 cards" } else { "Gain 3[credit]" };
        let t = plan::play(
            &mut vm,
            Plan::corp()
                // 5.2.4: a [click] ability is an ACTION, offered in the
                // action window.
                .when(Match::action().once(), Reply::take("gain 3 or draw 3"))
                .when(Match::options(), Reply::ChooseNamed(option))
                .stop_at_action(),
            Plan::runner(),
        );
        if draw_branch {
            assert_eq!(
                vm.st.hand[&Side::Corp].len(),
                4,
                "the 5.3 mandatory draw, then 3 more (draw_branch): {}",
                t.tail(20)
            );
            assert_eq!(vm.st.corp.credits, 0, "the draw branch pays no credits (draw_branch)");
        } else {
            assert_eq!(vm.st.corp.credits, 3, "gained 3 (gain branch): {}", t.tail(20));
            assert_eq!(vm.st.hand[&Side::Corp].len(), 1, "only the mandatory draw");
        }
    }
}

// ---------------------------------------------------------------------------
// Mezzie's Asa — the ice
// ---------------------------------------------------------------------------

/// Vanilla: "[subroutine] End the run."
#[test]
fn vanilla_ends_the_run() {
    let mut vm = Vm::empty(9101);
    tk::install_ice(&mut vm, card("Vanilla"), ServerId::Hq, true);
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
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "the one printed subroutine ended the run: {}",
        t.tail(12)
    );
}

/// Tour Guide: "This ice gains \"[subroutine] End the run.\" for each rezzed
/// asset."
///
/// The count is a 9.12.2b calculated quantity inside a STATIC ability, so it
/// is not a number the ice was given once: 9.12.1d–e recompute effective
/// characteristics from printed ones every time they are read. Here that is
/// asserted directly against a real board — an unrezzed asset and a rezzed
/// UPGRADE are both on the table and neither counts.
#[test]
fn tour_guide_has_one_end_the_run_per_rezzed_asset() {
    let mut vm = Vm::empty(9102);
    let tg = tk::install_ice(&mut vm, card("Tour Guide"), ServerId::Hq, true);
    assert_eq!(
        vm.current_subs(tg).len(),
        0,
        "no rezzed asset, no subroutines — the printed card has none of its own"
    );

    tk::install_root(&mut vm, tk::vanilla_asset("First Asset", 1, 2), ServerId::Remote(1), true);
    assert_eq!(vm.current_subs(tg).len(), 1, "9.8.3d: one rezzed asset, one gained subroutine");

    tk::install_root(&mut vm, tk::vanilla_asset("Second Asset", 1, 2), ServerId::Remote(2), true);
    assert_eq!(vm.current_subs(tg).len(), 2, "two rezzed assets, two gained subroutines");

    // Neither of these is "a rezzed asset": one is an asset that is not
    // rezzed, the other is rezzed but is not an asset.
    tk::install_root(&mut vm, tk::vanilla_asset("Hidden Asset", 1, 2), ServerId::Remote(3), false);
    tk::install_root(&mut vm, tk::vanilla_upgrade("Some Upgrade", 1), ServerId::Hq, true);
    assert_eq!(
        vm.current_subs(tg).len(),
        2,
        "still two: an unrezzed asset and a rezzed upgrade are neither of them a rezzed asset"
    );
}

/// Tour Guide, on a board: with no rezzed asset the Runner walks past it, and
/// with one the run ends.
///
/// The rez happens INSIDE the run — in the paid ability window 6.9.2b opens
/// while the ice is approached — so the ice had nothing at all when the run
/// began, and the subroutine the Runner faces at 6.9.3c exists only because
/// the count was read again after the board changed. A lingering effect
/// created at rez time could not do this: 9.10.1 would have fixed the number
/// then and there.
#[test]
fn tour_guide_re_reads_the_count_during_the_run() {
    for rez_mid_run in [false, true] {
        let mut vm = Vm::empty(9103);
        let tg = tk::install_ice(&mut vm, card("Tour Guide"), ServerId::Hq, true);
        let asset =
            tk::install_root(&mut vm, tk::vanilla_asset("Gift Shop", 1, 2), ServerId::Remote(1), false);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Runner);
        assert_eq!(
            vm.current_subs(tg).len(),
            0,
            "the ice starts the run with nothing to resolve (rez_mid_run={rez_mid_run})"
        );

        let mut corp = Plan::corp();
        if rez_mid_run {
            corp = corp.when(Match::paid().approaching_ice().once(), Reply::Take(Pick::Rez(asset)));
        }
        let t = plan::play(
            &mut vm,
            corp,
            Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&asset].faceup, rez_mid_run,
            "the asset was rezzed exactly when the plan rezzed it (rez_mid_run={rez_mid_run}): {}",
            t.tail(20)
        );
        assert_eq!(
            vm.current_subs(tg).len(),
            usize::from(rez_mid_run),
            "the subroutine list is re-read, not remembered (rez_mid_run={rez_mid_run}): {}",
            t.tail(20)
        );
        let ended =
            vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. }));
        assert_eq!(
            ended, rez_mid_run,
            "the run ends exactly when the rez gave the ice a subroutine to resolve \
             (rez_mid_run={rez_mid_run}): {}",
            t.tail(20)
        );
    }
}

/// Tour Guide, the other direction: the count SHRINKS when a rezzed asset
/// leaves the board, and 9.8.3d takes the last-gained subroutine back first.
///
/// The asset is trashed the way the Runner really trashes one — accessed on a
/// run and paid for (7.1.5) — so nothing about the board is manufactured, and
/// the very next run walks through the ice that stopped them a moment ago.
#[test]
fn tour_guide_loses_a_subroutine_when_the_asset_is_trashed() {
    let mut vm = Vm::empty(9110);
    let tg = tk::install_ice(&mut vm, card("Tour Guide"), ServerId::Hq, true);
    tk::install_root(&mut vm, tk::vanilla_asset("Gift Shop", 1, 2), ServerId::Remote(1), true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);
    assert_eq!(vm.current_subs(tg).len(), 1, "one rezzed asset, one gained subroutine");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access(), Reply::trash_accessed())
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert_eq!(
        vm.current_subs(tg).len(),
        0,
        "the asset is gone, so the subroutine it was paying for is gone: {}",
        t.tail(24)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "and the run on HQ that followed met no subroutine at all: {}",
        t.tail(24)
    );
}

/// Drafter: "[subroutine] You may add 1 card from Archives to HQ."
/// "[subroutine] You may install 1 card from Archives or HQ, ignoring all
/// costs."
///
/// The install is the interesting half: 8.5.11a charges the Corp 1[credit]
/// for each piece of ice already protecting the destination server, and the
/// Corp here has nothing at all — so a card that lands anyway is 1.16.5c's
/// "ignoring all costs" observed rather than assumed.
#[test]
fn drafter_recovers_from_archives_and_installs_ignoring_all_costs() {
    let mut vm = Vm::empty(9104);
    tk::install_ice(&mut vm, card("Drafter"), ServerId::Rnd, true);
    // A remote already protected by two pieces of ice: installing a third
    // there normally costs 2[credit].
    tk::install_ice(&mut vm, tk::vanilla_ice("Outer Guard", 1, 1), ServerId::Remote(1), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Inner Guard", 1, 1), ServerId::Remote(1), false);
    let recovered = vm.new_object(tk::corp_filler("Recovered Card"), Zone::Discard(Side::Corp));
    let buried_ice = vm.new_object(tk::vanilla_ice("Buried Ice", 4, 4), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().extend([recovered, buried_ice]);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            // Both subroutines print "you may", which is 9.6.9d's optional
            // part inside the instruction: the Corp says yes twice.
            .when(Match::optional(), Reply::Optional(true))
            .when(Match::targets().once(), Reply::target(recovered))
            .when(Match::targets().once(), Reply::target(buried_ice))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1))),
            ),
        Plan::runner().when(Match::action().first(), Reply::run(ServerId::Rnd)).stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&recovered].zone,
        Zone::Hand(Side::Corp),
        "the first subroutine moved a card from Archives to HQ: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&buried_ice].zone,
        Zone::Ice(ServerId::Remote(1)),
        "the second subroutine installed a card out of Archives: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.corp.credits, 0,
        "1.16.5c: 8.5.11a's 2[credit] for the two pieces of ice already there was ignored: {}",
        t.tail(24)
    );
}

/// Vertigo: "[subroutine] The Runner loses [click]."
#[test]
fn vertigo_subroutine_takes_a_click_off_the_runner() {
    let mut vm = Vm::empty(9105);
    tk::install_ice(&mut vm, card("Vertigo"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);
    let allotted = vm.st.runner.allotted_clicks;
    assert!(allotted >= 2, "5.6.1: the Runner's turn allots clicks to lose");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    assert_eq!(
        vm.st.runner.clicks,
        allotted - 2,
        "1.11.3b: one click spent on the run action, one LOST to the subroutine: {}",
        t.tail(14)
    );
}

/// Vertigo: "When the Runner passes this ice, if they have no [click]
/// remaining, they cannot steal or trash Corp cards for the remainder of this
/// run."
///
/// Both acts the sentence names, on one board, against a control that differs
/// in nothing but the Runner's click pool at the moment of the pass. The
/// remote's root holds an agenda and a trashable upgrade, so one breach offers
/// the Runner exactly the two acts "steal or trash" forbids.
///
/// The click arithmetic is the whole setup, and Vertigo's own subroutine is
/// part of it. The Runner is allotted four clicks in both arms:
///
/// * PROHIBITED — three basic credit actions, then the run. One click is left
///   to spend on the run, so the Runner meets the ice at 0; the subroutine's
///   1.11.3b loss finds nothing to take and leaves them at 0; the pass reads 0.
/// * FREE — one basic credit action, then the run. Two clicks survive the run
///   action, the subroutine takes one, and the pass reads 1.
///
/// 9.6.5d is why the requirement is checked at the pass and not earlier: this
/// card's "if" follows its "when", which is Underworld Contact's template and
/// the CR's own example of a requirement that lives in the instructions.
///
/// 1.2.2 shows up differently for the two acts, and both are asserted:
/// stealing is not an option at all (7.2.3 makes it happen during the access),
/// so the agenda is accessed and simply stays in the root; trashing IS an
/// option, and the "cannot" WITHHOLDS it rather than failing it — the Runner
/// could pay the 2[credit] in either arm.
#[test]
fn vertigo_shuts_the_run_to_stealing_and_trashing_when_the_runner_has_no_clicks() {
    for (credit_actions, prohibited) in [(1usize, false), (3usize, true)] {
        let mut vm = Vm::empty(9106);
        tk::install_ice(&mut vm, card("Vertigo"), ServerId::Remote(1), true);
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Loose Agenda", 3, 2),
            ServerId::Remote(1),
            false,
        );
        let upgrade = tk::install_root(
            &mut vm,
            {
                let mut u = tk::vanilla_upgrade("Trashable Upgrade", 0);
                u.trash_cost = Some(2);
                u
            },
            ServerId::Remote(1),
            true,
        );
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);
        assert_eq!(
            vm.st.runner.allotted_clicks, 4,
            "5.6.1: the arithmetic above is written for the four clicks a Runner's turn allots"
        );

        let t = plan::play(
            &mut vm,
            // Stop once the Runner's whole turn has gone by.
            Plan::corp().when(Match::action().first(), Reply::Halt),
            Plan::runner()
                .when(Match::action().times(credit_actions), Reply::credit())
                .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                .when(Match::mid_access().once(), Reply::Take(Pick::BasicTrash))
                .otherwise_click_credit(),
        );

        for card_id in [agenda, upgrade] {
            assert!(
                vm.changes
                    .log
                    .iter()
                    .any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == card_id)),
                "7.2/7.3: both root cards were accessed either way — the sentence forbids \
                 the acts, not the access (prohibited={prohibited}): {}",
                t.tail(40)
            );
        }
        assert_eq!(
            vm.st.objects[&agenda].zone == Zone::ScoreArea(Side::Runner),
            !prohibited,
            "1.2.2/7.5: the agenda is stolen only when the Runner still had a [click] at \
             the pass (prohibited={prohibited}): {}",
            t.tail(40)
        );
        let trash_was_offered = t
            .entries
            .iter()
            .flat_map(|e| e.options())
            .any(|o| matches!(o, jinteki_cr::decision::WindowOption::BasicTrash { card, .. } if *card == upgrade));
        assert_eq!(
            trash_was_offered, !prohibited,
            "1.2.2/7.1.5: the basic trash ability is WITHHELD rather than failed, and the \
             Runner held 5[credit] against a trash cost of 2 in both arms \
             (prohibited={prohibited}): {}",
            t.tail(40)
        );
        assert_eq!(
            vm.st.objects[&upgrade].zone == Zone::Discard(Side::Corp),
            !prohibited,
            "…so the upgrade is trashed only when nothing forbade it \
             (prohibited={prohibited}): {}",
            t.tail(40)
        );
    }
}

/// Fairchild 3.0: "Lose [click][click][click]: Break up to 3 subroutines on
/// this ice. **Only the Runner can use this ability.**"
///
/// The last clause is CR 1.14.4b — "some abilities state that they can only
/// be used by a specific player; the specified player controls each such
/// ability, **even if they do not control its source**" — and it is asserted
/// in both directions, because the rule it modifies has two halves. 1.14.4
/// gives an ability to its source's controller *by default* and lets a player
/// use "only abilities they control", so naming the Runner does two things at
/// once: the Runner is offered it, and the Corp — who owns the ice it is
/// printed on — never is.
///
/// 1.14.3 is the third assertion: "a player can only pay costs using objects
/// they control". The three clicks come out of the RUNNER's pool, which is
/// shown twice over — the Runner's pool empties, and a Runner who is one
/// click short is not offered the ability at all (1.16.1), while the Corp's
/// pool is 0 in both arms and could never have paid for anything.
///
/// 5.2.1a is what makes any of it reachable: "other costs can contain [click]
/// symbols without denoting an action", so a cost beginning with *Lose* is
/// used in a paid window (9.5.1) rather than an action window — and an
/// encounter, which is where 9.5.6a confines a break ability, has paid windows
/// and no action windows.
#[test]
fn fairchild_3_0_break_is_the_runners_ability_paid_from_the_runners_own_pool() {
    // Enough clicks: 4 allotted − 1 for the run action leaves exactly 3.
    let mut vm = Vm::empty(9110);
    tk::install_ice(&mut vm, card("Fairchild 3.0"), ServerId::Hq, true);
    let prog = tk::install_rig(&mut vm, tk::vanilla_runner_card("Sacrifice", CardType::Program));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 7;
    vm.start_turn(Side::Runner);
    assert_eq!(vm.st.corp.clicks, 0, "1.11.2: the Corp has no clicks during the Runner's turn");

    // The Corp's first action window is the stopping point, so the transcript
    // is exactly the Runner's turn.
    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().offering("bioroid break").once(), Reply::take("bioroid break"))
            // The subroutine announcement falls through to the neutral
            // policy, which takes the first `count` offered — all three.
            .stop_at_action(),
    );

    // 1.14.4b, first half: the named player controls it and is offered it.
    assert!(
        t.ever_offered_to(Side::Runner, "bioroid break"),
        "1.14.4b: 'Only the Runner can use this ability' gives the Runner an \
         ability printed on the Corp's ice: {}",
        t.tail(24)
    );
    // 1.14.4b, second half: and takes it away from the source's controller.
    assert!(
        !t.ever_offered_to(Side::Corp, "bioroid break"),
        "1.14.4/1.14.4b: a player can only use abilities they control, and \
         this one is no longer the Corp's even though the ice is: {}",
        t.tail(24)
    );
    // 1.14.3: the cost is paid with objects the ability's controller
    // controls — three clicks off the RUNNER, recorded as a loss (1.11.3b).
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::ClicksLost { side: Side::Runner, amount: 3 })),
        "1.14.3/5.2.1a: the three clicks were LOST by the RUNNER — the ability's \
         controller pays it, not the controller of the ice: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.clicks, 0,
        "1.11.2b: 4 allotted − 1 spent on the run action − 3 lost to the break: {}",
        t.tail(24)
    );
    assert_eq!(
        t.windows(Kind::Action, Side::Runner).len(),
        1,
        "…and an emptied pool is the observable half of it: the Runner got one \
         action window all turn, because the break took the other three: {}",
        t.tail(24)
    );
    // And what the sentence promises happened: three subroutines broken, so
    // none of the three resolved.
    assert_eq!(
        vm.st.runner.credits, 7,
        "both 'pay 3[credit] or trash' subroutines were broken, so neither ran: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&prog].zone,
        Zone::Rig,
        "the installed card the second subroutine could have taken is still installed: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.core_damage, 0,
        "the third subroutine was broken too, so no core damage: {}",
        t.tail(24)
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "…and it could not end the run either: {}",
        t.tail(24)
    );

    // One click short. Nothing else changes — and the Corp still has 0
    // clicks, so if the pool consulted were the source controller's the
    // ability could never be offered at all.
    let mut vm = Vm::empty(9111);
    tk::install_ice(&mut vm, card("Fairchild 3.0"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 7;
    vm.start_turn(Side::Runner);
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::options(), Reply::ChooseNamed("end the run")),
        Plan::runner()
            // A basic credit first, so only 2[click] are left at the encounter.
            .when(Match::action().once(), Reply::credit())
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::paid().offering("bioroid break").once(), Reply::take("bioroid break"))
            .stop_at_action(),
    );
    assert!(
        !t.ever_offered("bioroid break"),
        "1.16.1: with 2[click] left the Runner cannot pay a 3[click] cost, so \
         the ability is not offered to anyone: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.credits, 2,
        "8 − 3 − 3: unbroken, the two mandatory-choice subroutines resolved: {}",
        t.tail(24)
    );

    // "…on THIS ice" (9.5.6c). Vanilla stands in front of Fairchild, so the
    // encounter the Runner reaches first is with a different piece of ice —
    // and the ability that refers to *this* one is not offered there, nor is
    // Vanilla's subroutine ever a candidate for it.
    let mut vm = Vm::empty(9112);
    tk::install_ice(&mut vm, card("Fairchild 3.0"), ServerId::Hq, true);
    tk::install_ice(&mut vm, card("Vanilla"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 7;
    vm.start_turn(Side::Runner);
    let t = plan::play(
        &mut vm,
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert!(
        !t.ever_offered("bioroid break"),
        "9.5.6c: 'break up to 3 subroutines on THIS ice' refers to the \
         encountered ice as being this card, so an encounter with Vanilla \
         does not meet the stipulation: {}",
        t.tail(24)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "…and with nothing to break it with, Vanilla's own subroutine ended \
         the run: {}",
        t.tail(24)
    );
}

/// Fairchild 3.0: "[subroutine] The Runner must pay 3[credit] or trash 1 of
/// their installed cards." — printed twice.
///
/// CR 9.12.3c is the whole of it: the Runner must choose an option that can
/// be **fully resolved**. A Runner with credits and an empty rig has exactly
/// one such option and pays; a Runner who cannot afford 3 has exactly one and
/// trashes.
///
/// 9.5.3 makes the bioroid break ability optional, and these Runners decline
/// it, so the subroutines are left to resolve.
#[test]
fn fairchild_3_0_subroutines_are_a_mandatory_choice() {
    // Rich, empty rig: only "pay 3[credit]" is fully resolvable, twice.
    let mut vm = Vm::empty(9106);
    tk::install_ice(&mut vm, card("Fairchild 3.0"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 7;
    vm.start_turn(Side::Runner);
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::options(), Reply::ChooseNamed("end the run")),
        Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    assert_eq!(
        vm.st.runner.credits, 1,
        "7 − 3 − 3: nothing installed, so 9.12.3c left only the payment: {}",
        t.tail(24)
    );

    // Poor, one installed card: only the trash is fully resolvable, and the
    // SECOND subroutine then has no resolvable option at all — 9.12.3c says
    // such an ability does nothing, so the 2[credit] survives.
    let mut vm = Vm::empty(9107);
    tk::install_ice(&mut vm, card("Fairchild 3.0"), ServerId::Hq, true);
    let prog = tk::install_rig(&mut vm, tk::vanilla_runner_card("Sacrifice", CardType::Program));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::options(), Reply::ChooseNamed("end the run")),
        Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&prog].zone,
        Zone::Discard(Side::Runner),
        "2[credit] cannot pay 3, so the only fully resolvable option was the trash: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.credits, 2,
        "9.12.3c: with nothing left to trash and 3[credit] unaffordable, the second \
         subroutine did nothing at all: {}",
        t.tail(24)
    );
}

/// Fairchild 3.0: "[subroutine] Do 1 core damage or end the run."
///
/// The sentence names no player, so 1.14.4 leaves the choice with the
/// ability's controller — the Corp — which is the whole difference from the
/// two subroutines above.
#[test]
fn fairchild_3_0_lets_the_corp_pick_damage_or_the_end_of_the_run() {
    for corp_ends_it in [false, true] {
        let mut vm = Vm::empty(9108);
        tk::install_ice(&mut vm, card("Fairchild 3.0"), ServerId::Hq, true);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        // Nothing installed and nothing to spend: 9.12.3c empties the first
        // two subroutines, leaving the third the only one that speaks.
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let option = if corp_ends_it { "end the run" } else { "do 1 core damage" };
        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::options(), Reply::ChooseNamed(option)),
            Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action(),
        );
        let ended =
            vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. }));
        assert_eq!(
            ended, corp_ends_it,
            "the run ends exactly when the Corp chose that branch (corp_ends_it={corp_ends_it}): {}",
            t.tail(24)
        );
        assert_eq!(
            vm.st.runner.core_damage,
            u32::from(!corp_ends_it),
            "the core damage lands exactly when the Corp chose that branch \
             (corp_ends_it={corp_ends_it}): {}",
            t.tail(24)
        );
    }
}

/// Tatu-Bola: "When the Runner passes this ice, you may swap it with a piece
/// of ice from HQ. If you do, gain 4[credit]. (The new ice is installed
/// unrezzed. You do not pay an install cost.)"
///
/// The Runner has to actually pass it, which means breaking the printed
/// subroutine first — an unrezzed piece of ice has no active abilities at all
/// (9.1.7), so there is no shortcut. 8.8.4a/b is the parenthetical: the card
/// that arrives from HQ takes the vacated position without the 8.5.16 install
/// procedure and enters the play area the way a Corp card enters it, unrezzed.
///
/// "If you do" is asked as well as told: with no ice in HQ there is nothing to
/// swap with, and the 4[credit] must not arrive.
#[test]
fn tatu_bola_trades_places_with_hq_and_only_then_gains_four() {
    for ice_in_hq in [true, false] {
        let mut vm = Vm::empty(9109);
        let tatu = tk::install_ice(&mut vm, card("Tatu-Bola"), ServerId::Hq, true);
        tk::install_rig(&mut vm, tk::break_button("Breaker"));
        let understudy = ice_in_hq.then(|| {
            let o = vm.new_object(tk::vanilla_ice("Understudy", 3, 3), Zone::Hand(Side::Corp));
            vm.st.hand.get_mut(&Side::Corp).unwrap().push(o);
            o
        });
        tk::fill_hand(&mut vm, Side::Corp, 2);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Runner);

        // 9.6.9c: the printed "you may" is the Corp's, offered as a reaction
        // window option once the Runner has passed the ice.
        let mut corp = Plan::corp()
            .when(Match::reaction().offering("trade places"), Reply::take("trade places"))
            .when(Match::optional(), Reply::Optional(true));
        if let Some(u) = understudy {
            corp = corp.when(Match::targets().once(), Reply::target(u));
        }
        let t = plan::play(
            &mut vm,
            corp,
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Hq))
                .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("break"))
                .when(Match::sub_targets().once(), Reply::SubroutineNamed("[sub]"))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.corp.credits,
            if ice_in_hq { 4 } else { 0 },
            "the gain follows the swap and nothing else (ice_in_hq={ice_in_hq}): {}",
            t.tail(28)
        );
        if let Some(u) = understudy {
            assert_eq!(
                vm.st.objects[&tatu].zone,
                Zone::Hand(Side::Corp),
                "Tatu-Bola went to HQ: {}",
                t.tail(28)
            );
            assert_eq!(
                vm.st.objects[&u].zone,
                Zone::Ice(ServerId::Hq),
                "and the piece of ice from HQ took its position: {}",
                t.tail(28)
            );
            assert!(
                !vm.st.objects[&u].faceup,
                "8.8.4a: the new ice is installed unrezzed: {}",
                t.tail(28)
            );
        } else {
            assert_eq!(
                vm.st.objects[&tatu].zone,
                Zone::Ice(ServerId::Hq),
                "with nothing in HQ to swap with, Tatu-Bola stayed put: {}",
                t.tail(28)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Mezzie's Asa — the assets
// ---------------------------------------------------------------------------

/// Estelle Moon: "Whenever you install a card in the root of a remote server,
/// place 1 power counter on this asset." / "[trash]: For each power counter
/// on this asset, gain 2[credit] and draw 1 card."
///
/// The same two install actions twice, changing only WHERE the cards go: into
/// the roots of two new remotes, or protecting two new remotes. 4.6.6e and
/// 4.6.9d are the whole of the difference and the payout is where it shows —
/// four credits and two cards against nothing whatever.
///
/// 9.5.5 is the other half: the [trash] cost uninstalls the source before the
/// effects resolve, and the counters are still counted because the rule sets
/// them aside as the cost is paid.
#[test]
fn estelle_moon_counts_remote_roots_and_not_the_ice_in_front_of_them() {
    for into_roots in [false, true] {
        let mut vm = Vm::empty(9120);
        let em = tk::install_root(&mut vm, card("Estelle Moon"), ServerId::Remote(1), true);
        let (c1, c2) = if into_roots {
            (
                vm.new_object(tk::vanilla_asset("Branch Office", 0, 2), Zone::Hand(Side::Corp)),
                vm.new_object(tk::vanilla_asset("Field Office", 0, 2), Zone::Hand(Side::Corp)),
            )
        } else {
            (
                vm.new_object(tk::vanilla_ice("Outer Guard", 0, 1), Zone::Hand(Side::Corp)),
                vm.new_object(tk::vanilla_ice("Inner Guard", 0, 1), Zone::Hand(Side::Corp)),
            )
        };
        vm.st.hand.get_mut(&Side::Corp).unwrap().extend([c1, c2]);
        tk::fill_deck(&mut vm, Side::Corp, 10);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        // Both destinations create a brand-new remote (8.5.2a/8.5.2d), so
        // neither arm pays 8.5.11a's per-ice install cost and the two arms
        // differ in nothing but the location word.
        let dest = if into_roots {
            jinteki_cr::instr::InstallDest::NewRemoteRoot
        } else {
            jinteki_cr::instr::InstallDest::NewRemoteProtecting
        };
        vm.start_turn(Side::Corp);

        let t1 = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::Take(Pick::InstallCard(c1)))
                .when(Match::destination().once(), Reply::Destination(dest))
                .when(Match::action(), Reply::Halt),
            Plan::runner(),
        );
        assert_eq!(
            vm.st.objects[&em].counter(CounterKind::Power),
            u32::from(into_roots),
            "one install, and the counter arrives only for the root (into_roots={into_roots}): {}",
            t1.tail(20)
        );

        let credits_before = vm.st.corp.credits;
        let hand_before = vm.st.hand[&Side::Corp].len();
        let t2 = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::Take(Pick::InstallCard(c2)))
                .when(Match::destination().once(), Reply::Destination(dest))
                .when(Match::paid().once(), Reply::take("estelle moon: cash the counters in"))
                .when(Match::action(), Reply::Halt),
            Plan::runner(),
        );
        let counters = if into_roots { 2u32 } else { 0 };
        assert_eq!(
            vm.st.objects[&em].zone,
            Zone::Discard(Side::Corp),
            "the [trash] trigger cost was paid whatever the count was \
             (into_roots={into_roots}): {}",
            t2.tail(24)
        );
        assert_eq!(
            vm.st.corp.credits,
            credits_before + 2 * counters,
            "2[credit] for each of the {counters} counters (into_roots={into_roots}): {}",
            t2.tail(24)
        );
        assert_eq!(
            vm.st.hand[&Side::Corp].len(),
            hand_before - 1 + counters as usize,
            "one card left HQ to be installed and {counters} came back \
             (into_roots={into_roots}): {}",
            t2.tail(24)
        );
    }
}

/// Lakshmi Smartfabrics: "Whenever you rez a card, place 1 power counter on
/// Lakshmi Smartfabrics."
///
/// The sentence says nothing at all about the card rezzed, so the card's own
/// rez counts — which is what the UFAQ was asked, and 8.1.3 is why: the card
/// is faceup and active before the rez finishes, so the ability is there to
/// be met by the occurrence that activated it. The second arm rezzes one more
/// card and the count follows it.
#[test]
fn lakshmi_smartfabrics_counts_its_own_rez_and_every_other() {
    for rez_the_neighbour in [false, true] {
        let mut vm = Vm::empty(9121);
        let lak =
            tk::install_root(&mut vm, card_partial("Lakshmi Smartfabrics"), ServerId::Remote(1), false);
        let other =
            tk::install_root(&mut vm, tk::vanilla_asset("Sample Room", 1, 2), ServerId::Remote(2), false);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Corp);

        let mut corp = Plan::corp().when(Match::paid().once(), Reply::rez(lak));
        if rez_the_neighbour {
            corp = corp.when(Match::paid().once(), Reply::rez(other));
        }
        let t = plan::play(&mut vm, corp.stop_at_action(), Plan::runner());

        assert!(
            vm.st.objects[&lak].faceup,
            "the asset was rezzed (rez_the_neighbour={rez_the_neighbour}): {}",
            t.tail(20)
        );
        assert_eq!(
            vm.st.objects[&other].faceup, rez_the_neighbour,
            "the neighbour was rezzed exactly when the plan rezzed it \
             (rez_the_neighbour={rez_the_neighbour}): {}",
            t.tail(20)
        );
        assert_eq!(
            vm.st.objects[&lak].counter(CounterKind::Power),
            1 + u32::from(rez_the_neighbour),
            "one counter for its own rez, and one more for the neighbour's \
             (rez_the_neighbour={rez_the_neighbour}): {}",
            t.tail(20)
        );
    }
}

/// Marilyn Campaign: "When you rez this asset, load 8[credit] onto it."
///
/// The trigger is the REZ and not the install, because a Corp card installed
/// facedown is inactive (9.1.8) and has no ability to meet anything with. The
/// two arms are the same board with the same asset in it, rezzed or not, and
/// the credits on the card are the difference.
///
/// The rez is made during the RUNNER's turn, where the Corp rezzes cards as
/// freely as on its own (8.1.1) and where nothing else on this card can move
/// a credit: "when your turn begins" names the CORP's turn, so the 8 loaded
/// here are the 8 the rez put there and not a payout's leavings.
#[test]
fn marilyn_campaign_loads_eight_credits_when_it_is_rezzed_and_not_before() {
    for rez_it in [false, true] {
        let mut vm = Vm::empty(9122);
        let mc = tk::install_root(&mut vm, card_partial("Marilyn Campaign"), ServerId::Remote(1), false);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 5;
        vm.start_turn(Side::Runner);

        let mut corp = Plan::corp();
        if rez_it {
            corp = corp.when(Match::paid().once(), Reply::rez(mc));
        }
        let t = plan::play(&mut vm, corp, Plan::runner().stop_at_action());

        assert_eq!(
            vm.st.objects[&mc].faceup, rez_it,
            "the asset was rezzed exactly when the plan rezzed it (rez_it={rez_it}): {}",
            t.tail(16)
        );
        assert_eq!(
            vm.st.objects[&mc].counter(CounterKind::Credit),
            if rez_it { 8 } else { 0 },
            "1.9.4: the 8[credit] are loaded by the rez and by nothing else (rez_it={rez_it}): {}",
            t.tail(16)
        );
        assert_eq!(
            vm.st.corp.credits,
            if rez_it { 3 } else { 5 },
            "the 2[credit] rez cost left the pool and none of the loaded credits arrived in it \
             (rez_it={rez_it}): {}",
            t.tail(16)
        );
    }
}

/// Marilyn Campaign: "When it is empty, trash it." / "When your turn begins,
/// take 2[credit] from this asset."
///
/// LOADING is what links the "empty" ability to the card (1.9.4), so both
/// ends are asserted from a loaded card: a payout that moves credits off the
/// card and into the pool (1.10.3a — a gain, which is why the card runs out),
/// and the self-trash once the last of them has gone.
#[test]
fn marilyn_campaign_pays_two_a_turn_and_trashes_itself_when_empty() {
    for (start_credits, left, trashed) in [(8u32, 6u32, false), (2u32, 0u32, true)] {
        let mut vm = Vm::empty(9123);
        let mc = tk::install_root(&mut vm, card_partial("Marilyn Campaign"), ServerId::Remote(1), true);
        let o = vm.st.objects.get_mut(&mc).unwrap();
        o.counters.insert(CounterKind::Credit, start_credits);
        o.loaded_kinds.insert(CounterKind::Credit);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());

        assert_eq!(
            vm.st.corp.credits, 2,
            "1.10.3a: the credits left the card for the pool (start_credits={start_credits}): {}",
            t.tail(16)
        );
        if trashed {
            assert_eq!(
                vm.st.objects[&mc].zone,
                Zone::Discard(Side::Corp),
                "emptied, so it trashed itself (start_credits={start_credits}): {}",
                t.tail(16)
            );
        } else {
            assert_eq!(
                vm.st.objects[&mc].counter(CounterKind::Credit),
                left,
                "two off the card, the rest still on it (start_credits={start_credits}): {}",
                t.tail(16)
            );
            assert_ne!(
                vm.st.objects[&mc].zone,
                Zone::Discard(Side::Corp),
                "not empty yet, so still installed (start_credits={start_credits}): {}",
                t.tail(16)
            );
        }
    }
}

/// MCA Austerity Policy: "Once per turn → [click]: Place 1 power counter on
/// this asset. When the Runner's next turn begins, they lose [click]."
///
/// The delayed conditional (9.6.13) is armed on the Corp's turn and resolves
/// at the beginning of the Runner's, and what it does is 1.11.3b's LOSS — so
/// the observable consequence is a whole action the Runner never gets. Both
/// arms spend every click the Runner has on the basic credit action (5.2.7b),
/// and the credits they end with count the clicks for us: four against three.
#[test]
fn mca_austerity_policy_costs_the_runner_a_click_next_turn() {
    for corp_uses_it in [false, true] {
        let mut vm = Vm::empty(9124);
        let mca = tk::install_root(&mut vm, card("MCA Austerity Policy"), ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        tk::fill_hand(&mut vm, Side::Runner, 3);
        vm.st.corp.credits = 5;
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Corp);

        // 1.11.3c: an ability whose trigger cost begins with [click] is an
        // ACTION, so it is offered in the action window and not in a paid one.
        let mut corp = Plan::corp();
        if corp_uses_it {
            corp = corp.when(
                Match::action().once(),
                Reply::take("mca austerity policy: a counter, and a click off the runner"),
            );
        }
        let t = plan::play(
            &mut vm,
            // The Corp's turn runs out, the Runner's whole turn passes, and
            // the driver stops at the Corp's next action window.
            corp.when(Match::action().nth(if corp_uses_it { 3 } else { 4 }), Reply::Halt)
                .otherwise_click_credit(),
            Plan::runner().otherwise_click_credit(),
        );

        assert_eq!(
            vm.st.objects[&mca].counter(CounterKind::Power),
            u32::from(corp_uses_it),
            "the counter is placed exactly when the ability is used \
             (corp_uses_it={corp_uses_it}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.runner.credits,
            if corp_uses_it { 3 } else { 4 },
            "1.11.3b: a click LOST at the start of the turn is an action the Runner never takes \
             (corp_uses_it={corp_uses_it}): {}",
            t.tail(30)
        );
    }
}

/// MCA Austerity Policy: "[click], [trash], 3 hosted power counters: Gain
/// [click][click][click][click]."
///
/// 1.16.1b decides both arms: the cost is three costs paid as one (1.16.10b)
/// and a card hosting only two counters cannot pay it at all, so the ability
/// is not there to be used. Where it can be paid, 9.5.5 is what makes it
/// payable — the [trash] uninstalls the source, so the counters are set aside
/// as the whole cost is paid rather than going back to the bank first.
///
/// The Corp spends every click it has left on the basic credit action, so the
/// credits it finishes with are the clicks it had: 3 − 1 spent + 4 gained
/// against a plain 3.
#[test]
fn mca_austerity_policy_cashes_in_for_four_clicks_only_with_three_counters() {
    for counters in [2u32, 3u32] {
        let mut vm = Vm::empty(9125);
        let mca = tk::install_root(&mut vm, card("MCA Austerity Policy"), ServerId::Remote(1), true);
        vm.st.objects.get_mut(&mca).unwrap().counters.insert(CounterKind::Power, counters);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(
                    Match::action().once(),
                    Reply::take("mca austerity policy: cash in for four clicks"),
                )
                .otherwise_click_credit(),
            Plan::runner().when(Match::action(), Reply::Halt),
        );

        let payable = counters == 3;
        assert_eq!(
            vm.st.objects[&mca].zone == Zone::Discard(Side::Corp),
            payable,
            "1.16.1b: the [trash] happens only where the whole cost could be paid \
             (counters={counters}): {}",
            t.tail(30)
        );
        assert_eq!(
            vm.st.corp.credits,
            if payable { 6 } else { 3 },
            "the clicks the Corp ended up with, counted in basic credit actions \
             (counters={counters}): {}",
            t.tail(30)
        );
    }
}

/// Jeeves Model Bioroids: "The first time you spend 3[click] on the same
/// action each turn, gain [click]."
///
/// 5.2.6h's basic purge is ONE action that costs three clicks, which is the
/// cleanest thing in the game that meets this condition — and the Corp's turn
/// allots exactly three clicks, so the whole turn goes into it. What the Corp
/// has left when the next action window opens is the card: 3 − 3 = 0 without
/// Jeeves, and 1 with it.
///
/// The control arm is the basic credit action, which costs one click. It shows
/// the threshold is real and, together with
/// [`jeeves_model_bioroids_does_not_count_three_separate_one_click_actions`],
/// that the condition is about CLICKS ON ONE ACTION and not about actions.
#[test]
fn jeeves_model_bioroids_hands_back_a_click_after_three_on_one_action() {
    for (purge, clicks_left) in [(false, 2u32), (true, 1u32)] {
        let mut vm = Vm::empty(9126);
        tk::install_root(&mut vm, card("Jeeves Model Bioroids"), ServerId::Remote(1), true);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 0;
        vm.start_turn(Side::Corp);
        assert_eq!(
            vm.st.corp.allotted_clicks, 3,
            "5.6.1: the Corp's turn allots three clicks, which is exactly one purge"
        );

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(
                    Match::action().once(),
                    if purge { Reply::Take(Pick::Purge) } else { Reply::credit() },
                )
                .stop_at_action(),
            Plan::runner(),
        );

        assert_eq!(
            t.ever_offered("three clicks on one action"),
            purge,
            "1.16.4d: the condition is met by the clicks spent to TAKE one action \
             (purge={purge}): {}",
            t.tail(20)
        );
        assert_eq!(
            vm.st.corp.clicks, clicks_left,
            "the Corp's pool after the action: 3 − 3 + 1 against 3 − 1 (purge={purge}): {}",
            t.tail(20)
        );
    }
}

/// Jeeves Model Bioroids, the other half of "on the SAME action": three basic
/// credit actions spend three clicks between them and meet nothing.
///
/// 5.2.5a makes all three of them the same action — the same basic action —
/// so this is also the arm that separates this card from The Collective's
/// "the same action three times in a row", which those three DO meet.
/// 1.16.4d counts the clicks spent to take ONE action, and each of these
/// actions cost one.
#[test]
fn jeeves_model_bioroids_does_not_count_three_separate_one_click_actions() {
    let mut vm = Vm::empty(9127);
    tk::install_root(&mut vm, card("Jeeves Model Bioroids"), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        // Stop once the Corp's whole turn has gone by.
        Plan::runner().when(Match::action().first(), Reply::Halt),
    );

    assert!(
        !t.ever_offered("three clicks on one action"),
        "1.16.4d / 5.2.5a: three actions costing one click each are not three clicks \
         spent on one action: {}",
        t.tail(30)
    );
    assert_eq!(
        vm.st.corp.credits, 3,
        "…and the Corp took exactly three basic credit actions, so three clicks were \
         spent in the turn: {}",
        t.tail(30)
    );
}

// ---------------------------------------------------------------------------
// Mezzie's Valencia (docs/vm/MEZZIE-QUEUE.md) — programs and hardware
// ---------------------------------------------------------------------------

/// Black Orchestra: "Whenever you encounter a code gate, you may install this
/// program from your heap." / "3[credit]: +2 strength. Then, if this program
/// can interface with the code gate you are encountering, break up to 2
/// subroutines."
///
/// Both halves of the sentence, on a board. CR 9.1.8b is what lets the first
/// one act from a zone 4.4.4 makes inactive; 9.6.5d is what puts the
/// interface question after "+2 strength", so a printed 2 that could never
/// have matched a strength-4 code gate when the ability was offered breaks
/// its subroutine anyway.
///
/// The code gate carries two subroutines and the Runner announces one of
/// them, which is what "up to 2" allows (9.8.6). The other resolves: the run
/// carrying on past a broken "End the run" is the printed promise, and the
/// 5[credit] the second subroutine hands over is the proof that the encounter
/// did not stop where the Corp wanted it to.
#[test]
fn black_orchestra_installs_itself_from_the_heap_and_breaks_the_code_gate() {
    for breaks in [false, true] {
        let mut vm = Vm::empty(9301);
        let gate = tk::install_ice(&mut vm, tk::little_engine_like("Sunburst"), ServerId::Hq, true);
        vm.st.objects.get_mut(&gate).unwrap().printed.subtypes = vec!["Code Gate"];
        let bo = vm.new_object(card("Black Orchestra"), Zone::Discard(Side::Runner));
        vm.st.discard.get_mut(&Side::Runner).unwrap().push(bo);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        vm.st.runner.credits = 8;
        vm.start_turn(Side::Runner);

        let mut runner = Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(
                Match::reaction().offering("black orchestra: out of the heap").once(),
                Reply::take("black orchestra: out of the heap"),
            )
            .when(Match::destination(), Reply::Destination(jinteki_cr::instr::InstallDest::Rig));
        if breaks {
            runner = runner
                .when(Match::paid().once(), Reply::take("black orchestra: pump and break"))
                .when(Match::sub_targets().once(), Reply::SubroutineNamed("End the run"));
        }
        let t = plan::play(&mut vm, Plan::corp(), runner.stop_at_action());

        // The install is the same in both arms: 9.1.8b does not depend on what
        // the Runner does next.
        assert_eq!(
            vm.st.objects[&bo].zone,
            Zone::Rig,
            "9.1.8b: the ability worked from the heap (breaks={breaks}): {}",
            t.tail(24)
        );
        let broken = vm
            .changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::SubroutineBroken { .. }))
            .count();
        assert_eq!(
            broken,
            usize::from(breaks),
            "9.6.5d: strength 2 + 2 clears the strength-4 code gate at RESOLUTION, \
             and only the arm that used the ability broke anything (breaks={breaks}): {}",
            t.tail(24)
        );
        assert_eq!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
            breaks,
            "the broken \"End the run\" did not end the run (breaks={breaks}): {}",
            t.tail(24)
        );
        assert_eq!(
            vm.st.runner.credits,
            // 8 − 3 install, then − 3 for the ability and + 5 from the
            // subroutine the Runner left unbroken.
            if breaks { 7 } else { 5 },
            "the unannounced subroutine still resolved, which it could only do \
             on the far side of the broken one (breaks={breaks}): {}",
            t.tail(24)
        );
    }
}

/// MKUltra: "Whenever you encounter a sentry, you may install this program
/// from your heap." / "3[credit]: +2 strength. Then, if this program can
/// interface with the sentry you are encountering, break up to 2
/// subroutines."
///
/// The same two sentences as Black Orchestra with a different subtype, so
/// what is under test here is the OTHER thing 9.6.5d decides: how many times
/// the ability has to be used before the break can happen at all. MKUltra's
/// printed 1 against a strength-4 sentry is 3 after one use and 5 after two,
/// and 3.9.5g is the comparison — so the first arm pays 3[credit], pumps, and
/// breaks nothing, while the second pays 6 and gets through.
#[test]
fn mkultra_breaks_the_sentry_only_once_it_has_pumped_high_enough() {
    for uses in [1usize, 2usize] {
        let mut vm = Vm::empty(9302);
        let sentry = tk::install_ice(&mut vm, tk::etr_ice("Grim Visage", 0, 4), ServerId::Hq, true);
        vm.st.objects.get_mut(&sentry).unwrap().printed.subtypes = vec!["Sentry"];
        let mk = vm.new_object(card("MKUltra"), Zone::Discard(Side::Runner));
        vm.st.discard.get_mut(&Side::Runner).unwrap().push(mk);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        vm.st.runner.credits = 10;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Hq))
                .when(
                    Match::reaction().offering("mkultra: out of the heap").once(),
                    Reply::take("mkultra: out of the heap"),
                )
                .when(Match::destination(), Reply::Destination(jinteki_cr::instr::InstallDest::Rig))
                .when(Match::paid().times(uses), Reply::take("mkultra: pump and break"))
                .when(Match::sub_targets().once(), Reply::SubroutineNamed("End the run"))
                .stop_at_action(),
        );

        assert_eq!(
            vm.st.objects[&mk].zone,
            Zone::Rig,
            "9.1.8b: the encounter installed it out of the heap (uses={uses}): {}",
            t.tail(28)
        );
        let broke = uses == 2;
        assert_eq!(
            vm.changes
                .log
                .iter()
                .filter(|c| matches!(c, GameChange::SubroutineBroken { .. }))
                .count(),
            usize::from(broke),
            "3.9.5g/9.6.5d: strength 1 + 2 is under the sentry's 4 and breaks nothing; \
             1 + 2 + 2 is over it and breaks (uses={uses}): {}",
            t.tail(28)
        );
        assert_eq!(
            vm.changes
                .log
                .iter()
                .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
            broke,
            "the run continued exactly where the subroutine was broken (uses={uses}): {}",
            t.tail(28)
        );
        assert_eq!(
            vm.st.runner.credits,
            // 10 − 2 install − 3 per use.
            if broke { 2 } else { 5 },
            "each use of the ability cost 3[credit] whether or not it broke anything \
             (uses={uses}): {}",
            t.tail(28)
        );
    }
}

/// Zer0: "Once per turn → [click], suffer 1 net damage: Gain 1[credit] and
/// draw 2 cards."
///
/// Everything left of the colon is cost (1.16.1) and everything right of it
/// is effect, and the drive is three of the Runner's turns so that both
/// halves of 9.3.6g's flag are observed: it is spent by USING (9.1.6), so the
/// second action window of a turn is not offered it — and it is spent per
/// TURN, so the next turn is.
///
/// The Runner's other clicks go on runs against an empty Archives, which pay
/// nothing, so the credit pool at the end counts Zer0's payouts and nothing
/// else.
#[test]
fn zer0_costs_a_click_and_a_card_and_pays_once_each_turn() {
    const ZER0: &str = "zer0: bleed for a credit and two cards";
    let mut vm = Vm::empty(9303);
    let z = tk::install_rig(&mut vm, card("Zer0"));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Runner, 12);
    tk::fill_deck(&mut vm, Side::Corp, 12);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().offering(ZER0).times(2), Reply::take(ZER0))
            // The third turn's offer is where the drive stops: reaching it is
            // itself the assertion that a new turn hands the flag back.
            .when(Match::action().offering(ZER0), Reply::Halt)
            .when(Match::action(), Reply::run(ServerId::Archives)),
    );

    assert_eq!(
        t.times_taken(ZER0),
        2,
        "9.3.6g: once per turn, on each of two turns: {}",
        t.tail(20)
    );
    // 1.11.3c put the ability in the ACTION window, so the offers are counted
    // among that window's options rather than among a paid window's.
    let offered = t
        .windows(Kind::Action, Side::Runner)
        .iter()
        .filter(|e| {
            e.actions()
                .iter()
                .any(|o| matches!(o, ActionOption::CardAction { label, .. } if label.contains(ZER0)))
        })
        .count();
    assert_eq!(
        offered, 3,
        "9.1.6: USING it spends the flag, so of the Runner's action windows only \
         the first of each turn offers it: {}",
        t.tail(20)
    );
    assert_eq!(
        t.windows(Kind::Action, Side::Runner).len(),
        9,
        "1.11.3c: an ability whose cost begins with [click] is an ACTION — two full \
         turns of four windows, and the first of the third: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.runner.credits,
        2,
        "\"Gain 1[credit]\", twice, and nothing else on the board pays: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.discard[&Side::Runner].len(),
        2,
        "1.16.1: the net damage was PAID, one card each time, before anything was \
         gained: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        5,
        "\"draw 2 cards\", twice, against a grip of 3 the two damage costs took \
         one card each from: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.objects[&z].zone,
        Zone::Rig,
        "the hardware is a cost the Runner pays with, not one it spends: {}",
        t.tail(20)
    );
}

/// Rezeki: "When your turn begins, gain 1[credit]."
///
/// A CONDITIONAL ability (9.6.1), not a static declaration: 9.4.1's statics
/// "continuously affect the game" and "do not resolve", and this one names a
/// moment and resolves at it. The drive starts on the CORP's turn, so both
/// halves of that are observable — two Runner turns pay 1 each, and the two
/// Corp turns in between pay nothing, because "your" is a stipulation on the
/// condition (9.6.5c) and not decoration.
///
/// The Runner's clicks go on runs against an empty Archives, which pay
/// nothing, so the pool counts Rezeki's payouts alone.
#[test]
fn rezeki_pays_when_the_runners_turn_begins_and_never_on_the_corps() {
    let mut vm = Vm::empty(9304);
    let rz = tk::install_rig(&mut vm, card("Rezeki"));
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 12);
    tk::fill_deck(&mut vm, Side::Runner, 12);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().nth(5), Reply::Halt)
            .when(Match::action(), Reply::run(ServerId::Archives)),
    );

    assert_eq!(
        vm.st.objects[&rz].zone,
        Zone::Rig,
        "the program is still installed and still active: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.runner.credits,
        2,
        "9.6.1: one credit at the beginning of each of the Runner's two turns — and \
         9.6.5c's \"your\": the two Corp turns in between paid nothing, which a \
         count of 4 would have caught: {}",
        t.tail(20)
    );
}

/// Mumba Temple: "2[recurring-credit]"
///
/// PARTIAL — "Use these credits to rez cards." is unsayable (see the card's
/// doc comment and MEZZIE-QUEUE.md's Blockers), and the test says so out loud
/// so the marker cannot quietly disappear. What IS printed and observable is
/// where the credits come from and when: 1.10.5b places them as soon as the
/// card becomes active, which for a Corp asset is the rez and nothing before
/// it, and 1.10.5d refills rather than accumulates them at the start of the
/// Corp's turn — so a card that has spent none of them holds exactly the 2 it
/// prints and never 4.
#[test]
fn mumba_temple_places_two_recurring_credits_on_the_rez_and_never_more() {
    let temple = jinteki_cards::find("Mumba Temple").expect("Mumba Temple is in the card layer");
    assert_eq!(
        temple.unimplemented,
        vec!["Use these credits to rez cards."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9128);
    let mt = tk::install_root(&mut vm, card_partial("Mumba Temple"), ServerId::Remote(1), false);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 3;
    vm.start_turn(Side::Runner);

    assert_eq!(
        vm.st.objects[&mt].counter(CounterKind::Credit),
        0,
        "1.10.5b: nothing is placed while the asset is still facedown and inactive"
    );

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp().when(Match::paid().once(), Reply::rez(mt)).stop_at_action(),
        Plan::runner().when(Match::action().first(), Reply::Halt).otherwise_click_credit(),
    );
    // The Runner's turn, up to their first action window: by then the Corp has
    // rezzed the temple in a paid window.
    g.run(&mut vm);
    assert!(vm.st.objects[&mt].faceup, "the asset was rezzed: {}", g.transcript().tail(12));
    assert_eq!(
        vm.st.objects[&mt].counter(CounterKind::Credit),
        2,
        "1.10.5b: the 2[recurring-credit] arrived with the rez: {}",
        g.transcript().tail(12)
    );
    assert_eq!(
        vm.st.corp.credits,
        2,
        "3 − the 1[credit] rez cost, and none of the placed credits reached the pool (1.13.3)"
    );

    // The Runner spends the turn; the Corp's begins and the refill happens.
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&mt].counter(CounterKind::Credit),
        2,
        "1.10.5d: refilled to the printed 2 rather than accumulated to 4: {}",
        g.transcript().tail(20)
    );
}

/// Spin Doctor: "When you rez this asset, draw 2 cards."
///
/// The trigger is the REZ and not the install (9.1.8: a facedown Corp card is
/// inactive and has no ability to meet anything with), so the two arms differ
/// in nothing but whether the Corp rezzed it, and the draw follows the rez.
#[test]
fn spin_doctor_draws_two_when_it_is_rezzed_and_not_before() {
    for rez_it in [false, true] {
        let mut vm = Vm::empty(9129);
        let sd = tk::install_root(&mut vm, card("Spin Doctor"), ServerId::Remote(1), false);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 3;
        vm.start_turn(Side::Runner);

        let mut corp = Plan::corp();
        if rez_it {
            corp = corp.when(Match::paid().once(), Reply::rez(sd));
        }
        let t = plan::play(&mut vm, corp, Plan::runner().stop_at_action());

        assert_eq!(
            vm.st.objects[&sd].faceup, rez_it,
            "the asset was rezzed exactly when the plan rezzed it (rez_it={rez_it}): {}",
            t.tail(16)
        );
        assert_eq!(
            vm.st.hand[&Side::Corp].len(),
            if rez_it { 2 } else { 0 },
            "the rez drew 2 cards into an HQ that was empty (rez_it={rez_it}): {}",
            t.tail(16)
        );
        assert_eq!(
            vm.st.deck[&Side::Corp].len(),
            if rez_it { 6 } else { 8 },
            "…and they came off the top of R&D (rez_it={rez_it}): {}",
            t.tail(16)
        );
    }
}

/// Spin Doctor: "Remove this asset from the game: Shuffle up to 2 cards from
/// Archives into R&D."
///
/// The cost removes the source from the game (4.9) BEFORE the effect resolves
/// (1.16.1), and 9.1.8g is what makes the shuffle happen anyway: an ability
/// whose cost has been paid resolves even though its source has left. "Up to
/// 2" is 1.15.2e's floor of zero, proven by answering with 2 out of 3 — the
/// third stays in Archives.
#[test]
fn spin_doctor_removes_itself_from_the_game_to_shuffle_two_of_three_back() {
    let mut vm = Vm::empty(9130);
    let sd = tk::install_root(&mut vm, card("Spin Doctor"), ServerId::Remote(1), true);
    let d1 = vm.new_object(tk::corp_filler("Spun-1"), Zone::Discard(Side::Corp));
    let d2 = vm.new_object(tk::corp_filler("Spun-2"), Zone::Discard(Side::Corp));
    let d3 = vm.new_object(tk::corp_filler("Spun-3"), Zone::Discard(Side::Corp));
    for d in [d1, d2, d3] {
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(d);
    }
    tk::fill_deck(&mut vm, Side::Corp, 4);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    // The RUNNER's turn: the Corp uses the ability in a paid window (8.1.1's
    // freedom, read for paid abilities), and no mandatory draw pulls a
    // just-shuffled card straight back out of R&D where the assertion could
    // not see it.
    vm.start_turn(Side::Runner);

    let deck_before = vm.st.deck[&Side::Corp].len();
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("spin doctor: shuffle archives back into r&d"))
            .when(Match::targets().once(), Reply::Targets(vec![d1, d2])),
        Plan::runner().stop_at_action(),
    );

    assert_eq!(
        vm.st.objects[&sd].zone,
        Zone::RemovedFromGame,
        "1.16.1: the trigger cost was paid before the effect resolved: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.deck[&Side::Corp].len(),
        deck_before + 2,
        "9.1.8g: the shuffle resolved although its source was already gone: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.objects[&d1].zone,
        Zone::Deck(Side::Corp),
        "the first chosen card went into R&D: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.objects[&d2].zone,
        Zone::Deck(Side::Corp),
        "…and so did the second: {}",
        t.tail(16)
    );
    assert_eq!(
        vm.st.objects[&d3].zone,
        Zone::Discard(Side::Corp),
        "…and the third stayed in Archives, which is what \"up to 2\" means: {}",
        t.tail(16)
    );
}

/// Enhanced Login Protocol: "This operation is not trashed until another
/// current is played or an agenda is stolen." (8.6.6c / 3.5.1b.)
///
/// PARTIAL — the additional cost on the basic run action is unsayable (see the
/// card's doc comment and MEZZIE-QUEUE.md's Blockers), and the test says so out
/// loud so the marker cannot quietly disappear. The sentence that IS expressed
/// is the one that keeps the card on the table at all: played, it stays in the
/// play area instead of being trashed at step 8.6.7g, and the Runner stealing
/// an agenda is what ends it.
#[test]
fn enhanced_login_protocol_stays_in_the_play_area_until_an_agenda_is_stolen() {
    let elp = jinteki_cards::find("Enhanced Login Protocol")
        .expect("Enhanced Login Protocol is in the card layer");
    assert_eq!(
        elp.unimplemented,
        vec!["As an additional cost to take the basic action to run a server for the first time each turn, the Runner must spend [click]."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9131);
    let card_id = vm.new_object(card_partial("Enhanced Login Protocol"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(card_id);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Loose Agenda", 3, 1),
        ServerId::Remote(1),
        false,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 5;
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Corp);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(card_id))
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
        vm.st.objects[&card_id].zone,
        Zone::PlayArea(Side::Corp),
        "8.6.6c: not trashed at step 8.6.7g: {}",
        g.transcript().tail(16)
    );

    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Runner),
        "the Runner stole it: {}",
        g.transcript().tail(20)
    );
    assert_eq!(
        vm.st.objects[&card_id].zone,
        Zone::Discard(Side::Corp),
        "3.5.1b: the steal ended the lingering effect and the current was trashed: {}",
        g.transcript().tail(20)
    );
}

/// Flood the Market: "As an additional cost to play this operation, spend
/// [click]."
///
/// The *double* is 1.16.10's additional cost, observable as a whole action the
/// Corp never gets: both arms spend every remaining click on the basic credit
/// action (5.2.7b), so the credits the Corp finishes with count the clicks the
/// play left it — one fewer than the same play at the same credit cost without
/// the extra [click]. (The card's other sentence is asserted below.)
#[test]
fn flood_the_market_costs_a_click_on_top_of_its_play_cost() {
    for double in [false, true] {
        let mut vm = Vm::empty(9132);
        // The control is the same 3[credit] play made by an operation with no
        // additional cost — one printed line apart, so what differs in the
        // outcome is the [click] and nothing else.
        let printed = if double {
            card("Flood the Market")
        } else {
            let mut c = tk::corp_filler("Plain Operation");
            c.cost = Some(3);
            c
        };
        let op = vm.new_object(printed, Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
        tk::fill_deck(&mut vm, Side::Corp, 8);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 3;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::action().once(), Reply::play_card(op))
                .otherwise_click_credit(),
            Plan::runner().when(Match::action(), Reply::Halt),
        );

        assert_eq!(
            vm.st.objects[&op].zone,
            Zone::Discard(Side::Corp),
            "the operation was played and trashed at 8.6.7g (double={double}): {}",
            t.tail(20)
        );
        assert_eq!(
            vm.st.corp.credits,
            if double { 1 } else { 2 },
            "3 − 3 to play, then one basic credit action per click the play left \
             (double={double}): {}",
            t.tail(20)
        );
    }
}

/// Flood the Market: "Choose 1 installed card you can advance. Place 1
/// advancement counter on that card for each remote server that has a card in
/// its root and is protected by ice."
///
/// One instruction (9.11.4c): the choice is announced and the counters land on
/// the card announced. Both halves are asserted — the chosen card gets them,
/// and the card that was not chosen gets none.
///
/// The board is built so that no count of CARDS is the answer. Two remotes
/// qualify; the cards in their roots number 3 (4.6.6e lets Remote 2's root
/// hold an asset AND an upgrade) and the ice protecting them numbers 3, so a
/// stand-in written over cards pays 3 where the sentence pays 2. The two
/// remotes that do not qualify are each missing exactly one half of the
/// description: Remote 3 has ice and an empty root, Remote 4 has a root card
/// and no ice.
#[test]
fn flood_the_market_places_one_advancement_counter_per_qualifying_remote_server() {
    let mut vm = Vm::empty(9137);
    let op = vm.new_object(card("Flood the Market"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
    // Remote 1 qualifies, and its root card is one of the two agendas the Corp
    // can advance (1.18.3).
    let chosen = tk::install_root(&mut vm, tk::vanilla_agenda("Front Runner", 3, 2), ServerId::Remote(1), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("R1 Ice", 1, 1), ServerId::Remote(1), false);
    // Remote 2 qualifies once, with two cards in its root and two pieces of ice.
    tk::install_root(&mut vm, tk::vanilla_asset("R2 Asset", 0, 2), ServerId::Remote(2), false);
    tk::install_root(&mut vm, tk::vanilla_upgrade("R2 Upgrade", 0), ServerId::Remote(2), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("R2 Inner", 1, 1), ServerId::Remote(2), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("R2 Outer", 1, 1), ServerId::Remote(2), false);
    // Remote 3: ice, empty root — 4.6.8d makes it a server, the description
    // does not reach it.
    tk::install_ice(&mut vm, tk::vanilla_ice("R3 Ice", 1, 1), ServerId::Remote(3), false);
    // Remote 4: a root card and no ice — the other half missing. Its agenda is
    // the card the Corp does NOT choose.
    let untouched = tk::install_root(&mut vm, tk::vanilla_agenda("Back Runner", 3, 2), ServerId::Remote(4), false);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 3;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(op))
            .when(Match::targets().once(), Reply::target(chosen))
            .stop_at_action(),
        Plan::runner(),
    );

    assert_eq!(
        vm.st.objects[&chosen].counter(CounterKind::Advancement),
        2,
        "4.6.6a: Remote 1 and Remote 2 match the description — not the 3 cards \
         in their roots and not the 3 pieces of ice protecting them: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.objects[&untouched].counter(CounterKind::Advancement),
        0,
        "1.15.2: the counters went to the card the Corp announced and nowhere \
         else: {}",
        t.tail(20)
    );
}

/// Friends in High Places: "After you resolve this operation, end your action
/// phase." / "Install up to 2 cards from Archives (paying all install costs)."
///
/// Both halves are asserted on one board. The installs are two instructions
/// (9.11.4b) out of an INACTIVE zone (4.4.4), each declaring its own 8.5.16b
/// destination — one into a root, one protecting a server that already has a
/// piece of ice, which is where "(paying all install costs)" bites: 8.5.11a
/// charges 1[credit] for that ice and this card ignores nothing. The terminal
/// is read off the credits: the Corp had three clicks and a plan that spends
/// every spare one on the basic credit action, and after the operation it
/// never gets another.
#[test]
fn friends_in_high_places_installs_two_out_of_archives_and_ends_the_action_phase() {
    let mut vm = Vm::empty(9133);
    let fhp = vm.new_object(card("Friends in High Places"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(fhp);
    // A remote already protected by one piece of ice, with an empty root.
    tk::install_ice(&mut vm, tk::vanilla_ice("Outer Guard", 1, 1), ServerId::Remote(1), false);
    let buried_asset = vm.new_object(tk::vanilla_asset("Buried Asset", 1, 2), Zone::Discard(Side::Corp));
    let buried_ice = vm.new_object(tk::vanilla_ice("Buried Ice", 2, 2), Zone::Discard(Side::Corp));
    for d in [buried_asset, buried_ice] {
        vm.st.discard.get_mut(&Side::Corp).unwrap().push(d);
    }
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.corp.credits = 3;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().once(), Reply::play_card(fhp))
            .when(Match::targets().once(), Reply::target(buried_asset))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Root(ServerId::Remote(1))),
            )
            .when(Match::targets().once(), Reply::target(buried_ice))
            .when(
                Match::destination().once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1))),
            )
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );

    assert_eq!(
        vm.st.objects[&buried_asset].zone,
        Zone::Root(ServerId::Remote(1)),
        "the first install took a card out of Archives (4.4.4) and into a root: {}",
        t.tail(28)
    );
    assert_eq!(
        vm.st.objects[&buried_ice].zone,
        Zone::Ice(ServerId::Remote(1)),
        "9.11.4b: the second install is its own instruction and it happened too: {}",
        t.tail(28)
    );
    assert_eq!(
        vm.st.corp.credits,
        0,
        "3 − 2 to play − 8.5.11a's 1[credit] for the ice already protecting that server, \
         and no basic credit action after it: {}",
        t.tail(28)
    );
}

/// Fully Operational: "Gain 2[credit] or draw 2 cards. Repeat this process for
/// each remote server that has a card in its root and is protected by ice."
///
/// The process is 9.11.4g's optioned effect and the repetition is the second
/// sentence, so the printed arithmetic is 1 + N: on a bare board the Corp
/// resolves it once, and with two qualifying remotes three times. Each
/// repetition is a FRESH choice (9.12.2b: an optioned effect is not one of
/// 9.12.2c's aggregated classes, so the group is performed once per unit
/// rather than once with its values multiplied) — which the mixed arm asserts
/// by taking the credits twice and the cards once.
///
/// The board is the one Flood the Market is tested on and for the same
/// reason: two remotes qualify while the cards in their roots number 3 and the
/// ice protecting them numbers 3, so a count of cards pays a different number.
#[test]
fn fully_operational_repeats_the_process_once_per_qualifying_remote_server() {
    for (board, picks, credits, hand, why) in [
        (
            false,
            vec!["gain 2[credit]"],
            4u32,
            1usize,
            "with no qualifying server the process happens exactly once: 3 − the \
             1[credit] play cost + 2",
        ),
        (
            false,
            vec!["draw 2 cards"],
            2,
            3,
            "…and the other half of the same one resolution draws instead",
        ),
        (
            true,
            vec!["gain 2[credit]", "gain 2[credit]", "gain 2[credit]"],
            8,
            1,
            "1 + N with N = 2 qualifying remotes: three resolutions, 6[credit], \
             and not the 3 root cards' or 3 ice's worth",
        ),
        (
            true,
            vec!["gain 2[credit]", "draw 2 cards", "gain 2[credit]"],
            6,
            3,
            "9.12.2b: each repetition is its own choice — two gains and one draw",
        ),
    ] {
        let mut vm = Vm::empty(9134);
        let op = vm.new_object(card("Fully Operational"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
        if board {
            // Two qualifying remotes, plus one of each near miss.
            tk::install_root(&mut vm, tk::vanilla_asset("R1 Asset", 0, 2), ServerId::Remote(1), false);
            tk::install_ice(&mut vm, tk::vanilla_ice("R1 Ice", 1, 1), ServerId::Remote(1), false);
            tk::install_root(&mut vm, tk::vanilla_asset("R2 Asset", 0, 2), ServerId::Remote(2), false);
            tk::install_root(&mut vm, tk::vanilla_upgrade("R2 Upgrade", 0), ServerId::Remote(2), false);
            tk::install_ice(&mut vm, tk::vanilla_ice("R2 Inner", 1, 1), ServerId::Remote(2), false);
            tk::install_ice(&mut vm, tk::vanilla_ice("R2 Outer", 1, 1), ServerId::Remote(2), false);
            tk::install_ice(&mut vm, tk::vanilla_ice("R3 Ice", 1, 1), ServerId::Remote(3), false);
            tk::install_root(&mut vm, tk::vanilla_asset("R4 Asset", 0, 2), ServerId::Remote(4), false);
        }
        tk::fill_deck(&mut vm, Side::Corp, 12);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.corp.credits = 3;
        vm.start_turn(Side::Corp);

        let mut corp = Plan::corp().when(Match::action().once(), Reply::play_card(op));
        for pick in &picks {
            corp = corp.when(Match::of(Kind::Options).once(), Reply::ChooseNamed(pick));
        }
        let t = plan::play(&mut vm, corp.stop_at_action(), Plan::runner());

        assert_eq!(vm.st.corp.credits, credits, "{why}: {}", t.tail(24));
        // The operation left HQ and the turn's mandatory draw put one back.
        assert_eq!(
            vm.st.hand[&Side::Corp].len(),
            hand,
            "…and the cards arrived exactly where a draw was chosen ({why}): {}",
            t.tail(24)
        );
    }
}

// ---------------------------------------------------------------------------
// Mezzie's Asa — the four agendas and the two upgrades
// ---------------------------------------------------------------------------

/// Global Food Initiative: "Global Food Initiative is worth 1 fewer agenda
/// point while in the Runner's score area."
///
/// Two printed copies, one into each score area, and the printed 3 reads
/// differently in each — which is 2.5's point value being a CHARACTERISTIC
/// (9.12.1a) rather than a number stamped on the card when it changed hands.
/// 4.5.4 is the reason this is not free: an agenda in the Runner's score area
/// is inactive "unless stated otherwise", so the whole card rests on 9.1.8b
/// keeping an ability that states its zone alive in that zone.
#[test]
fn global_food_initiative_is_worth_three_scored_and_two_stolen() {
    let mut vm = Vm::empty(9305);
    let scored = tk::install_root(&mut vm, card("Global Food Initiative"), ServerId::Remote(1), false);
    let stolen = tk::install_root(&mut vm, card("Global Food Initiative"), ServerId::Remote(2), false);
    vm.st.objects.get_mut(&scored).unwrap().counters.insert(CounterKind::Advancement, 5);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.start_turn(Side::Corp);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp().when(Match::paid().once(), Reply::score(scored)).otherwise_click_credit(),
        Plan::runner()
            // Halt once to read the Corp's score before the Runner moves.
            .when(Match::action().first(), Reply::Halt)
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&scored].zone,
        Zone::ScoreArea(Side::Corp),
        "the Corp scored one copy: {}",
        g.transcript().tail(14)
    );
    assert_eq!(
        vm.score(Side::Corp),
        3,
        "the printed 3 — the declaration says nothing about the Corp's score area: {}",
        g.transcript().tail(14)
    );

    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&stolen].zone,
        Zone::ScoreArea(Side::Runner),
        "the Runner stole the other copy: {}",
        g.transcript().tail(14)
    );
    assert_eq!(
        vm.score(Side::Runner),
        2,
        "1 fewer than the printed 3, read where the card now is: {}",
        g.transcript().tail(14)
    );
    assert_eq!(vm.score(Side::Corp), 3, "and the Corp's copy is untouched by any of it");
}

/// Luminal Transubstantiation: "When you score this agenda, gain
/// [click][click][click]. You cannot score agendas for the remainder of the
/// turn."
///
/// Both sentences of the one conditional ability (9.11.3 — two sentences, two
/// instructions, one trigger), each asserted as what it promises a player.
///
/// The clicks are read off the turn itself: 5.6.4a allots the Corp three, the
/// score happens in the paid window of the first action window, and the turn
/// then runs to six actions instead of three.
///
/// The prohibition is asserted on an agenda that DID NOT EXIST IN PLAY when
/// the sentence resolved — it is installed from HQ a click later, needs no
/// advancement at all, and is still never offered. That is the half a
/// description gets and a named card does not: 9.10.1's lingering effect
/// carries the criteria and they are re-read every time the (S) option is
/// weighed, so the agenda is inside the prohibition the moment it arrives.
/// And 1.2.2 makes it an option WITHHELD rather than an offer that fails,
/// which is what the assertion looks for.
///
/// The last two assertions are the duration doing its own work: "the remainder
/// of the turn" ends with the turn, and the same agenda is scorable next turn
/// with nothing else about the board changed.
#[test]
fn luminal_transubstantiation_pays_three_clicks_and_shuts_the_turn_to_agendas() {
    let lt = jinteki_cards::find("Luminal Transubstantiation")
        .expect("Luminal Transubstantiation is in the card layer");
    assert!(
        lt.unimplemented.is_empty(),
        "every printed sentence is sayable now: {:?}",
        lt.unimplemented
    );

    let mut vm = Vm::empty(9306);
    let luminal = tk::install_root(&mut vm, card_partial("Luminal Transubstantiation"), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&luminal).unwrap().counters.insert(CounterKind::Advancement, 3);
    // 4.6.8d: a second remote exists because a piece of ice protects it, so
    // its root is free for the agenda that arrives mid-turn.
    tk::install_ice(&mut vm, tk::vanilla_ice("Guard", 0, 1), ServerId::Remote(2), false);
    let other = vm.new_object(tk::vanilla_agenda("Free Agenda", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(other);
    tk::fill_deck(&mut vm, Side::Corp, 8);
    tk::fill_deck(&mut vm, Side::Runner, 8);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::score(luminal))
            .when(Match::action().once(), Reply::Take(Pick::InstallCard(other)))
            .when(
                Match::of(Kind::Destination).once(),
                Reply::Destination(jinteki_cr::instr::InstallDest::Root(ServerId::Remote(2))),
            )
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.st.objects[&luminal].zone,
        Zone::ScoreArea(Side::Corp),
        "Luminal Transubstantiation was scored: {}",
        t.tail(20)
    );
    let corp_actions = t
        .entries
        .iter()
        .filter(|e| e.side == Side::Corp && e.kind() == Kind::Action && e.answer.is_some())
        .count();
    assert_eq!(
        corp_actions, 6,
        "5.6.4a's three clicks plus the three the agenda gained, spent as actions: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.objects[&other].zone,
        Zone::Root(ServerId::Remote(2)),
        "the second agenda is installed and needs no advancement: {}",
        t.tail(30)
    );
    assert!(
        !offered_options(&t).iter().any(|o| matches!(
            o,
            jinteki_cr::decision::WindowOption::Score { card } if *card == other
        )),
        "1.2.2: the (S) option is WITHHELD for every agenda for the rest of the turn — \
         including one that reached the board after the sentence resolved, because the \
         description is re-read where the option is offered: {}",
        t.tail(30)
    );
    assert_eq!(vm.score(Side::Corp), 2, "Luminal's 2 points and nothing else");

    let t2 = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::score(other)).otherwise_click_credit(),
        Plan::runner().otherwise_click_credit(),
    );
    assert_eq!(
        vm.st.turn_side,
        Side::Corp,
        "the Corp's next turn came round: {}",
        t2.tail(40)
    );
    assert_eq!(
        vm.st.objects[&other].zone,
        Zone::ScoreArea(Side::Corp),
        "…and 'the remainder of the turn' ran out with the turn it named — 9.10.1's \
         effect expired on its own duration and the same agenda scored: {}",
        t2.tail(40)
    );
}

/// Project Vitruvius: "When you score this agenda, place 1 agenda counter on it
/// for each hosted advancement counter past 3." / "Hosted agenda counter: Add 1
/// card from Archives to HQ."
///
/// Scored on 5 advancement counters, so 2 agenda counters — read through
/// 1.17.8, since 1.17.5 had already returned all five to the bank by the time
/// the ability resolved. Then one of those counters is spent as a trigger cost
/// (1.16.1) and a card comes back out of Archives.
#[test]
fn project_vitruvius_counts_the_advancements_past_three_and_spends_one_counter() {
    let mut vm = Vm::empty(9307);
    let vit = tk::install_root(&mut vm, card("Project Vitruvius"), ServerId::Remote(1), false);
    vm.st.objects.get_mut(&vit).unwrap().counters.insert(CounterKind::Advancement, 5);
    let buried = vm.new_object(tk::vanilla_asset("Buried Asset", 1, 2), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(buried);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::score(vit))
            .when(Match::any().once(), Reply::take("archives to hq"))
            .when(Match::targets().once(), Reply::target(buried))
            .otherwise_click_credit(),
        Plan::runner().when(Match::action(), Reply::Halt),
    );
    assert_eq!(
        vm.st.objects[&vit].zone,
        Zone::ScoreArea(Side::Corp),
        "the agenda was scored out of the remote: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.objects[&vit].counter(CounterKind::Advancement),
        0,
        "1.17.5: the advancement counters went back to the bank with the score"
    );
    assert_eq!(
        vm.st.objects[&vit].counter(CounterKind::Agenda),
        1,
        "2 placed for the 2 advancements past 3 (1.17.8's last known number), \
         1 spent on the paid ability: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.objects[&buried].zone,
        Zone::Hand(Side::Corp),
        "the counter bought a card out of Archives: {}",
        t.tail(20)
    );
}

/// Project Vacheron: "[interrupt] → When this agenda would be added to the
/// Runner's score area from anywhere except Archives, instead it is added to
/// their score area with 4 hosted agenda counters."
///
/// PARTIAL — the second printed sentence, which is what the counters are FOR,
/// is unsayable (see the card's doc comment and MEZZIE-QUEUE.md's Blockers),
/// and the test says so out loud so the marker cannot quietly disappear.
///
/// 9.9.9c is the half worth asserting: the agenda still ENTERS the Runner's
/// score area, so this is a replacement and not a prevention. Both halves of
/// "from anywhere except Archives" are on the board — a copy in a remote root
/// and a copy in Archives — and only one of them arrives with counters.
#[test]
fn project_vacheron_is_stolen_with_four_agenda_counters_except_out_of_archives() {
    let vach = jinteki_cards::find("Project Vacheron").expect("Project Vacheron is in the card layer");
    assert_eq!(
        vach.unimplemented.len(),
        1,
        "exactly one printed sentence is still unsayable, and it is the second one"
    );

    let mut vm = Vm::empty(9308);
    let installed = tk::install_root(&mut vm, card_partial("Project Vacheron"), ServerId::Remote(1), false);
    let binned = vm.new_object(card_partial("Project Vacheron"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(binned);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
            .when(Match::action().once(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&installed].zone,
        Zone::ScoreArea(Side::Runner),
        "9.9.9c: the replacement's result still includes the steal it replaced: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&installed].counter(CounterKind::Agenda),
        4,
        "…and it arrived with the 4 hosted agenda counters (1.9.5: agenda counters, \
         which 1.17.5 never touches): {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&binned].zone,
        Zone::ScoreArea(Side::Runner),
        "the Archives copy was stolen too: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&binned].counter(CounterKind::Agenda),
        0,
        "…and it came out of Archives, which the sentence excludes by name: {}",
        t.tail(24)
    );
}

/// Ash 2X3ZB9CY: "Whenever there is a successful run on this server, Trace[4].
/// If successful, the Runner cannot access any cards other than Ash 2X3ZB9CY
/// for the remainder of this run."
///
/// The trace resolves in the reaction window at step 6.9.5a — after the run is
/// declared successful and BEFORE the Runner breaches at 6.9.5b — which is the
/// only order in which the restriction can mean anything. 7.4.2 is what it
/// does: the agenda sharing the root stops being a candidate, so the Runner's
/// breach finds only the upgrade. The same board with the trace LOST is the
/// control, and there the agenda is stolen.
#[test]
fn ash_2x3zb9cy_wins_a_trace_and_leaves_the_runner_only_itself_to_access() {
    for (runner_spend, stolen) in [(0u32, false), (5u32, true)] {
        let mut vm = Vm::empty(9309);
        let ash = tk::install_root(&mut vm, card("Ash 2X3ZB9CY"), ServerId::Remote(1), true);
        let agenda = tk::install_root(&mut vm, tk::vanilla_agenda("Loose Agenda", 3, 1), ServerId::Remote(1), false);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 6);
        tk::fill_deck(&mut vm, Side::Runner, 6);
        vm.st.corp.credits = 5;
        vm.st.runner.credits = 8;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp().when(Match::trace_spend(), Reply::Spend(0)),
            Plan::runner()
                .when(Match::action().once(), Reply::run(ServerId::Remote(1)))
                .when(Match::trace_spend(), Reply::Spend(runner_spend))
                .stop_at_action(),
        );
        assert_eq!(
            vm.st.objects[&agenda].zone == Zone::ScoreArea(Side::Runner),
            stolen,
            "trace[4] against {runner_spend}[credit]: the agenda in the same root \
             is a candidate only when the trace failed: {}",
            t.tail(24)
        );
        assert_eq!(
            vm.st.objects[&ash].zone,
            Zone::Root(ServerId::Remote(1)),
            "Ash 2X3ZB9CY itself stayed put either way: {}",
            t.tail(24)
        );
    }
}

/// Manegarm Skunkworks is BLOCKED, and the test pins the marker rather than
/// the behaviour: nothing of its one printed sentence can be said yet (see the
/// card's doc comment and MEZZIE-QUEUE.md's Blockers), so the card denotes into
/// no ability at all and a later wave that quietly deleted the marker without
/// writing the sentence would fail here.
#[test]
fn manegarm_skunkworks_is_still_only_its_printed_text() {
    let ms = jinteki_cards::find("Manegarm Skunkworks").expect("Manegarm Skunkworks is in the card layer");
    assert_eq!(
        ms.unimplemented,
        vec!["Whenever the Runner approaches this server, end the run unless they either spend [click][click] or pay 5[credit]."],
        "the card's only printed sentence is still unsayable"
    );
    assert!(
        ms.printed.abilities.is_empty(),
        "…so it denotes into nothing: an approximation would show up here as an ability"
    );
}

// ---------------------------------------------------------------------------
// Mezzie's Valencia — the events and resources of her own list
// ---------------------------------------------------------------------------

/// Moshing: "As an additional cost to play this event, trash 3 cards from
/// your grip." / "Gain 3[credit] and draw 3 cards."
///
/// The 1.16.10 cost is what is under test, in both directions. With three
/// spare cards in the grip the Runner announces which three go (1.15.2), pays
/// them at step 8.6.7c, and only then gains and draws — so a grip of four
/// (Moshing plus three) ends the play holding exactly the three cards the
/// draw brought. With a grip of two, 1.16.1b makes the cost unpayable and the
/// basic play action does not offer the card at all: that is the half an
/// "instruction" reading of the sentence would get wrong, because an
/// instruction would have played the event and then trashed what it could.
#[test]
fn moshing_pays_three_grip_cards_before_it_gains_or_draws() {
    for spare in [3usize, 1] {
        let mut vm = Vm::empty(9401);
        let mosh = vm.new_object(card("Moshing"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(mosh);
        let fodder = tk::fill_hand(&mut vm, Side::Runner, spare);
        tk::fill_deck(&mut vm, Side::Runner, 8);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(mosh))
                .when(Match::payment_cards().once(), Reply::Targets(fodder.clone()))
                .stop_at_action(),
        );

        if spare < 3 {
            assert_eq!(
                vm.st.objects[&mosh].zone,
                Zone::Hand(Side::Runner),
                "1.16.1b: three cards cannot be trashed out of a grip that holds \
                 two in all, so the play was never offered: {}",
                t.tail(16)
            );
            assert_eq!(vm.st.runner.credits, 0, "and nothing was gained: {}", t.tail(16));
            continue;
        }

        assert_eq!(
            vm.st.runner.credits, 3,
            "0 − the 0[credit] play cost + 3 gained: {}",
            t.tail(20)
        );
        for f in &fodder {
            assert_eq!(
                vm.st.objects[f].zone,
                Zone::Discard(Side::Runner),
                "the three announced cards paid the 1.16.10 cost: {}",
                t.tail(20)
            );
        }
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            3,
            "the grip is exactly the three cards the draw brought: {}",
            t.tail(20)
        );
    }
}

/// Levy AR Lab Access: "Shuffle your grip and heap into your stack. Draw 5
/// cards. Remove Levy AR Lab Access from the game instead of trashing it."
///
/// All three sentences on one board. The grip and the heap are named
/// together as one description (`any_of`), so both zones empty into the stack
/// before a single card is drawn — which is the only reason the five cards
/// can come off a stack that started with one. 8.6.7a is why Levy itself is
/// not among them: it is in the play area while its own ability resolves. And
/// 8.2.2's replaced destination is read off the zone it ends in — removed
/// from the game, not the heap it would otherwise have been shuffled back
/// out of.
#[test]
fn levy_shuffles_the_grip_and_heap_into_the_stack_before_drawing_five() {
    let mut vm = Vm::empty(9402);
    let levy = vm.new_object(card("Levy AR Lab Access"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(levy);
    let grip = tk::fill_hand(&mut vm, Side::Runner, 2);
    let heap: Vec<ObjectId> = (0..4)
        .map(|i| {
            let name: &'static str = Box::leak(format!("burnt-{i}").into_boxed_str());
            let id = vm.new_object(tk::runner_filler(name), Zone::Discard(Side::Runner));
            vm.st.discard.get_mut(&Side::Runner).unwrap().push(id);
            id
        })
        .collect();
    tk::fill_deck(&mut vm, Side::Runner, 1);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(levy))
            .stop_at_action(),
    );

    assert!(
        vm.st.discard[&Side::Runner].is_empty(),
        "8.3: the heap went into the stack, all of it: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        5,
        "1 card left in the stack + 2 from the grip + 4 from the heap = 7, \
         and 5 of them were drawn: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.deck[&Side::Runner].len(),
        2,
        "…leaving 2 behind, which is the arithmetic only a shuffle BEFORE the \
         draw produces: {}",
        t.tail(20)
    );
    // Every card that was in the grip or the heap is now in one of the two
    // places the shuffle and the draw can have put it.
    for c in grip.iter().chain(heap.iter()) {
        assert!(
            matches!(
                vm.st.objects[c].zone,
                Zone::Deck(Side::Runner) | Zone::Hand(Side::Runner)
            ),
            "{:?} left its zone for the stack: {}",
            vm.st.objects[c].zone,
            t.tail(20)
        );
    }
    assert_eq!(
        vm.st.objects[&levy].zone,
        Zone::RemovedFromGame,
        "8.2.2 / 8.6.7g: still trashed, but removed from the game instead of \
         landing in the heap: {}",
        t.tail(20)
    );
}

/// Clan Vengeance: "Whenever you suffer any amount of damage, place 1 power
/// counter on Clan Vengeance." / "[trash]: Trash 1 card from HQ at random for
/// each power counter on Clan Vengeance."
///
/// Both sentences on one board, and the arithmetic is what proves them apart.
/// The Corp does 2 net damage and then 1: that is TWO occurrences, so two
/// counters — one per occurrence and not one per point, which is 9.12.2c
/// aggregating a damage instruction into a single effect. Then the [trash]
/// ability takes exactly two cards out of HQ, which is only possible if
/// 9.5.5's set-aside counters are still counted after the cost has already
/// put the resource in the heap.
#[test]
fn clan_vengeance_counts_occurrences_and_spends_them_from_the_heap() {
    const VENGEANCE: &str = "clan vengeance: trash HQ at random, one card per counter";
    let mut vm = Vm::empty(9403);
    let cv = tk::install_rig(&mut vm, card("Clan Vengeance"));
    tk::install_root(&mut vm, tk::net_damage_button("Hurt Two", 2), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::net_damage_button("Hurt One", 1), ServerId::Remote(2), true);
    tk::fill_hand(&mut vm, Side::Runner, 6);
    tk::fill_hand(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Corp);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp()
            .when(Match::paid().offering("do net damage").times(2), Reply::take("do net damage"))
            .stop_at_action(),
        Plan::runner()
            // The ability costs no [click], so 9.1.6/1.11.3c offer it in a PAID
            // window. Halt at the first offer to read the counters, then take it.
            .when(Match::paid().offering(VENGEANCE).first(), Reply::Halt)
            .when(Match::paid().offering(VENGEANCE).once(), Reply::take(VENGEANCE))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&cv].counter(CounterKind::Power),
        2,
        "two damage OCCURRENCES, two counters — not three for three points: {}",
        g.transcript().tail(30)
    );
    let hq_before: Vec<ObjectId> = vm.st.hand[&Side::Corp].clone();

    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&cv].zone,
        Zone::Discard(Side::Runner),
        "the [trash] trigger cost was paid: {}",
        g.transcript().tail(30)
    );
    let trashed = hq_before
        .iter()
        .filter(|c| vm.st.objects[c].zone == Zone::Discard(Side::Corp))
        .count();
    assert_eq!(
        trashed, 2,
        "9.5.5: the counters set aside by the [trash] cost are still counted, \
         so exactly 2 cards left HQ: {}",
        g.transcript().tail(30)
    );
}

/// Same Old Thing: "[click], [click], [trash]: Play an event from your heap
/// (paying its play cost)."
///
/// The heap is the point. 4.4.4 leaves the cards there inactive, and 9.1.8b is
/// what lets an installed resource's ability name them anyway — so a Sure
/// Gamble that has already been played once is played again, out of the
/// discard pile, and the parenthetical's play cost is really paid: the Runner
/// starts on 5 and ends on 9, which is 5 − 5 + 9 and not 5 + 9.
#[test]
fn same_old_thing_replays_an_event_out_of_the_heap_paying_for_it() {
    const SOT: &str = "same old thing: replay an event out of the heap";
    let mut vm = Vm::empty(9404);
    let sot = tk::install_rig(&mut vm, card("Same Old Thing"));
    let gamble = vm.new_object(card("Sure Gamble"), Zone::Discard(Side::Runner));
    vm.st.discard.get_mut(&Side::Runner).unwrap().push(gamble);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().offering(SOT).once(), Reply::take(SOT))
            .when(Match::targets().once(), Reply::target(gamble))
            .stop_at_action(),
    );

    assert_eq!(
        vm.st.runner.credits, 9,
        "5 − Sure Gamble's 5[credit] play cost + 9: the parenthetical is 8.6.7b \
         and it was charged: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&sot].zone,
        Zone::Discard(Side::Runner),
        "the [trash] half of the trigger cost was paid: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.objects[&gamble].zone,
        Zone::Discard(Side::Runner),
        "…and the event resolved and was trashed again at 8.6.7g: {}",
        t.tail(24)
    );
}

/// Blackmail: "Play only if the Corp has at least 1 bad publicity." / "Run any
/// server." (the run half of its second printed line.)
///
/// PARTIAL — the rez prohibition is unsayable (see the card's doc comment and
/// MEZZIE-QUEUE.md's Blockers), and the test says so out loud so the marker
/// cannot quietly disappear.
///
/// The restriction is asserted in both directions on the same board, which is
/// what 9.1.8c and 1.2.2 between them require: with no bad publicity the basic
/// play action does not offer the card at all, and with one it does. 10.6.1's
/// bad publicity is a count on the player, so the difference between the two
/// arms is one counter and nothing else. What follows the legal play is
/// 6.9.1a's announcement over every server: the effect names none, so the
/// Runner declares the attacked one as the run is initiated.
#[test]
fn blackmail_is_playable_only_against_a_corp_with_bad_publicity() {
    let bm = jinteki_cards::find("Blackmail").expect("Blackmail is in the card layer");
    assert_eq!(
        bm.unimplemented,
        vec!["The Corp cannot rez ice during that run."],
        "exactly one printed sentence is still unsayable"
    );

    for bad_publicity in [0u32, 1u32] {
        let mut vm = Vm::empty(9405);
        let card_id = vm.new_object(card_partial("Blackmail"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 3;
        vm.st.corp.bad_publicity = bad_publicity;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(card_id))
                .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
                .stop_at_action(),
        );

        let offered = t
            .first_window(Kind::Action, Side::Runner)
            .actions()
            .iter()
            .any(|o| matches!(o, ActionOption::BasicPlayOperation { card } if *card == card_id));
        assert_eq!(
            offered,
            bad_publicity >= 1,
            "9.1.8c gates the basic play action on 10.6.1's count \
             (bad_publicity={bad_publicity}): {}",
            t.tail(16)
        );

        if bad_publicity >= 1 {
            assert!(
                vm.changes.log.iter().any(|c| matches!(
                    c,
                    GameChange::RunDeclaredSuccessful { server: ServerId::Archives, .. }
                )),
                "the run went to the server the Runner named: {}",
                t.tail(16)
            );
            assert_eq!(vm.st.runner.credits, 2, "3 − the 1[credit] play cost: {}", t.tail(16));
        } else {
            assert_eq!(
                vm.st.objects[&card_id].zone,
                Zone::Hand(Side::Runner),
                "1.2.2: the card that could not be played is still in the grip: {}",
                t.tail(16)
            );
        }
    }
}

/// Hacktivist Meeting: "This card is not trashed until another current is
/// played or an agenda is scored."
///
/// PARTIAL — the additional cost to rez is unsayable (see the card's doc
/// comment and MEZZIE-QUEUE.md's Blockers), and the test says so out loud so
/// the marker cannot quietly disappear. The sentence that IS expressed is the
/// one that keeps the card on the table: played, it stays in the play area
/// instead of being trashed at step 8.6.7g (3.7.1b / 8.6.6c), and the Corp
/// scoring an agenda is what ends it.
#[test]
fn hacktivist_meeting_stays_in_the_play_area_until_an_agenda_is_scored() {
    let hm = jinteki_cards::find("Hacktivist Meeting").expect("Hacktivist Meeting is in the card layer");
    assert_eq!(
        hm.unimplemented,
        vec!["As an additional cost to rez non-ice cards, the Corp must randomly trash a card from HQ."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9406);
    let card_id = vm.new_object(card_partial("Hacktivist Meeting"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Ready Agenda", 3, 1),
        ServerId::Remote(1),
        false,
    );
    vm.st.objects.get_mut(&agenda).unwrap().counters.insert(CounterKind::Advancement, 3);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp().when(Match::paid(), Reply::score(agenda)).stop_at_action(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(card_id))
            .when(Match::action().first(), Reply::Halt)
            .otherwise_click_credit(),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&card_id].zone,
        Zone::PlayArea(Side::Runner),
        "8.6.6c: not trashed at step 8.6.7g: {}",
        g.transcript().tail(16)
    );

    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Corp),
        "the Corp scored it: {}",
        g.transcript().tail(24)
    );
    assert_eq!(
        vm.st.objects[&card_id].zone,
        Zone::Discard(Side::Runner),
        "3.7.1b: the score ended the lingering effect and the current was trashed: {}",
        g.transcript().tail(24)
    );
}

/// I've Had Worse: "Draw 3 cards."
///
/// PARTIAL — "Whenever I've Had Worse is trashed by taking net or meat
/// damage, draw 3 cards." is unsayable (see the card's doc comment and
/// MEZZIE-QUEUE.md's Blockers), and the test says so out loud so the marker
/// cannot quietly disappear.
#[test]
fn ive_had_worse_draws_three() {
    let ihw = jinteki_cards::find("I've Had Worse").expect("I've Had Worse is in the card layer");
    assert_eq!(
        ihw.unimplemented,
        vec!["Whenever I've Had Worse is trashed by taking net or meat damage, draw 3 cards."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9407);
    let card_id = vm.new_object(card_partial("I've Had Worse"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::play_card(card_id)).stop_at_action(),
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 3, "three cards drawn: {}", t.tail(12));
    assert_eq!(vm.st.runner.credits, 0, "1 − the 1[credit] play cost: {}", t.tail(12));
}

/// Steelskin Scarring: "Draw 3 cards."
///
/// PARTIAL — "When this event is trashed from your grip or stack, you may
/// draw 2 cards." is unsayable (see the card's doc comment and
/// MEZZIE-QUEUE.md's Blockers), and the test says so out loud so the marker
/// cannot quietly disappear.
#[test]
fn steelskin_scarring_draws_three() {
    let ss = jinteki_cards::find("Steelskin Scarring").expect("Steelskin Scarring is in the card layer");
    assert_eq!(
        ss.unimplemented,
        vec!["When this event is trashed from your grip or stack, you may draw 2 cards."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9408);
    let card_id = vm.new_object(card_partial("Steelskin Scarring"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::play_card(card_id)).stop_at_action(),
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 3, "three cards drawn: {}", t.tail(12));
}

/// Inject: "Reveal the top 4 cards of your stack and trash all programs
/// revealed. Gain 1[credit] for each program trashed, and add the rest of the
/// revealed cards to your grip."
///
/// The whole card, on one board. The top four are shown to both players
/// (1.21.3); the two programs among them go to the heap and the two that are
/// not stay where they were, which is what makes the trash a description and
/// not a mill; two credits arrive, one per program trashed; and the two
/// survivors — and NOTHING from deeper in the stack — end up in the grip.
///
/// The last assertion is the one 1.21.6 earns. "The rest of the revealed
/// cards" is the four this ability revealed minus the two it named for the
/// trash, and by the time that sentence resolves the top of the stack is two
/// cards further down: a description reading the stack again would take the
/// two fillers instead. They are asserted to be exactly where they started.
#[test]
fn inject_reveals_four_trashes_the_programs_and_banks_the_rest() {
    let mut vm = Vm::empty(9409);
    let card_id = vm.new_object(card("Inject"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
    // The stack from the TOP down: the four the card reveals — program,
    // resource, program, resource.
    let top: Vec<ObjectId> = [
        ("Injected Program A", CardType::Program),
        ("Injected Resource A", CardType::Resource),
        ("Injected Program B", CardType::Program),
        ("Injected Resource B", CardType::Resource),
    ]
    .into_iter()
    .map(|(name, ty)| {
        let id = vm.new_object(tk::vanilla_runner_card(name, ty), Zone::Deck(Side::Runner));
        vm.st.deck.get_mut(&Side::Runner).unwrap().push(id);
        id
    })
    .collect();
    // …and two more below the window the card reaches, so "the top 4" is a
    // real window and not the whole stack — and so "the rest of the revealed
    // cards" has something wrong it could reach.
    let below = tk::fill_deck(&mut vm, Side::Runner, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().once(), Reply::play_card(card_id)).stop_at_action(),
    );

    for p in [top[0], top[2]] {
        assert_eq!(
            vm.st.objects[&p].zone,
            Zone::Discard(Side::Runner),
            "every program among the four revealed was trashed: {}",
            t.tail(20)
        );
    }
    assert!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CardRevealed { .. }))
            .count()
            >= 4,
        "1.21.3: all four were revealed, not just the ones that moved: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.runner.credits,
        1 - 1 + 2,
        "1[credit] for each of the two programs trashed, after the 1[credit] play cost: {}",
        t.tail(20)
    );
    for r in [top[1], top[3]] {
        assert_eq!(
            vm.st.objects[&r].zone,
            Zone::Hand(Side::Runner),
            "1.21.6: the revealed cards the trash did not name are \"the rest\": {}",
            t.tail(20)
        );
    }
    for b in &below {
        assert_eq!(
            vm.st.objects[b].zone,
            Zone::Deck(Side::Runner),
            "…and the cards below the window were never revealed, so they are not \"the \
             rest\" — a description reading the top of the stack again would have taken \
             them: {}",
            t.tail(20)
        );
    }
}

/// Mad Dash: "Run any server. When that run ends, if you stole an agenda
/// during that run, add this event to your score area as an agenda worth 1
/// agenda point. Otherwise, suffer 1 meat damage."
///
/// Both branches, on one board that differs only in which server the Runner
/// names. The remote holds a loose agenda and Archives is empty, so the run
/// the Runner announces decides whether a steal happens — and the card's
/// question is asked afterwards, of the run's HISTORY, which is the whole
/// point of it being a count and not a trigger condition.
///
/// The negative branch is asserted by the card the meat damage takes out of
/// the grip (10.4.2a), and the positive one by the event reaching the score
/// area with a point on it while the grip is untouched. 1.17.3e/f is asserted
/// too, in the form that matters: Mad Dash is ADDED, so it is not stolen —
/// only the agenda the Runner actually stole is recorded as a steal.
#[test]
fn mad_dash_pays_out_on_the_steal_and_bites_without_one() {
    for (server, stole) in [(ServerId::Archives, false), (ServerId::Remote(1), true)] {
        let mut vm = Vm::empty(9410);
        let card_id = vm.new_object(card("Mad Dash"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
        let agenda = tk::install_root(
            &mut vm,
            tk::vanilla_agenda("Loose Agenda", 3, 2),
            ServerId::Remote(1),
            false,
        );
        // Three more cards in the grip, so a meat damage has something to take.
        tk::fill_hand(&mut vm, Side::Runner, 3);
        tk::fill_hand(&mut vm, Side::Corp, 3);
        tk::fill_deck(&mut vm, Side::Corp, 5);
        tk::fill_deck(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);
        let grip_before = vm.st.hand[&Side::Runner].len();

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::action().once(), Reply::play_card(card_id))
                .when(Match::attacked_server().once(), Reply::Server(server))
                .stop_at_action(),
        );

        assert_eq!(
            vm.st.objects[&agenda].zone == Zone::ScoreArea(Side::Runner),
            stole,
            "the run only reached the agenda on the remote (server={server:?}): {}",
            t.tail(24)
        );
        assert_eq!(
            vm.st.objects[&card_id].zone,
            if stole { Zone::ScoreArea(Side::Runner) } else { Zone::Discard(Side::Runner) },
            "10.1.3: the event is added to the score area as an agenda only where a steal \
             happened during the run (stole={stole}): {}",
            t.tail(24)
        );
        // grip_before counted Mad Dash itself, which the play took out either
        // way; the meat damage is the second card gone.
        assert_eq!(
            vm.st.hand[&Side::Runner].len(),
            if stole { grip_before - 1 } else { grip_before - 2 },
            "10.4.2a: the other branch trashes a randomly-chosen card from the grip \
             (stole={stole}): {}",
            t.tail(24)
        );
        if stole {
            assert_eq!(
                vm.score(Side::Runner),
                3,
                "2 for the agenda stolen, 1 for the event added as an agenda: {}",
                t.tail(24)
            );
            assert_eq!(
                vm.changes
                    .log
                    .iter()
                    .filter(|c| matches!(c, GameChange::AgendaStolen { .. }))
                    .count(),
                1,
                "1.17.3e/f: a card ADDED to a score area is not stolen, so Mad Dash \
                 records no second steal: {}",
                t.tail(24)
            );
        }
    }
}

/// Raindrops Cut Stone: "Run any server." / "When that run ends, draw 1 card
/// for each hosted power counter and gain 3[credit]."
///
/// PARTIAL — the counter sentence is unsayable (see the card's doc comment and
/// MEZZIE-QUEUE.md's Blockers), and the test says so out loud so the marker
/// cannot quietly disappear. Both expressed halves are asserted: the run
/// happens, and the pay-off fires when it ENDS — which is only possible
/// because 4.6.4e keeps a played event active in the play area while 5.2.2b
/// suspends its resolution for the run. With the counter sentence marked, the
/// hosted count is 0 and the draw half is worth nothing; the 3[credit] is
/// what the sentence pays regardless, and it is what the assertion reads.
#[test]
fn raindrops_cut_stone_pays_out_when_the_run_it_made_ends() {
    let rcs = jinteki_cards::find("Raindrops Cut Stone").expect("Raindrops Cut Stone is in the card layer");
    assert_eq!(
        rcs.unimplemented,
        vec!["Whenever a subroutine resolves during that run (including a subroutine that ends the run), place 1 power counter on this event."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9411);
    let card_id = vm.new_object(card_partial("Raindrops Cut Stone"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(card_id))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            .stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::RunDeclaredSuccessful { server: ServerId::Archives, .. }
        )),
        "the run happened: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.runner.credits, 3,
        "1 − the 1[credit] play cost + the 3[credit] the run's end pays: {}",
        t.tail(20)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        0,
        "…and 0 cards, because the marked sentence is what would have put \
         counters there: {}",
        t.tail(20)
    );
}

/// Stimhack: "Place 9[credit] on this event, then run any server." / "When
/// that run ends, suffer 1 core damage. This damage cannot be prevented."
///
/// PARTIAL — the credit-pool sentence is unsayable (see the card's doc comment
/// and MEZZIE-QUEUE.md's Blockers), and the test says so out loud so the
/// marker cannot quietly disappear.
///
/// Everything else is asserted on one board, and the unpreventability is
/// asserted against a card that really would have stopped it: a free paid
/// interrupt printing "prevent all core damage" is installed, and 9.9.7's
/// relevance test never offers it, because `EffectAtom::unpreventable` leaves
/// no preventable atom to be relevant to. 10.4.2b is the rest — a card out of
/// the grip AND a permanent maximum-hand-size reduction.
#[test]
fn stimhack_loads_nine_credits_and_the_core_damage_cannot_be_prevented() {
    let sh = jinteki_cards::find("Stimhack").expect("Stimhack is in the card layer");
    assert_eq!(
        sh.unimplemented,
        vec!["During that run, hosted credits are considered to be in your credit pool."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9412);
    let card_id = vm.new_object(card_partial("Stimhack"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(card_id);
    tk::install_rig(&mut vm, tk::prevent_all_like("Core Shield", jinteki_cr::effects::DamageKind::Core));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 0;
    let max_before = vm.st.runner.max_hand_size_base;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::play_card(card_id))
            .when(Match::attacked_server().once(), Reply::Server(ServerId::Archives))
            // If the prevention is ever offered, taking it is the failure this
            // test is looking for.
            .when(Match::interrupt().offering("chrome-parlor"), Reply::take("chrome-parlor"))
            .stop_at_action(),
    );

    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::RunDeclaredSuccessful { server: ServerId::Archives, .. }
        )),
        "the run happened after the credits were placed: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.core_damage, 1,
        "9.9.7: the prevention could not apply, so the core damage landed: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.runner.max_hand_size_base,
        max_before - 1,
        "10.4.2b: and it permanently cost a card of maximum hand size: {}",
        t.tail(24)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "10.4.2b: one random card out of the grip of three: {}",
        t.tail(24)
    );
}

/// Mystic Maemi: "When your turn begins and whenever you steal an agenda,
/// place 1[credit] on this resource." / "When your turn ends, if there are 3
/// or more hosted credits, you must trash 1 card from your grip at random or
/// trash this resource."
///
/// PARTIAL — the spend permission is unsayable (see the card's doc comment and
/// MEZZIE-QUEUE.md's Blockers), and the test says so out loud so the marker
/// cannot quietly disappear.
///
/// The two expressed sentences are asserted together, and the drive is what
/// makes the disjunction observable: the turn begins with two credits already
/// on her (board state), the turn-begin half puts a third there, and the
/// turn's END then finds three and puts 9.11.4g's choice to the Runner —
/// which is only reached because the 9.6.5c requirement counted the credit the
/// FIRST sentence placed. The Runner takes the option that trashes her, and
/// she goes.
#[test]
fn mystic_maemi_banks_a_credit_each_turn_and_asks_to_be_paid_at_its_end() {
    let mm = jinteki_cards::find("Mystic Maemi").expect("Mystic Maemi is in the card layer");
    assert_eq!(
        mm.unimplemented,
        vec!["You can spend hosted credits to play events."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9413);
    let maemi = tk::install_rig(&mut vm, card_partial("Mystic Maemi"));
    tk::place_counters(&mut vm, maemi, CounterKind::Credit, 2);
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);

    let mut g = jinteki_cr::plan::Script::new(
        Plan::corp().stop_at_action(),
        Plan::runner()
            .when(Match::action().first(), Reply::Halt)
            .when(Match::options().once(), Reply::ChooseNamed("trash this resource"))
            .otherwise_click_credit(),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&maemi].counter(CounterKind::Credit),
        3,
        "the turn began and a third credit went on her: {}",
        g.transcript().tail(16)
    );

    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&maemi].zone,
        Zone::Discard(Side::Runner),
        "9.11.4g: at the turn's end the Runner chose to trash her rather than \
         the card from their grip: {}",
        g.transcript().tail(24)
    );
}

/// Tsakhia "Bankhar" Gantulga: "When your turn begins, you may choose a
/// server."
///
/// PARTIAL — the subroutine replacement is unsayable (see the card's doc
/// comment and MEZZIE-QUEUE.md's Blockers), and the test says so out loud so
/// the marker cannot quietly disappear. The choice is asserted as 9.10.3's
/// maintained choice, which is what the second sentence would read: the
/// decision is put to the Runner over the servers, and the answer is still
/// remembered by the card afterwards.
#[test]
fn bankhar_chooses_a_server_when_the_turn_begins_and_remembers_it() {
    let bg = jinteki_cards::find("Tsakhia \"Bankhar\" Gantulga")
        .expect("Tsakhia \"Bankhar\" Gantulga is in the card layer");
    assert_eq!(
        bg.unimplemented,
        vec!["During the first encounter each turn with a piece of ice protecting the chosen server, whenever the Corp would resolve a subroutine, instead they resolve \"[subroutine] Do 1 net damage.\"."],
        "exactly one printed sentence is still unsayable"
    );

    let mut vm = Vm::empty(9414);
    let bankhar = tk::install_rig(&mut vm, card_partial("Tsakhia \"Bankhar\" Gantulga"));
    tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 1, 1), ServerId::Remote(1), false);
    tk::fill_hand(&mut vm, Side::Runner, 3);
    tk::fill_deck(&mut vm, Side::Runner, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().once(), Reply::take("bankhar"))
            .when(Match::optional().once(), Reply::Optional(true))
            .when(Match::choose_server().once(), Reply::Server(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.maintained_choice(bankhar, "bankhar server"),
        Some(jinteki_cr::lingering::ChoiceValue::Server(ServerId::Remote(1))),
        "9.10.3: the server the Runner named is remembered by the card: {}",
        t.tail(20)
    );
}

