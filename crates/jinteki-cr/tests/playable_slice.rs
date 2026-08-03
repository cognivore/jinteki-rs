//! The W1 playable-slice proof: a full scripted game driven exclusively
//! through `Vm::step()` decisions — setup (§1.6: draw 5, mulligan offers),
//! a full Corp turn (allotted clicks, (P)(R)(S) paid ability windows,
//! mandatory draw, action-window loop, discard phase), a full Runner turn,
//! and three runs: unrezzed-ice pass path, rezzed-encounter path where a
//! vanilla "End the run" subroutine ends the run, and an ice-free run
//! reaching Success → breach (11.5) → access (11.6, mid-access window,
//! agenda steal) → Run Ends with the bad-publicity fund emptying.
//!
//! The script is DATA (ARCHITECTURE §12 rule 5): one plan per player per
//! narrative phase, folded by the shared driver. Each phase's plans end in
//! `forbidding_the_rest()`, which is this test's central claim — the machine
//! asks for these decisions and *no others*.

use jinteki_cr::change::GameChange;
use jinteki_cr::decision::{ActionOption, DecisionSpec, WindowOption, Yield};
use jinteki_cr::object::{CounterKind, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Kind, Match, Pick, Plan, Reply, Script};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::{GameSetup, Vm};

/// Close a phase plan: after its own rules, pass every remaining priority
/// window and jack in — and fail on any decision the phase did not
/// anticipate. That last clause is this test's central claim.
fn quiet(p: Plan) -> Plan {
    p.when(Match::paid(), Reply::Pass)
        .when(Match::mid_access(), Reply::Pass)
        .when(Match::jack_out(), Reply::JackOut(false))
        .forbidding_the_rest()
}

