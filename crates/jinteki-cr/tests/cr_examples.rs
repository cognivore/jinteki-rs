//! DP-7a: the CR's own worked examples as executable tests.
//!
//! Each test carries the example id from docs/rules/examples.json in a
//! comment and asserts the outcome the rules authors state. The tracking
//! test at the bottom is the DP-7a odometer (implemented / 243); the
//! `#[ignore]`d placeholder enumerates every example id still to be done.

use jinteki_cr::change::GameChange;
use jinteki_cr::decision::{ActionOption, DecisionAnswer, DecisionSpec, WindowOption, Yield};
use jinteki_cr::effects::DamageKind;
use jinteki_cr::object::{CounterKind, ServerId, Side, Zone};
use jinteki_cr::testkit as tk;
use jinteki_cr::vm::Vm;

const EXAMPLES_JSON: &str = include_str!("../../../docs/rules/examples.json");

/// Example ids implemented as tests in this file (the DP-7a ledger).
const IMPLEMENTED: &[&str] = &[
    "example_rule_chain_reaction_1",
    "example_rule_active_exception_conditional_move_to_inactive_zone_1",
    "example_rule_reaction_window_closing_timing_structure_1",
    "example_rule_condition_met_multiple_times_1",
    "example_rule_conditional_ability_static_condition_no_effect_1",
    "example_rule_instruction_requirements_past_state_1",
    "example_rule_conditional_ability_lose_pending_when_ability_becomes_inactive_1",
    "example_rule_ordinal_would_1",
    "example_rule_negative_values_imminent_1",
    "example_rule_prevent_all_1",
    "example_rule_negative_values_resolution_1",
    "example_rule_run_ends_close_reaction_window_1",
    "example_rule_not_unsuccessful_when_reached_success_phase_1",
    "example_step_checkpoint_duration_abilities_1",
    "example_rule_checkpoint_after_timing_structure_1",
];

fn decision(vm: &mut Vm) -> (Side, DecisionSpec) {
    match vm.step() {
        Yield::Decision(s, d) => (s, d),
        other => panic!("expected decision, got {other:?}"),
    }
}

/// Drive until an action window for `side` (auto-passing windows and
/// declining choices), asserting no game-over on the way.
fn drive_to_action_window(vm: &mut Vm, side: Side) -> Vec<ActionOption> {
    for _ in 0..300 {
        let (s, spec) = decision(vm);
        match spec {
            DecisionSpec::TakeAction { options } => {
                assert_eq!(s, side);
                return options;
            }
            other => {
                let a = tk::default_answer(&other);
                vm.answer(a);
            }
        }
    }
    panic!("action window never reached");
}

fn window_options(spec: &DecisionSpec) -> Vec<WindowOption> {
    match spec {
        DecisionSpec::PaidWindow { options, .. }
        | DecisionSpec::ReactionWindow { options, .. }
        | DecisionSpec::InterruptWindow { options, .. }
        | DecisionSpec::MidAccessWindow { options } => options.clone(),
        _ => Vec::new(),
    }
}

// ===========================================================================
// §9.1 — abilities in general
// ===========================================================================

