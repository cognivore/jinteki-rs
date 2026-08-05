//! The complete cards, played in the VM.
//!
//! Parsing is not proof (SYS-D-12): a card counts as complete only when the
//! instructions it denotes into actually do, in the rules engine, what the
//! printed text says. Each test below takes the card straight out of the deck
//! file — no hand-written `PrintedCard` — puts it on a board and drives it
//! with the shared plan driver, then asserts the printed sentence's effect.

use jinteki_cr::change::GameChange;

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
        additional_identities: Default::default(),
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
            .when(Match::targets().once(), Reply::Targets(Vec::new()))
            .stop_at_action(),
    );
    // 1.15.2b: the announcement is made with as many targets as there are,
    // and there are none — the Shaper is in the pile and is not a candidate.
    let announced = t.of_kind(Kind::Targets);
    assert!(
        matches!(
            &announced[0].spec,
            jinteki_cr::decision::DecisionSpec::ChooseTargets { candidates, .. }
                if candidates.is_empty()
        ),
        "no same-faction identity to name: {:?}",
        announced[0].spec
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
            |c| matches!(c, GameChange::RunDeclaredSuccessful { server } if *server == ServerId::Rnd)
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
            required_choice: None,
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

/// Where Apex's two sentences meet: the prohibition still bites on the
/// facedown install.
///
/// 8.1.4a blanks a facedown Runner card that is INSTALLED, and 8.5.16a says
/// the card it has just placed facedown "is not yet installed or active" — so
/// at the 1.15.2 announcement the card is still a non-virtual resource, and
/// 1.2.2 gives the "cannot" precedence over the install the identity's own
/// second sentence directs. The virtual resource beside it is offered, which
/// is what keeps this an assertion about the description and not about the
/// facedown install refusing resources.
#[test]
fn apexs_facedown_install_still_cannot_take_a_non_virtual_resource() {
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
            .when(Match::targets().once(), Reply::target(virt))
            .stop_at_action(),
    );
    let announced = t.of_kind(Kind::Targets);
    assert_eq!(announced.len(), 1, "the identity announced its card: {}", t.tail(30));
    let candidates = announced[0].candidates();
    assert!(
        !candidates.contains(&plain),
        "the card is still a non-virtual resource while it is being chosen: {}",
        t.tail(30)
    );
    assert!(
        candidates.contains(&virt),
        "and a virtual one, which the sentence does not describe, is offered: {}",
        t.tail(30)
    );
    assert_eq!(vm.st.objects[&virt].zone, Zone::Rig, "it went in facedown: {}", t.tail(30));
    assert!(!vm.st.objects[&virt].faceup, "facedown: {}", t.tail(30));
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
            GameChange::RunDeclaredSuccessful { server } => Some(*server),
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
