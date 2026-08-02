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
    // Wave 2e: once-per-turn (9.3.6g), delayed durations (9.6.13),
    // restriction sets (10.3.1e).
    "example_rule_once_per_turn_flag_1",
    "example_step_checkpoint_card_restrictions_1",
    "example_step_checkpoint_card_restrictions_2",
    "example_rule_delayed_conditional_ability_specified_duration_1",
    "example_rule_delayed_conditional_ability_relevant_once_1",
    "example_rule_delayed_run_ends_condition_outside_run_1",
    // Wave 2f: quantities & sets (9.12.2), must-choices (9.12.3).
    "example_rule_act_on_multiple_cards_1",
    "example_rule_calculated_quantity_1",
    "example_rule_calculated_quantity_2",
    "example_rule_mandatory_choice_1",
    "example_rule_mandatory_choice_effects_can_be_modified_1",
    // Wave 3a: install instructions (§8.5), 9.6.5b, 10.3.1j declaration.
    "example_rule_install_one_at_a_time_1",
    "example_rule_no_reveal_for_default_install_1",
    "example_rule_no_reveal_for_default_install_2",
    "example_rule_no_reveal_for_server_limitation_1",
    "example_rule_reveal_for_ability_limitations_1",
    "example_rule_reveal_for_install_and_rez_1",
    "example_rule_reveal_for_install_and_rez_2",
    "example_rule_install_to_invalid_destination_1",
    "example_rule_condition_only_met_while_active_1",
    "example_rule_condition_only_met_while_active_2",
    "example_step_checkpoint_card_entering_root_during_breach_1",
    // Wave 3b: play instructions (§8.6), 9.6.5c/d.
    "example_rule_playing_one_at_a_time_1",
    "example_rule_playing_lingering_effects_1",
    "example_rule_play_no_trash_left_play_area_1",
    "example_step_checkpoint_duration_abilities_2",
    "example_rule_condition_requirements_part_of_condition_1",
    "example_rule_condition_requirements_part_of_effect_1",
    // Wave 3c: replacement-effect ordering (9.9.11a).
    "example_rule_replacement_effect_must_have_something_to_replace_1",
    "example_rule_replacement_effect_must_have_something_to_replace_2",
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
// Wave 2e — 9.3.6g / 9.6.13 / 10.3.1e
// ===========================================================================