#[test]
fn full_game_slice() {
    let setup = GameSetup {
        corp_deck: (0..10)
            .map(|i| {
                let name: &'static str = Box::leak(format!("corp-{i}").into_boxed_str());
                tk::corp_filler(name)
            })
            .collect(),
        runner_deck: (0..10)
            .map(|i| {
                let name: &'static str = Box::leak(format!("run-{i}").into_boxed_str());
                tk::runner_filler(name)
            })
            .collect(),
        corp_identity: None,
        runner_identity: None,
        seed: 7,
        shuffle: true,
    };
    let mut vm = Vm::new_game(setup);

    // Board seeding through the test-support layer (the card layer that
    // installs through actions is a later wave): one unrezzed ETR ice on HQ,
    // an agenda in a remote, an unrezzed asset for the (R)/(S) window check.
    let ice = tk::install_ice(&mut vm, tk::etr_ice("Vanilla Wall", 2, 3), ServerId::Hq, false);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Test Priority", 2, 2),
        ServerId::Remote(1),
        false,
    );
    let _asset = tk::install_root(
        &mut vm,
        tk::vanilla_asset("Test Asset", 0, 3),
        ServerId::Remote(2),
        false,
    );
    vm.st.corp.bad_publicity = 2;

    // ---- Setup: mulligan offers (1.6.6a), Corp first ----
    // The Corp keeps, the Runner mulligans; the driver stops at the Corp's
    // first paid window, which is still before the mandatory draw.
    let mut g = Script::new(
        quiet(
            Plan::corp()
                .when(Match::mulligan(), Reply::Keep)
                .when(Match::paid().once(), Reply::Halt),
        ),
        quiet(Plan::runner().when(Match::mulligan(), Reply::Mulligan)),
    );
    g.run(&mut vm);
    assert_eq!(
        g.transcript().of_kind(Kind::Mulligan).iter().map(|e| e.side).collect::<Vec<_>>(),
        vec![Side::Corp, Side::Runner],
        "1.6.6a: both players are offered a mulligan, the Corp first"
    );
    assert_eq!(vm.st.hand[&Side::Corp].len(), 5, "1.6.6: starting hand of 5");
    // The Runner mulliganed: the new hand is still 5 cards.
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5);

    // ---- Corp turn ----
    // The first decision of the turn is the draw-phase PAW with (P)(R)(S)
    // classes (the Corp has a rez option, so the window yields a decision).
    let paw = g.transcript().last().expect("halted at the 5.6.1b PAW");
    assert_eq!(paw.side, Side::Corp);
    let DecisionSpec::PaidWindow { classes, options } = &paw.spec else {
        panic!("expected the 5.6.1b PAW, got {:?}", paw.spec);
    };
    assert!(classes.paid && classes.rez && classes.score, "(P)(R)(S) classes");
    assert!(
        options.iter().any(|o| matches!(o, WindowOption::Rez { .. })),
        "unrezzed asset offers an (R) option"
    );
    assert_eq!(vm.st.corp.clicks, 3, "1.11.2a: Corp allotted 3 clicks");

    // The mandatory draw happens before the action-phase PAW yields.
    let mut g = Script::new(
        quiet(Plan::corp().when(Match::action().once(), Reply::Halt)),
        quiet(Plan::runner()),
    );
    g.run(&mut vm);
    let first_action = g.transcript().last().expect("halted at the Corp action window");
    assert!(first_action.actions().contains(&ActionOption::BasicCredit));
    assert!(first_action.actions().contains(&ActionOption::BasicDraw));
    assert_eq!(vm.st.hand[&Side::Corp].len(), 6, "5.6.1e: mandatory draw");

    // Three actions: credit, credit, credit (5 → 8 credits), then the
    // discard phase demands exactly one discard (hand is 6 > 5).
    let mut g = Script::new(
        quiet(
            Plan::corp()
                .when(Match::action().times(3), Reply::credit())
                .when(Match::discard().once(), Reply::Halt)
                .when(Match::discard(), Reply::Default),
        ),
        quiet(Plan::runner()),
    );
    g.run(&mut vm);
    let disc = g.transcript().last().expect("halted at the discard decision");
    assert_eq!(disc.side, Side::Corp);
    assert_eq!(
        match &disc.spec {
            DecisionSpec::DiscardCards { count, .. } => *count,
            other => panic!("expected the discard decision, got {other:?}"),
        },
        1,
        "5.5.4c: discard down to max hand size"
    );
    let discarded = disc.candidates()[0];
    assert_eq!(vm.st.corp.credits, 8);

    // ---- Runner turn ----
    // The neutral discard answer takes the first card of the hand, which is
    // the one the assertion above named.
    let mut g = Script::new(
        quiet(Plan::corp().when(Match::discard(), Reply::Default)),
        quiet(Plan::runner().when(Match::action().once(), Reply::Halt)),
    );
    g.run(&mut vm);
    let runner_action = g.transcript().last().expect("halted at the Runner action window");
    assert_eq!(runner_action.side, Side::Runner);
    assert_eq!(vm.st.runner.clicks, 4, "1.11.2b: Runner allotted 4");
    assert!(runner_action
        .actions()
        .iter()
        .any(|o| matches!(o, ActionOption::BasicRun { server: ServerId::Hq })));
    assert_eq!(
        vm.st.objects[&discarded].zone,
        Zone::Discard(Side::Corp),
        "corp discard landed in Archives"
    );

    // === Run 1: HQ, ice stays unrezzed → pass path, breach HQ ===
    // Halt inside the approach-ice paid window to read the fund and the
    // 9.2.7e option, then let the run play out to the next action window.
    let mut g = Script::new(
        quiet(Plan::corp().when(Match::paid().approaching_ice().once(), Reply::Halt)),
        quiet(Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action()),
    );
    g.run(&mut vm);
    let approach = g.transcript().last().expect("halted in the approach-ice PAW");
    assert_eq!(approach.side, Side::Corp);
    assert!(
        approach
            .options()
            .iter()
            .any(|o| matches!(o, WindowOption::RezApproachedIce { card } if *card == ice)),
        "9.2.7e: the approached ice can be rezzed here"
    );
    assert_eq!(vm.st.bp_fund, 2, "6.9.1b: bad publicity fund filled");
    g.run(&mut vm);
    let t = g.transcript();
    assert!(
        !t.of_kind(Kind::JackOut).is_empty(),
        "6.9.4c: jack out offered in the Movement Phase"
    );
    let hq_accesses = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardAccessed { obj }
            if vm.st.hand[&Side::Corp].contains(obj)))
        .count();
    assert_eq!(hq_accesses, 1, "7.3.6: one access from HQ");
    assert_eq!(vm.st.bp_fund, 0, "6.9.6b: fund emptied in the Run Ends Phase");
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::IcePassed { ice: i, .. } if *i == ice)));
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: ServerId::Hq })));

    // === Run 2: HQ again, Corp rezzes → encounter, subroutine ends the run ===
    let corp_credits_before = vm.st.corp.credits;
    let mut g = Script::new(
        quiet(Plan::corp().when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce))),
        quiet(Plan::runner().when(Match::action().first(), Reply::run(ServerId::Hq)).stop_at_action()),
    );
    g.run(&mut vm);
    assert!(vm.st.objects[&ice].faceup, "ice rezzed");
    assert_eq!(vm.st.corp.credits, corp_credits_before - 2, "8.1.2e: rez cost paid");
    assert!(
        vm.changes.log.iter().any(
            |c| matches!(c, GameChange::SubroutineResolved { ice: i, index: 0 } if *i == ice)
        ),
        "6.9.3c: the unbroken subroutine resolved"
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { server: ServerId::Hq })),
        "6.8.4: an ETR'd run is declared unsuccessful"
    );
    assert!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. }))
            .count()
            == 1,
        "run 2 was not successful"
    );

    // === Run 3: the remote with the agenda → Success → breach → steal ===
    // Then the last click takes a credit and the Runner turn completes
    // cleanly into the next Corp turn (≤5 cards, so no discard is demanded).
    let mut g = Script::new(
        quiet(Plan::corp().when(Match::action().once(), Reply::Halt)),
        quiet(
            Plan::runner()
                .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
                .when(Match::action(), Reply::credit()),
        ),
    );
    g.run(&mut vm);
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Runner),
        "7.2.3: the accessed agenda was stolen"
    );
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::AgendaStolen { obj, points: 2 } if *obj == agenda)));
    // Next Corp turn reached: the Runner turn completed.
    let next = g.transcript().last().expect("halted at the next Corp action window");
    assert_eq!(next.side, Side::Corp);
    assert_eq!(vm.st.turn_side, Side::Corp);

    // Structural sanity: turn changes recorded in order.
    let turn_begins: Vec<Side> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::TurnBegan { side } => Some(*side),
            _ => None,
        })
        .collect();
    assert_eq!(turn_begins, vec![Side::Corp, Side::Runner, Side::Corp]);
    // The agenda is 2 points: no 7-point win fired.
    assert!(matches!(vm.step(), Yield::Decision(..)));
}

