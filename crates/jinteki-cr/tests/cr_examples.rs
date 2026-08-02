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
    // Wave 2a: the cost system (§1.16, §9.5) + 9.8.7c.
    "example_rule_cost_zero_1",
    "example_rule_cost_no_interrupt_1",
    "example_rule_cost_interrupt_static_mandatory_2",
    "example_rule_cost_restrictions_1",
    "example_rule_decline_additional_cost_1",
    "example_rule_additonal_cost_simultaenous_1",
    "example_rule_nested_cost_unless_1",
    "example_rule_trash_ability_keeps_track_of_hosted_objects_1",
    "example_rule_trash_ability_keeps_track_of_hosted_objects_3",
    "example_rule_paid_ability_refers_to_encountered_ice_1",
    "example_rule_paid_ability_refers_to_approached_ice_1",
    "example_rule_resolve_subroutines_run_ends_1",
    // Wave 2b: interrupts & replacement effects (§9.9), persistent (9.12.5).
    "example_rule_expected_effects_1",
    "example_rule_expected_effects_2",
    "example_rule_would_relevant_1",
    "example_rule_trigger_conditional_ability_interrupt_1",
    "example_rule_modified_values_retain_properties_1",
    "example_rule_replace_imminent_effects_1",
    "example_rule_persistent_applicability_1",
    // Wave 2c: traces (10.8) and bidding/psi (10.14).
    "example_rule_trace_conditional_abilities_1",
    "example_rule_bid_possible_1",
    "example_rule_bid_possible_2",
    // Wave 2d: subroutine origin categories (9.8.3) and candidates (7.4).
    "example_rule_subroutine_origin_static_after_1",
    "example_rule_subroutine_origin_static_after_2",
    "example_rule_subroutine_origin_external_after_1",
    "example_rule_prohibiting_access_1",
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
            DecisionSpec::NestedCost { cost } => {
                assert_eq!(cost.credits, 1);
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
// §1.16 — costs (wave 2a)
// ===========================================================================

/// example_rule_cost_zero_1 (1.16.1d): a Khumalo-class ability trashes an
/// accessed 0-cost card by "spending" nothing — the zero cost is still
/// really paid (a CostPaid event exists) and the trash happens.
#[test]
fn example_rule_cost_zero_1() {
    let mut vm = Vm::empty(30);
    let beanstalk = vm.new_object(tk::corp_filler("Beanstalk-like"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(beanstalk);
    tk::install_rig(&mut vm, tk::khumalo_like("Khumalo-like"));
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Rnd }));

    let mut used = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::MidAccessWindow { options } if s == Side::Runner && !used => {
                if let Some(opt) = tk::option_labeled(options, "khumalo") {
                    used = true;
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
    assert!(used, "the mid-access ability was offered and used");
    assert_eq!(
        vm.st.objects[&beanstalk].zone,
        Zone::Discard(Side::Corp),
        "the accessed card was trashed"
    );
    // 1.16.1d: the zero cost was really paid — a payment event exists.
    assert!(vm.changes.log.iter().any(
        |c| matches!(c, GameChange::CostPaid { side: Side::Runner, credits: 0, clicks: 0, .. })
    ));
}

/// example_rule_cost_no_interrupt_1 (1.16.1a): trashing Clone Chip as its
/// own trigger cost cannot be prevented — no interrupt window opens against
/// a cost payment, so the LLDS-class preventer is never offered.
#[test]
fn example_rule_cost_no_interrupt_1() {
    let mut vm = Vm::empty(31);
    let chip = tk::install_rig(&mut vm, tk::clone_chip_like("CloneChip-like"));
    tk::install_rig(&mut vm, tk::llds_like("LLDS-like", chip));
    vm.start_turn(Side::Runner);

    let mut triggered = false;
    let mut llds_ever_offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        if let DecisionSpec::InterruptWindow { options, .. } = &spec {
            if tk::option_labeled(options, "llds").is_some() {
                llds_ever_offered = true;
            }
        }
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner && !triggered => {
                if let Some(opt) = tk::option_labeled(options, "clone-chip") {
                    triggered = true;
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
    assert!(triggered);
    assert_eq!(vm.st.objects[&chip].zone, Zone::Discard(Side::Runner));
    assert!(
        !llds_ever_offered,
        "1.16.1a: cost payment cannot be interrupted or prevented"
    );
    assert_eq!(vm.st.runner.credits, 1, "the ability still resolved");
}

/// example_rule_cost_interrupt_static_mandatory_2 (1.16.1b): with a
/// mandatory Jesminder-class tag-avoid active during a run, the Runner
/// CANNOT take 1 tag to pay Funhouse's nested cost — so Funhouse ends the
/// run.
#[test]
fn example_rule_cost_interrupt_static_mandatory_2() {
    let mut vm = Vm::empty(32);
    tk::install_ice(&mut vm, tk::funhouse_like("Funhouse-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::jesminder_like("Jesminder-like"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut nested_cost_offered = false;
    for _ in 0..300 {
        let (_, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::NestedCost { .. } => {
                nested_cost_offered = true;
                vm.answer(DecisionAnswer::PayNestedCost(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(
        !nested_cost_offered,
        "1.16.1b: the unpayable cost is never offered as a choice"
    );
    assert_eq!(vm.st.runner.tags, 0, "no tag was taken");
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })),
        "Funhouse ended the run"
    );
}

/// example_rule_cost_restrictions_1 (1.16.1c): a Zer0-class once-per-turn
/// ability with a damage component in its cost cannot even be ATTEMPTED a
/// second time — no extra damage is suffered.
#[test]
fn example_rule_cost_restrictions_1() {
    let mut vm = Vm::empty(33);
    tk::install_rig(&mut vm, tk::zer0_like("Zer0-like"));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut uses = 0;
    let mut offered_again_after_use = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner => {
                match tk::option_labeled(options, "zer0") {
                    Some(opt) if uses == 0 => {
                        uses += 1;
                        vm.answer(DecisionAnswer::Take(opt));
                    }
                    Some(_) => {
                        offered_again_after_use = true;
                        vm.answer(DecisionAnswer::Pass);
                    }
                    None => vm.answer(DecisionAnswer::Pass),
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(uses, 1);
    assert!(
        !offered_again_after_use,
        "1.16.1c: the once-per-turn restriction forbids attempting the cost again"
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 4, "exactly 1 net damage suffered");
    assert_eq!(vm.st.runner.credits, 1);
}

/// example_rule_decline_additional_cost_1 (1.16.10a): stealing is normally
/// mandatory, but an agenda with an additional steal cost can be declined —
/// the Runner suffers nothing and does not steal.
#[test]
fn example_rule_decline_additional_cost_1() {
    let mut vm = Vm::empty(34);
    let obokata = tk::install_root(
        &mut vm,
        tk::obokata_like("Obokata-like", 3),
        ServerId::Remote(1),
        false,
    );
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
        server: ServerId::Remote(1),
    }));

    let mut offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::NestedCost { cost } => {
                assert_eq!(s, Side::Runner);
                assert_eq!(cost.net_damage, 4);
                offered = true;
                vm.answer(DecisionAnswer::PayNestedCost(false));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(offered, "the pay-or-decline choice was presented");
    assert_eq!(
        vm.st.objects[&obokata].zone,
        Zone::Root(ServerId::Remote(1)),
        "declined: the agenda was not stolen"
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "no damage suffered");
}

/// example_rule_additonal_cost_simultaenous_1 (1.16.10b): Obokata-class 4
/// net + Musashi-class +2 net + Predictive-class +2[c] combine into ONE
/// all-at-once payment; abilities triggered by the payment resolve after
/// it, before the steal.
#[test]
fn example_rule_additonal_cost_simultaenous_1() {
    let mut vm = Vm::empty(35);
    let obokata = tk::install_root(
        &mut vm,
        tk::obokata_like("Obokata-like", 3),
        ServerId::Remote(1),
        false,
    );
    tk::install_root(&mut vm, tk::musashi_like("Musashi-like"), ServerId::Remote(1), true);
    let pred = vm.new_object(tk::predictive_like("Predictive-like"), Zone::PlayArea(Side::Corp));
    vm.st.objects.get_mut(&pred).unwrap().faceup = true;
    tk::install_rig(&mut vm, tk::sol_like("Sol-like"));
    tk::fill_hand(&mut vm, Side::Runner, 6);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
        server: ServerId::Remote(1),
    }));

    for _ in 0..400 {
        let (_, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::NestedCost { cost } => {
                // One aggregated all-at-once payment (1.16.10b).
                assert_eq!(cost.net_damage, 6);
                assert_eq!(cost.credits, 2);
                vm.answer(DecisionAnswer::PayNestedCost(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(
        vm.st.objects[&obokata].zone,
        Zone::ScoreArea(Side::Runner),
        "paid in full: stolen"
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 0, "6 net damage as one payment");
    // 3 - 2 (cost) + 1 (Sol-class trigger after payment) = 2.
    assert_eq!(vm.st.runner.credits, 2);
    // Ordering: the damage payment precedes the Sol-class gain, which
    // precedes the steal (1.16.10b: triggers resolve after payment).
    let log = &vm.changes.log;
    let dmg = log.iter().position(|c| matches!(c, GameChange::DamageSuffered { amount: 6, .. }))
        .expect("one aggregated damage payment");
    let gain = log.iter().position(
        |c| matches!(c, GameChange::CreditsGained { side: Side::Runner, amount: 1 }),
    )
    .expect("sol-class trigger resolved");
    let steal = log.iter().position(|c| matches!(c, GameChange::AgendaStolen { .. })).unwrap();
    assert!(dmg < gain && gain < steal);
}

/// example_rule_nested_cost_unless_1 (1.16.11b): "End the run unless the
/// Runner pays 1[credit]." — paying suppresses the end-the-run.
#[test]
fn example_rule_nested_cost_unless_1() {
    let mut vm = Vm::empty(36);
    tk::install_ice(&mut vm, tk::etr_unless_pay_ice("Toll-like"), ServerId::Hq, true);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut paid = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::NestedCost { cost } => {
                assert_eq!(s, Side::Runner);
                assert_eq!(cost.credits, 1);
                paid = true;
                vm.answer(DecisionAnswer::PayNestedCost(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(paid);
    assert_eq!(vm.st.runner.credits, 4);
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "paying the nested cost meant the subroutine did not end the run"
    );
}

// ===========================================================================
// §9.5 — paid abilities (wave 2a)
// ===========================================================================

/// example_rule_trash_ability_keeps_track_of_hosted_objects_1 (9.5.5): a
/// Fermenter-class [trash] ability sets its 4 hosted virus counters aside as
/// the cost is paid; the effect still counts them: the Runner gains 8.
#[test]
fn example_rule_trash_ability_keeps_track_of_hosted_objects_1() {
    let mut vm = Vm::empty(37);
    let ferm = tk::install_rig(&mut vm, tk::fermenter_like("Fermenter-like"));
    vm.st
        .objects
        .get_mut(&ferm)
        .unwrap()
        .counters
        .insert(CounterKind::Virus, 4);
    vm.start_turn(Side::Runner);

    let mut used = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner && !used => {
                if let Some(opt) = tk::option_labeled(options, "fermenter") {
                    used = true;
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
    assert!(used);
    assert_eq!(vm.st.objects[&ferm].zone, Zone::Discard(Side::Runner));
    assert_eq!(
        vm.st.runner.credits, 8,
        "9.5.5: the set-aside counters still counted as hosted"
    );
    assert_eq!(
        vm.st.objects[&ferm].counter(CounterKind::Virus),
        0,
        "the counters were returned to the bank afterwards"
    );
}

/// example_rule_trash_ability_keeps_track_of_hosted_objects_3 (9.5.5): the
/// Reconstruction-Contract-class sequence in detail — pay the [trash] cost
/// setting the advancement counters aside, checkpoint, choose the target,
/// interrupt window, then move the set-aside counters to the target.
#[test]
fn example_rule_trash_ability_keeps_track_of_hosted_objects_3() {
    let mut vm = Vm::empty(38);
    let rc = tk::install_root(
        &mut vm,
        tk::reconstruction_like("Reconstruction-like"),
        ServerId::Remote(1),
        true,
    );
    vm.st
        .objects
        .get_mut(&rc)
        .unwrap()
        .counters
        .insert(CounterKind::Advancement, 3);
    let wall = tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 1), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let mut used = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && !used => {
                if let Some(opt) = tk::option_labeled(options, "reconstruction") {
                    used = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::ChooseTargets { candidates, .. } => {
                // 9.5.7c: the target is chosen after the cost-paid
                // checkpoint, as the instruction becomes imminent.
                assert!(candidates.contains(&wall));
                vm.answer(DecisionAnswer::Targets(vec![wall]));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(used);
    assert_eq!(vm.st.objects[&rc].zone, Zone::Discard(Side::Corp));
    assert_eq!(
        vm.st.objects[&wall].counter(CounterKind::Advancement),
        3,
        "9.5.5: the set-aside counters were moved to the target"
    );
}

/// example_rule_paid_ability_refers_to_encountered_ice_1 (9.5.6c): an
/// Arruaceiras-class ability cannot be triggered at an arbitrary time —
/// only during an encounter.
#[test]
fn example_rule_paid_ability_refers_to_encountered_ice_1() {
    let mut vm = Vm::empty(39);
    tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 1), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::arruaceiras_like("Arruaceiras-like"));
    vm.start_turn(Side::Runner);

    let mut offered_outside_encounter = false;
    let mut offered_during_encounter = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        if let DecisionSpec::PaidWindow { options, .. } = &spec {
            if s == Side::Runner && tk::option_labeled(options, "arruaceiras").is_some() {
                if vm.st.encounter.is_some() {
                    offered_during_encounter = true;
                } else {
                    offered_outside_encounter = true;
                }
            }
        }
        match &spec {
            DecisionSpec::TakeAction { options, .. } => {
                if !offered_during_encounter {
                    assert!(options.iter().any(
                        |o| matches!(o, ActionOption::BasicRun { server: ServerId::Hq })
                    ));
                    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
                        server: ServerId::Hq,
                    }));
                } else {
                    break;
                }
            }
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(
        !offered_outside_encounter,
        "9.5.6c: not usable at an arbitrary time"
    );
    assert!(offered_during_encounter, "usable during the encounter");
}

/// example_rule_paid_ability_refers_to_approached_ice_1 (9.5.6b): a
/// Wotan-class ability is usable only while the Runner is approaching a
/// REZZED *bioroid* piece of ice.
#[test]
fn example_rule_paid_ability_refers_to_approached_ice_1() {
    // Non-bioroid approach: never offered.
    let mut vm = Vm::empty(40);
    tk::install_ice(&mut vm, tk::vanilla_ice("Wall", 0, 1), ServerId::Hq, true);
    let w = vm.new_object(tk::wotan_like("Wotan-like"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(w);
    vm.start_turn(Side::Runner);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    let mut offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        if let DecisionSpec::PaidWindow { options, .. } = &spec {
            if s == Side::Corp && tk::option_labeled(options, "wotan").is_some() {
                offered = true;
            }
        }
        match &spec {
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(!offered, "9.5.6b: not offered while approaching non-bioroid ice");

    // Rezzed bioroid approach: offered in the approach window.
    let mut vm = Vm::empty(41);
    let mut bio = tk::vanilla_ice("Eli-like", 0, 3);
    bio.subtypes = vec!["bioroid", "barrier"];
    tk::install_ice(&mut vm, bio, ServerId::Hq, true);
    let w = vm.new_object(tk::wotan_like("Wotan-like"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(w);
    vm.start_turn(Side::Runner);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    let mut offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        if let DecisionSpec::PaidWindow { classes, options } = &spec {
            if s == Side::Corp
                && classes.rez_approached_ice
                && tk::option_labeled(options, "wotan").is_some()
            {
                offered = true;
            }
        }
        match &spec {
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(offered, "9.5.6b: offered while approaching rezzed bioroid ice");
}

// ===========================================================================
// §9.8 — subroutines (wave 2a)
// ===========================================================================

/// example_rule_resolve_subroutines_run_ends_1 (9.8.7c): Little Engine's
/// first subroutine ends the run; the encounter is over, so the second
/// subroutine never resolves and the Runner does not gain 5[credit].
#[test]
fn example_rule_resolve_subroutines_run_ends_1() {
    let mut vm = Vm::empty(42);
    tk::install_ice(&mut vm, tk::little_engine_like("LittleEngine-like"), ServerId::Hq, true);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    let _ = drive_to_action_window(&mut vm, Side::Runner);

    assert_eq!(
        vm.st.runner.credits, 0,
        "9.8.7c: the encounter ended; the 5-credit subroutine never resolved"
    );
    let subs_resolved = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::SubroutineResolved { .. }))
        .count();
    assert_eq!(subs_resolved, 1, "only the first subroutine resolved");
}

// ===========================================================================
// §9.9 — expected effects & replacements (wave 2b)
// ===========================================================================

/// example_rule_expected_effects_1 (9.9.2): "Gain 2[c] and draw 1 card." —
/// the expected effects are exactly that, and both occur.
#[test]
fn example_rule_expected_effects_1() {
    let mut vm = Vm::empty(50);
    tk::install_rig(&mut vm, tk::process_automation_like("ProcAuto-like"));
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    tk::take_labeled(&mut vm, Side::Runner, "process-automation", 100);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    assert_eq!(vm.st.runner.credits, 2);
    assert_eq!(vm.st.hand[&Side::Runner].len(), 1, "drew 1");
}

/// example_rule_expected_effects_2 (9.9.2): the same instruction while a
/// Lockdown-class "cannot draw" static is active — the expected effect is
/// only the 2 credits; no draw happens.
#[test]
fn example_rule_expected_effects_2() {
    let mut vm = Vm::empty(51);
    tk::install_rig(&mut vm, tk::process_automation_like("ProcAuto-like"));
    tk::install_root(&mut vm, tk::lockdown_like("Lockdown-like"), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    tk::take_labeled(&mut vm, Side::Runner, "process-automation", 100);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    assert_eq!(vm.st.runner.credits, 2, "the credits still happen");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 0, "9.9.2: the draw was never expected");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardDrawn { side: Side::Runner, .. })),
        "no draw occurred"
    );
}

/// example_rule_would_relevant_1 (9.9.3d): a Class-Act-class "the first time
/// each turn you would draw" interrupt is relevant to an instruction
/// expected to draw cards, even though it does not modify the draw.
#[test]
fn example_rule_would_relevant_1() {
    let mut vm = Vm::empty(52);
    tk::install_rig(&mut vm, tk::process_automation_like("ProcAuto-like"));
    tk::install_rig(&mut vm, tk::class_act_like("ClassAct-like"));
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let mut fired = false;
    let mut class_act_offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner && !fired => {
                if let Some(opt) = tk::option_labeled(options, "process-automation") {
                    fired = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "class-act") {
                    class_act_offered = true;
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
    assert!(
        class_act_offered,
        "9.9.3d: the would-draw trigger makes the ability relevant to the imminent draw"
    );
    assert_eq!(vm.st.runner.credits, 2 + 1, "its effect resolved too");
}

/// example_rule_trigger_conditional_ability_interrupt_1 (9.9.4c): once
/// Sacrificial Construct prevents Harbinger's trash, Harbinger's pending
/// interrupt is no longer relevant and cannot be triggered.
#[test]
fn example_rule_trigger_conditional_ability_interrupt_1() {
    let mut vm = Vm::empty(53);
    let harb = tk::install_rig(&mut vm, tk::harbinger_like("Harbinger-like"));
    tk::install_rig(&mut vm, tk::sac_con_like("SacCon-like", harb));
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Trasher", vec![harb]),
        ServerId::Remote(1),
        true,
    );
    vm.start_turn(Side::Runner);

    let mut corp_fired = false;
    let mut harbinger_offered_before = false;
    let mut used_sac = false;
    let mut harbinger_offered_after = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && !corp_fired => {
                if let Some(opt) = tk::option_labeled(options, "corp-trash") {
                    corp_fired = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner => {
                if !used_sac {
                    if tk::option_labeled(options, "harbinger").is_some() {
                        harbinger_offered_before = true;
                    }
                    if let Some(opt) = tk::option_labeled(options, "sac-con") {
                        used_sac = true;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
                }
                if used_sac && tk::option_labeled(options, "harbinger").is_some() {
                    harbinger_offered_after = true;
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
    assert!(used_sac);
    assert!(harbinger_offered_before, "relevant while its trash was expected");
    assert!(
        !harbinger_offered_after,
        "9.9.4c: still pending, but no longer relevant — cannot be triggered"
    );
    assert!(vm.st.objects[&harb].zone.is_installed(), "the trash was prevented");
}

/// example_rule_modified_values_retain_properties_1 (9.9.7e): unpreventable
/// 2 meat damage increased to 3 by a Cleaners-class static — ALL of it stays
/// unpreventable.
#[test]
fn example_rule_modified_values_retain_properties_1() {
    let mut vm = Vm::empty(54);
    tk::install_root(&mut vm, tk::flare_like("Flare-like"), ServerId::Remote(1), true);
    let cl = vm.new_object(tk::cleaners_static_like("Cleaners-like"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(cl);
    tk::install_rig(&mut vm, tk::biometric_like("Biometric-like", DamageKind::Meat));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let mut fired = false;
    let mut biometric_offered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        if let DecisionSpec::InterruptWindow { options, .. } = &spec {
            if tk::option_labeled(options, "biometric").is_some() {
                biometric_offered = true;
            }
        }
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && !fired => {
                if let Some(opt) = tk::option_labeled(options, "flare") {
                    fired = true;
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
    assert!(fired);
    assert!(
        !biometric_offered,
        "9.9.7e: the increased damage keeps the cannot-be-prevented property"
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "all 3 points landed (2 + 1)");
}

/// example_rule_replace_imminent_effects_1 (9.9.10): a Tori-class interrupt
/// replaces the imminent net damage with core damage; the replacement
/// applies immediately, and later relevance follows the NEW expected
/// effects (the net-damage preventer can no longer be used).
#[test]
fn example_rule_replace_imminent_effects_1() {
    let mut vm = Vm::empty(55);
    tk::install_root(&mut vm, tk::net_damage_button("Zapper", 1), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::tori_replace_like("Tori-like"), ServerId::Remote(2), true);
    vm.st.corp.credits = 2;
    tk::install_rig(&mut vm, tk::biometric_like("Biometric-like", DamageKind::Net));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut corp_fired = false;
    let mut used_tori = false;
    let mut net_preventer_offered_after = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && !corp_fired => {
                if let Some(opt) = tk::option_labeled(options, "do net damage") {
                    corp_fired = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::InterruptWindow { options, .. } => {
                if s == Side::Corp && !used_tori {
                    if let Some(opt) = tk::option_labeled(options, "tori-replace") {
                        used_tori = true;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
                }
                if used_tori
                    && s == Side::Runner
                    && tk::option_labeled(options, "biometric").is_some()
                {
                    net_preventer_offered_after = true;
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
    assert!(used_tori);
    assert!(
        !net_preventer_offered_after,
        "9.9.10: relevance follows the replaced (core) expected effects"
    );
    assert_eq!(vm.st.runner.core_damage, 1, "the damage resolved as core");
    assert_eq!(vm.max_hand_size(Side::Runner), 4);
    assert_eq!(vm.st.hand[&Side::Runner].len(), 4);
}

// ===========================================================================
// 9.12.5 — persistent (wave 2b)
// ===========================================================================

/// example_rule_persistent_applicability_1 (9.12.5d): AMAZE trashed during a
/// run persists; when the run ends its instance pends; a Doppelgänger-class
/// second run during that window CANNOT create new instances — the Runner
/// ends at exactly 2 tags.
#[test]
fn example_rule_persistent_applicability_1() {
    let mut vm = Vm::empty(56);
    let amaze = tk::install_root(
        &mut vm,
        tk::amaze_persistent_like("AMAZE-like"),
        ServerId::Remote(1),
        true,
    );
    tk::install_rig(&mut vm, tk::doppel_like("Doppel-like", ServerId::Remote(1)));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
        server: ServerId::Remote(1),
    }));

    let mut trashed_amaze = false;
    let mut ran_again = false;
    for _ in 0..500 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::MidAccessWindow { options } if !trashed_amaze => {
                if let Some(opt) = options
                    .iter()
                    .find(|o| matches!(o, WindowOption::BasicTrash { card, .. } if *card == amaze))
                    .cloned()
                {
                    trashed_amaze = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner && !ran_again => {
                if let Some(opt) = tk::option_labeled(options, "doppel") {
                    ran_again = true;
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
    assert!(trashed_amaze, "the persist began with the trash-during-access");
    assert!(ran_again, "a second run happened during the reaction window");
    assert_eq!(
        vm.st.runner.tags, 2,
        "9.12.5d: only the first run's instance resolved; the second run created none"
    );
    let amaze_resolutions = vm
        .resolution_log
        .iter()
        .filter(|l| l.starts_with("AMAZE-like"))
        .count();
    assert_eq!(amaze_resolutions, 1);
    let runs_ended = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::RunEnded { .. }))
        .count();
    assert_eq!(runs_ended, 2, "two runs actually ended");
}

// ===========================================================================
// §10.8 / §10.14 — traces and psi (wave 2c)
// ===========================================================================

/// example_rule_trace_conditional_abilities_1 (10.8.5): a Gemini-class
/// "Trace 3" with an "if successful" body AND a "when determined, if trace
/// strength ≥ 5" body. Corp spends 3 → strength 6 vs link 0: both pend
/// (2 net total). A second attempt with 0 spent → strength 3: successful
/// but < 5, so only 1 net.
#[test]
fn example_rule_trace_conditional_abilities_1() {
    let mut vm = Vm::empty(60);
    tk::install_root(&mut vm, tk::gemini_like("Gemini-like"), ServerId::Remote(1), true);
    vm.st.corp.credits = 3;
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut fired = 0;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && fired < 2 => {
                if let Some(opt) = tk::option_labeled(options, "gemini") {
                    fired += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::TraceSpend { corp_side: true, max, .. } => {
                // First attempt: spend 3 (strength 6); second: spend 0.
                let n = if fired == 1 { (*max).min(3) } else { 0 };
                vm.answer(DecisionAnswer::SpendCredits(n));
            }
            DecisionSpec::TraceSpend { corp_side: false, .. } => {
                vm.answer(DecisionAnswer::SpendCredits(0));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(fired, 2);
    let determinations: Vec<(bool, i64)> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::TraceDetermined { success, trace_strength, .. } => {
                Some((*success, *trace_strength))
            }
            _ => None,
        })
        .collect();
    assert_eq!(determinations, vec![(true, 6), (true, 3)]);
    // 2 net from the first (both bodies) + 1 net from the second.
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5 - 3);
    assert_eq!(vm.st.corp.credits, 0, "the 3 credits were openly spent");
}

/// example_rule_bid_possible_1 (10.14.3): with 0 pool credits and 1
/// spendable hosted credit, the Runner cannot bid 2 but CAN bid 1.
#[test]
fn example_rule_bid_possible_1() {
    let mut vm = Vm::empty(61);
    tk::install_root(&mut vm, tk::psi_button("Adrian-like"), ServerId::Remote(1), true);
    let fencer = tk::install_rig(&mut vm, tk::fencer_like("Fencer-like", 1));
    vm.st
        .objects
        .get_mut(&fencer)
        .unwrap()
        .counters
        .insert(CounterKind::Credit, 1);
    vm.st.runner.credits = 0;
    vm.st.corp.credits = 2;
    vm.start_turn(Side::Runner);

    let mut runner_legal = None;
    let mut fired = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp => {
                match tk::option_labeled(options, "psi") {
                    Some(opt) if !fired => {
                        fired = true;
                        vm.answer(DecisionAnswer::Take(opt));
                    }
                    _ => vm.answer(DecisionAnswer::Pass),
                }
            }
            DecisionSpec::PsiBid { legal } if s == Side::Corp => {
                vm.answer(DecisionAnswer::Bid(0));
                let _ = legal;
            }
            DecisionSpec::PsiBid { legal } if s == Side::Runner => {
                runner_legal = Some(legal.clone());
                vm.answer(DecisionAnswer::Bid(1));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(
        runner_legal,
        Some(vec![0, 1]),
        "10.14.3: cannot bid 2, can bid 1 via the hosted credit; 0 always legal"
    );
    assert_eq!(
        vm.st.objects[&fencer].counter(CounterKind::Credit),
        0,
        "the hosted credit paid the bid"
    );
    assert_eq!(vm.st.runner.tags, 1, "bids differed (0 vs 1): the differ branch ran");
}

/// example_rule_bid_possible_2 (10.14.3): an RSVP-class prohibition on
/// spending credits forces the Runner to bid 0.
#[test]
fn example_rule_bid_possible_2() {
    let mut vm = Vm::empty(62);
    tk::install_root(&mut vm, tk::psi_button("Psi-like"), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::rsvp_like("RSVP-like"), ServerId::Remote(2), true);
    vm.st.runner.credits = 5;
    vm.st.corp.credits = 2;
    vm.start_turn(Side::Runner);

    let mut runner_legal = None;
    let mut fired = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp => {
                match tk::option_labeled(options, "psi") {
                    Some(opt) if !fired => {
                        fired = true;
                        vm.answer(DecisionAnswer::Take(opt));
                    }
                    _ => vm.answer(DecisionAnswer::Pass),
                }
            }
            DecisionSpec::PsiBid { .. } if s == Side::Corp => {
                vm.answer(DecisionAnswer::Bid(0));
            }
            DecisionSpec::PsiBid { legal } if s == Side::Runner => {
                runner_legal = Some(legal.clone());
                vm.answer(DecisionAnswer::Bid(0));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(
        runner_legal,
        Some(vec![0]),
        "10.14.3: unable to spend, the Runner must bid 0"
    );
    assert_eq!(vm.st.runner.credits, 5, "nothing was spent");
    assert_eq!(vm.st.corp.credits, 3, "bids matched at 0: the match branch ran (+1)");
}

// ===========================================================================
// §9.8.3 / §7.4 — subroutine origins and candidates (wave 2d)
// ===========================================================================

/// example_rule_subroutine_origin_static_after_1 (9.8.3d): Ashigaru with 3
/// cards in HQ has 3 subs; the Runner breaks all 3; the Corp draws a 4th
/// card mid-encounter — the new subroutine is added AFTER the previous 3,
/// arrives unbroken, and the Runner can break it.
#[test]
fn example_rule_subroutine_origin_static_after_1() {
    let mut vm = Vm::empty(70);
    let ash = tk::install_ice(&mut vm, tk::ashigaru_like("Ashigaru-like"), ServerId::Hq, true);
    tk::install_root(&mut vm, tk::panic_button_like("Panic-like"), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::break_button("Breaker"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut breaks = 0;
    let mut corp_drew = false;
    let mut subs_after_draw = 0;
    for _ in 0..500 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if vm.st.encounter.is_some() => {
                if s == Side::Runner {
                    if breaks < 3 || (corp_drew && breaks < 4) {
                        if let Some(opt) = tk::option_labeled(options, "break") {
                            breaks += 1;
                            vm.answer(DecisionAnswer::Take(opt));
                            continue;
                        }
                    }
                    vm.answer(DecisionAnswer::Pass);
                } else {
                    // Corp: after the 3 breaks, draw with the panic button.
                    if breaks >= 3 && !corp_drew {
                        if let Some(opt) = tk::option_labeled(options, "panic-button") {
                            corp_drew = true;
                            vm.answer(DecisionAnswer::Take(opt));
                            subs_after_draw = 0; // measured next priority
                            continue;
                        }
                    }
                    vm.answer(DecisionAnswer::Pass);
                }
                if corp_drew && subs_after_draw == 0 {
                    subs_after_draw = vm.current_subs(ash).len();
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(corp_drew);
    assert_eq!(breaks, 4, "the 4th (new, unbroken) subroutine could be broken too");
    assert_eq!(subs_after_draw, 4, "9.8.3d: the new sub was added after the previous 3");
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        0,
        "everything was broken; nothing resolved"
    );
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })));
}

/// example_rule_subroutine_origin_static_after_2 (9.8.3d): with 3 subs, the
/// Runner breaks the FIRST, then forces the Corp to discard 2 — Ashigaru
/// loses its LAST 2 subroutines, leaving exactly the already-broken one.
#[test]
fn example_rule_subroutine_origin_static_after_2() {
    let mut vm = Vm::empty(71);
    let ash = tk::install_ice(&mut vm, tk::ashigaru_like("Ashigaru-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::break_button("Breaker"));
    tk::install_rig(&mut vm, tk::utopia_button("Utopia-like"));
    tk::fill_hand(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut broke_first = false;
    let mut used_utopia = false;
    for _ in 0..500 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. }
                if s == Side::Runner && vm.st.encounter.is_some() =>
            {
                if !broke_first {
                    if let Some(opt) = tk::option_labeled(options, "break") {
                        broke_first = true;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
                }
                if broke_first && !used_utopia {
                    if let Some(opt) = tk::option_labeled(options, "utopia") {
                        used_utopia = true;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
                }
                if used_utopia {
                    // 9.8.3d: lost last-first — only the broken sub remains.
                    assert_eq!(vm.current_subs(ash).len(), 1);
                }
                vm.answer(DecisionAnswer::Pass);
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(broke_first && used_utopia);
    assert_eq!(vm.st.hand[&Side::Corp].len(), 1);
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        0,
        "the surviving subroutine was the broken one; nothing resolved"
    );
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })));
}

/// example_rule_subroutine_origin_external_after_1 (9.8.3e): an ETR sub
/// granted by an older external effect (Marker class) orders BEFORE the
/// core-damage subs a Brainstorm-class ability grants at encounter start —
/// so the ETR resolves first and the cores never do.
#[test]
fn example_rule_subroutine_origin_external_after_1() {
    let mut vm = Vm::empty(72);
    let brain = tk::install_ice(&mut vm, tk::brainstorm_like("Brainstorm-like"), ServerId::Hq, true);
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));
    // The Marker-class lingering exists before the encounter begins.
    tk::grant_external_sub(
        &mut vm,
        brain,
        jinteki_cr::ability::AbilityDef::subroutine(vec![
            jinteki_cr::instr::Instruction::EndTheRun,
        ])
        .labeled("[sub] ETR (marker)"),
        false,
        false, // turn-bound: the lingering exists before the run begins
    );
    let _ = drive_to_action_window(&mut vm, Side::Runner);

    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::SubroutineResolved { .. })).count(),
        1,
        "only the first (oldest-granted) subroutine resolved"
    );
    assert_eq!(vm.st.runner.core_damage, 0, "the newer core subs never resolved");
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::RunDeclaredUnsuccessful { .. })));
}

/// example_rule_prohibiting_access_1 (7.4.2): a successful Ash-class trace
/// prohibits accessing anything else — Ash is the only candidate, then no
/// candidates remain and the breach ends; the agenda beside it is safe.
#[test]
fn example_rule_prohibiting_access_1() {
    let mut vm = Vm::empty(73);
    let ash = tk::install_root(&mut vm, tk::ash_like("Ash-like"), ServerId::Remote(1), true);
    let agenda = tk::install_root(
        &mut vm,
        tk::vanilla_agenda("Vitruvius-like", 3, 2),
        ServerId::Remote(1),
        false,
    );
    vm.st.corp.credits = 0;
    vm.st.runner.credits = 1; // cannot pay Ash's 2[c] trash cost
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
        server: ServerId::Remote(1),
    }));
    let _ = drive_to_action_window(&mut vm, Side::Runner);

    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == ash)));
    assert!(
        !vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == agenda)),
        "7.4.2: nothing but Ash could be accessed"
    );
    assert_eq!(vm.st.objects[&agenda].zone, Zone::Root(ServerId::Remote(1)));
    assert!(vm
        .changes
        .log
        .iter()
        .any(|c| matches!(c, GameChange::TraceDetermined { success: true, .. })));
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