/// example_rule_chain_reaction_1 (9.1.2a): accessing a Snare!-class card
/// with 2 cards in grip; the Runner triggers a Decoy-class interrupt whose
/// trigger cost meets a Geist-class ability's condition. The chained ability
/// (draw 1) resolves FIRST, then Decoy avoids the tag, then the 3 net damage
/// resolves — and the Runner survives on exactly 0 cards.
#[test]
fn example_rule_chain_reaction_1() {
    let mut vm = Vm::empty(11);
    let snare = vm.new_object(tk::snare_like("Snare-like"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(snare);
    let decoy = tk::install_rig(&mut vm, tk::decoy_like("Decoy-like"));
    let geist = tk::install_rig(&mut vm, tk::geist_like("Geist-like"));
    tk::fill_hand(&mut vm, Side::Runner, 2);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Rnd }));

    let mut used_decoy = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner && !used_decoy => {
                let opt = options
                    .iter()
                    .find(|o| matches!(o, WindowOption::TriggerPaid { label, .. } if label.contains("decoy")))
                    .cloned()
                    .expect("Decoy-class interrupt offered while the tag is imminent");
                used_decoy = true;
                vm.answer(DecisionAnswer::Take(opt));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(used_decoy);
    // Outcomes stated by the example:
    assert_eq!(vm.st.runner.tags, 0, "the tag was avoided");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 0, "2 + 1 drawn - 3 damage");
    assert!(vm.game_over.is_none(), "the Runner survives");
    assert_eq!(vm.st.objects[&decoy].zone, Zone::Discard(Side::Runner));
    // Chain order: Geist-class draw resolved before Decoy's avoidance, which
    // resolved before Snare finished (frame completion order).
    let log = &vm.resolution_log;
    let gi = log.iter().position(|l| l.starts_with("Geist-like")).expect("geist resolved");
    let di = log.iter().position(|l| l.starts_with("Decoy-like")).expect("decoy resolved");
    let si = log.iter().position(|l| l.starts_with("Snare-like")).expect("snare resolved");
    assert!(gi < di && di < si, "9.1.2a: most recent condition resolves first: {log:?}");
    let _ = geist;
}

/// example_rule_active_exception_conditional_move_to_inactive_zone_1
/// (9.1.8g): a Singularity-class effect simultaneously trashes a rezzed
/// Hostile Infrastructure and 2 other cards. HI moves to Archives and is
/// inactive, but its ability remains active: 3 instances become pending,
/// each doing 1 net damage.
#[test]
fn example_rule_active_exception_conditional_move_to_inactive_zone_1() {
    let mut vm = Vm::empty(12);
    let hi = tk::install_root(&mut vm, tk::hostile_infra_like("HI-like"), ServerId::Remote(1), true);
    let u1 = tk::install_root(&mut vm, tk::vanilla_asset("U1", 0, 3), ServerId::Remote(2), true);
    let u2 = tk::install_root(&mut vm, tk::vanilla_asset("U2", 0, 3), ServerId::Remote(3), true);
    let button = tk::install_rig(&mut vm, tk::trash_set_button("Singularity-like", vec![hi, u1, u2]));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // Trigger the trash button in the first PAW (once).
    let mut corp_reaction_options = None;
    let mut fired = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner => {
                match tk::option_labeled(options, "trash the set") {
                    Some(opt) if !fired => {
                        fired = true;
                        vm.answer(DecisionAnswer::Take(opt));
                    }
                    _ => vm.answer(DecisionAnswer::Pass),
                }
            }
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Corp => {
                if corp_reaction_options.is_none() {
                    corp_reaction_options = Some(options.clone());
                }
                let a = tk::default_answer(&spec);
                vm.answer(a);
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    let opts = corp_reaction_options.expect("corp reaction window opened");
    assert_eq!(
        opts.iter()
            .filter(|o| matches!(o, WindowOption::TriggerInstance { label, .. } if label.contains("hostile-infra")))
            .count(),
        3,
        "9.1.8g + 9.6.4b: 3 instances pending though HI is in Archives"
    );
    assert_eq!(vm.st.objects[&hi].zone, Zone::Discard(Side::Corp));
    let dmg = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::DamageSuffered { kind: DamageKind::Net, amount: 1, .. }))
        .count();
    assert_eq!(dmg, 3, "each instance did 1 net damage");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2);
    let _ = button;
}

// ===========================================================================
// §9.2 — priority windows
// ===========================================================================