/// Scoring an agenda to 7 points wins at the checkpoint (10.3.1c), not
/// before: the win is detected in the checkpoint following the score.
#[test]
fn seven_point_win_at_checkpoint() {
    let mut vm = Vm::empty(3);
    // Corp already has 5 points scored; a 2-point agenda is ready to score.
    let scored = vm.new_object(tk::vanilla_agenda("Done Deal", 3, 5), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(scored);
    let ready = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("The Finisher", 2, 2),
        ServerId::Remote(1),
        false,
    );
    vm.st
        .objects
        .get_mut(&ready)
        .unwrap()
        .counters
        .insert(CounterKind::Advancement, 2);
    tk::fill_hand(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 5);

    // Corp turn: score the agenda in the first paid window that offers it.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid(), Reply::score(ready)).forbidding_the_rest(),
        Plan::runner().forbidding_the_rest(),
    );
    let scoring = t.first_window(Kind::Paid, Side::Corp);
    assert!(
        matches!(&scoring.spec, DecisionSpec::PaidWindow { classes, .. } if classes.score),
        "the (S) class is open in the Corp's paid window"
    );
    assert!(
        scoring
            .options()
            .iter()
            .any(|o| matches!(o, WindowOption::Score { card } if *card == ready)),
        "scorable agenda offered (1.17/9.2.7d)"
    );
    assert_eq!(
        t.result,
        Some(jinteki_cr::decision::GameResult::AgendaPoints(Side::Corp)),
        "10.3.1c: 7 points wins at the checkpoint"
    );
}