/// example_rule_once_per_turn_flag_1 (9.3.6g): declining the optional part
/// of a Zahya-class once-per-turn ability does not "use" it; a later run
/// the same turn can still gain the credit, and after actually using it,
/// no further instance pends.
#[test]
fn example_rule_once_per_turn_flag_1() {
    let mut vm = Vm::empty(80);
    tk::install_rig(&mut vm, tk::zahya_like("Zahya-like"));
    vm.start_turn(Side::Runner);

    let mut offers = 0;
    let mut runs = 0;
    for _ in 0..600 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::TakeAction { .. } if runs < 3 => {
                runs += 1;
                vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
                    server: ServerId::Archives,
                }));
            }
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "zahya") {
                    offers += 1;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::OptionalEffect { .. } => {
                // Run 1: decline (not "used"); run 2: accept.
                vm.answer(DecisionAnswer::ResolveOptional(offers >= 2));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(runs, 3);
    assert_eq!(
        offers, 2,
        "9.3.6g: pended after runs 1 and 2; after actually using it, never again"
    );
    assert_eq!(vm.st.runner.credits, 1, "gained exactly once");
}

/// example_step_checkpoint_card_restrictions_1 (10.3.1e): rezzing a
/// Tithonium-class ice makes the hosted Chisel-class program illegal — it
/// is trashed at the next checkpoint.
#[test]
fn example_step_checkpoint_card_restrictions_1() {
    let mut vm = Vm::empty(81);
    let tith = tk::install_ice(&mut vm, tk::tithonium_like("Tithonium-like", 3), ServerId::Hq, false);
    let chisel = tk::install_rig(&mut vm, tk::chisel_like("Chisel-like"));
    tk::host_on(&mut vm, chisel, tith);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut rezzed = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { classes, options } if s == Side::Corp && !rezzed => {
                if classes.rez_approached_ice {
                    if let Some(opt) = options
                        .iter()
                        .find(|o| matches!(o, WindowOption::RezApproachedIce { .. }))
                        .cloned()
                    {
                        rezzed = true;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
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
    assert!(rezzed);
    assert_eq!(
        vm.st.objects[&chisel].zone,
        Zone::Discard(Side::Runner),
        "10.3.1e: hosted in an illegal location — trashed at the checkpoint"
    );
}

/// example_step_checkpoint_card_restrictions_2 (10.3.1e): Bad Times drops
/// the memory limit by 2 with 4[mu] installed. The appropriate sets are
/// exactly {the 2[mu] program} and {the two 1[mu] programs}; no set may
/// contain the 0[mu] program.
#[test]
fn example_step_checkpoint_card_restrictions_2() {
    let mut vm = Vm::empty(82);
    let p1 = tk::install_rig(&mut vm, tk::program_mu("One-A", 1));
    let p2 = tk::install_rig(&mut vm, tk::program_mu("One-B", 1));
    let p3 = tk::install_rig(&mut vm, tk::program_mu("Two", 2));
    let p0 = tk::install_rig(&mut vm, tk::program_mu("Zero", 0));
    tk::install_root(&mut vm, tk::bad_times_button("BadTimes-like"), ServerId::Remote(1), true);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let mut fired = false;
    let mut offered_sets: Option<Vec<Vec<jinteki_cr::ObjectId>>> = None;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Corp && !fired => {
                if let Some(opt) = tk::option_labeled(options, "bad-times") {
                    fired = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    vm.answer(DecisionAnswer::Pass);
                }
            }
            DecisionSpec::MinimalSet { sets } => {
                offered_sets = Some(sets.clone());
                // Choose the {2mu} singleton.
                let idx = sets.iter().position(|s| s == &vec![p3]).expect("2mu set offered");
                vm.answer(DecisionAnswer::ChooseSet(idx));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    let sets = offered_sets.expect("a minimal-set choice was demanded");
    assert_eq!(sets.len(), 2, "exactly two appropriate sets");
    assert!(sets.contains(&vec![p3]), "the single 2[mu] program");
    assert!(
        sets.iter().any(|s| {
            s.len() == 2 && s.contains(&p1) && s.contains(&p2)
        }),
        "the two 1[mu] programs together"
    );
    assert!(
        sets.iter().all(|s| !s.contains(&p0)),
        "10.3.1e: the 0[mu] program can never be part of a minimal set"
    );
    assert_eq!(vm.st.objects[&p3].zone, Zone::Discard(Side::Runner));
    assert!(vm.st.objects[&p1].zone.is_installed());
    assert!(vm.st.objects[&p2].zone.is_installed());
}

/// example_rule_delayed_conditional_ability_specified_duration_1 (9.6.13b):
/// a delayed conditional with an explicit "this turn" duration triggers
/// EVERY time its condition is met, expiring only at end of turn.
#[test]
fn example_rule_delayed_conditional_ability_specified_duration_1() {
    let mut vm = Vm::empty(83);
    tk::install_rig(&mut vm, tk::groove_button("Groove-like"));
    tk::install_rig(&mut vm, tk::take_tag_button("TagMe"));
    vm.start_turn(Side::Runner);

    let mut armed = false;
    let mut tags_taken = 0;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner => {
                if !armed {
                    if let Some(opt) = tk::option_labeled(options, "groove:") {
                        armed = true;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
                }
                if armed && tags_taken < 2 {
                    if let Some(opt) = tk::option_labeled(options, "take 1 tag") {
                        tags_taken += 1;
                        vm.answer(DecisionAnswer::Take(opt));
                        continue;
                    }
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
    assert_eq!(tags_taken, 2);
    assert_eq!(
        vm.st.runner.credits, 2,
        "9.6.13b: the delayed conditional resolved on BOTH occurrences"
    );
}

/// example_rule_delayed_conditional_ability_relevant_once_1 (9.6.13c): a
/// delayed conditional with no stated duration resolves once — at turn end
/// — and is then no longer maintained.
#[test]
fn example_rule_delayed_conditional_ability_relevant_once_1() {
    let mut vm = Vm::empty(84);
    tk::install_rig(&mut vm, tk::joshua_button("Joshua-like"));
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    tk::take_labeled(&mut vm, Side::Runner, "joshua:", 100);
    // Drain the runner turn (spend all clicks on credits) into the corp
    // turn, then let the next runner turn end too.
    let mut runner_turn_ends = 0;
    for _ in 0..800 {
        match vm.step() {
            Yield::Decision(_, spec) => match &spec {
                DecisionSpec::TakeAction { .. } => {
                    vm.answer(DecisionAnswer::Action(ActionOption::BasicCredit));
                }
                other => {
                    let a = tk::default_answer(other);
                    vm.answer(a);
                }
            },
            Yield::Progressed => continue,
            Yield::GameOver(r) => panic!("game over {r:?}"),
        }
        runner_turn_ends = vm
            .changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::TurnEnded { side: Side::Runner }))
            .count();
        if runner_turn_ends >= 2 {
            break;
        }
    }
    assert!(runner_turn_ends >= 2, "two runner turns ended");
    let gains = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CreditsGained { side: Side::Runner, amount: 1 }))
        .count();
    // 4 clicks/turn × 2 turns of basic credits = 8 one-credit gains, plus
    // exactly ONE from the delayed conditional (9.6.13c).
    assert_eq!(gains, 8 + 1, "the turn-end trigger fired exactly once");
}

/// example_rule_delayed_run_ends_condition_outside_run_1 (9.6.13d): arming
/// a "when this run ends" delayed conditional with NO run in progress
/// creates nothing — a later run does not trash the Mayfly-class source.
#[test]
fn example_rule_delayed_run_ends_condition_outside_run_1() {
    let mut vm = Vm::empty(85);
    let mayfly = tk::install_rig(&mut vm, tk::mayfly_button("Mayfly-like"));
    vm.start_turn(Side::Runner);

    // Use the button OUTSIDE any run.
    tk::take_labeled(&mut vm, Side::Runner, "mayfly:", 100);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    assert!(
        vm.lingering.is_empty(),
        "9.6.13d: the lingering effect was never created"
    );
    // A later run ends — nothing fires.
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Archives }));
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    assert!(
        vm.st.objects[&mayfly].zone.is_installed(),
        "Mayfly was not trashed by the later run"
    );
}

// ===========================================================================
// Wave 2f — 9.12.2 quantities / 9.12.3 must-choices
// ===========================================================================

/// example_rule_act_on_multiple_cards_1 (9.12.2a): trashing a Warroid-class
/// and a Hostile-Infrastructure-class card simultaneously — HI's
/// per-occurrence condition pends twice; Warroid's set-condition pends once.
#[test]
fn example_rule_act_on_multiple_cards_1() {
    let mut vm = Vm::empty(90);
    let hi = tk::install_root(&mut vm, tk::hostile_infra_like("HI-like"), ServerId::Remote(1), true);
    let w = tk::install_root(&mut vm, tk::warroid_like("Warroid-like"), ServerId::Remote(2), true);
    tk::install_rig(&mut vm, tk::trash_set_button("Singularity-like", vec![hi, w]));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut corp_first_offer: Option<Vec<WindowOption>> = None;
    let mut fired = false;
    for _ in 0..400 {
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
    let hi_count = opts.iter().filter(|o| matches!(o, WindowOption::TriggerInstance { label, .. } if label.contains("hostile-infra"))).count();
    let w_count = opts.iter().filter(|o| matches!(o, WindowOption::TriggerInstance { label, .. } if label.contains("warroid"))).count();
    assert_eq!(hi_count, 2, "9.12.2a: HI sees both trashed cards");
    assert_eq!(w_count, 1, "9.12.2a: Warroid sees one event");
}

/// example_rule_calculated_quantity_1 (9.12.2b): "draw 3 cards" is ONE
/// instance of drawing 3 — a Class-Act-class would-draw interrupt gets one
/// relevant imminence.
#[test]
fn example_rule_calculated_quantity_1() {
    let mut vm = Vm::empty(91);
    tk::install_rig(&mut vm, tk::ritual_button("Ritual-like"));
    tk::install_rig(&mut vm, tk::class_act_like("ClassAct-like"));
    tk::fill_deck(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let mut class_act_offers = 0;
    let mut fired = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::PaidWindow { options, .. } if s == Side::Runner => {
                match tk::option_labeled(options, "ritual") {
                    Some(opt) if !fired => {
                        fired = true;
                        vm.answer(DecisionAnswer::Take(opt));
                    }
                    _ => vm.answer(DecisionAnswer::Pass),
                }
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner => {
                if let Some(opt) = tk::option_labeled(options, "class-act") {
                    class_act_offers += 1;
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
    assert_eq!(class_act_offers, 1, "9.12.2b: one instance of drawing 3");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 3, "all three cards drawn together");
}

/// example_rule_calculated_quantity_2 (9.12.2b): Urtica-class "2 net plus 1
/// per advancement counter" with 3 counters = a single 5-damage instance;
/// a prevent-2 interrupt applies once, leaving 3.
#[test]
fn example_rule_calculated_quantity_2() {
    let mut vm = Vm::empty(92);
    let urtica = vm.new_object(tk::urtica_like("Urtica-like"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(urtica);
    vm.st.objects.get_mut(&urtica).unwrap().counters.insert(CounterKind::Advancement, 3);
    tk::install_rig(&mut vm, tk::biometric_like("Biometric-like", DamageKind::Net));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Rnd }));

    let mut prevented = false;
    for _ in 0..300 {
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
    let dmg: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::DamageSuffered { kind: DamageKind::Net, amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(dmg, vec![3], "one aggregated instance: 2+3 = 5, minus 2 prevented");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2);
}

/// example_rule_mandatory_choice_1 (9.12.3c): with 3[c] and no installed
/// cards, the first "pay 2 or trash an installed card" sub forces the pay;
/// the second, with 1[c] left and still nothing installed, does NOTHING.
#[test]
fn example_rule_mandatory_choice_1() {
    let mut vm = Vm::empty(93);
    tk::install_ice(&mut vm, tk::fairchild_like("Fairchild-like"), ServerId::Hq, true);
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);
    // No runner installed cards: empty the rig of incidental installs.
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut choice_decisions = 0;
    for _ in 0..300 {
        let (_, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ChooseOption { .. } => {
                choice_decisions += 1;
                vm.answer(DecisionAnswer::Option(0));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(
        choice_decisions, 0,
        "9.12.3c: with exactly one resolvable option (or none) no choice is offered"
    );
    assert_eq!(
        vm.st.runner.credits,
        1,
        "first sub forced the 2[c] payment; second sub could do nothing"
    );
}

/// example_rule_mandatory_choice_effects_can_be_modified_1 (9.12.3d): the
/// Runner chooses "take 1 tag" on a Data-Raven-class encounter, then avoids
/// the tag with a Decoy-class interrupt — the run does NOT end.
#[test]
fn example_rule_mandatory_choice_effects_can_be_modified_1() {
    let mut vm = Vm::empty(94);
    tk::install_ice(&mut vm, tk::data_raven_like("DataRaven-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::decoy_like("Decoy-like"));
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut chose_tag = false;
    let mut used_decoy = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ChooseOption { options } => {
                let i = options.iter().position(|l| l.contains("tag")).unwrap();
                chose_tag = true;
                vm.answer(DecisionAnswer::Option(i));
            }
            DecisionSpec::InterruptWindow { options, .. } if s == Side::Runner && !used_decoy => {
                if let Some(opt) = tk::option_labeled(options, "decoy") {
                    used_decoy = true;
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
    assert!(chose_tag && used_decoy);
    assert_eq!(vm.st.runner.tags, 0, "the tag was avoided AFTER the choice");
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "9.12.3d: avoiding the chosen tag does not resurrect the end-the-run option"
    );
}

// ===========================================================================
// §8.5 — installing (W3a): the 8.5.16 steps, reveal rules, 10.3.1j
// ===========================================================================

/// example_rule_install_one_at_a_time_1 (8.5.5): a Mass-Install-class effect
/// installs three programs ONE AT A TIME, each a separate instruction. The
/// Runner installs a Dhegdheer-class host first, then hosts the second
/// program on it to reduce that program's install cost by 1.
#[test]
fn example_rule_install_one_at_a_time_1() {
    let mut vm = Vm::empty(311);
    let dheg = vm.new_object(tk::dhegdheer_like("Dhegdheer-like", 2), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(dheg);
    let pa = vm.new_object(tk::program_cost("Prog-A", 3), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(pa);
    let pb = vm.new_object(tk::program_cost("Prog-B", 3), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(pb);
    tk::install_rig(&mut vm, tk::mass_install_button("MassInstall-like", 3));
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    tk::take_labeled(&mut vm, Side::Runner, "mass-install", 100);
    // Install order: Dhegdheer (to the rig), Prog-A (hosted on Dhegdheer),
    // Prog-B (to the rig).
    let mut picks = 0usize;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ChooseTargets { candidates, .. } if s == Side::Runner => {
                if candidates.contains(&dheg) && picks == 0 {
                    picks += 1;
                    vm.answer(DecisionAnswer::Targets(vec![dheg]));
                } else if candidates.contains(&pa) {
                    picks += 1;
                    vm.answer(DecisionAnswer::Targets(vec![pa]));
                } else if candidates == &vec![dheg] {
                    // Host choice for Prog-A: host it on Dhegdheer.
                    vm.answer(DecisionAnswer::Targets(vec![dheg]));
                } else if candidates.contains(&pb) {
                    picks += 1;
                    vm.answer(DecisionAnswer::Targets(vec![pb]));
                } else {
                    // Host choice for Prog-B: Dhegdheer is full — but if
                    // offered, decline to the rig.
                    vm.answer(DecisionAnswer::Targets(vec![]));
                }
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(picks, 3, "three separate one-at-a-time picks (8.5.5)");
    assert_eq!(vm.st.objects[&dheg].zone, Zone::Rig);
    assert_eq!(vm.st.objects[&pa].zone, Zone::Rig);
    assert_eq!(vm.st.objects[&pa].host, Some(dheg), "Prog-A hosted on Dhegdheer");
    assert_eq!(vm.st.objects[&pb].zone, Zone::Rig);
    // 10 - 2 (Dhegdheer) - 2 (Prog-A at 3-1 hosted discount) - 3 (Prog-B).
    assert_eq!(vm.st.runner.credits, 3, "hosting reduced Prog-A's install cost");
}

/// example_rule_no_reveal_for_default_install_1 (8.5.13a): the Corp installs
/// an asset into a root already holding a facedown asset. The old asset is
/// trashed (8.5.6a must-trash), facedown into Archives (8.5.7), and NO card
/// is revealed to verify card types.
#[test]
fn example_rule_no_reveal_for_default_install_1() {
    let mut vm = Vm::empty(312);
    let old = tk::install_root(&mut vm, tk::vanilla_asset("Old-Asset", 0, 3), ServerId::Remote(1), false);
    let newc = vm.new_object(tk::vanilla_asset("New-Asset", 0, 3), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(newc);
    tk::install_root(
        &mut vm,
        tk::corp_install_button("Install-Button", newc, jinteki_cr::instr::InstallDest::Root(ServerId::Remote(1))),
        ServerId::Remote(2),
        true,
    );
        tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);
    tk::take_labeled(&mut vm, Side::Corp, "corp-install", 100);
    let _ = drive_to_action_window(&mut vm, Side::Corp);

    assert_eq!(vm.st.objects[&newc].zone, Zone::Root(ServerId::Remote(1)));
    assert!(!vm.st.objects[&newc].faceup, "Corp cards install facedown (8.5.2)");
    assert!(!vm.st.objects[&newc].staged);
    assert_eq!(vm.st.objects[&old].zone, Zone::Discard(Side::Corp), "8.5.6a must-trash");
    assert!(!vm.st.objects[&old].faceup, "8.5.7: trashed with its facedown status");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { .. })),
        "8.5.13a: no reveal to verify card types"
    );
}

/// example_rule_no_reveal_for_default_install_2 (8.5.13a): a Brân-class
/// subroutine installs ice from HQ directly inward. The installed card is
/// not revealed to verify that it is a piece of ice.
#[test]
fn example_rule_no_reveal_for_default_install_2() {
    let mut vm = Vm::empty(313);
    let hq_ice = vm.new_object(tk::vanilla_ice("HQ-Ice", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hq_ice);
    let bran = tk::install_ice(&mut vm, tk::bran_like("Bran-like", hq_ice), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 3), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Remote(1) }));
    // Let the encounter resolve the install subroutine, then jack out.
    for _ in 0..300 {
        let (_, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::JackOut => {
                vm.answer(DecisionAnswer::JackOut(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(vm.st.objects[&hq_ice].zone, Zone::Ice(ServerId::Remote(1)));
    assert_eq!(
        vm.st.ice[&ServerId::Remote(1)],
        vec![hq_ice, bran],
        "installed directly inward of Brân (innermost-first order)"
    );
    assert!(!vm.st.objects[&hq_ice].faceup, "installed unrezzed");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { .. })),
        "8.5.13a: no reveal to verify the card is ice"
    );
}

/// example_rule_no_reveal_for_server_limitation_1 (8.5.13b): installing a
/// card into a root with a rezzed region does not reveal the new card to
/// verify it is not a region.
#[test]
fn example_rule_no_reveal_for_server_limitation_1() {
    let mut vm = Vm::empty(314);
    let region = tk::install_root(&mut vm, tk::region_upgrade("Old-Region", 2), ServerId::Remote(1), true);
    let newu = vm.new_object(tk::vanilla_upgrade("New-Upgrade", 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(newu);
    tk::install_root(
        &mut vm,
        tk::corp_install_button("Install-Button", newu, jinteki_cr::instr::InstallDest::Root(ServerId::Remote(1))),
        ServerId::Remote(2),
        true,
    );
        tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);
    tk::take_labeled(&mut vm, Side::Corp, "corp-install", 100);
    let _ = drive_to_action_window(&mut vm, Side::Corp);

    assert_eq!(vm.st.objects[&newu].zone, Zone::Root(ServerId::Remote(1)));
    assert_eq!(
        vm.st.objects[&region].zone,
        Zone::Root(ServerId::Remote(1)),
        "a non-region upgrade does not force the region out"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { .. })),
        "8.5.13b: no reveal to verify server limitations"
    );
}

/// example_rule_reveal_for_ability_limitations_1 (8.5.13c): an Ob-class
/// ability installs-and-rezzes a card from R&D subject to a printed-rez-cost
/// requirement; the Corp declines the additional rez cost, so the card stays
/// facedown and MUST be revealed to verify the installation.
#[test]
fn example_rule_reveal_for_ability_limitations_1() {
    let mut vm = Vm::empty(315);
    let mut archer = tk::vanilla_ice("Archer-like", 4, 6);
    archer.additional_rez_cost = Some(jinteki_cr::ability::Cost::credits(1));
    let archer = vm.new_object(archer, Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(archer);
    tk::install_root(
        &mut vm,
        tk::corp_install_rez_button(
            "Ob-Button",
            archer,
            jinteki_cr::instr::InstallDest::Protecting(ServerId::Remote(1)),
            true,
            Some(jinteki_cr::instr::RevealCheck::PrintedRezCostAtMost(4)),
        ),
        ServerId::Remote(1),
        true,
    );
    vm.st.corp.credits = 5;
        tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);
    tk::take_labeled(&mut vm, Side::Corp, "corp-install-rez", 100);
    let mut declined = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::NestedCost { .. } if s == Side::Corp => {
                declined = true;
                vm.answer(DecisionAnswer::PayNestedCost(false));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(declined, "the additional rez cost was offered and declined (1.16.4c)");
    assert_eq!(vm.st.objects[&archer].zone, Zone::Ice(ServerId::Remote(1)));
    assert!(!vm.st.objects[&archer].faceup, "not rezzed after declining");
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::CardRevealed { obj } if *obj == archer)).count(),
        1,
        "8.5.13c/d: the hidden-provenance card is revealed exactly once"
    );
}

/// example_rule_reveal_for_install_and_rez_1 (8.5.13d): a Trust-Operation
/// class effect installs an agenda with "install and rez". Agendas cannot be
/// rezzed, so the Corp must reveal the installed agenda.
#[test]
fn example_rule_reveal_for_install_and_rez_1() {
    let mut vm = Vm::empty(316);
    let agenda = vm.new_object(tk::vanilla_agenda("Buried-Plans", 3, 1), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::install_root(
        &mut vm,
        tk::corp_install_rez_button(
            "TrustOp-Button",
            agenda,
            jinteki_cr::instr::InstallDest::NewRemoteRoot,
            false,
            None,
        ),
        ServerId::Remote(1),
        true,
    );
        tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);
    tk::take_labeled(&mut vm, Side::Corp, "corp-install-rez", 100);
    let _ = drive_to_action_window(&mut vm, Side::Corp);

    assert!(matches!(vm.st.objects[&agenda].zone, Zone::Root(ServerId::Remote(_))));
    assert!(!vm.st.objects[&agenda].faceup, "agendas cannot be rezzed (8.1.2c)");
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::CardRevealed { obj } if *obj == agenda)).count(),
        1,
        "8.5.13d: the unrezzable install-and-rez target is revealed"
    );
}

/// example_rule_reveal_for_install_and_rez_2 (8.5.13d): an Ad-Blitz-class
/// "install and rez, if able" effect cannot choose cards the Corp is unable
/// to rez — the agenda in hand is never a candidate, and nothing is
/// revealed.
#[test]
fn example_rule_reveal_for_install_and_rez_2() {
    let mut vm = Vm::empty(317);
    let ice = vm.new_object(tk::vanilla_ice("Blitz-Ice", 3, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ice);
    let agenda = vm.new_object(tk::vanilla_agenda("Hand-Agenda", 3, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(agenda);
    tk::install_root(
        &mut vm,
        tk::ad_blitz_button("AdBlitz-Button", 2, ServerId::Remote(1)),
        ServerId::Remote(1),
        true,
    );
        tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);
    tk::take_labeled(&mut vm, Side::Corp, "ad-blitz", 100);
    let mut saw_choice = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ChooseTargets { candidates, .. } if s == Side::Corp => {
                saw_choice = true;
                assert!(
                    !candidates.contains(&agenda),
                    "8.5.13d 'if able': unrezzable cards cannot be chosen"
                );
                assert!(candidates.contains(&ice));
                vm.answer(DecisionAnswer::Targets(vec![ice]));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(saw_choice);
    assert_eq!(vm.st.objects[&ice].zone, Zone::Ice(ServerId::Remote(1)));
    assert!(vm.st.objects[&ice].faceup, "installed and rezzed");
    assert_eq!(vm.st.objects[&agenda].zone, Zone::Hand(Side::Corp));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardRevealed { .. })),
        "nothing to reveal when every choice was rezzable"
    );
}

/// example_rule_install_to_invalid_destination_1 (8.5.14): resolving a
/// Brân-class subroutine on a copy in Archives — "directly inward" cannot be
/// evaluated for ice that is not protecting a server, so the install has no
/// effect.
#[test]
fn example_rule_install_to_invalid_destination_1() {
    let mut vm = Vm::empty(318);
    let hq_ice = vm.new_object(tk::vanilla_ice("HQ-Ice", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hq_ice);
    let bran = vm.new_object(tk::bran_like("Bran-Archives", hq_ice), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(bran);
    vm.st.corp.credits = 5;
    // Nanisivik-class driver: resolve Brân's subroutine while it sits in
    // Archives.
        tk::fill_deck(&mut vm, Side::Corp, 5);
vm.push_ability_frame(
        jinteki_cr::frames::ResolutionKind::Subroutine,
        jinteki_cr::ability::AbilityRef { obj: bran, index: 0 },
        Side::Corp,
        vec![jinteki_cr::instr::Instruction::InstallCard {
            card: jinteki_cr::instr::TargetSpec::Objects(vec![hq_ice]),
            dest: jinteki_cr::instr::InstallDest::InwardFromSource,
            and_rez: false,
            ignore_costs: true,
            reveal_check: None,
        }],
        None,
        Some(0),
    );
    let _ = tk::until_decision(&mut vm);

    assert_eq!(
        vm.st.objects[&hq_ice].zone,
        Zone::Hand(Side::Corp),
        "8.5.14: no installation takes place"
    );
    assert_eq!(vm.st.corp.credits, 5, "no cost was paid");
}

// ===========================================================================
// §9.6.5b — conditions only met while active (rides on §8.5)
// ===========================================================================

/// example_rule_condition_only_met_while_active_1 (9.6.5b): while resolving
/// "turn begins" abilities, the Corp trashes a Reaper-class card to install
/// and rez a Nico-Campaign-class asset. Nico's own "turn begins" condition
/// has already been processed — the Corp cannot take its credit this turn.
#[test]
fn example_rule_condition_only_met_while_active_1() {
    let mut vm = Vm::empty(319);
    let nico = vm.new_object(tk::nico_like("Nico-like", 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(nico);
    tk::install_root(&mut vm, tk::reaper_like("Reaper-like", nico), ServerId::Remote(1), true);
    vm.st.corp.credits = 10;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Corp);

    let mut fired = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Corp && !fired => {
                if let Some(opt) = tk::option_labeled(options, "reaper") {
                    fired = true;
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
    assert!(fired, "the Reaper-class turn-begins ability was triggered");
    assert!(matches!(vm.st.objects[&nico].zone, Zone::Root(ServerId::Remote(_))));
    assert!(vm.st.objects[&nico].faceup, "installed and rezzed mid-window");
    // 10 - 2 (Nico's rez cost), and NO +1 from Nico's ability: its trigger
    // condition was already processed when it became active (9.6.5b).
    assert_eq!(vm.st.corp.credits, 8, "Nico's turn-begins ability cannot fire this turn");
}

/// example_rule_condition_only_met_while_active_2 (9.6.5b): an ADT-class
/// effect installs and rezzes THG "ignoring all costs". The rez-cost step
/// still happens (1.16.5c) and is still followed by a checkpoint (1.16.3a);
/// that checkpoint processes the CardInstalled change while THG is not yet
/// active, so no instance of its ability is created. Once rezzed, a LATER
/// install in its server does trigger it.
#[test]
fn example_rule_condition_only_met_while_active_2() {
    let mut vm = Vm::empty(320);
    let thg = vm.new_object(tk::thg_like("THG-like", 3), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(thg);
    let other = vm.new_object(tk::vanilla_asset("Later-Asset", 0, 3), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(other);
    tk::install_root(&mut vm, tk::adt_button("ADT-Button", thg), ServerId::Remote(1), true);
    // ADT's NewRemoteRoot deterministically mints Remote(100) (the VM's
    // remote counter starts there); the control-arm button installs into it.
    let mut later = tk::corp_install_button(
        "Later-Install",
        other,
        jinteki_cr::instr::InstallDest::Root(ServerId::Remote(100)),
    );
    later.abilities[0] = later.abilities[0].clone().labeled("later-install");
    tk::install_root(&mut vm, later, ServerId::Remote(2), true);
    vm.st.corp.credits = 5;
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    tk::take_labeled(&mut vm, Side::Corp, "adt", 100);
    // Let the install-and-rez frame resolve; the window then re-offers.
    let _ = tk::until_decision(&mut vm);

    assert!(vm.st.objects[&thg].faceup, "installed and rezzed");
    assert_eq!(vm.st.objects[&thg].zone, Zone::Root(ServerId::Remote(100)));
    // All costs ignored AND no self-trigger: exactly 5 credits.
    assert_eq!(
        vm.st.corp.credits, 5,
        "9.6.5b: THG was not active when its own install was processed"
    );
    // The zero rez cost was still a real payment step (1.16.1d/1.16.3a).
    assert!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::CostPaid { credits: 0, .. })).count() >= 2,
        "install- and rez-cost steps happened at cost 0"
    );

    // Control arm, same priority window: a later install into THG's server
    // DOES pend its instance — THG is active when the change is processed.
    tk::take_labeled(&mut vm, Side::Corp, "later-install", 100);
    let _ = drive_to_action_window(&mut vm, Side::Corp);
    assert_eq!(
        vm.st.corp.credits, 6,
        "an install while THG is active pends its instance normally"
    );
}

/// example_step_checkpoint_card_entering_root_during_breach_1 (10.3.1j): a
/// Ganked-class access ability installs a card into the root of the server
/// being breached. At the next checkpoint the Runner DECLARES whether the
/// new card becomes a candidate; declaring yes lets them access it later in
/// the same breach.
#[test]
fn example_step_checkpoint_card_entering_root_during_breach_1() {
    let mut vm = Vm::empty(321);
    let drafted = vm.new_object(tk::vanilla_asset("Drafted-Asset", 0, 3), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(drafted);
    let ganked = tk::install_root(&mut vm, tk::ganked_like("Ganked-like", drafted), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);

    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Remote(1) }));

    let mut declared = false;
    let mut triggered_install = false;
    for _ in 0..400 {
        let (s, spec) = decision(&mut vm);
        let _ = s;
        match &spec {
            // The Ganked-class ability is the CORP's optional reaction.
            DecisionSpec::ReactionWindow { options, .. } if !triggered_install => {
                if let Some(opt) = tk::option_labeled(options, "ganked") {
                    triggered_install = true;
                    vm.answer(DecisionAnswer::Take(opt));
                } else {
                    let a = tk::default_answer(&spec);
                    vm.answer(a);
                }
            }
            DecisionSpec::DeclareBreachCandidate { card } => {
                assert_eq!(*card, drafted);
                declared = true;
                vm.answer(DecisionAnswer::ResolveOptional(true));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(triggered_install, "the Ganked-class install resolved mid-breach");
    assert!(declared, "10.3.1j: the Runner declared candidacy");
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == drafted)),
        "the declared candidate was accessed later in the breach"
    );
    assert_eq!(vm.st.objects[&drafted].zone, Zone::Root(ServerId::Remote(1)));
    let _ = ganked;
}

// ===========================================================================
// §8.6 — playing events and operations (W3b)
// ===========================================================================

/// example_rule_playing_one_at_a_time_1 (8.6.3): a Subcontract-class effect
/// plays two operations ONE AT A TIME; the credits gained from the first
/// (Hedge-Fund-class) pay for the second, which was unaffordable before.
#[test]
fn example_rule_playing_one_at_a_time_1() {
    let mut vm = Vm::empty(331);
    let hf = vm.new_object(
        tk::operation("HedgeFund-like", 1, vec![jinteki_cr::instr::Instruction::GainCredits(Side::Corp, 4)]),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hf);
    let second = vm.new_object(
        tk::operation("Second-Op", 3, vec![jinteki_cr::instr::Instruction::GainCredits(Side::Corp, 1)]),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(second);
    tk::install_root(&mut vm, tk::subcontract_button("Subcontract-Button", 2), ServerId::Remote(1), true);
    vm.st.corp.credits = 1;
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    tk::take_labeled(&mut vm, Side::Corp, "subcontract", 100);
    let mut picks: Vec<Vec<jinteki_cr::object::ObjectId>> = Vec::new();
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ChooseTargets { candidates, .. } if s == Side::Corp => {
                picks.push(candidates.clone());
                let pick = *candidates.first().unwrap();
                vm.answer(DecisionAnswer::Targets(vec![pick]));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert_eq!(picks.len(), 2, "two separate one-at-a-time picks (8.6.3)");
    assert_eq!(picks[0], vec![hf], "the second op was unaffordable at the first pick");
    assert_eq!(picks[1], vec![second], "Hedge Fund's credits made it affordable");
    assert_eq!(vm.st.objects[&hf].zone, Zone::Discard(Side::Corp));
    assert_eq!(vm.st.objects[&second].zone, Zone::Discard(Side::Corp));
    // 1 - 1 + 4 - 3 + 1 = 2.
    assert_eq!(vm.st.corp.credits, 2);
}

/// example_rule_playing_lingering_effects_1 (8.6.4): a Test-Run-class event
/// creates a delayed conditional ability, then is fully resolved and
/// trashed once it finishes installing a program — while the lingering
/// effect lives on independently.
#[test]
fn example_rule_playing_lingering_effects_1() {
    let mut vm = Vm::empty(332);
    let prog = vm.new_object(tk::program_cost("Deck-Prog", 2), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(prog);
    let test_run = vm.new_object(
        tk::event(
            "TestRun-like",
            0,
            vec![
                jinteki_cr::instr::Instruction::InstallCard {
                    card: jinteki_cr::instr::TargetSpec::Objects(vec![prog]),
                    dest: jinteki_cr::instr::InstallDest::Rig,
                    and_rez: false,
                    ignore_costs: true,
                    reveal_check: None,
                },
                jinteki_cr::instr::Instruction::CreateDelayedConditional {
                    def: Box::new(jinteki_cr::ability::AbilityDef::conditional(
                        jinteki_cr::ability::TriggerCond::TurnEnds(Side::Runner),
                        vec![jinteki_cr::instr::Instruction::GainCredits(Side::Runner, 1)],
                        false,
                    )
                    .labeled("testrun-delayed: at end of turn")),
                    duration: jinteki_cr::lingering::WantedDuration::UntilResolved,
                },
            ],
        ),
        Zone::Hand(Side::Runner),
    );
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(test_run);
    tk::install_rig(&mut vm, tk::play_event_button("Play-Button", test_run));
    vm.start_turn(Side::Runner);

    tk::take_labeled(&mut vm, Side::Runner, "play-event", 100);
    let _ = tk::until_decision(&mut vm);

    assert_eq!(vm.st.objects[&prog].zone, Zone::Rig, "the program was installed");
    assert_eq!(
        vm.st.objects[&test_run].zone,
        Zone::Discard(Side::Runner),
        "8.6.4: the event is fully resolved and trashed once it finishes installing"
    );
    assert!(
        vm.lingering
            .iter()
            .any(|l| matches!(l.payload, jinteki_cr::lingering::Payload::DelayedConditional { .. })),
        "the lingering effect lives on, independent of the trashed event"
    );
}

/// example_rule_play_no_trash_left_play_area_1 (8.6.6a): an Ashen-Epilogue
/// class event removes itself from the game with its last play ability; it
/// is no longer in the play area at 8.6.7g, so it is not trashed.
#[test]
fn example_rule_play_no_trash_left_play_area_1() {
    let mut vm = Vm::empty(333);
    let ashen = vm.new_object(
        tk::event(
            "Ashen-like",
            0,
            vec![
                jinteki_cr::instr::Instruction::GainCredits(Side::Runner, 1),
                jinteki_cr::instr::Instruction::RemoveSelfFromGame,
            ],
        ),
        Zone::Hand(Side::Runner),
    );
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(ashen);
    tk::install_rig(&mut vm, tk::play_event_button("Play-Button", ashen));
    vm.start_turn(Side::Runner);

    tk::take_labeled(&mut vm, Side::Runner, "play-event", 100);
    let _ = tk::until_decision(&mut vm);

    assert_eq!(
        vm.st.objects[&ashen].zone,
        Zone::RemovedFromGame,
        "8.6.6a: a played card that left the play area is not trashed"
    );
}

/// example_step_checkpoint_duration_abilities_2 (10.3.1b / 8.6.6c): a
/// Targeted-Marketing-class operation stays in the play area after
/// resolving. When the Runner steals an agenda, the next checkpoint
/// recognizes the shield no longer applies and the Corp trashes it as if
/// completing its resolution.
#[test]
fn example_step_checkpoint_duration_abilities_2() {
    let mut vm = Vm::empty(334);
    let tm = vm.new_object(tk::targeted_marketing_like("TM-like"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(tm);
    tk::install_root(&mut vm, tk::play_operation_button("Play-Op", tm), ServerId::Remote(1), true);
    let agenda = tk::install_root(&mut vm, tk::vanilla_agenda("Prize", 3, 1), ServerId::Remote(2), false);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    tk::take_labeled(&mut vm, Side::Corp, "play-op", 100);
    let _ = tk::until_decision(&mut vm);
    assert_eq!(
        vm.st.objects[&tm].zone,
        Zone::PlayArea(Side::Corp),
        "8.6.6c: not trashed until the Runner steals an agenda"
    );

    // Run out the Corp turn; the Runner steals the agenda.
    let _ = drive_to_action_window(&mut vm, Side::Corp);
    for _ in 0..600 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::TakeAction { .. } if s == Side::Runner => {
                vm.answer(DecisionAnswer::Action(ActionOption::BasicRun {
                    server: ServerId::Remote(2),
                }));
            }
            _ => {
                if vm.st.score_area[&Side::Runner].contains(&agenda)
                    && vm.st.objects[&tm].zone == Zone::Discard(Side::Corp)
                {
                    break;
                }
                let a = tk::default_answer(&spec);
                vm.answer(a);
            }
        }
    }
    assert!(vm.st.score_area[&Side::Runner].contains(&agenda), "the agenda was stolen");
    assert_eq!(
        vm.st.objects[&tm].zone,
        Zone::Discard(Side::Corp),
        "the checkpoint after the steal trashes the shielded operation (10.3.1b)"
    );
}

// ===========================================================================
// §9.6.5c/d — trigger-condition vs instruction requirements (W3b)
// ===========================================================================

/// example_rule_condition_requirements_part_of_condition_1 (9.6.5c): the QPM
/// requirement "if the Runner is tagged when accessed" is PART OF the
/// trigger condition — a Casting-Call-class rider granting tags on the same
/// access can never make QPM's condition met, in any order.
#[test]
fn example_rule_condition_requirements_part_of_condition_1() {
    // Arm 1: untagged at access time — QPM never pends even though the
    // rider gives 2 tags during the same access.
    let mut vm = Vm::empty(335);
    let qpm = tk::install_root(&mut vm, tk::qpm_with_casting_call("QPM-like"), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Remote(1) }));
    for _ in 0..300 {
        let (_, spec) = decision(&mut vm);
        if matches!(spec, DecisionSpec::TakeAction { .. }) {
            break;
        }
        let a = tk::default_answer(&spec);
        vm.answer(a);
    }
    assert_eq!(vm.st.runner.tags, 2, "the Casting-Call rider fired");
    assert_eq!(
        vm.st.corp.credits, 0,
        "9.6.5c: QPM cannot meet its condition — the tags came after the access occurred"
    );

    // Arm 2 (control): already tagged when accessed — QPM pends normally.
    let mut vm = Vm::empty(336);
    let qpm2 = tk::install_root(&mut vm, tk::qpm_with_casting_call("QPM-like"), ServerId::Remote(1), false);
    vm.st.runner.tags = 1;
    vm.start_turn(Side::Runner);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Remote(1) }));
    for _ in 0..300 {
        let (_, spec) = decision(&mut vm);
        if matches!(spec, DecisionSpec::TakeAction { .. }) {
            break;
        }
        let a = tk::default_answer(&spec);
        vm.answer(a);
    }
    assert_eq!(vm.st.corp.credits, 1, "tagged at access time: QPM's condition is met");
    let _ = (qpm, qpm2);
}

/// example_rule_condition_requirements_part_of_effect_1 (9.6.5d): link
/// requirements in Underworld Contact's INSTRUCTIONS are checked when they
/// resolve. Both UC and The Supplier meet "turn begins" together; resolving
/// The Supplier first installs the Dyson Mem Chip, so UC sees 2 link and
/// pays out — even though the Runner had 1 link when UC became pending.
#[test]
fn example_rule_condition_requirements_part_of_effect_1() {
    let mut vm = Vm::empty(337);
    tk::install_rig(&mut vm, tk::dyson_like("Base-Link"));
    let dyson2 = vm.new_object(tk::dyson_like("Dyson-Mem-Chip"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(dyson2);
    tk::install_rig(&mut vm, tk::supplier_like("Supplier-like", dyson2));
    tk::install_rig(&mut vm, tk::underworld_contact_like("UC-like"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut took_supplier = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ReactionWindow { options, .. } if s == Side::Runner && !took_supplier => {
                if let Some(opt) = tk::option_labeled(options, "supplier") {
                    took_supplier = true;
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
    assert!(took_supplier, "The Supplier resolved first");
    assert_eq!(vm.st.objects[&dyson2].zone, Zone::Rig, "the Dyson was installed");
    assert_eq!(vm.runner_link(), 2);
    assert_eq!(
        vm.st.runner.credits,
        5 + 1,
        "9.6.5d: UC's link requirement is checked at resolution, not at pend time"
    );
}

// ===========================================================================
// §9.9.11 — replacement-effect ordering (W3c)
// ===========================================================================

/// example_rule_replacement_effect_must_have_something_to_replace_1
/// (9.9.11a): Security-Testing and Account-Siphon class replacements both
/// target the imminent breach. The Runner chooses which applies first; since
/// neither creates a new breach, the one not chosen has nothing to replace
/// and does not apply.
#[test]
fn example_rule_replacement_effect_must_have_something_to_replace_1() {
    let mut vm = Vm::empty(341);
    let sectest = tk::install_rig(&mut vm, tk::vanilla_runner_card("SecurityTesting-like", jinteki_cr::object::CardType::Resource));
    let siphon = tk::install_rig(&mut vm, tk::vanilla_runner_card("AccountSiphon-like", jinteki_cr::object::CardType::Event));
    tk::fill_hand(&mut vm, Side::Corp, 2);
    vm.start_turn(Side::Runner);
    let _ = drive_to_action_window(&mut vm, Side::Runner);
    tk::inject_breach_replacement(
        &mut vm,
        sectest,
        jinteki_cr::lingering::ReplacementTransform::SuppressAndGainCredits(2),
    );
    tk::inject_breach_replacement(
        &mut vm,
        siphon,
        jinteki_cr::lingering::ReplacementTransform::SuppressAndGainCredits(3),
    );
    vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Hq }));

    let mut ordered = false;
    for _ in 0..300 {
        let (s, spec) = decision(&mut vm);
        match &spec {
            DecisionSpec::ChooseOption { options } if s == Side::Runner && !ordered => {
                assert_eq!(options.len(), 2, "9.9.11: both replacements offered for ordering");
                ordered = true;
                let i = options.iter().position(|l| l.contains("SecurityTesting")).unwrap();
                vm.answer(DecisionAnswer::Option(i));
            }
            DecisionSpec::TakeAction { .. } => break,
            other => {
                let a = tk::default_answer(other);
                vm.answer(a);
            }
        }
    }
    assert!(ordered, "the order Decision was presented at imminence-open");
    assert_eq!(
        vm.st.runner.credits,
        2,
        "only the chosen replacement applied; the other had nothing to replace"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "the breach itself was replaced"
    );
}

/// example_rule_replacement_effect_must_have_something_to_replace_2
/// (9.9.11a): with Security-Testing and Showing-Off class replacements, the
/// order matters mechanically but not in outcome: Showing Off REPLACES the
/// breach with a bottom-up breach that is still expected, so Security
/// Testing can still replace that; chosen the other way, Showing Off has
/// nothing left to replace. Either way: gain 2, no breach.
#[test]
fn example_rule_replacement_effect_must_have_something_to_replace_2() {
    for pick_showing_off_first in [true, false] {
        let mut vm = Vm::empty(342);
        let sectest = tk::install_rig(&mut vm, tk::vanilla_runner_card("SecurityTesting-like", jinteki_cr::object::CardType::Resource));
        let showoff = tk::install_rig(&mut vm, tk::vanilla_runner_card("ShowingOff-like", jinteki_cr::object::CardType::Event));
        tk::fill_deck(&mut vm, Side::Corp, 3);
        vm.start_turn(Side::Runner);
        let _ = drive_to_action_window(&mut vm, Side::Runner);
        tk::inject_breach_replacement(
            &mut vm,
            sectest,
            jinteki_cr::lingering::ReplacementTransform::SuppressAndGainCredits(2),
        );
        tk::inject_breach_replacement(
            &mut vm,
            showoff,
            jinteki_cr::lingering::ReplacementTransform::BreachFromBottom,
        );
        vm.answer(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Rnd }));

        let mut ordered = false;
        for _ in 0..300 {
            let (s, spec) = decision(&mut vm);
            match &spec {
                DecisionSpec::ChooseOption { options } if s == Side::Runner && !ordered => {
                    ordered = true;
                    let needle =
                        if pick_showing_off_first { "ShowingOff" } else { "SecurityTesting" };
                    let i = options.iter().position(|l| l.contains(needle)).unwrap();
                    vm.answer(DecisionAnswer::Option(i));
                }
                DecisionSpec::TakeAction { .. } => break,
                other => {
                    let a = tk::default_answer(other);
                    vm.answer(a);
                }
            }
        }
        assert!(ordered);
        assert_eq!(
            vm.st.runner.credits, 2,
            "either order: the Runner gains 2 and does not breach (pick_showing_off_first={pick_showing_off_first})"
        );
        assert!(
            !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
            "no breach either way"
        );
    }
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