/// example_rule_reaction_window_closing_timing_structure_1 (9.2.8f): the
/// Runner bypasses a Tollbooth-class ice from inside the encounter-begins
/// reaction window; the encounter ends, the window closes immediately, and
/// the pending mandatory "when encountered" ability is never triggered: the
/// Runner does not pay, and the run does not end.
#[test]
fn example_rule_reaction_window_closing_timing_structure_1() {
    let mut vm = Vm::empty(13);
    let booth = tk::install_ice(&mut vm, tk::tollbooth_like("Tollbooth-like"), ServerId::Hq, true);
    let femme = tk::install_rig(&mut vm, tk::femme_like("Femme-like"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut used_femme = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner && !used_femme => {
                if let Some(opt) = tk::option_labeled(options, "femme") {
                    used_femme = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::NestedCost { cost_credits } => {
                assert_eq!(*cost_credits, 1);
                vm.answer(DecisionAnswer::PayNestedCost(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(used_femme);
    assert_eq!(vm.st.runner.credits, 4, "paid exactly the 1[c] bypass cost, never 3");
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run did not end; it continued and succeeded"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::IcePassed { ice } if *ice == booth)),
        "bypassing passes the ice"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { .. })),
        "no subroutines resolved"
    );
    let _ = femme;
}

// ===========================================================================
// §9.6 — conditional abilities
// ===========================================================================

/// example_rule_condition_met_multiple_times_1 (9.6.4b): trashing 3 Corp
/// cards simultaneously meets a Hostile-Infrastructure-class condition
/// separately for each card: 3 instances become pending in the same
/// reaction window. (A Warroid-class per-event ability pends once — 9.12.2a.)
#[test]
fn example_rule_condition_met_multiple_times_1() {
    let mut vm = Vm::empty(14);
    let hi = tk::install_root(&mut vm, tk::hostile_infra_like("HI-like"), ServerId::Remote(1), true);
    let w = tk::install_root(&mut vm, tk::warroid_like("Warroid-like"), ServerId::Remote(2), true);
    let t1 = tk::install_root(&mut vm, tk::vanilla_asset("T1", 0, 3), ServerId::Remote(3), true);
    let t2 = tk::install_root(&mut vm, tk::vanilla_asset("T2", 0, 3), ServerId::Remote(4), true);
    let t3 = tk::install_root(&mut vm, tk::vanilla_asset("T3", 0, 3), ServerId::Remote(5), true);
    tk::install_rig(&mut vm, tk::trash_set_button("Singularity-like", vec![t1, t2, t3]));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut corp_first_offer: Option<Vec<WindowOption>> = None;
    let mut fired = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner => {
                match tk::option_labeled(options, "trash the set") {
                    Some(opt) if !fired => {
                        fired = true;
                        vm.answer(DecisionAnswer::Take(opt));
                    }
                    _ => vm.answer(DecisionAnswer::Pass),
                }
            }
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Corp => {
                if corp_first_offer.is_none() {
                    corp_first_offer = Some(options.clone());
                }
                let a = tk::default_answer(&spec);
                vm.answer(a);
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    let opts = corp_first_offer.expect("reaction window opened");
    let hi_count = opts
        .iter()
        .filter(|o| matches!(o, WindowOption::TriggerInstance { label, .. } if label.contains("hostile-infra")))
        .count();
    let w_count = opts
        .iter()
        .filter(|o| matches!(o, WindowOption::TriggerInstance { label, .. } if label.contains("warroid")))
        .count();
    assert_eq!(hi_count, 3, "9.6.4b: one instance per trashed card");
    assert_eq!(w_count, 1, "9.12.2a: the set-trigger sees one event");
    let _ = (hi, w);
}

/// example_rule_conditional_ability_static_condition_no_effect_1 (9.6.7d):
/// a Parasite-class static-condition ability on a 0-strength ice.
/// (a) If an interrupt prevents the trash, the condition stays true and the
/// ability pends again immediately.
/// (b) If an Architect-class static prohibits the trash, there are no
/// expected effects and the ability resolves only once per structure step.
#[test]
fn example_rule_conditional_ability_static_condition_no_effect_1() {
    // ---- (a) prevention: re-pends, second resolution trashes the ice ----
    let mut vm = Vm::empty(15);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Weak Ice", 0, 0), ServerId::Hq, true);
    let para = tk::install_rig(&mut vm, tk::parasite_like("Parasite-like"));
    tk::host_on(&mut vm, para, ice);
    let sac = tk::install_rig(&mut vm, tk::sac_con_like("SacCon-like", ice));
    vm.start_turn(Side::Runner);

    let mut used_sac = false;
    let mut parasite_triggers = 0;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "parasite") {
                    parasite_triggers += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner && !used_sac => {
                if let Some(opt) = tk::option_labeled(options, "sac-con") {
                    used_sac = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(used_sac, "the trash was prevented once");
    assert_eq!(
        parasite_triggers, 2,
        "prevented → condition still true → pends again (9.6.7d)"
    );
    assert_eq!(vm.st.objects[&ice].zone, Zone::Discard(Side::Corp), "second try trashed it");
    assert_eq!(vm.st.objects[&sac].zone, Zone::Discard(Side::Runner));

    // ---- (b) prohibition: no expected effects → once per structure step ----
    let mut vm = Vm::empty(16);
    let arch = tk::install_ice(&mut vm, tk::architect_like("Architect-like"), ServerId::Hq, true);
    let para = tk::install_rig(&mut vm, tk::parasite_like("Parasite-like"));
    tk::host_on(&mut vm, para, arch);
    vm.start_turn(Side::Runner);

    let mut parasite_triggers = 0;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "parasite") {
                    parasite_triggers += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(vm.st.objects[&arch].zone.is_installed(), "the ice was never trashed");
    // Steps before the first action window: allotted clicks, PAW, refill,
    // turn-begins, PAW, action-branch — the throttle limits the ability to
    // at most one resolution per completed step, and the machine does not
    // livelock.
    assert!(
        (1..=8).contains(&parasite_triggers),
        "one resolution per structure step, no livelock: {parasite_triggers}"
    );
}

/// example_rule_instruction_requirements_past_state_1 (9.6.6a): a Built to
/// Last-class ability gains 2[c] only when advancing a card that HAD no
/// advancement counters, judged against the state at the previous
/// checkpoint's step (a) — so the first advancement pays, the second does
/// not.
#[test]
fn example_rule_instruction_requirements_past_state_1() {
    let mut vm = Vm::empty(17);
    let id_card = vm.new_object(tk::built_to_last_like("BtL-like"), Zone::PlayArea(Side::Corp));
    vm.st.objects.get_mut(&id_card).unwrap().faceup = true;
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 1), ServerId::Hq, true);
    tk::install_root(&mut vm, tk::advance_button_card("Advancer", ice), ServerId::Remote(1), true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    for advance_round in 0..2 {
        let options = drive_to_action_window(&mut vm, Side::Corp);
        let adv = options
            .iter()
            .find(|o| matches!(o, ActionOption::CardAction { label, .. } if label.contains("advance")))
            .cloned()
            .expect("advance action available");
        vm.answer(DecisionAnswer::Action(adv));
        let _ = advance_round;
    }
    let _ = drive_to_action_window(&mut vm, Side::Corp);

    assert_eq!(vm.st.objects[&ice].counter(CounterKind::Advancement), 2);
    assert_eq!(
        vm.st.corp.credits,
        5 + 2,
        "9.6.6a: only the first advancement met the 'had no counters' condition"
    );
    let btl_resolutions = vm
        .resolution_log
        .iter()
        .filter(|l| l.starts_with("BtL-like"))
        .count();
    assert_eq!(btl_resolutions, 1);
}

/// example_rule_conditional_ability_lose_pending_when_ability_becomes_inactive_1
/// (9.6.10): Aesop's-class and Drug-Dealer-class both pend at turn begin;
/// triggering Aesop's first and trashing Drug Dealer with it drops Drug
/// Dealer's pending instance — the Runner does not lose a credit.
#[test]
fn example_rule_conditional_ability_lose_pending_when_ability_becomes_inactive_1() {
    let mut vm = Vm::empty(18);
    let _aesops = tk::install_rig(&mut vm, tk::aesops_like("Aesops-like"));
    let dd = tk::install_rig(&mut vm, tk::drug_dealer_like("DrugDealer-like"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut triggered_aesops = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "aesops") {
                    triggered_aesops = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    // After Aesop's resolves, Drug Dealer's pending must be
                    // GONE: passing must be legal (no mandatory pendings).
                    if triggered_aesops {
                        assert!(
                            tk::option_labeled(options, "drug-dealer").is_none(),
                            "9.6.10: Drug Dealer's instance lost its pending status"
                        );
                    }
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::ChooseTargets { candidates, .. } => {
                assert!(candidates.contains(&dd));
                vm.answer(DecisionAnswer::Targets(vec![dd]));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(triggered_aesops);
    assert_eq!(vm.st.objects[&dd].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.runner.credits, 5 + 3, "gained 3, never lost the 1");
}

// ===========================================================================
// §9.9 — interrupts, values, prevention
// ===========================================================================

/// example_rule_ordinal_would_1 (9.9.5a): a Tori-Hanzō-class "the first time
/// you would do net damage each run" interrupt. The first imminent net
/// damage is fully prevented; the second imminence is the SECOND "would",
/// so the ability can never be used this run.
#[test]
fn example_rule_ordinal_would_1() {
    let mut vm = Vm::empty(19);
    tk::install_root(&mut vm, tk::tori_like("Tori-like"), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::net_damage_button("Zapper", 1), ServerId::Remote(2), true);
    tk::install_rig(&mut vm, tk::feedback_like("Feedback-like"));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Archives }));

    let mut zaps = 0;
    let mut tori_ever_offered = false;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        for o in window_options(&spec) {
            if let WindowOption::TriggerInstance { label, .. } = o {
                if label.contains("tori") {
                    tori_ever_offered = true;
                }
            }
        }
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && zaps < 2 => {
                if let Some(opt) = tk::option_labeled(options, "do net damage") {
                    zaps += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "feedback") {
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(zaps, 2, "two net-damage instructions became imminent");
    assert!(
        !tori_ever_offered,
        "9.9.5a: after the first (prevented) imminence, Tori is never usable"
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        4,
        "first damage prevented, second resolved for 1"
    );
}

/// example_rule_negative_values_imminent_1 (9.9.7a): 1 meat damage is
/// imminent; the Runner prevents 2 (value −1); the Corp adds +1 twice
/// (0, then 1); the ability resolves with 1 damage.
#[test]
fn example_rule_negative_values_imminent_1() {
    let mut vm = Vm::empty(20);
    tk::install_root(&mut vm, tk::mr_stone_like("Stone-like"), ServerId::Remote(1), true);
    // Two Cleaners-class free "+1 meat" interrupts, per the example's flow.
    let c1 = vm.new_object(tk::cleaners_like("Cleaners-A"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(c1);
    let c2 = vm.new_object(tk::cleaners_like("Cleaners-B"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(c2);
    tk::install_rig(&mut vm, tk::take_tag_button("TagMe"));
    tk::install_rig(&mut vm, tk::biometric_like("Biometric-like", DamageKind::Meat));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut took_tag = false;
    let mut prevented = false;
    let mut cleaners_uses = 0;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner && !took_tag => {
                if let Some(opt) = tk::option_labeled(options, "take 1 tag") {
                    took_tag = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner && !prevented => {
                if let Some(opt) = tk::option_labeled(options, "biometric") {
                    prevented = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Corp && cleaners_uses < 2 => {
                if let Some(opt) = tk::option_labeled(options, "cleaners") {
                    cleaners_uses += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(took_tag && prevented);
    assert_eq!(cleaners_uses, 2, "the value was modifiable at 0 and below (9.9.7a)");
    let meat: u32 = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::DamageSuffered { kind: DamageKind::Meat, amount, .. } => Some(*amount),
            _ => None,
        })
        .sum();
    assert_eq!(meat, 1, "resolved with exactly 1 damage");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 4);
}

/// example_rule_prevent_all_1 (9.9.7b): preventing ALL of the imminent
/// damage removes the damage from the expected effects entirely — there is
/// no longer a value, so a Cleaners-class "+1" is not relevant and cannot
/// be used.
#[test]
fn example_rule_prevent_all_1() {
    let mut vm = Vm::empty(21);
    let c1 = vm.new_object(tk::cleaners_like("Cleaners-A"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(c1);
    tk::install_root(
        &mut vm,
        tk::meat_damage_button("Damager", 2),
        ServerId::Remote(1),
        true,
    );
    tk::install_rig(&mut vm, tk::prevent_all_like("ChromeParlor-like", DamageKind::Meat));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    // Corp's turn so the Corp holds priority first and passes it (the
    // example's sequence has the Runner prevent before the Corp adds).
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let mut fired = false;
    let mut prevented_all = false;
    let mut cleaners_offered_after_prevent = false;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        if prevented_all {
            if let DecisionSpec::InterruptWindow { options, .. } = &spec {
                if s == Side::Corp && tk::option_labeled(options, "cleaners").is_some() {
                    cleaners_offered_after_prevent = true;
                }
            }
        }
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && !fired => {
                if let Some(opt) = tk::option_labeled(options, "do meat damage") {
                    fired = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner && !prevented_all => {
                if let Some(opt) = tk::option_labeled(options, "chrome-parlor") {
                    prevented_all = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(fired && prevented_all);
    assert!(
        !cleaners_offered_after_prevent,
        "9.9.7b: with the damage effect removed, +1 is no longer relevant"
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "no damage occurred");
}

/// example_rule_negative_values_resolution_1 (9.9.7d): a Breached-Dome-class
/// "do 1 net damage and trash the top card of the stack" is accessed; the
/// Runner prevents 2 damage (value −1). At resolution the dead value drops
/// the damage part only: the top of the stack is still trashed.
#[test]
fn example_rule_negative_values_resolution_1() {
    let mut vm = Vm::empty(22);
    let dome = vm.new_object(tk::breached_dome_like("Dome-like"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(dome);
    tk::install_rig(&mut vm, tk::biometric_like("Biometric-like", DamageKind::Net));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    let stack = tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Archives }));

    let mut prevented = false;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner && !prevented => {
                if let Some(opt) = tk::option_labeled(options, "biometric") {
                    prevented = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(prevented);
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "the damage did not occur");
    assert_eq!(
        vm.st.objects[&stack[0]].zone,
        Zone::Discard(Side::Runner),
        "the rest of the instruction resolved: top of stack trashed"
    );
}

// ===========================================================================
// §6.8 — the Run Ends phase
// ===========================================================================

/// example_rule_run_ends_close_reaction_window_1 (6.8.2b): in the
/// encounter-begins reaction window the Runner resolves a Security-Nexus-
/// class ability that ends the run; the other pending "when encountered"
/// ability (a tag tax here) is in the same window and is never resolved.
#[test]
fn example_rule_run_ends_close_reaction_window_1() {
    let mut vm = Vm::empty(23);
    let mut taxice = tk::vanilla_ice("TagTax", 0, 3);
    taxice.abilities = vec![jinteki_cr::ability::AbilityDef::conditional(
        jinteki_cr::ability::TriggerCond::SelfEncountered,
        vec![jinteki_cr::instr::Instruction::GainTags(2)],
        false,
    )
    .labeled("tag-tax: 2 tags on encounter")];
    tk::install_ice(&mut vm, taxice, ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::nexus_like("Nexus-like"));
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut used_nexus = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner && !used_nexus => {
                if let Some(opt) = tk::option_labeled(options, "nexus") {
                    used_nexus = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::OptionalEffect { .. } => {
                vm.answer(DecisionAnswer::ResolveOptional(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(used_nexus);
    assert_eq!(vm.st.runner.tags, 0, "6.8.2b: the pending tag tax was never resolved");
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })));
}

/// example_rule_not_unsuccessful_when_reached_success_phase_1 (6.8.4a): a
/// Crisium-class ability stops the run from being declared successful, but
/// the run is not declared unsuccessful either.
#[test]
fn example_rule_not_unsuccessful_when_reached_success_phase_1() {
    let mut vm = Vm::empty(24);
    tk::install_root(&mut vm, tk::crisium_like("Crisium-like"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    let _ = drive_to_action_window(&mut vm, Side::Runner);

    assert!(vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })));
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "not declared successful"
    );
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "6.8.4a: …and not declared unsuccessful either"
    );
}

// ===========================================================================
// §10.3 — checkpoints
// ===========================================================================

/// example_step_checkpoint_duration_abilities_1 (10.3.1b): strength boosts
/// on an icebreaker expire in the checkpoint after the encounter ends, and
/// the strength drops back.
#[test]
fn example_step_checkpoint_duration_abilities_1() {
    let mut vm = Vm::empty(25);
    tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 3), ServerId::Hq, true);
    let breaker = tk::install_rig(&mut vm, tk::pump_breaker("Breaker", 1));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut pumped = 0;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. }
                if s == Side::Runner && vm.st.encounter.is_some() =>
            {
                if pumped == 2 {
                    // Both pumps have resolved (9.10.4a duration in force).
                    assert_eq!(
                        vm.effective_strength(breaker),
                        Some(5),
                        "1 base + 2×2 pump while the encounter lasts"
                    );
                    vm.answer(DecisionAnswer::Pass);
                } else if let Some(opt) = tk::option_labeled(options, "pump") {
                    pumped += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(pumped, 2);
    assert_eq!(
        vm.effective_strength(breaker),
        Some(1),
        "10.3.1b: the pumps expired with the encounter"
    );
}

/// example_rule_checkpoint_after_timing_structure_1 (10.3.6): an AMAZE-class
/// "whenever a run on this server ends" ability pends in the reaction window
/// AFTER `step_run_complete` — outside the run — so a Jesminder-class
/// "avoid a tag during a run" interrupt cannot apply, and the Runner takes
/// both tags.
#[test]
fn example_rule_checkpoint_after_timing_structure_1() {
    let mut vm = Vm::empty(26);
    tk::install_root(&mut vm, tk::amaze_like("AMAZE-like"), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::jesminder_like("Jesminder-like"));
    vm.st.runner.credits = 1; // cannot pay the 3[c] trash cost
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
        server: ServerId::Remote(1),
    }));

    let mut jesminder_offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        for o in window_options(&spec) {
            if let WindowOption::TriggerInstance { label, .. } = o {
                if label.contains("jesminder") {
                    jesminder_offered = true;
                }
            }
        }
        match &spec {
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let _ = s;
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(
        vm.st.runner.tags, 2,
        "10.3.6: the tags landed outside the run; Jesminder could not apply"
    );
    assert!(
        !jesminder_offered,
        "the interrupt was never relevant (no run in progress)"
    );
}

// ===========================================================================
// DP-7a odometer
// ===========================================================================

/// Tracking test: implemented / total examples, and ledger integrity.
#[test]
fn dp7a_odometer() {
    let v: serde_json::Value = serde_json::from_str(EXAMPLES_JSON).unwrap();
    let all: Vec<String> = v["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    for id in IMPLEMENTED {
        assert!(
            all.iter().any(|a| a == id),
            "ledger entry {id} is not a real example id"
        );
    }
    println!(
        "DP-7a odometer: {} implemented / {} total CR examples ({:.1}%)",
        IMPLEMENTED.len(),
        all.len(),
        100.0 * IMPLEMENTED.len() as f64 / all.len() as f64
    );
    assert!(IMPLEMENTED.len() >= 12, "W1 mandate: at least 12 examples");
}

/// The full remaining-work list: every CR example id not yet implemented.
/// Un-ignore as the suite grows toward DP-7a = 100%.
#[test]
#[ignore = "DP-7a backlog: 243 examples total; see dp7a_odometer for progress"]
fn dp7a_backlog_placeholder() {
    let v: serde_json::Value = serde_json::from_str(EXAMPLES_JSON).unwrap();
    let mut missing: Vec<String> = v["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .filter(|id| !IMPLEMENTED.contains(&id.as_str()))
        .collect();
    missing.sort();
    panic!(
        "{} CR examples not yet implemented:\n{}",
        missing.len(),
        missing.join("\n")
    );
}
