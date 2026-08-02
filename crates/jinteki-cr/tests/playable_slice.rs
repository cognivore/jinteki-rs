//! The W1 playable-slice proof: a full scripted game driven exclusively
//! through `Vm::step()` decisions — setup (§1.6: draw 5, mulligan offers),
//! a full Corp turn (allotted clicks, (P)(R)(S) paid ability windows,
//! mandatory draw, action-window loop, discard phase), a full Runner turn,
//! and three runs: unrezzed-ice pass path, rezzed-encounter path where a
//! vanilla "End the run" subroutine ends the run, and an ice-free run
//! reaching Success → breach (11.5) → access (11.6, mid-access window,
//! agenda steal) → Run Ends with the bad-publicity fund emptying.

use jinteki_cr::change::GameChange;
use jinteki_cr::decision::{ActionOption, DecisionAnswer, DecisionSpec, WindowOption, Yield};
use jinteki_cr::object::{CounterKind, ServerId, Side, Zone};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::{GameSetup, Vm};

fn decision(vm: &mut Vm) -> (Side, DecisionSpec) {
    match vm.step() {
        Yield::Decision(s, d) => (s, d),
        other => panic!("expected decision, got {other:?}"),
    }
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
    let (side, spec) = decision(&mut vm);
    assert_eq!((side, &spec), (Side::Corp, &DecisionSpec::Mulligan));
    assert_eq!(vm.st.hand[&Side::Corp].len(), 5, "1.6.6: starting hand of 5");
    vm.answer(DecisionAnswer::KeepHand);
    let (side, spec) = decision(&mut vm);
    assert_eq!((side, &spec), (Side::Runner, &DecisionSpec::Mulligan));
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5);
    // Runner mulligans: new hand is still 5 cards.
    vm.answer(DecisionAnswer::TakeMulligan);

    // ---- Corp turn ----
    // First decision of the turn: the draw-phase PAW with (P)(R)(S) classes
    // (the Corp has a rez option, so the window yields a decision).
    let (side, spec) = decision(&mut vm);
    assert_eq!(side, Side::Corp);
    let DecisionSpec::PaidWindow { classes, options } = &spec else {
        panic!("expected the 5.6.1b PAW, got {spec:?}");
    };
    assert!(classes.paid && classes.rez && classes.score, "(P)(R)(S) classes");
    assert!(
        options.iter().any(|o| matches!(o, WindowOption::Rez { .. })),
        "unrezzed asset offers an (R) option"
    );
    assert_eq!(vm.st.corp.clicks, 3, "1.11.2a: Corp allotted 3 clicks");
    vm.answer(DecisionAnswer::Pass);

    // Mandatory draw happened before the action-phase PAW gives a decision.
    let hand_before_actions = loop {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { .. } => {
                assert_eq!(s, Side::Corp);
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::TakeAction { options } => {
                assert!(options.contains(&ActionOption::BasicCredit));
                assert!(options.contains(&ActionOption::BasicDraw));
                break vm.st.hand[&Side::Corp].len();
            }
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(hand_before_actions, 6, "5.6.1e: mandatory draw");

    // Three actions: credit, credit, credit (5 → 8 credits).
    vm.answer(DecisionAnswer::Action(ActionOption::BasicCredit));
    for _ in 0..2 {
        loop {
            let (s, spec) = decision(&mut vm);
            match &spec {
                DecisionSpec::PaidWindow { .. } => {
                    let _ = s;
                    vm.answer(DecisionAnswer::Pass);
                }
                DecisionSpec::TakeAction { .. } => {
                    vm.answer(DecisionAnswer::Action(ActionOption::BasicCredit));
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    // Discard phase: hand is 6 > 5, so exactly one discard is demanded.
    let discarded = loop {
        let (s, spec) = decision(&mut vm);
        match spec {
            DecisionSpec::PaidWindow { .. } => {
                let _ = s;
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::DiscardCards { count, hand } => {
                assert_eq!(s, Side::Corp);
                assert_eq!(count, 1, "5.5.4c: discard down to max hand size");
                let pick = hand[0];
                vm.answer(DecisionAnswer::Discard(vec![pick]));
                break pick;
            }
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(vm.st.corp.credits, 8);

    // ---- Runner turn ----
    // Drive to the Runner's first action window.
    loop {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { .. } => {
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::TakeAction { options } => {
                assert_eq!(s, Side::Runner);
                assert_eq!(vm.st.runner.clicks, 4, "1.11.2b: Runner allotted 4");
                assert!(options
                    .iter()
                    .any(|o| matches!(o, ActionOption::BasicRun { server: ServerId::Hq })));
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(
        vm.st.objects[&discarded].zone,
        Zone::Discard(Side::Corp),
        "corp discard landed in Archives"
    );

    // === Run 1: HQ, ice stays unrezzed → pass path, breach HQ ===
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    let mut saw_approach_rez_offer = false;
    let mut saw_jack_out = false;
    loop {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { classes, options } => {
                if classes.rez_approached_ice {
                    assert_eq!(s, Side::Corp);
                    assert!(
                        options
                            .iter()
                            .any(|o| matches!(o, WindowOption::RezApproachedIce { card } if *card == ice)),
                        "9.2.7e: the approached ice can be rezzed here"
                    );
                    saw_approach_rez_offer = true;
                    assert_eq!(vm.st.bp_fund, 2, "6.9.1b: bad publicity fund filled");
                }
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::JackOut => {
                saw_jack_out = true;
                vm.answer(DecisionAnswer::JackOut(false));
            }
            DecisionSpec::MidAccessWindow { .. } => {
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::TakeAction { .. } => break, // run over, next action
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(saw_approach_rez_offer);
    assert!(saw_jack_out, "6.9.4c: jack out offered in the Movement Phase");
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
        .any(|c| matches!(c, GameChange::IcePassed { ice: i } if *i == ice)));
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server: ServerId::Hq })));

    // === Run 2: HQ again, Corp rezzes → encounter, subroutine ends the run ===
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    let corp_credits_before = vm.st.corp.credits;
    loop {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { classes, options } => {
                if classes.rez_approached_ice && s == Side::Corp {
                    let rez = options
                        .iter()
                        .find(|o| matches!(o, WindowOption::RezApproachedIce { .. }))
                        .cloned();
                    if let Some(r) = rez {
                        vm.answer(DecisionAnswer::Take(r));
                        continue;
                    }
                }
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::JackOut => vm.answer(DecisionAnswer::JackOut(false)),
            DecisionSpec::TakeAction { .. } => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert!(vm.st.objects[&ice].faceup, "ice rezzed");
    assert_eq!(
        vm.st.corp.credits,
        corp_credits_before - 2,
        "8.1.2e: rez cost paid"
    );
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
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
        server: ServerId::Remote(1),
    }));
    loop {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { .. } => {
                let _ = s;
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::JackOut => vm.answer(DecisionAnswer::JackOut(false)),
            DecisionSpec::MidAccessWindow { .. } => vm.answer(DecisionAnswer::Pass),
            DecisionSpec::TakeAction { .. } => break,
            other => panic!("unexpected {other:?}"),
        }
    }
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

    // Last click: take a credit; then the Runner turn completes cleanly into
    // the next Corp turn (the discard step demands nothing at ≤5 cards).
    vm.answer(DecisionAnswer::Action(ActionOption::BasicCredit));
    loop {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { .. } => vm.answer(DecisionAnswer::Pass),
            DecisionSpec::TakeAction { .. } => {
                // Next Corp turn reached: the Runner turn completed.
                assert_eq!(s, Side::Corp);
                assert_eq!(vm.st.turn_side, Side::Corp);
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }

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

    // Corp turn: in the first (P)(R)(S) window, score the agenda.
    let r = loop {
        match vm.step() {
            Yield::Decision(side, DecisionSpec::PaidWindow { classes, options }) => {
                assert_eq!(side, Side::Corp);
                assert!(classes.score);
                let score_opt = options
                    .iter()
                    .find(|o| matches!(o, WindowOption::Score { card } if *card == ready))
                    .cloned()
                    .expect("scorable agenda offered (1.17/9.2.7d)");
                vm.answer(DecisionAnswer::Take(score_opt));
            }
            Yield::Decision(_, other) => panic!("unexpected {other:?}"),
            Yield::Progressed => continue,
            Yield::GameOver(r) => break r,
        }
    };
    assert_eq!(
        r,
        jinteki_cr::decision::GameResult::AgendaPoints(Side::Corp),
        "10.3.1c: 7 points wins at the checkpoint"
    );
}
