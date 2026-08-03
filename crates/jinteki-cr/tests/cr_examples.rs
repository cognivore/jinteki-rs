//! DP-7a: the CR's own worked examples as executable tests.
//!
//! Each test carries the example id from docs/rules/examples.json in a
//! comment and asserts the outcome the rules authors state. The tracking
//! test at the bottom is the DP-7a odometer (implemented / 243); the
//! `#[ignore]`d placeholder enumerates every example id still to be done.

use jinteki_cr::change::GameChange;
use jinteki_cr::decision::{ActionOption, DecisionAnswer, DecisionSpec, WindowOption};
use jinteki_cr::effects::DamageKind;
use jinteki_cr::instr::Quantity;
use jinteki_cr::object::{CounterKind, ServerId, Side, Zone};
use jinteki_cr::plan::{self, Kind, Match, Pick, Plan, Reply};
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
    // Wave 3d: vacuous truth (9.12.2d) and run-ends conditions (6.8.5).
    "example_rule_vacuous_truth_1",
    "example_rule_run_ends_condition_1",
    "example_rule_run_ends_condition_2",
    // Wave 3e: candidates (7.4.3, 7.4.7a).
    "example_rule_candidates_already_accessed_1",
    "example_rule_candidates_already_accessed_2",
    "example_rule_rnd_topmost_eligibile_candidate_1",
    "example_rule_rnd_topmost_eligibile_candidate_2",
    // Wave 3f: mid-window installs (9.9.4c/d), X-values (9.12.2e).
    "example_rule_trigger_conditional_ability_interrupt_2",
    "example_rule_trigger_paid_ability_interrupt_1",
    "example_rule_values_defined_by_x_1",
    "example_rule_values_defined_by_x_2",
    "example_rule_independent_effects_1",
    "example_rule_independent_effects_2",
    "example_rule_this_server_1",
    "example_rule_this_server_2",
    "example_rule_previous_object_source_1",
    "example_rule_limit_remote_servers_1",
    "example_sec_old_self_reference_rules_1",
    "example_sec_old_self_reference_rules_2",
    "example_rule_must_with_choice_1",
    "example_rule_must_without_choice_1",
    "example_rule_forced_mid_access_ability_optional_1",
    "example_rule_lingering_effect_maintaining_choice_default_duration_1",
    "example_rule_lingering_effect_maintaining_choice_turn_begins_duration_1",
    "example_rule_lingering_effect_maintaining_choice_duration_other_cases_1",
    "example_rule_object_move_known_location_1",
    // Wave 5a: searching, finding and shuffling (§8.7), 9.11.4d.
    "example_rule_valid_search_target_install_play_1",
    "example_rule_valid_search_target_install_play_2",
    "example_rule_valid_search_target_install_play_3",
    "example_rule_shuffle_deck_after_search_1",
    "example_rule_search_instruction_1",
    // Wave 5b: hosting (§1.13/8.3) and hosted counters (1.13.3, 9.1.6c).
    "example_rule_host_via_install_1",
    "example_rule_host_via_ability_1",
    "example_rule_host_on_ability_1",
    "example_rule_host_transitivity_1",
    "example_rule_hosted_object_same_zone_as_host_1",
    "example_rule_trash_hosted_objects_when_host_trashed_1",
    "example_rule_trash_hosted_objects_when_host_trashed_2",
    "example_rule_trash_hosted_objects_when_host_trashed_3",
    "example_rule_hosted_counters_not_on_player_1",
    "example_rule_hosted_counters_not_on_player_2",
    "example_rule_hosted_counters_not_on_player_3",
    "example_rule_hosted_counter_used_condition_1",
    // Wave 6a: credits (§1.10), score/threat (§1.17), [trash] (1.19.4),
    // memory (1.20.2), damage attribution and simultaneity (§10.4).
    "example_rule_lose_credits_1",
    "example_rule_spend_credits_1",
    "example_rule_spend_credits_2",
    "example_rule_spend_credits_3",
    "example_rule_recurring_credits_do_not_accumulate_1",
    "example_rule_threat_level_1",
    "example_rule_trash_symbol_1",
    "example_rule_memory_limit_1",
    "example_rule_suffer_or_take_damage_1",
    "example_rule_multiple_damage_taken_simultaneously_1",
    "example_rule_controller_choices_1",
    "example_rule_trigger_condition_effect_by_player_1",
    "example_rule_trigger_condition_effect_by_player_2",
    // Wave 6b: strength modifications and their durations (§3.9.5, §9.10).
    "example_rule_icebreaker_strength_increase_implicit_1",
    "example_rule_icebreaker_strength_increase_specified_1",
    "example_rule_icebreaker_strength_increase_outside_of_encounter_1",
    "example_rule_modify_duration_of_lingering_effect_1",
    "example_rule_static_no_lingering_effects_1",
    "example_rule_replacement_on_static_ability_must_remain_active_1",
    // Wave 6c: sabotage (§10.12) and static value modification (9.4.5).
    "example_rule_sabotage_all_remaining_cards_1",
    "example_rule_sabotage_all_remaining_cards_2",
    "example_rule_sabotage_all_remaining_cards_3",
    "example_rule_static_modification_keep_restrictions_1",
    "example_rule_paid_ability_refers_to_encountered_ice_2",
    // Wave 7a: target announcements (§1.15.2c/e).
    "example_rule_targets_must_be_in_play_area_1",
    "example_rule_distinct_targets_1",
    // Wave 7b: several announcements per instruction, subroutine targets.
    "example_rule_target_2",
    "example_rule_target_4",
    "example_rule_break_all_but_x_subroutines_targets_1",
    // Wave 7c: 1.15.4 targets beyond a move.
    "example_rule_target_beyond_move_1",
    "example_rule_target_1",
    // Wave 7d: identifying instructions (§9.11.2/9.11.4), subtypes (2.16.5).
    "example_rule_choose_instruction_1",
    "example_rule_add_remove_subtypes_1",
    "example_rule_choice_instruction_1",
    "example_rule_split_up_instruction_1",
    "example_rule_step_sequences_2",
    // Wave 7e: deck construction (§1.4), actions (§5.2), stray run rules.
    "example_rule_influence_by_copy_1",
    "example_rule_54+_1",
    "example_rule_costs_with_click_1",
    "example_rule_action_timing_structure_completion_1",
    "example_rule_end_run_no_run_or_encounter_1",
    "example_rule_candidates_leaving_server_1",
    // Wave 7f: object identity across moves (§1.12).
    "example_rule_no_memory_1",
    "example_rule_object_turn_faceup_facedown_1",
    "example_rule_identify_object_after_move_1",
    "example_rule_previous_object_2",
    // Wave 8a: positions as objects (§6.2).
    "example_rule_ice_change_during_movement_1",
    "example_rule_ice_change_during_movement_2",
    "example_rule_ice_change_outward_1",
    "example_rule_ice_change_inward_1",
    "example_rule_count_positions_1",
    "example_rule_count_positions_2",
    "example_rule_ice_change_encounter_move_swap_1",
    // Wave 8b: swapping cards (§8.8).
    "example_rule_swap_installed_cards_preserves_hosting_1",
    "example_rule_swap_only_to_valid_location_1",
    "example_rule_swap_become_installed_1",
    // Wave 8c: costs, continued (§1.16).
    "example_rule_install_and_rez_reducing_total_1",
    "example_rule_cost_quantities_1",
    "example_rule_cost_interrupt_static_mandatory_1",
    // Wave 9a: encounters as a timing structure (§6.5.9, §6.1.4b, §6.5.8c).
    "example_rule_forced_encounter_1",
    "example_rule_forced_encounter_during_run_1",
    "example_rule_end_encounter_outside_run_1",
    "example_rule_bypass_during_encounter_1",
    "example_rule_active_exception_encounter_not_installed_1",
    "example_rule_no_position_after_approach_server_1",
    "example_rule_ice_strength_modification_duration_1",
    // Wave 9b: advancement (§1.18) and the dividends keyword (§10.13).
    "example_rule_placing_advancement_counter_1",
    "example_rule_dividends_1",
    "example_rule_dividends_timing_1",
    // Wave 9c: the attacked server (§6.1.2d, §6.3.2a) and restrictions (§9.3.3f,
    // §9.11.4a).
    "example_rule_change_attacked_server_directly_1",
    "example_rule_cannot_run_abilities_1",
    "example_rule_use_restrictions_1",
    "example_rule_variable_restriction_1",
    // Wave 11a: resolving an ability by class (9.6.14d), rez by ability
    // (8.1.2b), forfeit as a cost (8.2.5).
    "example_rule_this_server_3",
    "example_rule_instructed_to_resolve_conditional_ability_1",
    // Wave 11b: §9.1 — resolution independence (9.1.4), "is resolving"
    // (9.1.2b).
    "example_rule_abilities_resolution_independent_1",
    "example_rule_is_resolving_1",
    "example_rule_is_resolving_2",
    // Wave 11c: exposing (1.21.4/9.6.4b), cancelled movements (8.2.2a),
    // paid windows closed by the run ending (6.8.2a).
    "example_rule_condition_met_multiple_times_2",
    "example_rule_cancelled_movement_1",
    "example_rule_run_ends_close_paws_1",
    "example_rule_prevent_as_trigger_condition_1",
    "example_rule_look_reveal_instruction_1",
    "example_rule_play_ability_1",
    "example_rule_reveal_from_hidden_1",
];

// The legacy scaffold — `decision`, `drive_to_action_window`, the local
// `window_options` — is gone: every test below declares setup, one plan per
// player, and assertions, and the shared `plan::Script` driver folds them
// (ARCHITECTURE §12 rule 5).

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

    // The plan: run R&D, and the first time an interrupt window offers Decoy
    // (while the tag is imminent), use it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .when(Match::interrupt().once(), Reply::take("decoy"))
            .stop_at_action(),
    );
    assert!(t.took("decoy"), "Decoy-class interrupt offered while the tag is imminent");
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

    // The plan: fire the trash button once, in the first paid window offering
    // it; everything else is neutral.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("trash the set"))
            .stop_at_action(),
    );
    assert_eq!(
        t.first_window(Kind::Reaction, Side::Corp).count("hostile-infra"),
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

    // The plan: run HQ, bypass with Femme from the encounter-begins reaction
    // window, and pay her bypass cost.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::reaction().once(), Reply::take("femme"))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert!(t.took("femme"));
    let costs: Vec<u32> =
        t.of_kind(Kind::NestedCost).iter().filter_map(|e| e.cost()).map(|c| c.flat_credits()).collect();
    assert_eq!(costs, vec![1], "the only cost put to a player was Femme's 1[c]");
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

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("trash the set"))
            .stop_at_action(),
    );
    let offer = t.first_window(Kind::Reaction, Side::Corp);
    assert_eq!(offer.count("hostile-infra"), 3, "9.6.4b: one instance per trashed card");
    assert_eq!(offer.count("warroid"), 1, "9.12.2a: the set-trigger sees one event");
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

    // The plan: always trigger Parasite when offered; prevent the trash once
    // with Sacrificial Construct.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction(), Reply::take("parasite"))
            .when(Match::interrupt().once(), Reply::take("sac-con"))
            .stop_at_action(),
    );
    assert!(t.took("sac-con"), "the trash was prevented once");
    assert_eq!(
        t.times_taken("parasite"),
        2,
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

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::reaction(), Reply::take("parasite")).stop_at_action(),
    );
    let parasite_triggers = t.times_taken("parasite");
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

    // The plan: spend the first two Corp actions advancing the ice.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().times(2), Reply::take("advance target"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(t.times_taken("advance target"), 2, "two advance actions were taken");

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

    // The plan: trigger Aesop's when offered and feed it Drug Dealer.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction(), Reply::take("aesops"))
            .when(Match::targets(), Reply::target(dd))
            .stop_at_action(),
    );
    assert!(t.took("aesops"));
    assert!(t.first_window(Kind::Targets, Side::Runner).candidates().contains(&dd));
    // After Aesop's resolves, Drug Dealer's pending must be GONE: no later
    // window may offer it.
    assert!(
        t.entries
            .iter()
            .skip_while(|e| !e.took("aesops"))
            .skip(1)
            .all(|e| !e.offered("drug-dealer")),
        "9.6.10: Drug Dealer's instance lost its pending status"
    );
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

    // The plan: the Runner runs Archives; the Corp zaps twice from its paid
    // windows; the Runner prevents with Feedback Filter whenever offered.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().times(2), Reply::take("do net damage")),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .when(Match::interrupt(), Reply::take("feedback"))
            .stop_at_action(),
    );
    assert_eq!(
        t.times_taken("do net damage"),
        2,
        "two net-damage instructions became imminent"
    );
    assert!(
        !t.ever_offered("tori"),
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

    // The plan: the Runner takes the tag (arming Mr. Stone) and prevents 2
    // with Biometric Spoofing; the Corp adds +1 twice from its interrupts.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::interrupt().times(2), Reply::take("cleaners")),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("take 1 tag"))
            .when(Match::interrupt().once(), Reply::take("biometric"))
            .stop_at_action(),
    );
    assert!(t.took("take 1 tag") && t.took("biometric"));
    assert_eq!(
        t.times_taken("cleaners"),
        2,
        "the value was modifiable at 0 and below (9.9.7a)"
    );
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

    // The plan: the Corp fires the damage button once; the Runner prevents
    // ALL of it with the Chrome-Parlor-class interrupt.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("do meat damage")),
        Plan::runner().when(Match::interrupt().once(), Reply::take("chrome-parlor")),
    );
    assert!(t.took("do meat damage") && t.took("chrome-parlor"));
    assert!(
        t.entries
            .iter()
            .skip_while(|e| !e.took("chrome-parlor"))
            .skip(1)
            .all(|e| !e.offered("cleaners")),
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

    // The plan: run Archives, access the Dome, prevent 2 of the 1 net damage.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .when(Match::interrupt().once(), Reply::take("biometric"))
            .stop_at_action(),
    );
    assert!(t.took("biometric"));
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

    // The plan: run HQ; in the first reaction window offering it, resolve
    // Security Nexus and say yes to its declineable "end the run".
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::reaction().once(), Reply::take("nexus"))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(t.took("nexus"));
    assert!(!t.took("tag-tax"), "6.8.2b: the tag tax never resolved");
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

    // The plan: run HQ and let the run play out; neither player acts.
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );

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

    // The plan: run HQ, pump twice in the encounter's paid ability window,
    // then halt in the next one so the boosted strength can be observed
    // while the encounter still lasts.
    let mut g = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").times(2), Reply::take("pump"))
            // The third such window (the first this rule sees, since the rule
            // above claims the first two).
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    g.run(&mut vm);
    // Both pumps have resolved (9.10.4a duration in force).
    assert_eq!(
        vm.effective_strength(breaker),
        Some(5),
        "1 base + 2×2 pump while the encounter lasts"
    );
    g.run(&mut vm);
    let t = g.transcript();

    assert_eq!(t.times_taken("pump"), 2);
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

    // The plan: run the remote and let the run end; nobody acts.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.runner.tags, 2,
        "10.3.6: the tags landed outside the run; Jesminder could not apply"
    );
    assert!(
        !t.ever_offered("jesminder"),
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

    // The plan: run R&D and trash the accessed card from the first mid-access
    // window that offers Khumalo.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .when(Match::mid_access().once(), Reply::take("khumalo"))
            .stop_at_action(),
    );
    assert!(t.took("khumalo"), "the mid-access ability was offered and used");
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

    // The plan: fire Clone Chip's [trash] ability from the first paid window
    // that offers it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("clone-chip"))
            .stop_at_action(),
    );
    assert!(t.took("clone-chip"));
    assert_eq!(vm.st.objects[&chip].zone, Zone::Discard(Side::Runner));
    assert!(
        !t.ever_offered("llds"),
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

    // The plan: run HQ; the Runner would pay any nested cost put to them —
    // but the tag cost is unpayable, so it is never put to them at all.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert!(
        t.of_kind(Kind::NestedCost).is_empty(),
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

    // The plan: use Zer0 once, from the first paid window that offers it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("zer0"))
            .stop_at_action(),
    );
    assert_eq!(t.times_taken("zer0"), 1);
    assert_eq!(
        t.offers("zer0"),
        1,
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

    // The plan: run the remote, access Obokata, and decline its additional
    // steal cost.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::nested_cost(), Reply::PayCost(false))
            .stop_at_action(),
    );
    let costs = t.of_kind(Kind::NestedCost);
    assert_eq!(
        costs.iter().filter_map(|e| e.cost()).map(|c| c.net_damage).collect::<Vec<_>>(),
        vec![4],
        "the pay-or-decline choice was presented, for 4 net damage"
    );
    assert!(costs.iter().all(|e| e.side == Side::Runner), "put to the Runner");
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

    // The plan: run the remote, access Obokata, and pay the combined
    // additional steal cost in full.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    // One aggregated all-at-once payment (1.16.10b).
    assert_eq!(
        t.of_kind(Kind::NestedCost)
            .iter()
            .filter_map(|e| e.cost())
            .map(|c| (c.net_damage, c.flat_credits()))
            .collect::<Vec<_>>(),
        vec![(6, 2)],
        "4 + 2 net and 2[c] arrived as ONE aggregated cost"
    );
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

    // The plan: run HQ and pay the subroutine's "unless" cost.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    let costs = t.of_kind(Kind::NestedCost);
    assert_eq!(
        costs.iter().filter_map(|e| e.cost()).map(|c| c.flat_credits()).collect::<Vec<_>>(),
        vec![1],
        "the only cost put to a player was the subroutine's 1[c]"
    );
    assert!(costs.iter().all(|e| e.side == Side::Runner), "put to the Runner");
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

    // The plan: cash Fermenter out from the first paid window offering it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("fermenter"))
            .stop_at_action(),
    );
    assert!(t.took("fermenter"));
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

    // The plan: trash Reconstruction Contract for its ability from the first
    // Corp paid window offering it, and send the counters to the Wall.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("reconstruction"))
            .when(Match::targets(), Reply::target(wall))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("reconstruction"));
    // 9.5.7c: the target is chosen after the cost-paid checkpoint, as the
    // instruction becomes imminent.
    let targets = t.of_kind(Kind::Targets);
    assert!(
        !targets.is_empty() && targets.iter().all(|e| e.candidates().contains(&wall)),
        "the installed Wall was a candidate at every target choice"
    );
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

    // The plan: run HQ and let the encounter happen; nobody triggers
    // anything — the question is only ever *what was offered*.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert!(
        t.first_window(Kind::Action, Side::Runner)
            .actions()
            .iter()
            .any(|o| matches!(o, ActionOption::BasicRun { server: ServerId::Hq })),
        "the run on HQ was there to take"
    );
    assert!(t.ever_offered("arruaceiras"), "usable during the encounter");
    assert!(
        t.entries
            .iter()
            .filter(|e| e.offered("arruaceiras"))
            .all(|e| e.step.as_deref() == Some("step_encounter_paw")),
        "9.5.6c: not usable at an arbitrary time — only in the encounter's paid window"
    );
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
    // The plan (both halves): run HQ and let the approach happen.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert!(!t.ever_offered("wotan"), "9.5.6b: not offered while approaching non-bioroid ice");

    // Rezzed bioroid approach: offered in the approach window.
    let mut vm = Vm::empty(41);
    let mut bio = tk::vanilla_ice("Eli-like", 0, 3);
    bio.subtypes = vec!["bioroid", "barrier"];
    tk::install_ice(&mut vm, bio, ServerId::Hq, true);
    let w = vm.new_object(tk::wotan_like("Wotan-like"), Zone::ScoreArea(Side::Corp));
    vm.st.score_area.get_mut(&Side::Corp).unwrap().push(w);
    vm.start_turn(Side::Runner);
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert!(
        t.entries.iter().any(|e| e.side == Side::Corp
            && matches!(&e.spec, DecisionSpec::PaidWindow { classes, .. } if classes.rez_approached_ice)
            && e.offered("wotan")),
        "9.5.6b: offered while approaching rezzed bioroid ice"
    );
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

    // The plan: run HQ and let Little Engine's subroutines resolve.
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );

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

    // The plan: fire Process Automation from the first paid window offering
    // it, then let the consequences play out.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("process-automation"))
            .stop_at_action(),
    );
    assert!(t.took("process-automation"), "the ability was offered and used");
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

    // The plan: the same one — fire Process Automation from the first paid
    // window offering it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("process-automation"))
            .stop_at_action(),
    );
    assert!(t.took("process-automation"), "the ability was offered and used");
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

    // The plan: fire Process Automation, and take the Class Act interrupt in
    // any interrupt window that offers it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("process-automation"))
            .when(Match::interrupt(), Reply::take("class-act"))
            .stop_at_action(),
    );
    assert!(
        t.of_kind(Kind::Interrupt)
            .iter()
            .any(|e| e.side == Side::Runner && e.took("class-act")),
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

    // The plan: the Corp fires its trash button; the Runner answers with
    // Sacrificial Construct from the first interrupt window offering it.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("corp-trash")),
        Plan::runner()
            .when(Match::interrupt().once(), Reply::take("sac-con"))
            .stop_at_action(),
    );
    let sac = t
        .entries
        .iter()
        .find(|e| e.took("sac-con"))
        .expect("the prevention was offered and used");
    assert!(
        t.entries.iter().any(|e| e.seq <= sac.seq
            && e.side == Side::Runner
            && e.kind() == Kind::Interrupt
            && e.offered("harbinger")),
        "relevant while its trash was expected"
    );
    assert!(
        !t.entries.iter().any(|e| e.seq > sac.seq && e.offered("harbinger")),
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

    // The plan: the Corp fires Flare from the first paid window offering it;
    // nobody else acts — the question is only ever *what was offered*.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("flare")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("flare"));
    assert!(
        !t.ever_offered("biometric"),
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

    // The plan: the Corp does the net damage and replaces it with core damage
    // via Tori, from the first interrupt window offering it; the Runner never
    // acts (their preventer is only ever *offered* or not).
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("do net damage"))
            .when(Match::interrupt().once(), Reply::take("tori-replace")),
        Plan::runner().stop_at_action(),
    );
    let tori = t
        .entries
        .iter()
        .find(|e| e.took("tori-replace"))
        .expect("the replacement was offered and used");
    assert!(
        !t.entries.iter().any(|e| e.seq > tori.seq
            && e.side == Side::Runner
            && e.kind() == Kind::Interrupt
            && e.offered("biometric")),
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

    // The plan: run the remote, trash AMAZE on access, then run again with
    // Doppelgänger from the reaction window the ending run opens; both sides
    // resolve every optional part they are offered.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::trash_accessed())
            .when(Match::reaction().once(), Reply::take("doppel"))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(
        t.entries.iter().any(|e| matches!(
            &e.answer,
            Some(DecisionAnswer::Take(WindowOption::BasicTrash { card, .. })) if *card == amaze
        )),
        "the persist began with the trash-during-access"
    );
    assert!(
        t.of_kind(Kind::Reaction).iter().any(|e| e.side == Side::Runner && e.took("doppel")),
        "a second run happened during the reaction window"
    );
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

    // The plan: the Corp fires Gemini twice, spending 3 openly on the first
    // trace (strength 6) and nothing on the second (strength 3); the Runner
    // never spends, so its link stays 0.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().times(2), Reply::take("gemini"))
            .when(Match::trace_spend().first(), Reply::Spend(3))
            .when(Match::trace_spend(), Reply::Spend(0)),
        Plan::runner().when(Match::trace_spend(), Reply::Spend(0)).stop_at_action(),
    );
    assert_eq!(t.times_taken("gemini"), 2);
    // Both trace spends were the Corp's own half of an open trace (10.8.6c),
    // and the first could afford everything it spent.
    let corp_spends = t
        .of_kind(Kind::TraceSpend)
        .iter()
        .filter(|e| e.side == Side::Corp)
        .map(|e| match &e.spec {
            DecisionSpec::TraceSpend { max, corp_side, .. } => (*max, *corp_side),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(corp_spends.len(), 2, "one Corp spend per trace");
    assert!(corp_spends.iter().all(|(_, corp_side)| *corp_side));
    assert!(corp_spends[0].0 >= 3, "the Corp could afford the 3 it spent");
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

    // The plan: the Corp plays the psi game and bids 0; the Runner bids 1,
    // which only the hosted credit can pay for.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("psi"))
            .when(Match::psi_bid(), Reply::Bid(0)),
        Plan::runner().when(Match::psi_bid(), Reply::Bid(1)).stop_at_action(),
    );
    let runner_legal = t
        .of_kind(Kind::PsiBid)
        .iter()
        .filter(|e| e.side == Side::Runner)
        .map(|e| match &e.spec {
            DecisionSpec::PsiBid { legal } => legal.clone(),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        runner_legal,
        vec![vec![0, 1]],
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

    // The plan: the Corp plays the psi game; both sides bid 0 — the Runner
    // because RSVP leaves them nothing else to bid.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("psi"))
            .when(Match::psi_bid(), Reply::Bid(0)),
        Plan::runner().when(Match::psi_bid(), Reply::Bid(0)).stop_at_action(),
    );
    let runner_legal = t
        .of_kind(Kind::PsiBid)
        .iter()
        .filter(|e| e.side == Side::Runner)
        .map(|e| match &e.spec {
            DecisionSpec::PsiBid { legal } => legal.clone(),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        runner_legal,
        vec![vec![0]],
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

    // The plan: run HQ; in the encounter's paid ability window the Runner
    // breaks the 3 subs and then passes, the Corp draws with the panic
    // button, and the Runner breaks the 4th, freshly-granted one. The driver
    // halts in the next such window so the sub count can be read while the
    // encounter still lasts.
    let mut g = plan::Script::new(
        Plan::corp()
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("panic-button")),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").times(3), Reply::take("break"))
            // Pass once, handing priority to the Corp for its draw…
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Pass)
            // …then break the subroutine the draw granted.
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("break"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert!(vm.st.encounter.is_some(), "the count below is taken mid-encounter");
    assert_eq!(
        vm.current_subs(ash).len(),
        4,
        "9.8.3d: the new sub was added after the previous 3"
    );
    g.run(&mut vm);
    let t = g.transcript();

    let draw = t.entries.iter().find(|e| e.took("panic-button")).expect("the Corp drew");
    assert_eq!(
        t.entries.iter().filter(|e| e.seq < draw.seq && e.took("break")).count(),
        3,
        "the 3 printed subroutines were broken before the draw granted a 4th"
    );
    assert_eq!(t.times_taken("break"), 4, "the 4th (new, unbroken) subroutine could be broken too");
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

    // The plan: run HQ; in the encounter's paid ability window the Runner
    // breaks the first subroutine, then makes the Corp discard 2. The driver
    // halts in the next such window, while the encounter still lasts, so the
    // surviving subroutines can be counted.
    let mut g = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("break"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("utopia"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert!(vm.st.encounter.is_some(), "the count below is taken mid-encounter");
    // 9.8.3d: lost last-first — only the broken sub remains.
    assert_eq!(vm.current_subs(ash).len(), 1);
    g.run(&mut vm);
    let t = g.transcript();

    let broke = t.entries.iter().find(|e| e.took("break")).expect("a subroutine was broken");
    let utopia = t.entries.iter().find(|e| e.took("utopia")).expect("the Corp was made to discard");
    assert!(broke.seq < utopia.seq, "the first subroutine was broken before the discard");
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
    tk::install_root(
        &mut vm,
        tk::subroutine_granter(
            "Marker-like",
            brain,
            jinteki_cr::ability::AbilityDef::subroutine(vec![
                jinteki_cr::instr::Instruction::EndTheRun,
            ])
            .labeled("[sub] ETR (marker)"),
            false,
            // turn-bound: the lingering exists before the run begins.
            jinteki_cr::lingering::WantedDuration::ThisTurn,
        ),
        ServerId::Remote(9),
        true,
    );
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.start_turn(Side::Runner);

    // The plan: the Corp makes the Marker-class grant from its FIRST paid
    // window — before the run, and therefore before Brainstorm's
    // encounter-start grant, which is the point of the example — and the
    // Runner then runs HQ.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("marker:")),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    // The Marker-class lingering exists before the encounter begins.
    let grant = t
        .entries
        .iter()
        .find(|e| e.took("marker:"))
        .expect("the Marker-class grant was made");
    assert!(
        !grant.stack.contains(&jinteki_cr::timing::StructKind::Run),
        "the grant was made outside the run"
    );
    assert!(
        t.entries
            .iter()
            .filter(|e| e.stack.contains(&jinteki_cr::timing::StructKind::Run))
            .all(|e| e.seq > grant.seq),
        "9.8.3e: the Marker grant predates everything the run — and so the \
         encounter-start grant — produced"
    );

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

    // The plan: run the remote and let the breach play out; neither side
    // spends on the trace, so the base 3 beats link 0.
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );

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

    // The plan: run Archives three times, taking the Zahya-class instance
    // whenever a reaction window offers it — declining its optional part the
    // first time (run 1) and accepting it the second (run 2).
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().times(3), Reply::run(ServerId::Archives))
            .when(Match::reaction(), Reply::take("zahya"))
            .when(Match::optional().once(), Reply::Optional(false))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    let runs = t
        .entries
        .iter()
        .filter(|e| {
            matches!(
                &e.answer,
                Some(DecisionAnswer::Action(ActionOption::BasicRun { server: ServerId::Archives }))
            )
        })
        .count();
    assert_eq!(runs, 3);
    assert_eq!(
        t.times_taken("zahya"),
        2,
        "9.3.6g: pended after runs 1 and 2; after actually using it, never again"
    );
    assert_eq!(t.offers("zahya"), 2, "…and was never even offered a third time");
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

    // The plan: the Runner runs HQ; the Corp rezzes the approached ice from
    // the first 9.2.7e window that offers it.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().approaching_ice().once(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert!(
        t.entries.iter().any(|e| matches!(
            &e.answer,
            Some(DecisionAnswer::Take(WindowOption::RezApproachedIce { card })) if *card == tith
        )),
        "the approached Tithonium-class ice was rezzed (9.2.7e)"
    );
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

    // The plan: the Corp fires the Bad-Times-class ability from the first paid
    // window offering it, and the Runner trashes the 2[mu] singleton at the
    // restriction checkpoint (the set the assertion below pins down).
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("bad-times")).stop_at_action(),
        Plan::runner().when(Match::minimal_set(), Reply::ChooseSet(1)),
    );
    let choice = t
        .of_kind(Kind::MinimalSet)
        .first()
        .copied()
        .expect("a minimal-set choice was demanded");
    let DecisionSpec::MinimalSet { sets } = &choice.spec else { unreachable!() };
    assert_eq!(
        choice.answer,
        Some(DecisionAnswer::ChooseSet(
            sets.iter().position(|s| s == &vec![p3]).expect("2mu set offered")
        )),
        "the plan chose the 2[mu] singleton"
    );
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

    // The plan: arm the Groove-class delayed conditional in the first paid
    // window offering it, then take a tag in each of the next two paid
    // windows that offer one.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("groove:"))
            .when(Match::paid().times(2), Reply::take("take 1 tag"))
            .stop_at_action(),
    );
    assert!(t.took("groove:"), "the delayed conditional was armed first");
    assert_eq!(t.times_taken("take 1 tag"), 2);
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

    // The plan: arm the Joshua-class delayed conditional from the first paid
    // window offering it, then BOTH players spend every action window on a
    // basic credit. The Corp gets 3 clicks a turn, so its 4th action window is
    // the first of its SECOND turn — by which point two Runner turns (the one
    // in progress and the next) have ended.
    let t = plan::Script::new(
        Plan::corp().when(Match::action().nth(4), Reply::Halt).otherwise_click_credit(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("joshua:"))
            .otherwise_click_credit(),
    )
    .budget(1200)
    .play(&mut vm);
    assert!(t.took("joshua:"), "the turn-end trigger was installed");
    assert!(t.result.is_none(), "no game over");
    let runner_turn_ends = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::TurnEnded { side: Side::Runner }))
        .count();
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

    // The plan: use the button OUTSIDE any run (the first paid window that
    // offers it), halt at the action window so the absent lingering effect can
    // be observed, then run Archives and halt again once that run has ended.
    let mut g = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("mayfly:"))
            .when(Match::action().first(), Reply::Halt)
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert!(
        vm.lingering.is_empty(),
        "9.6.13d: the lingering effect was never created"
    );
    // A later run ends — nothing fires.
    g.run(&mut vm);
    assert!(
        vm.st.objects[&mayfly].zone.is_installed(),
        "Mayfly was not trashed by the later run"
    );
    assert!(g.transcript().took("mayfly:"), "the button was used outside the run");
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

    // The plan: fire the trash button once, in the first paid window offering
    // it; the Corp's first reaction window then carries the pending instances.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("trash the set"))
            .stop_at_action(),
    );
    let offer = t.first_window(Kind::Reaction, Side::Corp);
    assert_eq!(offer.count("hostile-infra"), 2, "9.12.2a: HI sees both trashed cards");
    assert_eq!(offer.count("warroid"), 1, "9.12.2a: Warroid sees one event");
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

    // The plan: draw 3 with the Ritual-class ability from the first paid window
    // offering it, and take the Class-Act-class would-draw interrupt every time
    // one is offered.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("ritual"))
            .when(Match::interrupt(), Reply::take("class-act"))
            .stop_at_action(),
    );
    assert_eq!(t.times_taken("class-act"), 1, "9.12.2b: one instance of drawing 3");
    assert_eq!(t.offers("class-act"), 1, "…and only one window ever offered it");
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

    // The plan: run R&D and prevent 2 of the damage from the first interrupt
    // window offering the Biometric-class ability.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .when(Match::interrupt().once(), Reply::take("biometric"))
            .stop_at_action(),
    );
    assert!(t.took("biometric"), "the prevention was offered and used");
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
    // The plan: run HQ and let the Fairchild-class subroutines resolve. Neither
    // player ever gets an optioned-effect decision — that is the claim.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .stop_at_action(),
    );
    assert_eq!(
        t.of_kind(Kind::Options).len(),
        0,
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

    // The plan: run HQ, choose the "take 1 tag" branch of the Data-Raven-class
    // mandatory choice, then avoid the tag with Decoy from the first interrupt
    // window offering it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::options(), Reply::ChooseNamed("tag"))
            .when(Match::interrupt().once(), Reply::take("decoy"))
            .stop_at_action(),
    );
    let choice = t
        .of_kind(Kind::Options)
        .into_iter()
        .find(|e| matches!(&e.answer, Some(DecisionAnswer::Option(i)) if e.choices()[*i].contains("tag")))
        .expect("the tag branch of the mandatory choice was chosen");
    let decoy = t.entries.iter().find(|e| e.took("decoy")).expect("Decoy avoided the tag");
    assert!(decoy.seq > choice.seq, "the tag was avoided AFTER the choice");
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

    // The plan: fire the mass-install button, then answer the one-at-a-time
    // target choices in the order they arrive — Dhegdheer to the rig, Prog-A
    // hosted on Dhegdheer, Prog-B to the rig — declining any further host
    // offer (Dhegdheer is full).
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("mass-install"))
            .when(Match::targets().once(), Reply::target(dheg))
            .when(Match::targets().once(), Reply::target(pa))
            .when(Match::targets().once(), Reply::target(dheg))
            .when(Match::targets().once(), Reply::target(pb))
            .when(Match::targets(), Reply::Targets(vec![]))
            .stop_at_action(),
    );
    assert!(t.took("mass-install"), "the mass-install ability was offered and used");
    let picks: Vec<Vec<jinteki_cr::ObjectId>> = t
        .windows(Kind::Targets, Side::Runner)
        .iter()
        .map(|e| match &e.answer {
            Some(DecisionAnswer::Targets(v)) => v.clone(),
            other => panic!("target choice answered with {other:?}"),
        })
        .collect();
    assert_eq!(
        picks,
        vec![vec![dheg], vec![pa], vec![dheg], vec![pb]],
        "8.5.5: three separate one-at-a-time install picks, with Prog-A's host \
         choice arriving between them"
    );
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

    // The plan: the Corp fires its install button from the first paid window
    // offering it, then plays on to its action window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("corp-install")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("corp-install"), "the install ability was offered and used");
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

    // The plan: run the remote, let the encounter resolve Brân's install
    // subroutine, then jack out whenever offered.
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::jack_out(), Reply::JackOut(true))
            .stop_at_action(),
    );
    assert_eq!(vm.st.objects[&hq_ice].zone, Zone::Ice(ServerId::Remote(1)));
    assert_eq!(
        vm.ice_at(ServerId::Remote(1)),
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

    // The plan: the Corp fires its install button from the first paid window
    // offering it, then plays on to its action window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("corp-install")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("corp-install"), "the install ability was offered and used");
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

    // The plan: the Corp fires its Ob-class install-and-rez button and then
    // declines every additional cost put to it.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-install-rez"))
            .when(Match::nested_cost(), Reply::PayCost(false))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("corp-install-rez"), "the install-and-rez ability was used");
    let costs = t.windows(Kind::NestedCost, Side::Corp);
    assert!(!costs.is_empty(), "the additional rez cost was offered and declined (1.16.4c)");
    assert!(
        costs.iter().all(|e| e.answer == Some(DecisionAnswer::PayNestedCost(false))),
        "the additional rez cost was offered and declined (1.16.4c)"
    );
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

    // The plan: the Corp fires its Trust-Operation-class install-and-rez button
    // from the first paid window offering it.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("corp-install-rez"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("corp-install-rez"), "the install-and-rez ability was used");
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

    // The plan: the Corp fires the Ad-Blitz-class button and picks the ice at
    // every target choice it is given.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("ad-blitz"))
            .when(Match::targets(), Reply::target(ice))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("ad-blitz"), "the Ad-Blitz-class ability was used");
    let choices = t.windows(Kind::Targets, Side::Corp);
    assert!(!choices.is_empty(), "the Corp was asked to choose what to install");
    for c in &choices {
        assert!(
            !c.candidates().contains(&agenda),
            "8.5.13d 'if able': unrezzable cards cannot be chosen"
        );
        assert!(c.candidates().contains(&ice));
    }
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
            reduce_total: Quantity::c(0),
        }],
        None,
        Some(0),
    );
    // The plan: neither player acts — the driver stops at the first decision
    // the machine puts to anyone, and fails if the game ends instead.
    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner().stopping_at_the_rest(),
    );
    assert!(t.halted, "the frame resolved into a decision, not a game over");

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

    // The plan: the Corp triggers the Reaper-class turn-begins ability from the
    // first reaction window offering it, then plays on to its action window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::reaction().once(), Reply::take("reaper")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("reaper"), "the Reaper-class turn-begins ability was triggered");
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

    // The plan: the Corp fires the ADT-class button from the first paid window
    // offering it and the driver halts as soon as that window re-offers, so the
    // install-and-rez can be read before the control arm runs; then it fires
    // the later-install button from the SAME window and plays on to the action
    // window.
    let mut g = plan::Script::new(
        Plan::corp()
            .when(Match::paid().once(), Reply::take("adt"))
            .when(Match::paid().offering("later-install").once(), Reply::Halt)
            .when(Match::paid().once(), Reply::take("later-install"))
            .stop_at_action(),
        Plan::runner(),
    );
    g.run(&mut vm);
    assert!(g.transcript().took("adt"), "the ADT-class ability was used");
    assert!(g.transcript().halted, "the paid window re-offered, rather than the game ending");

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
    g.run(&mut vm);
    assert!(g.transcript().took("later-install"), "the control-arm install was made");
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

    // The plan: the Runner runs the remote and declares every card entering the
    // breached root a candidate (10.3.1j); the Ganked-class install is the
    // CORP's optional reaction, taken the first time it is offered.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::reaction().once(), Reply::take("ganked")),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::declare_candidate(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(t.took("ganked"), "the Ganked-class install resolved mid-breach");
    let declarations = t.of_kind(Kind::DeclareCandidate);
    assert!(!declarations.is_empty(), "10.3.1j: the Runner declared candidacy");
    for d in &declarations {
        assert!(
            matches!(&d.spec, DecisionSpec::DeclareBreachCandidate { card } if *card == drafted),
            "the declaration was about the card that entered the root: {:?}",
            d.spec
        );
        assert_eq!(d.answer, Some(DecisionAnswer::ResolveOptional(true)));
    }
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
        tk::operation("HedgeFund-like", 1, vec![jinteki_cr::instr::Instruction::GainCredits(Side::Corp, Quantity::c(4))]),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hf);
    let second = vm.new_object(
        tk::operation("Second-Op", 3, vec![jinteki_cr::instr::Instruction::GainCredits(Side::Corp, Quantity::c(1))]),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(second);
    tk::install_root(&mut vm, tk::subcontract_button("Subcontract-Button", 2), ServerId::Remote(1), true);
    vm.st.corp.credits = 1;
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    // The plan: the Corp fires the Subcontract-class button from the first paid
    // window offering it and answers each one-at-a-time pick with the neutral
    // policy (the first offered candidate), stopping at the action window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("subcontract")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("subcontract"), "the Subcontract-class ability was offered and used");
    let picks: Vec<Vec<jinteki_cr::object::ObjectId>> = t
        .windows(Kind::Targets, Side::Corp)
        .iter()
        .map(|e| e.candidates().to_vec())
        .collect();
    assert!(
        t.windows(Kind::Targets, Side::Corp)
            .iter()
            .all(|e| matches!(&e.answer, Some(DecisionAnswer::Targets(v)) if v.len() == 1)),
        "each pick chose exactly one card"
    );
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
                    reduce_total: Quantity::c(0),
                },
                jinteki_cr::instr::Instruction::CreateDelayedConditional {
                    def: Box::new(jinteki_cr::ability::AbilityDef::conditional(
                        jinteki_cr::ability::TriggerCond::TurnEnds(Side::Runner),
                        vec![jinteki_cr::instr::Instruction::GainCredits(Side::Runner, Quantity::c(1))],
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

    // The plan: the Runner plays the Test-Run-class event from the first paid
    // window offering it, then plays on to the action window.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("play-event")).stop_at_action(),
    );
    assert!(t.took("play-event"), "the Test-Run-class event was played");

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
                jinteki_cr::instr::Instruction::GainCredits(Side::Runner, Quantity::c(1)),
                jinteki_cr::instr::Instruction::RemoveSelfFromGame,
            ],
        ),
        Zone::Hand(Side::Runner),
    );
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(ashen);
    tk::install_rig(&mut vm, tk::play_event_button("Play-Button", ashen));
    vm.start_turn(Side::Runner);

    // The plan: the Runner plays the Ashen-Epilogue-class event from the first
    // paid window offering it, then plays on to the action window.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("play-event")).stop_at_action(),
    );
    assert!(t.took("play-event"), "the Ashen-Epilogue-class event was played");

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

    // The plan: the Corp plays the Targeted-Marketing-class operation from the
    // first paid window offering it and the driver halts in the next Corp paid
    // window, so the play area can be read before anything is stolen; then the
    // Corp runs out its turn on basic credits and the Runner runs the remote
    // holding the agenda, stopping at its next action window.
    let mut g = plan::Script::new(
        Plan::corp()
            .when(Match::paid().once(), Reply::take("play-op"))
            .when(Match::paid().once(), Reply::Halt)
            .otherwise_click_credit(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Remote(2)))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert!(g.transcript().took("play-op"), "the operation was played");
    assert_eq!(
        vm.st.objects[&tm].zone,
        Zone::PlayArea(Side::Corp),
        "8.6.6c: not trashed until the Runner steals an agenda"
    );

    // Run out the Corp turn; the Runner steals the agenda.
    g.run(&mut vm);
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
    // The plan: the Runner runs the remote and lets the access play out; both
    // sides are otherwise neutral.
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
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
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .stop_at_action(),
    );
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

    // The plan: the Runner triggers The Supplier from the first reaction window
    // offering it, so it resolves before the Underworld-Contact instance that
    // pended alongside it; then play on to the action window.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::reaction().once(), Reply::take("supplier")).stop_at_action(),
    );
    assert!(t.took("supplier"), "The Supplier resolved first");
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
    let sectest = tk::install_rig(
        &mut vm,
        tk::breach_replacement_card(
            "SecurityTesting-like",
            "sectest: gain 2 instead of breaching",
            jinteki_cr::lingering::ReplacementTransform::SuppressAndGainCredits(2),
        ),
    );
    let siphon = tk::install_rig(
        &mut vm,
        tk::breach_replacement_card(
            "AccountSiphon-like",
            "siphon: gain 3 instead of breaching",
            jinteki_cr::lingering::ReplacementTransform::SuppressAndGainCredits(3),
        ),
    );
    tk::fill_hand(&mut vm, Side::Corp, 2);
    vm.start_turn(Side::Runner);

    // The plan: the Runner arms BOTH breach replacements before running — the
    // paid window simply re-opens after the first ability, since several paid
    // abilities may be used in one window (9.2.7f) — then runs HQ and orders
    // the two replacements Security-Testing-first.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("sectest:"))
            .when(Match::paid().once(), Reply::take("siphon:"))
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::options().once(), Reply::ChooseNamed("SecurityTesting"))
            .stop_at_action(),
    );
    assert!(t.took("sectest:") && t.took("siphon:"), "both replacements were created");
    let order = t
        .windows(Kind::Options, Side::Runner)
        .first()
        .copied()
        .expect("the order Decision was presented at imminence-open");
    assert_eq!(order.choices().len(), 2, "9.9.11: both replacements offered for ordering");
    assert_eq!(
        order.answer,
        Some(DecisionAnswer::Option(
            order.choices().iter().position(|l| l.contains("SecurityTesting")).unwrap()
        )),
        "the Runner put Security Testing first"
    );
    assert_eq!(
        vm.st.runner.credits,
        2,
        "only the chosen replacement applied; the other had nothing to replace"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
        "the breach itself was replaced"
    );
    let _ = (sectest, siphon);
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
        let sectest = tk::install_rig(
            &mut vm,
            tk::breach_replacement_card(
                "SecurityTesting-like",
                "sectest: gain 2 instead of breaching",
                jinteki_cr::lingering::ReplacementTransform::SuppressAndGainCredits(2),
            ),
        );
        let showoff = tk::install_rig(
            &mut vm,
            tk::breach_replacement_card(
                "ShowingOff-like",
                "showoff: breach from the bottom",
                jinteki_cr::lingering::ReplacementTransform::BreachFromBottom,
            ),
        );
        tk::fill_deck(&mut vm, Side::Corp, 3);
        vm.start_turn(Side::Runner);

        // The plan: arm both breach replacements in the paid windows before the
        // run (9.2.7f re-opens the window after each), run R&D, and put this
        // iteration's card first in the replacement order. `ChooseNamed` takes a
        // `&'static str`, so the plan is built inside the loop.
        let needle: &'static str =
            if pick_showing_off_first { "ShowingOff" } else { "SecurityTesting" };
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::paid().once(), Reply::take("sectest:"))
                .when(Match::paid().once(), Reply::take("showoff:"))
                .when(Match::action().first(), Reply::run(ServerId::Rnd))
                .when(Match::options().once(), Reply::ChooseNamed(needle))
                .stop_at_action(),
        );
        assert!(t.took("sectest:") && t.took("showoff:"), "both replacements were created");
        let order = t
            .windows(Kind::Options, Side::Runner)
            .first()
            .copied()
            .expect("the order Decision was presented at imminence-open");
        assert_eq!(order.choices().len(), 2, "9.9.11: both replacements offered for ordering");
        assert_eq!(
            order.answer,
            Some(DecisionAnswer::Option(
                order.choices().iter().position(|l| l.contains(needle)).unwrap()
            )),
            "this iteration ordered {needle} first"
        );
        assert_eq!(
            vm.st.runner.credits, 2,
            "either order: the Runner gains 2 and does not breach (pick_showing_off_first={pick_showing_off_first})"
        );
        assert!(
            !vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachBegan { .. })),
            "no breach either way"
        );
        let _ = (sectest, showoff);
    }
}

// ===========================================================================
// §9.12.2d vacuous truth and §6.8.5 run-ends conditions (W3d)
// ===========================================================================

/// example_rule_vacuous_truth_1 (9.12.2d): a Forked-class effect trashes
/// encountered ice whose subroutines were ALL broken. Troll has zero
/// subroutines: if its "when encountered" ability does not end the run, the
/// Runner is automatically considered to have broken all zero subroutines
/// as soon as step 6.9.3b begins — and Troll is trashed.
#[test]
fn example_rule_vacuous_truth_1() {
    let mut vm = Vm::empty(351);
    let troll = tk::install_ice(&mut vm, tk::troll_like("Troll-like"), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::forked_button("Forked-like", ServerId::Remote(1)));
    vm.start_turn(Side::Runner);

    // The plan: the Runner fires the Forked-class ability from the first paid
    // window offering it; the Corp is neutral, so it declines Troll's optional
    // end-the-run, and everything else defaults.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("forked")).stop_at_action(),
    );
    assert!(t.took("forked"), "the Forked-class ability was used");
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::AllSubsBroken { ice } if *ice == troll)),
        "9.12.2d: zero subroutines are vacuously all-broken at step 6.9.3b"
    );
    assert_eq!(
        vm.st.objects[&troll].zone,
        Zone::Discard(Side::Corp),
        "Forked trashes the fully-broken ice"
    );
}

/// example_rule_run_ends_condition_1 (6.8.5): during a Noble-Path-class
/// run, a Chum-class delayed conditional meets its condition when the
/// encounter ends (via "end the run") and resolves during the Run Ends
/// Phase — where The Noble Path's effect STILL applies, so the damage
/// fails. Durations bound to the run expire only at step 6.9.6d.
#[test]
fn example_rule_run_ends_condition_1() {
    let mut vm = Vm::empty(352);
    let wall = tk::install_ice(&mut vm, tk::etr_ice("WallOfStatic-like", 3, 3), ServerId::Remote(1), false);
    tk::install_root(&mut vm, tk::chum_like("Chum-Marker"), ServerId::Remote(2), true);
    tk::install_rig(&mut vm, tk::noble_path_like("NoblePath-like"));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    // The plan: the Runner runs the remote and raises the Noble-Path shield
    // from a paid window INSIDE the run — 9.10.4 binds a "this run" duration to
    // the run instance in progress, so used outside one it would expire at the
    // next checkpoint. In the same approach window the Corp arms the Chum-class
    // delayed conditional (its rule is first, so both fire there) and then
    // rezzes the approached ice, whose ETR subroutine ends the run.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("chum:"),
            )
            .when(Match::paid().approaching_ice(), Reply::Take(Pick::RezApproachedIce)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("noble-path:"),
            )
            .stop_at_action(),
    );
    assert!(
        t.took("noble-path:") && t.took("chum:"),
        "the shield was raised inside the run and the encounter-end damage was armed"
    );
    assert!(
        t.entries.iter().any(|e| matches!(
            &e.answer,
            Some(DecisionAnswer::Take(WindowOption::RezApproachedIce { card })) if *card == wall
        )),
        "the approached ETR ice was rezzed, so the encounter ends by ending the run"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "the run completed"
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        3,
        "6.8.5: the Run Ends Phase damage was prevented — the shield lives until 6.9.6d"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::DamageSuffered { .. })),
        "no damage was suffered during the run"
    );
}

/// example_rule_run_ends_condition_2 (6.8.5): a Dedicated-Response-Team
/// class ability meets its "run ends" condition at the same time The Noble
/// Path's run-bound effect expires (both at step 6.9.6d) — so the Runner
/// suffers the 2 meat damage.
#[test]
fn example_rule_run_ends_condition_2() {
    let mut vm = Vm::empty(353);
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 0), ServerId::Remote(1), false);
    tk::install_root(&mut vm, tk::drt_like("DRT-like"), ServerId::Remote(2), true);
    tk::install_rig(&mut vm, tk::noble_path_like("NoblePath-like"));
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.start_turn(Side::Runner);

    // The plan: the Runner runs the remote and raises the Noble-Path shield
    // from a paid window INSIDE the run, so 9.10.4 binds its "this run"
    // duration to the run in progress rather than expiring it at the next
    // checkpoint.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("noble-path:"),
            )
            .stop_at_action(),
    );
    assert!(t.took("noble-path:"), "the shield was raised inside the run");
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        2,
        "6.8.5: DRT's damage resolves after the shield expired at 6.9.6d"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::DamageSuffered { kind: DamageKind::Meat, amount: 2, .. }
        )),
        "the 2 meat damage was suffered"
    );
}

// ===========================================================================
// §7.4 — candidates (W3e): chosen-ever, access replacement/costs, R&D order
// ===========================================================================

/// example_rule_candidates_already_accessed_1 (7.4.3): during an Archives
/// breach made with an Immolation-Script-class effect, the Runner chooses a
/// piece of ice and applies a replacement to trash another card INSTEAD of
/// accessing it. The chosen ice is no longer a candidate — but the
/// newly-trashed card becomes one.
#[test]
fn example_rule_candidates_already_accessed_1() {
    let mut vm = Vm::empty(361);
    let dead_ice = vm.new_object(tk::vanilla_ice("Dead-Ice", 0, 1), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(dead_ice);
    let victim = tk::install_root(&mut vm, tk::vanilla_asset("Victim", 0, 0), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::access_replacement_card("ImmolationScript-like", victim));
    vm.start_turn(Side::Runner);

    // The plan: the Runner arms the Immolation-Script-class access replacement
    // from a paid window before the run — the effect is turn-bound, so before
    // the run is fine — then runs Archives and lets the breach play out.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("immolation"))
            .when(Match::action().first(), Reply::run(ServerId::Archives))
            .stop_at_action(),
    );
    assert!(t.took("immolation"), "the access replacement was created before the run");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == dead_ice)),
        "7.4.3: the chosen ice was never accessed and cannot be chosen again"
    );
    assert_eq!(vm.st.objects[&victim].zone, Zone::Discard(Side::Corp), "trashed instead");
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == victim)),
        "the newly-trashed card became a candidate and was accessed"
    );
}

/// example_rule_candidates_already_accessed_2 (7.4.3): a Gagarin-class
/// additional cost to access is declined — the access does not occur, but
/// the chosen card still ceases to be a candidate and the breach ends.
#[test]
fn example_rule_candidates_already_accessed_2() {
    let mut vm = Vm::empty(362);
    let bait = tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 0), ServerId::Remote(1), false);
    tk::install_root(&mut vm, tk::gagarin_like("Gagarin-like"), ServerId::Remote(2), true);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // The plan: the Runner runs the remote and declines every additional cost
    // the access puts to it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::nested_cost(), Reply::PayCost(false))
            .stop_at_action(),
    );
    let costs = t.windows(Kind::NestedCost, Side::Runner);
    assert!(!costs.is_empty(), "the additional access cost was offered and declined");
    assert!(
        costs.iter().all(|e| e.answer == Some(DecisionAnswer::PayNestedCost(false))),
        "the additional access cost was offered and declined"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAccessed { .. })),
        "no access was performed"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::BreachEnded { .. })),
        "the card ceased to be a candidate, so the breach completed"
    );
    assert_eq!(vm.st.objects[&bait].zone, Zone::Root(ServerId::Remote(1)));
}

/// example_rule_rnd_topmost_eligibile_candidate_1 (7.4.7a): a Maker's-Eye
/// class breach of R&D accesses 3 cards. After the 2nd is stolen, a
/// Bacterial-Programming-class ability "rearranges" R&D leaving the same
/// top card — the returned cards are NEW OBJECTS, so the Runner continues
/// from the top and accesses the first card AGAIN.
#[test]
fn example_rule_rnd_topmost_eligibile_candidate_1() {
    let mut vm = Vm::empty(363);
    let cg = vm.new_object(tk::corp_filler("CelebrityGift-like"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(cg);
    let bp = vm.new_object(tk::bacterial_like("Bacterial-like"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(bp);
    let c3 = vm.new_object(tk::corp_filler("Third-Card"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(c3);
    tk::install_rig(&mut vm, tk::additional_access_card("MakersEye-like", ServerId::Rnd, 2));
    vm.start_turn(Side::Runner);

    // The plan: the Runner arms the Maker's-Eye-class additional access from a
    // paid window before the run — the effect is turn-bound, so before the run
    // is fine — then runs R&D and lets the breach play out.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("makers-eye"))
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    assert!(t.took("makers-eye"), "the additional-access effect was created before the run");
    assert!(vm.st.score_area[&Side::Runner].contains(&bp), "the agenda was stolen");
    let cg_accesses = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == cg))
        .count();
    assert_eq!(
        cg_accesses, 2,
        "7.4.7a: after the rearrange, the same physical top card is a NEW object and is accessed again"
    );
    let _ = c3;
}

/// example_rule_rnd_topmost_eligibile_candidate_2 (7.4.7a): with a random
/// access limit of 4, the Runner accesses the top card (leaving it), steals
/// the 2nd (paying a Strongbox-class click), a Seidr-class ability adds an
/// Archives card to the top (3rd candidate), and the 4th candidate is the
/// card now third from the top — skipping the already-chosen ones.
#[test]
fn example_rule_rnd_topmost_eligibile_candidate_2() {
    let mut vm = Vm::empty(364);
    let c1 = vm.new_object(tk::corp_filler("First-Card"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(c1);
    let agenda2 = vm.new_object(tk::vanilla_agenda("Deck-Agenda", 3, 1), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(agenda2);
    let c3 = vm.new_object(tk::corp_filler("Third-Card"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(c3);
    let c4 = vm.new_object(tk::corp_filler("Fourth-Card"), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(c4);
    let arc = vm.new_object(tk::corp_filler("Archives-Card"), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(arc);
    tk::install_root(&mut vm, tk::strongbox_like("Strongbox-like"), ServerId::Rnd, true);
    tk::install_root(&mut vm, tk::seidr_like("Seidr-like", arc), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::additional_access_card("MakersEye-like", ServerId::Rnd, 3));
    vm.start_turn(Side::Runner);

    // The plan: the Runner arms the Maker's-Eye-class additional access from a
    // paid window before the run (turn-bound), runs R&D, and pays every
    // additional cost the breach puts to it.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("makers-eye"))
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert!(t.took("makers-eye"), "the additional-access effect was created before the run");
    let costs = t.windows(Kind::NestedCost, Side::Runner);
    assert!(!costs.is_empty(), "the additional steal cost was paid");
    for c in &costs {
        assert_eq!(
            c.cost().expect("a nested cost was put to the Runner").clicks,
            1,
            "the Strongbox-class click cost"
        );
        assert_eq!(c.answer, Some(DecisionAnswer::PayNestedCost(true)));
    }
    assert!(vm.st.score_area[&Side::Runner].contains(&agenda2));
    let accessed: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CardAccessed { obj } => Some(*obj),
            _ => None,
        })
        .collect();
    assert_eq!(
        accessed,
        vec![c1, agenda2, arc, c3],
        "7.4.7a: candidates descend past already-chosen objects; new top cards slot in"
    );
    let _ = c4;
}

// ===========================================================================
// §9.9.4c/d — mid-window installs (W3f, rides on §8.5)
// ===========================================================================

/// example_rule_trigger_conditional_ability_interrupt_2 (9.9.4c): 2 tags
/// are imminent; Decoy avoids 1, a Thunder-Art-Gallery-class chain installs
/// No One Home while the interrupt window is still open. NOH's conditional
/// interrupt is RELEVANT (1 tag still expected) — but it was not active
/// when the window opened, so it is not pending and cannot be triggered.
#[test]
fn example_rule_trigger_conditional_ability_interrupt_2() {
    let mut vm = Vm::empty(371);
    let noh = vm.new_object(tk::noh_like("NoOneHome-like"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(noh);
    tk::install_rig(&mut vm, tk::decoy_like("Decoy-like"));
    tk::install_rig(&mut vm, tk::gallery_like("Gallery-like", noh));
    tk::install_root(&mut vm, tk::corp_tags_button("Tags-Button", 2), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    // The plan: the Corp gives the 2 tags from the first paid window offering
    // it; the Runner avoids one with Decoy from the interrupt window and takes
    // the Gallery-class chain from the reaction window it opens.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("give tags")).stop_at_action(),
        Plan::runner()
            .when(Match::interrupt().once(), Reply::take("decoy"))
            .when(Match::reaction().once(), Reply::take("gallery")),
    );
    assert!(t.took("decoy") && t.took("gallery"), "the chain reaction ran mid-window");
    assert_eq!(vm.st.objects[&noh].zone, Zone::Rig, "NOH was installed mid-window");
    assert!(
        !t.ever_offered("no-one-home"),
        "9.9.4b/c: a conditional interrupt activated after the window opened is not pending"
    );
    assert_eq!(vm.st.runner.tags, 1, "only Decoy's avoidance applied");
}

/// example_rule_trigger_paid_ability_interrupt_1 (9.9.4d): the same chain
/// installs a SECOND Decoy instead. Paid-ability interrupts may be used
/// even though they were not active when the window opened — the Runner
/// avoids the other tag too.
#[test]
fn example_rule_trigger_paid_ability_interrupt_1() {
    let mut vm = Vm::empty(372);
    let decoy2 = vm.new_object(tk::decoy_like("Decoy-Two"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(decoy2);
    tk::install_rig(&mut vm, tk::decoy_like("Decoy-One"));
    tk::install_rig(&mut vm, tk::gallery_like("Gallery-like", decoy2));
    tk::install_root(&mut vm, tk::corp_tags_button("Tags-Button", 2), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    // The plan: the Corp gives the 2 tags from the first paid window offering
    // it; the Runner takes Decoy in EVERY interrupt window that offers one —
    // including the second, whose Decoy was installed after the window opened —
    // and takes the Gallery-class chain from the reaction window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("give tags")).stop_at_action(),
        Plan::runner()
            .when(Match::interrupt(), Reply::take("decoy"))
            .when(Match::reaction().once(), Reply::take("gallery")),
    );
    assert!(t.took("gallery"));
    assert_eq!(
        t.times_taken("decoy"),
        2,
        "9.9.4d: the mid-window-installed Decoy's paid interrupt may be used"
    );
    assert_eq!(vm.st.runner.tags, 0, "both tags avoided");
}

// ===========================================================================
// §9.12.2e — values defined by X (W3f)
// ===========================================================================

/// example_rule_values_defined_by_x_1 (9.12.2e): a ZATO-class effect
/// trashes Surveyor and resolves its subroutine. Surveyor is in Archives
/// when the trace initiates, so the ability defining X is inactive and the
/// base trace strength is 0.
#[test]
fn example_rule_values_defined_by_x_1() {
    let mut vm = Vm::empty(373);
    let other_ice = tk::install_ice(&mut vm, tk::vanilla_ice("Other-Ice", 0, 1), ServerId::Remote(1), true);
    let surveyor = tk::install_ice(&mut vm, tk::surveyor_like("Surveyor-like"), ServerId::Remote(1), true);
    assert_eq!(
        vm.effective_strength(surveyor),
        Some(4),
        "sanity: X = 2 x 2 ice while installed and active"
    );
    // ZATO-class: trash Surveyor, then resolve its subroutine.
    vm.trash_card(surveyor, Side::Corp);
    vm.push_ability_frame(
        jinteki_cr::frames::ResolutionKind::Subroutine,
        jinteki_cr::ability::AbilityRef { obj: surveyor, index: 1 },
        Side::Corp,
        vec![jinteki_cr::instr::Instruction::Trace {
            base: Quantity::XOfSource(Box::new(Quantity::Times(
                2,
                Box::new(Quantity::Count(
                    jinteki_cr::instr::TargetFilter::IceProtectingSourceServer,
                )),
            ))),
            if_successful: vec![jinteki_cr::instr::Instruction::GainTags(1)],
            if_unsuccessful: vec![],
            determined_min: None,
        }],
        None,
        Some(0),
    );
    // The plan: neither player acts — the driver stops at the first decision the
    // machine puts to anyone, and fails if the game ends instead.
    let t = plan::play(
        &mut vm,
        Plan::corp().stopping_at_the_rest(),
        Plan::runner().stopping_at_the_rest(),
    );
    assert!(t.halted, "the subroutine resolved into a decision, not a game over");
    let e = t.entries.last().expect("a decision was reached");
    assert_eq!(e.side, Side::Corp);
    match &e.spec {
        DecisionSpec::TraceSpend { strength_so_far, .. } => {
            assert_eq!(
                *strength_so_far, 0,
                "9.12.2e: the X-defining ability is inactive in Archives, so X = 0"
            );
        }
        other => panic!("expected the Corp trace spend, got {other:?}"),
    }
    let _ = other_ice;
}

/// example_rule_values_defined_by_x_2 (9.12.2e): Hush hosted on Surveyor
/// removes the ability defining X — Surveyor's strength is 0.
#[test]
fn example_rule_values_defined_by_x_2() {
    // The plan: neither player is asked anything — the claim is a pure reading
    // of the characteristics pipeline, made without stepping the machine, so
    // there is nothing for a plan to answer.
    let mut vm = Vm::empty(374);
    tk::install_ice(&mut vm, tk::vanilla_ice("Other-Ice", 0, 1), ServerId::Remote(1), true);
    let surveyor = tk::install_ice(&mut vm, tk::surveyor_like("Surveyor-like"), ServerId::Remote(1), true);
    assert_eq!(vm.effective_strength(surveyor), Some(4), "X = 2 x 2 without Hush");
    // Hush: hosted program removing all host abilities.
    let hush = tk::install_rig(&mut vm, tk::hush_like("Hush-like"));
    tk::host_on(&mut vm, hush, surveyor);
    assert_eq!(
        vm.effective_strength(surveyor),
        Some(0),
        "9.12.2e: the ability defining X is lost, so X = 0"
    );
}

/// example_rule_independent_effects_1 (9.12.1d/e): Mother Goddess, Ansel 1.0
/// and Warden Fatuma are rezzed with Hush hosted on Mother Goddess. Hush's
/// effect is the only independent one, so it applies first and Mother Goddess's
/// ability is gone; Warden Fatuma's effect then applies to Ansel 1.0 (a
/// bioroid) but not to Mother Goddess, which never gained the subtype.
#[test]
fn example_rule_independent_effects_1() {
    // The plan: nothing is asked of either player — the claim is a reading of
    // the characteristics pipeline and of the 9.8.2 subroutine order, both of
    // which are pure queries over board state.
    use jinteki_cr::ability::AbilityDef;
    use jinteki_cr::instr::Instruction;
    let mut vm = Vm::empty(1012);
    let mut ansel = tk::vanilla_ice("Ansel-1.0-like", 4, 4);
    ansel.subtypes = vec!["bioroid", "sentry"];
    ansel.abilities = vec![AbilityDef::subroutine(vec![Instruction::Damage {
        kind: DamageKind::Net,
        amount: Quantity::c(1),
        responsible: Side::Corp,
    }])
    .labeled("[sub] ansel printed")];
    let ansel = tk::install_ice(&mut vm, ansel, ServerId::Remote(1), true);
    let mg = tk::install_ice(
        &mut vm,
        tk::mother_goddess_like("Mother-Goddess-like"),
        ServerId::Remote(1),
        true,
    );
    let fatuma = tk::install_ice(
        &mut vm,
        tk::warden_fatuma_like(
            "Warden-Fatuma-like",
            "bioroid",
            AbilityDef::subroutine(vec![Instruction::LoseCredits(Side::Runner, 1)])
                .labeled("[sub] the runner loses a credit"),
        ),
        ServerId::Remote(2),
        true,
    );

    // Without Hush, Mother Goddess's own effect is independent and gives it
    // every other rezzed ice's subtypes — including bioroid, which is what
    // makes Warden Fatuma's effect depend on it.
    assert!(vm.has_subtype(mg, "bioroid"), "9.12.1d: Mother Goddess gains Ansel's subtypes");
    assert_eq!(
        vm.current_subs(mg).len(),
        1,
        "and so Warden Fatuma's effect applies to it: one granted subroutine"
    );

    // Hush hosted on Mother Goddess: its effect depends on nothing, so it is
    // applied first and Mother Goddess's ability no longer exists.
    let hush = tk::install_rig(&mut vm, tk::hush_like("Hush-like"));
    tk::host_on(&mut vm, hush, mg);

    assert!(
        !vm.has_subtype(mg, "bioroid"),
        "9.12.1e: Hush is applied first, so Mother Goddess's effect never applies"
    );
    let mg_subs = vm.current_subs(mg);
    assert!(
        mg_subs.is_empty(),
        "Warden Fatuma's effect grants nothing to a non-bioroid: {mg_subs:?}"
    );
    // Ansel 1.0 still gains it, and 9.8.3a puts an external "before" grant
    // ahead of the printed subroutines.
    let ansel_subs = vm.current_subs(ansel);
    assert_eq!(ansel_subs.len(), 2, "the grant plus Ansel's printed subroutine");
    assert_eq!(ansel_subs[0].1.label, "[sub] the runner loses a credit");
    assert_eq!(ansel_subs[1].1.label, "[sub] ansel printed");
    // Warden Fatuma does not grant to itself — the effect names OTHER ice.
    assert!(vm.current_subs(fatuma).is_empty());
}

/// example_rule_independent_effects_2 (9.12.1e): Hush hosted on Magnet. Each
/// effect removes the other's source's abilities, so the dependencies form a
/// loop; the hosted object's effect ignores its dependence on its host's, is
/// applied first, and Magnet's effect is never applied.
#[test]
fn example_rule_independent_effects_2() {
    // The plan: no decisions — the claim is a pure reading of the pipeline.
    let mut vm = Vm::empty(1013);
    let magnet = tk::install_ice(&mut vm, tk::magnet_like("Magnet-like"), ServerId::Remote(1), true);
    let hush = tk::install_rig(&mut vm, tk::hush_like("Hush-like"));
    tk::host_on(&mut vm, hush, magnet);

    assert!(
        !vm.ability_present(magnet, 0),
        "9.12.1e: Hush's effect is treated as independent and applied first"
    );
    assert!(
        vm.ability_present(hush, 0),
        "Magnet's ability no longer exists, so its effect is never applied"
    );
}

// ===========================================================================
// §9.10.3 — maintained choices
// ===========================================================================

/// example_rule_lingering_effect_maintaining_choice_default_duration_1
/// (9.10.3a): a choice referred to only by another lingering effect is
/// maintained for that effect's duration — the chosen subtype and the effect
/// adding it to the encountered ice both expire at the end of the encounter.
#[test]
fn example_rule_lingering_effect_maintaining_choice_default_duration_1() {
    use jinteki_cr::lingering::ChoiceValue;
    let mut vm = Vm::empty(1301);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Plain-Ice", 0, 1), ServerId::Hq, true);
    let pelangi = tk::install_rig(&mut vm, tk::pelangi_like("Pelangi-like", &["barrier", "sentry"]));
    tk::install_rig(&mut vm, tk::break_button("Break-button"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // The plan: run HQ, use Pelangi in the encounter's paid window choosing
    // "sentry", then halt in the NEXT paid window of that encounter so the
    // effect can be read while the encounter still lasts.
    let mut g = plan::Script::new(
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("pelangi"))
            .when(Match::options().once(), Reply::ChooseNamed("sentry"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert!(vm.st.encounter.is_some(), "the readings below are taken mid-encounter");
    assert_eq!(
        vm.maintained_choice(pelangi, "pelangi-subtype"),
        Some(ChoiceValue::Subtype("sentry")),
        "9.10.3: the choice is maintained by a lingering effect"
    );
    assert!(vm.has_subtype(ice, "sentry"), "and the effect it feeds is applying");

    g.run(&mut vm);
    assert!(vm.st.encounter.is_none(), "the encounter is over");
    assert_eq!(
        vm.maintained_choice(pelangi, "pelangi-subtype"),
        None,
        "9.10.3a: the choice expires with the effect that referred to it"
    );
    assert!(!vm.has_subtype(ice, "sentry"), "and so does the subtype it granted");
}

/// example_rule_lingering_effect_maintaining_choice_turn_begins_duration_1
/// (9.10.3b): a "when your turn begins" ability whose only effect is a choice
/// maintains that choice until the turn ends. The second ability always looks
/// for the server chosen THIS turn — a successful run on any other server
/// never meets its condition, and neither does one when no server was chosen.
#[test]
fn example_rule_lingering_effect_maintaining_choice_turn_begins_duration_1() {
    use jinteki_cr::lingering::ChoiceValue;
    let mut vm = Vm::empty(1304);
    let st = tk::install_rig(
        &mut vm,
        tk::security_testing_choice_like("SecTest-like", &[ServerId::Hq, ServerId::Rnd]),
    );
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    // The plan: choose HQ at turn begin, then run R&D — the server that was
    // NOT chosen.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::reaction().once(), Reply::take("sectest"))
            .when(Match::options().once(), Reply::ChooseNamed("Hq"))
            .when(Match::action().first(), Reply::run(ServerId::Rnd))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(t.halted);
    assert_eq!(
        vm.maintained_choice(st, "sectest-server"),
        Some(ChoiceValue::Server(ServerId::Hq)),
        "the choice made this turn is maintained"
    );
    assert_eq!(
        vm.st.runner.credits, 0,
        "9.10.3b: a successful run on a server that was not chosen never meets the condition"
    );

    // The turn ends: the choice expires with it, so nothing is remembered.
    vm.start_turn(Side::Corp);
    vm.checkpoint_and_react(None);
    assert_eq!(
        vm.maintained_choice(st, "sectest-server"),
        None,
        "9.10.3b: the lingering effect maintaining the choice expires when the turn ends"
    );
}

/// example_rule_lingering_effect_maintaining_choice_duration_other_cases_1
/// (9.10.3c): a choice that neither 9.10.3a nor 9.10.3b covers is maintained
/// for as long as its source is active — across encounters and turns — and
/// expires when the source stops being active.
#[test]
fn example_rule_lingering_effect_maintaining_choice_duration_other_cases_1() {
    use jinteki_cr::lingering::ChoiceValue;
    let mut vm = Vm::empty(1302);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Chosen-Ice", 0, 1), ServerId::Remote(1), true);
    let femme = tk::install_rig(&mut vm, tk::femme_choice_like("Femme-like"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // The plan: the turn-begins conditional makes the choice; stop at the
    // first action window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner().when(Match::optional(), Reply::Optional(true)).stop_at_action(),
    );
    assert!(t.halted);
    assert_eq!(
        vm.maintained_choice(femme, "femme-ice"),
        Some(ChoiceValue::Object(ice)),
        "the ice the Runner chose is being remembered"
    );

    // The turn ends and the next begins: a `WhileSourceActive` choice does
    // not care (9.10.3c), unlike the 9.10.3b turn-scoped case.
    vm.start_turn(Side::Corp);
    assert_eq!(
        vm.maintained_choice(femme, "femme-ice"),
        Some(ChoiceValue::Object(ice)),
        "9.10.3c: the choice outlives the turn it was made in"
    );

    // The source stops being active: the lingering effect expires at the next
    // checkpoint and the choice is gone.
    vm.trash_card(femme, Side::Corp);
    vm.checkpoint_and_react(None);
    assert_eq!(
        vm.maintained_choice(femme, "femme-ice"),
        None,
        "9.10.3c: it expires when the source becomes inactive"
    );
}

/// example_rule_object_move_known_location_1 (1.12.4): a card moved to another
/// known location WITHIN the play area does not become a new object, so a
/// maintained choice naming it still names it — the Runner can still use the
/// remembering ability on the ice a Thimblerig-class swap moved.
#[test]
fn example_rule_object_move_known_location_1() {
    use jinteki_cr::lingering::ChoiceValue;
    let mut vm = Vm::empty(1303);
    let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Chosen-Ice", 0, 1), ServerId::Remote(1), true);
    let thimble =
        tk::install_ice(&mut vm, tk::thimblerig_like("Thimblerig-like"), ServerId::Remote(2), true);
    let femme = tk::install_rig(&mut vm, tk::femme_choice_like("Femme-like"));
    vm.st.runner.credits = 5;
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let gen_before = vm.st.objects[&ice].generation;
    // The plan: the turn-begins conditional makes the Runner's choice, then
    // the Corp swaps the chosen ice into another position from the paid
    // window before the Runner's first action (8.8 / 6.2.2f).
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("thimblerig"))
            .when(Match::targets().once(), Reply::Targets(vec![ice]))
            .when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::targets().once(), Reply::Targets(vec![ice]))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(t.took("thimblerig"), "the ice was moved: {}", t.tail(8));
    assert_ne!(vm.st.objects[&ice].zone, Zone::Ice(ServerId::Remote(1)), "it moved");
    assert_eq!(
        vm.st.objects[&ice].generation, gen_before,
        "1.12.4: it never left the play area, so it is the same object"
    );
    assert_eq!(
        vm.maintained_choice(femme, "femme-ice"),
        Some(ChoiceValue::Object(ice)),
        "so the maintained choice still names it"
    );
    let _ = thimble;
}

// ===========================================================================
// §9.12.3 — "must"
// ===========================================================================

/// example_rule_must_with_choice_1 (9.12.3a): a "must" that does not stipulate
/// how forces the Runner to make any decision that satisfies it, including
/// using another card's ability. With 4 credits, an Imp and a Scrubber, the
/// Runner cannot pass the mid-access window: they must spend the virus counter
/// or pay the trash cost with the Scrubber's credits.
#[test]
fn example_rule_must_with_choice_1() {
    let mut vm = Vm::empty(1201);
    let mvt = tk::install_root(
        &mut vm,
        tk::must_trash_accessed_like("MVT-like", 5),
        ServerId::Remote(1),
        true,
    );
    let imp = tk::install_rig(&mut vm, tk::imp_like("Imp-like"));
    tk::place_counters(&mut vm, imp, CounterKind::Virus, 1);
    let scrubber = tk::install_rig(&mut vm, tk::scrubber_like("Scrubber-like", 2));
    tk::place_counters(&mut vm, scrubber, CounterKind::Credit, 2);
    vm.st.runner.credits = 4;
    vm.start_turn(Side::Runner);

    // The plan: run the remote, then HALT at the mid-access window so the
    // offer itself can be inspected.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::Halt)
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    let e = t.entries.last().expect("the mid-access window was reached");
    match &e.spec {
        DecisionSpec::MidAccessWindow { options, can_pass } => {
            assert!(!can_pass, "9.12.3a: the 'must' leaves the Runner no pass");
            assert!(
                options.iter().any(|o| matches!(o, WindowOption::BasicTrash { card, .. } if *card == mvt)),
                "1.10.3c: 4 credits plus the Scrubber's 2 pay the trash cost of 5: {options:?}"
            );
            assert!(
                options.iter().any(|o| matches!(o, WindowOption::TriggerPaid { label, .. } if label.contains("imp"))),
                "and the Imp is the other way to satisfy it: {options:?}"
            );
        }
        other => panic!("expected the mid-access window, got {other:?}"),
    }
}

/// example_rule_must_without_choice_1 (9.12.3b): a "must" that stipulates the
/// means ("if you can pay its trash cost") cannot be satisfied any other way.
/// With the trash cost unaffordable, the Runner is not required to spend a
/// counter from Imp, even though Imp could trash the card.
#[test]
fn example_rule_must_without_choice_1() {
    let mut vm = Vm::empty(1202);
    let target =
        tk::install_root(&mut vm, tk::vanilla_asset("Expensive-Asset", 0, 5), ServerId::Remote(1), true);
    let imp = tk::install_rig(&mut vm, tk::imp_like("Imp-like"));
    tk::place_counters(&mut vm, imp, CounterKind::Virus, 1);
    tk::install_rig(&mut vm, tk::must_trash_by_paying_like("NAT-like"));
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::Halt)
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    let e = t.entries.last().expect("the mid-access window was reached");
    match &e.spec {
        DecisionSpec::MidAccessWindow { options, can_pass } => {
            assert!(
                !options.iter().any(|o| matches!(o, WindowOption::BasicTrash { .. })),
                "2 credits cannot pay a trash cost of 5: {options:?}"
            );
            assert!(
                options.iter().any(|o| matches!(o, WindowOption::TriggerPaid { label, .. } if label.contains("imp"))),
                "the Imp is still usable — it is simply not required"
            );
            assert!(
                *can_pass,
                "9.12.3b: the means was stipulated, so Imp cannot be compelled"
            );
        }
        other => panic!("expected the mid-access window, got {other:?}"),
    }
    assert_eq!(vm.st.objects[&imp].counter(CounterKind::Virus), 1, "no counter was spent");
    assert_eq!(vm.st.objects[&target].zone, Zone::Root(ServerId::Remote(1)));
}

/// example_rule_forced_mid_access_ability_optional_1 (9.5.3a): a paid ability
/// is optional even when a "must" is in force, so a prohibition on using it
/// removes it from what the "must" can compel. With Imp prohibited for the run
/// and the trash cost unaffordable, nothing compels the Runner at all.
#[test]
fn example_rule_forced_mid_access_ability_optional_1() {
    let mut vm = Vm::empty(1203);
    let mvt = tk::install_root(
        &mut vm,
        tk::must_trash_accessed_like("MVT-like", 5),
        ServerId::Remote(1),
        true,
    );
    let imp = tk::install_rig(&mut vm, tk::imp_like("Imp-like"));
    tk::place_counters(&mut vm, imp, CounterKind::Virus, 1);
    tk::install_ice(
        &mut vm,
        tk::use_prohibition_ice("Wendigo-like", imp),
        ServerId::Remote(1),
        true,
    );
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);

    // The plan: run the remote, halting at the jack-out decision — which comes
    // after the subroutine has resolved and while the run, and therefore the
    // prohibition, is still in progress — then resume into the access.
    let mut g = plan::Script::new(
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::jack_out().once(), Reply::Halt)
            .when(Match::jack_out(), Reply::JackOut(false))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    g.run(&mut vm);
    assert!(
        vm.ability_use_prohibited(imp),
        "9.5.3a: the subroutine's prohibition is in force for the run"
    );
    g.run(&mut vm);
    let t = g.transcript();
    assert!(
        t.of_kind(Kind::MidAccess).is_empty(),
        "9.5.3a/9.12.3a: nothing the Runner could be compelled to use was on offer"
    );
    assert_eq!(vm.st.objects[&imp].counter(CounterKind::Virus), 1, "no counter was spent");
    assert_eq!(vm.st.objects[&mvt].zone, Zone::Root(ServerId::Remote(1)), "and nothing is trashed");
}

// ===========================================================================
// §4.6.6i / §4.6.8f — "this server", and limits on remote servers
// ===========================================================================

/// example_rule_this_server_1 (4.6.6i): the Runner trashes a rezzed Warroid
/// Tracker. By the time the checkpoint pends its instance the card is in
/// Archives, but "this server" in the condition — and in the effect — still
/// names the server it was trashed from.
#[test]
fn example_rule_this_server_1() {
    let mut vm = Vm::empty(1101);
    // Two ice protect the remote and one protects Archives, so each candidate
    // reading of "this server" gives a different number of credits.
    tk::install_ice(&mut vm, tk::vanilla_ice("Remote-Ice-A", 0, 1), ServerId::Remote(1), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Remote-Ice-B", 0, 1), ServerId::Remote(1), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Archives-Ice", 0, 1), ServerId::Archives, false);
    let warroid =
        tk::install_root(&mut vm, tk::warroid_this_server_like("Warroid-like", 2), ServerId::Remote(1), true);
    vm.st.runner.credits = 5;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    // The plan: run the remote, pass the unrezzed ice, trash the asset on
    // access; every optional part either side is offered is resolved.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::trash_accessed())
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(
        t.entries.iter().any(|e| matches!(
            &e.answer,
            Some(DecisionAnswer::Take(WindowOption::BasicTrash { card, .. })) if *card == warroid
        )),
        "the Runner trashed the asset"
    );
    assert_eq!(
        vm.st.objects[&warroid].zone,
        Zone::Discard(Side::Corp),
        "the source is in Archives when the ability resolves"
    );
    assert_eq!(
        vm.this_server(warroid),
        Some(ServerId::Remote(1)),
        "4.6.6i: 'this server' is the server it was trashed FROM, not Archives"
    );
    assert_eq!(
        vm.st.corp.credits, 2,
        "the effect counted the 2 ice protecting the remote, not the 1 protecting Archives"
    );
}

/// example_rule_this_server_2 (4.6.6i): an ability initiated by a COST that
/// trashes its own source out of the server it was protecting still reads
/// "this server" as that server — and the source, no longer protecting it, is
/// not among the ice counted.
#[test]
fn example_rule_this_server_2() {
    let mut vm = Vm::empty(1102);
    tk::install_ice(&mut vm, tk::vanilla_ice("Other-Ice", 0, 1), ServerId::Remote(1), false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Archives-Ice-A", 0, 1), ServerId::Archives, false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Archives-Ice-B", 0, 1), ServerId::Archives, false);
    tk::install_ice(&mut vm, tk::vanilla_ice("Archives-Ice-C", 0, 1), ServerId::Archives, false);
    let bc =
        tk::install_ice(&mut vm, tk::border_control_like("Border-Control-like"), ServerId::Remote(1), true);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    // The plan: the Corp uses the [trash] ability in its first paid window.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().first(), Reply::take("border control")).stop_at_action(),
        Plan::runner().stopping_at_the_rest(),
    );
    assert!(t.ever_offered("border control"), "the [trash] ability was offered");
    assert_eq!(
        vm.st.objects[&bc].zone,
        Zone::Discard(Side::Corp),
        "the source was trashed to pay the cost"
    );
    assert_eq!(
        vm.this_server(bc),
        Some(ServerId::Remote(1)),
        "4.6.6i: 'this server' is the server the source was protecting"
    );
    assert_eq!(
        vm.st.corp.credits, 1,
        "1 ice still protects that server — the trashed source is not counted"
    );
}

/// example_rule_previous_object_source_1 (1.12.6a): the source of a persistent
/// ability is the object that was trashed during access, so "this server" in
/// that ability still names the attacked server even though the card is now a
/// new object in Archives.
#[test]
fn example_rule_previous_object_source_1() {
    let mut vm = Vm::empty(1103);
    let amaze = tk::install_root(
        &mut vm,
        tk::amaze_persistent_like("AMAZE-like"),
        ServerId::Remote(1),
        true,
    );
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // The plan: run the remote and trash the upgrade on access; the persisting
    // ability then resolves out of the run's own ending.
    let gen_before = vm.st.objects[&amaze].generation;
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Remote(1)))
            .when(Match::mid_access().once(), Reply::trash_accessed())
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert!(t.halted);
    assert!(
        vm.st.objects[&amaze].generation > gen_before,
        "1.12.3: the trashed card is a NEW object in Archives"
    );
    assert_eq!(
        vm.this_server(amaze),
        Some(ServerId::Remote(1)),
        "1.12.6a/4.6.6i: the persistent ability's source still names the attacked server"
    );
    assert_eq!(vm.st.runner.tags, 2, "so the ability resolved when the run ended");
}

/// example_rule_limit_remote_servers_1 (4.6.8f): while "Limit 1 remote server"
/// is active and a remote exists, the Corp cannot create a new remote server —
/// an install naming one has no identifiable destination (8.5.14).
#[test]
fn example_rule_limit_remote_servers_1() {
    let mut vm = Vm::empty(1104);
    tk::install_identity(&mut vm, tk::remote_limit_like("Earth-Station-like", 1), Side::Corp);
    assert!(vm.can_create_new_remote(), "no remote exists yet");
    tk::install_root(&mut vm, tk::corp_filler("Asset-A"), ServerId::Remote(1), false);
    assert_eq!(vm.remote_servers().len(), 1);
    assert!(
        !vm.can_create_new_remote(),
        "4.6.8f: a remote already exists, so the limit forbids creating another"
    );

    // The install effect still runs; its destination just cannot be identified.
    let hand = vm.new_object(tk::vanilla_asset("Asset-B", 0, 3), Zone::Hand(Side::Corp));
    tk::install_root(&mut vm, tk::adt_button("Installer", hand), ServerId::Hq, true);
    vm.start_turn(Side::Corp);
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().first(), Reply::take("adt")).stop_at_action(),
        Plan::runner().stopping_at_the_rest(),
    );
    assert!(t.ever_offered("adt"), "the installing ability was used");
    assert_eq!(
        vm.st.objects[&hand].zone,
        Zone::Hand(Side::Corp),
        "8.5.14: no destination could be identified, so no installation took place"
    );
    assert_eq!(vm.remote_servers().len(), 1, "and no second remote server exists");
}

// ===========================================================================
// §10.1.5 — old self-reference rules
// ===========================================================================

/// example_sec_old_self_reference_rules_1 (10.1.5): a card naming itself in its
/// own text means "this card" — the original Kitsune's "trash Kitsune" is
/// `TrashSelf`, and it trashes the ice the subroutine is on, not another copy.
#[test]
fn example_sec_old_self_reference_rules_1() {
    use jinteki_cr::ability::AbilityDef;
    use jinteki_cr::instr::Instruction;
    let mut vm = Vm::empty(1105);
    let mut kitsune = tk::vanilla_ice("Kitsune-like", 0, 1);
    kitsune.abilities =
        vec![AbilityDef::subroutine(vec![Instruction::TrashSelf]).labeled("[sub] trash Kitsune")];
    let other = tk::install_ice(&mut vm, kitsune.clone(), ServerId::Remote(1), true);
    let encountered = tk::install_ice(&mut vm, kitsune, ServerId::Hq, true);
    vm.start_turn(Side::Runner);

    // The plan: run HQ, meet the ice, break nothing; the subroutine resolves.
    let _t = plan::play(
        &mut vm,
        Plan::corp().when(Match::optional(), Reply::Optional(true)),
        Plan::runner()
            .when(Match::action().first(), Reply::run(ServerId::Hq))
            .when(Match::optional(), Reply::Optional(true))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&encountered].zone,
        Zone::Discard(Side::Corp),
        "10.1.5: 'trash Kitsune' means 'trash this ice'"
    );
    assert_eq!(
        vm.st.objects[&other].zone,
        Zone::Ice(ServerId::Remote(1)),
        "the other copy of the same card is untouched"
    );
}

/// example_sec_old_self_reference_rules_2 (10.1.5): "a copy of Boomerang" is
/// NOT self-referential — it describes any card with that name, so the other
/// copy is a valid target for the ability naming it.
#[test]
fn example_sec_old_self_reference_rules_2() {
    use jinteki_cr::instr::TargetFilter;
    let mut vm = Vm::empty(1106);
    let a = tk::install_rig(&mut vm, tk::vanilla_runner_card("Boomerang-like", jinteki_cr::object::CardType::Hardware));
    let b = tk::install_rig(&mut vm, tk::vanilla_runner_card("Boomerang-like", jinteki_cr::object::CardType::Hardware));
    let unrelated =
        tk::install_rig(&mut vm, tk::vanilla_runner_card("Other-Card", jinteki_cr::object::CardType::Hardware));

    // "a copy of Boomerang", read from the source `a`, describes both copies.
    let named = vm.candidates_matching(&[TargetFilter::HasName("Boomerang-like")], Some(a));
    assert!(named.contains(&a) && named.contains(&b), "both copies match the name: {named:?}");
    assert!(!named.contains(&unrelated));
    // "this card" (self-reference) describes only the source, which is what
    // 10.1.5's other reading covers.
    let others = vm.candidates_matching(
        &[TargetFilter::HasName("Boomerang-like"), TargetFilter::OtherThanSource],
        Some(a),
    );
    assert_eq!(others, vec![b], "and 'another copy' excludes the source itself");
}

// ===========================================================================
// §8.7 — searching, finding and shuffling
// ===========================================================================

/// Index of the first change matching a predicate, for the ordering claims
/// §8.7.3 makes ("the shuffle takes precedence over…").
fn change_at(vm: &Vm, pred: impl Fn(&GameChange) -> bool) -> usize {
    vm.changes
        .log
        .iter()
        .position(pred)
        .unwrap_or_else(|| panic!("expected change not in the log:\n{:?}", vm.changes.log))
}

/// example_rule_valid_search_target_install_play_1 (8.7.2b): the Runner uses
/// an Artist-Colony-class ability to search their stack for a card and
/// install it. They cannot find an event — events are never installed — nor a
/// program/hardware/resource whose install cost they cannot afford.
#[test]
fn example_rule_valid_search_target_install_play_1() {
    let mut vm = Vm::empty(381);
    // The stack: one event, one program they can afford, one they cannot.
    let event = vm.new_object(tk::runner_filler("Stack-Event"), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(event);
    let cheap = vm.new_object(tk::program_cost("Cheap-Program", 1), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(cheap);
    let dear = vm.new_object(tk::program_cost("Dear-Program", 9), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(dear);
    let resource = vm.new_object(
        tk::vanilla_runner_card("Dear-Resource", jinteki_cr::object::CardType::Resource),
        Zone::Deck(Side::Runner),
    );
    vm.st.objects.get_mut(&resource).unwrap().printed.cost = Some(7);
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(resource);
    tk::install_rig(&mut vm, tk::artist_colony_like("ArtistColony-like"));
    vm.st.runner.credits = 3;
    vm.start_turn(Side::Runner);

    // The plan: fire the Artist-Colony-class ability and find the affordable
    // program at the one find choice it produces.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("artist-colony"))
            .when(Match::targets().once(), Reply::target(cheap))
            .stop_at_action(),
    );
    assert!(t.took("artist-colony"), "the search ability was offered and used");
    let find = t.first_window(Kind::Targets, Side::Runner);
    assert!(
        !find.candidates().contains(&event),
        "8.7.2b: an event can never be found by a search followed by an install"
    );
    assert!(
        !find.candidates().contains(&dear),
        "8.7.2b: a program the Runner cannot afford to install cannot be found"
    );
    assert!(
        !find.candidates().contains(&resource),
        "8.7.2b: nor a resource they cannot afford to install"
    );
    assert_eq!(
        find.candidates(),
        &[cheap],
        "only the affordable installable card is a valid find"
    );
    assert_eq!(vm.st.objects[&cheap].zone, Zone::Rig, "the found card was installed");
    assert_eq!(vm.st.runner.credits, 2, "its install cost was paid");
    assert_eq!(vm.st.objects[&event].zone, Zone::Deck(Side::Runner));
}

/// example_rule_valid_search_target_install_play_2 (8.7.2b): the Runner uses
/// Self-modifying Code with no credits left but an installed Patchwork. Imp
/// can be found and installed — they must use Patchwork, trashing a card from
/// their grip. With an empty grip Patchwork is unusable, so Imp cannot be
/// found at all.
#[test]
fn example_rule_valid_search_target_install_play_2() {
    // One scenario builder, run twice: with a card in the grip and without.
    // (Two machines, no driver loop — the plan answers each in one fold.)
    fn scenario(grip: usize) -> (Vm, plan::Transcript, jinteki_cr::ObjectId) {
        let mut vm = Vm::empty(382);
        let imp = vm.new_object(tk::virus_program("Imp-like", 2), Zone::Deck(Side::Runner));
        vm.st.deck.get_mut(&Side::Runner).unwrap().push(imp);
        tk::fill_deck(&mut vm, Side::Runner, 3);
        tk::install_rig(&mut vm, tk::patchwork_like("Patchwork-like"));
        tk::install_rig(&mut vm, tk::smc_like("SMC-like"));
        tk::fill_hand(&mut vm, Side::Runner, grip);
        // 2 credits: exactly Self-modifying Code's trigger cost, so the
        // Runner has none left when the search resolves.
        vm.st.runner.credits = 2;
        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::paid().once(), Reply::take("smc"))
                .when(Match::targets().once(), Reply::target(imp))
                .stop_at_action(),
        );
        (vm, t, imp)
    }

    // With a card in the grip: Imp is a valid find, and installing it uses
    // Patchwork — the grip card is trashed and Imp costs nothing.
    let (vm, t, imp) = scenario(1);
    assert!(t.took("smc"), "the search ability was offered and used");
    assert_eq!(vm.st.runner.credits, 0, "no credits left after SMC's trigger cost");
    let find = t.first_window(Kind::Targets, Side::Runner);
    assert!(
        find.candidates().contains(&imp),
        "8.7.2b: affordability counts the cost-reducing ability the Runner would have to use"
    );
    assert_eq!(vm.st.objects[&imp].zone, Zone::Rig, "Imp was installed");
    assert!(
        vm.st.hand[&Side::Runner].is_empty(),
        "Patchwork's own cost — 1 card from the grip — was paid"
    );
    assert_eq!(vm.st.discard[&Side::Runner].len(), 2, "the grip card and SMC itself");

    // With an empty grip Patchwork cannot be used, so the 2-credit install is
    // unaffordable and Imp is not a valid find at all.
    let (vm2, t2, imp2) = scenario(0);
    assert!(t2.took("smc"));
    assert!(
        t2.windows(Kind::Targets, Side::Runner).is_empty(),
        "8.7.2b: with nothing to trash for Patchwork, Imp is not a valid find — \
         the Runner looks through the stack and is offered nothing to find"
    );
    assert_eq!(
        vm2.st.objects[&imp2].zone,
        Zone::Deck(Side::Runner),
        "Imp stayed in the stack"
    );
    assert!(
        vm2.changes.log.iter().any(|c| matches!(c, GameChange::DeckShuffled { side } if *side == Side::Runner)),
        "8.7.3: the stack is reshuffled whether or not anything was found"
    );
}

/// example_rule_valid_search_target_install_play_3 (8.7.2b): the Corp uses a
/// Tucana-class ability to search R&D for a piece of ice while holding 0
/// credits. Pharos is a valid find — installability is what 8.7.2b tests —
/// and although it cannot be rezzed, 8.5.13d makes the Corp reveal it.
#[test]
fn example_rule_valid_search_target_install_play_3() {
    let mut vm = Vm::empty(383);
    let pharos = vm.new_object(tk::vanilla_ice("Pharos-like", 5, 5), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(pharos);
    tk::fill_deck(&mut vm, Side::Corp, 4);
    tk::install_root(
        &mut vm,
        tk::tucana_like("Tucana-like", ServerId::Remote(1)),
        ServerId::Remote(1),
        true,
    );
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    // The plan: fire the Tucana-class ability and find Pharos.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("tucana"))
            .when(Match::targets().once(), Reply::target(pharos))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("tucana"), "the search ability was offered and used");
    let find = t.first_window(Kind::Targets, Side::Corp);
    assert!(
        find.candidates().contains(&pharos),
        "8.7.2b: a card that can be installed but not rezzed is still a valid find"
    );
    assert_eq!(
        vm.st.objects[&pharos].zone,
        Zone::Ice(ServerId::Remote(1)),
        "Pharos was installed"
    );
    assert!(!vm.st.objects[&pharos].faceup, "0 credits: it could not be rezzed");
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::CardRevealed { obj } if *obj == pharos))
            .count(),
        1,
        "8.5.13d: the Corp must reveal the card it is unable to rez"
    );
}

/// example_rule_shuffle_deck_after_search_1 (8.7.3): the Corp, playing a
/// Near-Earth-Hub-class identity, uses a Tech-Startup-class ability to search
/// R&D for an asset and install it. R&D is reshuffled the instant the search
/// completes — before the install resolves and before the identity's
/// conditional ability triggers.
#[test]
fn example_rule_shuffle_deck_after_search_1() {
    let mut vm = Vm::empty(384);
    tk::install_identity(&mut vm, tk::near_earth_hub_like("NEH-like"), Side::Corp);
    let asset = vm.new_object(tk::vanilla_asset("Deck-Asset", 0, 3), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(asset);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    tk::install_root(
        &mut vm,
        tk::tech_startup_like("TechStartup-like"),
        ServerId::Remote(1),
        true,
    );
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    // The plan: fire the Tech-Startup-class ability, find the asset, and let
    // the identity's conditional resolve wherever it is offered.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("tech-startup"))
            .when(Match::targets().once(), Reply::target(asset))
            .when(Match::reaction(), Reply::take("neh"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("tech-startup"), "the search ability was offered and used");
    assert!(t.took("neh"), "the identity's conditional ability resolved");
    let shuffled = change_at(&vm, |c| {
        matches!(c, GameChange::DeckShuffled { side } if *side == Side::Corp)
    });
    let installed = change_at(&vm, |c| {
        matches!(c, GameChange::CardInstalled { obj, .. } if *obj == asset)
    });
    let drew = change_at(&vm, |c| matches!(c, GameChange::CardDrawn { side, .. } if *side == Side::Corp));
    assert!(
        shuffled < installed,
        "8.7.3: R&D is shuffled before the found asset is installed"
    );
    assert!(
        installed < drew,
        "the identity's draw is the chain reaction the install set off"
    );
    assert!(
        shuffled < drew,
        "8.7.3: the shuffle takes precedence over the chain reaction the search's \
         install sets off — Near-Earth Hub's draw comes after it"
    );
    assert!(
        matches!(vm.st.objects[&asset].zone, Zone::Root(ServerId::Remote(_))),
        "the asset was installed after the shuffle"
    );
}

/// example_rule_search_instruction_1 (9.11.4d): the Runner uses a Djinn-class
/// ability — one printed sentence that searches and then acts on the found
/// card — while the Corp has Personality Profiles in their score area. The
/// search ENDS an instruction: the found Datasucker is set aside, the stack is
/// shuffled, and the Corp's "whenever the Runner searches" ability resolves in
/// a reaction window BEFORE the next instruction adds Datasucker to the grip.
#[test]
fn example_rule_search_instruction_1() {
    let mut vm = Vm::empty(385);
    tk::put_in_score_area(
        &mut vm,
        tk::personality_profiles_like("PersonalityProfiles-like", 1),
        Side::Corp,
    );
    let datasucker = vm.new_object(tk::virus_program("Datasucker-like", 1), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(datasucker);
    tk::fill_deck(&mut vm, Side::Runner, 4);
    tk::install_rig(&mut vm, tk::djinn_like("Djinn-like"));
    let grip = tk::fill_hand(&mut vm, Side::Runner, 1);
    vm.st.runner.credits = 2;
    vm.start_turn(Side::Runner);

    // The plan: the Runner fires Djinn and finds Datasucker; the Corp resolves
    // its mandatory ability wherever it is offered.
    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::reaction(), Reply::take("personality-profiles")),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("djinn"))
            .when(Match::targets().once(), Reply::target(datasucker))
            .stop_at_action(),
    );
    assert!(t.took("djinn"), "the search ability was offered and used");
    let reaction = t
        .windows(Kind::Reaction, Side::Corp)
        .into_iter()
        .find(|e| e.offered("personality-profiles"))
        .expect("8.7.5: the search-condition ability pends in a reaction window");
    assert!(
        reaction.took("personality-profiles"),
        "the Corp resolved it from that window"
    );
    let shuffled = change_at(&vm, |c| {
        matches!(c, GameChange::DeckShuffled { side } if *side == Side::Runner)
    });
    let trashed = change_at(&vm, |c| {
        matches!(c, GameChange::CardTrashed { obj, .. } if *obj == grip[0])
    });
    let added = change_at(&vm, |c| {
        matches!(c, GameChange::CardMoved { obj, to: Zone::Hand(Side::Runner), .. }
            if *obj == datasucker)
    });
    assert!(
        shuffled < trashed,
        "8.7.5: the ability pends only after the search completes and the stack is shuffled"
    );
    assert!(
        trashed < added,
        "9.11.4d: the reaction window resolves before the NEXT instruction — the \
         grip card is trashed before Datasucker is added to the grip"
    );
    assert_eq!(
        vm.st.hand[&Side::Runner],
        vec![datasucker],
        "the second instruction added the set-aside card to the grip"
    );
    assert_eq!(vm.st.objects[&grip[0]].zone, Zone::Discard(Side::Runner));
}

// ===========================================================================
// §1.13 — host, hosted, and hosting
// ===========================================================================

/// The candidates put to a player at the nth (1-based) target choice.
fn nth_targets<'a>(t: &'a plan::Transcript, side: Side, n: usize) -> &'a plan::Entry {
    t.nth_window(Kind::Targets, side, n)
}

/// Was this target choice a "you may decline" one (`up_to`)? 1.13.6a's "or
/// as normal" is exactly that flag.
fn declinable(e: &plan::Entry) -> bool {
    matches!(e.spec, DecisionSpec::ChooseTargets { up_to: true, .. })
}

/// example_rule_host_via_install_1 (1.13.6a): an Off-Campus-Apartment-class
/// card declares "can host any number of connections" and has no ability
/// that hosts cards onto itself. So whenever the Runner installs a
/// connection, the host is offered as an installation destination alongside
/// the normal one — and for a card it cannot host, it is not offered at all.
#[test]
fn example_rule_host_via_install_1() {
    fn scenario(connection: bool) -> (Vm, plan::Transcript, jinteki_cr::ObjectId) {
        let mut vm = Vm::empty(511);
        let oca = tk::install_rig(&mut vm, tk::off_campus_like("OffCampus-like"));
        let mut card = tk::vanilla_runner_card("Installee", jinteki_cr::object::CardType::Resource);
        if connection {
            card.subtypes = vec!["connection"];
        }
        let installee = vm.new_object(card, Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(installee);
        tk::install_rig(&mut vm, tk::runner_install_button("Install-Button", 1));
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::paid().once(), Reply::take("install-button"))
                .when(Match::targets().once(), Reply::target(installee))
                .when(Match::targets().once(), Reply::target(oca))
                .stop_at_action(),
        );
        (vm, t, installee)
    }

    // A connection: the host is one of the destinations on offer, and the
    // offer is declinable ("or as normal directly into the play area").
    let (vm, t, installee) = scenario(true);
    let host_choice = nth_targets(&t, Side::Runner, 2);
    let oca = vm.st.objects[&installee].host.expect("the connection was hosted");
    assert_eq!(
        host_choice.candidates(),
        &[oca],
        "1.13.6a: the card describing what it can host is an eligible install destination"
    );
    assert!(
        declinable(host_choice),
        "1.13.6a: …or as normal directly into the play area — the Runner may decline"
    );
    assert_eq!(vm.st.objects[&installee].zone, Zone::Rig, "1.13.12: the host's zone");
    assert!(
        vm.st.objects[&oca].hosted.contains(&installee),
        "the host knows what it is hosting"
    );

    // A non-connection: Off-Campus Apartment is not a destination for it, so
    // no destination choice arises at all and it installs as normal.
    let (vm2, t2, other) = scenario(false);
    assert_eq!(
        t2.windows(Kind::Targets, Side::Runner).len(),
        1,
        "only the card choice: a card the host cannot host gets no host offer"
    );
    assert_eq!(vm2.st.objects[&other].host, None);
    assert_eq!(vm2.st.objects[&other].zone, Zone::Rig);
}

/// example_rule_host_via_ability_1 (1.13.6b): Glenn Station declares "can
/// host a single card" AND has a paid ability that hosts a card onto itself.
/// It therefore hosts ONLY through that ability: an install effect is not
/// offered Glenn Station as a destination, though it is offered a card whose
/// hosting text is the declaration alone.
#[test]
fn example_rule_host_via_ability_1() {
    let mut vm = Vm::empty(512);
    let glenn = tk::install_root(
        &mut vm,
        tk::glenn_station_like("GlennStation-like"),
        ServerId::Remote(1),
        true,
    );
    // The control: the same declaration, without the hosting ability.
    let control = tk::install_root(
        &mut vm,
        tk::can_host_card(
            "Host-Only-Upgrade",
            Side::Corp,
            jinteki_cr::object::CardType::Upgrade,
            Vec::new(),
            Some(1),
            "host-only: hosts a single card",
        ),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(
        &mut vm,
        tk::corp_install_from_hq_button("Install-Button", ServerId::Remote(2)),
        ServerId::Remote(2),
        true,
    );
    let hq: Vec<jinteki_cr::ObjectId> = tk::fill_hand(&mut vm, Side::Corp, 2);
    vm.st.corp.credits = 5;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Corp);

    // The plan: install a card from HQ (declining the host offer to see what
    // it contained), then use Glenn Station's own ability on the other card.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("install-button"))
            .when(Match::targets().once(), Reply::target(hq[0]))
            .when(Match::targets().once(), Reply::Targets(vec![]))
            .when(Match::action().once(), Reply::take("glenn-station: host"))
            .when(Match::targets().once(), Reply::target(hq[1]))
            .stop_at_action(),
        Plan::runner(),
    );
    let host_choice = nth_targets(&t, Side::Corp, 2);
    assert!(
        !host_choice.candidates().contains(&glenn),
        "1.13.6b: a card with an ability that hosts onto itself is NOT an \
         installation destination"
    );
    assert_eq!(
        host_choice.candidates(),
        &[control],
        "…while the same declaration without such an ability is one (1.13.6a)"
    );
    assert!(t.took("glenn-station: host"), "Glenn Station's own ability was used");
    assert_eq!(
        vm.st.objects[&hq[1]].host,
        Some(glenn),
        "1.13.6b: hosting happens through the paid ability"
    );
    assert_eq!(
        vm.st.objects[&hq[1]].zone,
        vm.st.objects[&glenn].zone,
        "1.13.12: the hosted card moved to the host's zone"
    );
    assert!(
        !jinteki_cr::object::card_active(&vm.st.objects[&hq[1]]),
        "1.13.2a: hosted without being installed, so not active"
    );
}

/// example_rule_host_on_ability_1 (1.13.6c): an Egret-class program states
/// "install only on a rezzed piece of ice". With no rezzed ice on the board
/// the Runner cannot install it at all; with one, the destination choice is
/// forced and Egret ends up hosted on that ice.
#[test]
fn example_rule_host_on_ability_1() {
    fn scenario(rezzed: bool) -> (Vm, plan::Transcript, jinteki_cr::ObjectId, jinteki_cr::ObjectId) {
        let mut vm = Vm::empty(513);
        let ice = tk::install_ice(&mut vm, tk::vanilla_ice("Ice-Wall-like", 0, 1), ServerId::Hq, rezzed);
        let egret = vm.new_object(tk::egret_like("Egret-like"), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(egret);
        let plain = vm.new_object(tk::program_cost("Plain-Program", 0), Zone::Hand(Side::Runner));
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(plain);
        tk::install_rig(&mut vm, tk::runner_install_button("Install-Button", 1));
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .when(Match::paid().once(), Reply::take("install-button"))
                .when(Match::targets().once(), Reply::target(egret))
                .when(Match::targets().once(), Reply::target(ice))
                .stop_at_action(),
        );
        (vm, t, egret, ice)
    }

    // No rezzed ice: Egret is not a card the Runner may choose to install.
    let mut vm_unrezzed = Vm::empty(514);
    {
        tk::install_ice(&mut vm_unrezzed, tk::vanilla_ice("Ice-Wall-like", 0, 1), ServerId::Hq, false);
        let egret = vm_unrezzed.new_object(tk::egret_like("Egret-like"), Zone::Hand(Side::Runner));
        vm_unrezzed.st.hand.get_mut(&Side::Runner).unwrap().push(egret);
        let plain =
            vm_unrezzed.new_object(tk::program_cost("Plain-Program", 0), Zone::Hand(Side::Runner));
        vm_unrezzed.st.hand.get_mut(&Side::Runner).unwrap().push(plain);
        tk::install_rig(&mut vm_unrezzed, tk::runner_install_button("Install-Button", 1));
        vm_unrezzed.st.runner.credits = 5;
        vm_unrezzed.start_turn(Side::Runner);
        let t = plan::play(
            &mut vm_unrezzed,
            Plan::corp(),
            Plan::runner()
                .when(Match::paid().once(), Reply::take("install-button"))
                .when(Match::targets().once(), Reply::target(plain))
                .stop_at_action(),
        );
        let pick = t.first_window(Kind::Targets, Side::Runner);
        assert!(
            !pick.candidates().contains(&egret),
            "1.13.6c: with no rezzed piece of ice available it is illegal to install Egret"
        );
        assert!(pick.candidates().contains(&plain), "…while ordinary programs are fine");
        assert_eq!(vm_unrezzed.st.objects[&egret].zone, Zone::Hand(Side::Runner));
    }

    // A rezzed piece of ice exists: Egret can be installed, and its
    // destination must be that ice.
    let (vm, t, egret, ice) = scenario(true);
    let pick = nth_targets(&t, Side::Runner, 1);
    assert!(pick.candidates().contains(&egret), "1.13.6c: a valid destination exists");
    let dest = nth_targets(&t, Side::Runner, 2);
    assert_eq!(
        dest.candidates(),
        &[ice],
        "the destination must match the description on the card"
    );
    assert!(
        !declinable(dest),
        "1.13.6c: the player installing it MUST choose a valid destination"
    );
    assert_eq!(vm.st.objects[&egret].host, Some(ice));
    assert_eq!(
        vm.st.objects[&egret].zone,
        Zone::Ice(ServerId::Hq),
        "1.13.12: the same zone as its host"
    );
}

/// example_rule_host_transitivity_1 (1.13.9): the Runner installs a
/// Leprechaun-class program hosted on a Dhegdheer-class one (whose install
/// discount applies), then installs a program hosted on the Leprechaun. Host
/// relationships are not transitive, so Dhegdheer's discount does not reach
/// it and it costs full price.
#[test]
fn example_rule_host_transitivity_1() {
    let mut vm = Vm::empty(515);
    let dheg = tk::install_rig(&mut vm, tk::dhegdheer_like("Dhegdheer-like", 2));
    let lep = vm.new_object(tk::leprechaun_like("Leprechaun-like", 2), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(lep);
    let prog = vm.new_object(tk::program_cost("Prog", 3), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(prog);
    tk::install_rig(&mut vm, tk::runner_install_button("Install-Button", 2));
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    // The plan: Leprechaun onto Dhegdheer, then the program onto Leprechaun.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("install-button"))
            .when(Match::targets().once(), Reply::target(lep))
            .when(Match::targets().once(), Reply::target(dheg))
            .when(Match::targets().once(), Reply::target(prog))
            .when(Match::targets().once(), Reply::target(lep))
            .stop_at_action(),
    );
    assert!(t.took("install-button"));
    let second_host = nth_targets(&t, Side::Runner, 4);
    assert_eq!(
        second_host.candidates(),
        &[lep],
        "Dhegdheer is full (1.13.5), so only Leprechaun is left to host"
    );
    assert_eq!(vm.st.objects[&lep].host, Some(dheg));
    assert_eq!(vm.st.objects[&prog].host, Some(lep));
    assert_eq!(
        vm.st.runner.credits,
        10 - 1 - 3,
        "1.13.9: Leprechaun cost 2-1 hosted on Dhegdheer, but the program hosted \
         on Leprechaun is NOT hosted on Dhegdheer and pays its full 3"
    );
}

/// example_rule_hosted_object_same_zone_as_host_1 (1.13.12): the Runner uses
/// a Madani-class action to host 2 programs from their grip on it. The
/// chosen cards change zones — grip to the play area, where their host is —
/// and, hosted without being installed, they are not active (1.13.2a).
#[test]
fn example_rule_hosted_object_same_zone_as_host_1() {
    let mut vm = Vm::empty(516);
    let madani = tk::install_rig(&mut vm, tk::madani_like("Madani-like", 2));
    let p1 = vm.new_object(tk::program_cost("Prog-A", 3), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(p1);
    let p2 = vm.new_object(tk::program_cost("Prog-B", 3), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(p2);
    let keep = vm.new_object(tk::runner_filler("Not-A-Program"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(keep);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::take("madani"))
            .when(Match::targets().once(), Reply::Targets(vec![p1, p2]))
            .stop_at_action(),
    );
    assert!(t.took("madani"), "the action was taken");
    for p in [p1, p2] {
        assert_eq!(
            vm.st.objects[&p].zone,
            vm.st.objects[&madani].zone,
            "1.13.12: creating the hosting relationship moved the card to the host's zone"
        );
        assert_eq!(vm.st.objects[&p].host, Some(madani));
        assert!(
            !vm.st.hand[&Side::Runner].contains(&p),
            "the chosen cards left the grip"
        );
        assert!(
            !jinteki_cr::object::card_active(&vm.st.objects[&p]),
            "1.13.2a: hosted without being installed — not installed, thus not active"
        );
    }
    assert_eq!(
        vm.st.hand[&Side::Runner],
        vec![keep],
        "only the chosen cards moved"
    );
}

/// example_rule_trash_hosted_objects_when_host_trashed_1 (1.13.13): the
/// Runner has a Detente-class program with Corp cards hosted on it, then
/// plays a Rejig-class event adding it to their grip. Its host card having
/// changed zones, the hosted Corp cards are trashed at the next checkpoint.
#[test]
fn example_rule_trash_hosted_objects_when_host_trashed_1() {
    let mut vm = Vm::empty(517);
    let detente = tk::install_rig(&mut vm, tk::detente_like("Detente-like"));
    let corp_a = tk::install_root(&mut vm, tk::vanilla_asset("Corp-A", 0, 3), ServerId::Remote(1), true);
    let corp_b = tk::install_root(&mut vm, tk::vanilla_asset("Corp-B", 0, 3), ServerId::Remote(2), true);
    let rejig = vm.new_object(tk::rejig_like("Rejig-like"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(rejig);
    tk::install_rig(&mut vm, tk::play_event_action("Play-Button", rejig));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    // The plan: host both Corp cards on Detente (one action each), then play
    // Rejig and add Detente to the grip.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().times(2), Reply::take("detente"))
            .when(Match::action().once(), Reply::take("play-event-action"))
            .when(Match::targets().once(), Reply::target(corp_a))
            .when(Match::targets().once(), Reply::target(corp_b))
            .when(Match::targets().once(), Reply::target(detente))
            .stop_at_action(),
    );
    assert_eq!(t.times_taken("detente"), 2, "both Corp cards were hosted");
    assert_eq!(
        vm.st.objects[&detente].zone,
        Zone::Hand(Side::Runner),
        "Rejig added the host to the grip"
    );
    for c in [corp_a, corp_b] {
        assert_eq!(
            vm.st.objects[&c].zone,
            Zone::Discard(Side::Corp),
            "1.13.13: the host changed zones, so everything hosted on it is trashed"
        );
    }
    assert!(vm.st.objects[&detente].hosted.is_empty());
}

/// example_rule_trash_hosted_objects_when_host_trashed_2 (1.13.13): the
/// Runner has an agenda in their score area with a hosted agenda counter.
/// The Corp plays an IP-Enforcement-class operation to install that agenda.
/// The host changed zones, so the hosted counter is trashed during the
/// installation — before the card becomes installed — and cannot be
/// prevented.
#[test]
fn example_rule_trash_hosted_objects_when_host_trashed_2() {
    let mut vm = Vm::empty(518);
    let agenda = tk::put_in_score_area(&mut vm, tk::vanilla_agenda("NextBigThing-like", 3, 1), Side::Runner);
    tk::place_counters(&mut vm, agenda, CounterKind::Agenda, 1);
    let ip = vm.new_object(tk::ip_enforcement_like("IPEnforcement-like"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ip);
    tk::install_root(
        &mut vm,
        tk::play_operation_button("Play-Button", ip),
        ServerId::Remote(1),
        true,
    );
    vm.st.corp.credits = 5;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("play-op"))
            .when(Match::targets().once(), Reply::target(agenda))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("play-op"), "the operation was played");
    assert!(
        matches!(vm.st.objects[&agenda].zone, Zone::Root(_)),
        "the agenda was installed"
    );
    assert_eq!(
        vm.st.objects[&agenda].counter(CounterKind::Agenda),
        0,
        "1.13.13: the hosted agenda counter was trashed when its host changed zones"
    );
    let removed = change_at(&vm, |c| {
        matches!(c, GameChange::CounterRemoved { obj: Some(o), kind: CounterKind::Agenda, .. }
            if *o == agenda)
    });
    let installed = change_at(&vm, |c| {
        matches!(c, GameChange::CardInstalled { obj, .. } if *obj == agenda)
    });
    assert!(
        removed < installed,
        "the counter goes during the installation — the checkpoint that trashes it \
         precedes step 8.5.16f, where the card becomes installed"
    );
}

/// example_rule_trash_hosted_objects_when_host_trashed_3 (1.13.13, 8.8.4c):
/// the same agenda with the same hosted counter, but the Corp plays an
/// Exchange-of-Information-class operation to swap it into their own score
/// area. Moving from one score area to another is the exception to 1.13.13,
/// so the counter stays hosted.
#[test]
fn example_rule_trash_hosted_objects_when_host_trashed_3() {
    let mut vm = Vm::empty(519);
    let theirs = tk::put_in_score_area(&mut vm, tk::vanilla_agenda("NextBigThing-like", 3, 1), Side::Runner);
    tk::place_counters(&mut vm, theirs, CounterKind::Agenda, 1);
    let ours = tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Corp-Agenda", 3, 1), Side::Corp);
    let eoi = vm.new_object(
        tk::exchange_of_information_like("ExchangeOfInformation-like", theirs, ours),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(eoi);
    tk::install_root(
        &mut vm,
        tk::play_operation_button("Play-Button", eoi),
        ServerId::Remote(1),
        true,
    );
    vm.st.corp.credits = 5;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("play-op")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("play-op"), "the operation was played");
    assert_eq!(
        vm.st.objects[&theirs].zone,
        Zone::ScoreArea(Side::Corp),
        "the agenda was swapped into the Corp's score area"
    );
    assert_eq!(vm.st.objects[&ours].zone, Zone::ScoreArea(Side::Runner));
    assert_eq!(
        vm.st.objects[&theirs].counter(CounterKind::Agenda),
        1,
        "1.13.13/8.8.4c: moving from a score area to another score area is the \
         exception — the agenda counter remains hosted"
    );
}

/// example_rule_hosted_counters_not_on_player_1 (1.13.3): a Whitespace-class
/// subroutine makes the Runner lose 3[credit] while they hold 1 in their
/// credit pool and 3 hosted on a card. Hosted credits are not "on" the
/// Runner: only the pool is emptied.
#[test]
fn example_rule_hosted_counters_not_on_player_1() {
    let mut vm = Vm::empty(520);
    tk::install_ice(&mut vm, tk::whitespace_like("Whitespace-like", 3), ServerId::Hq, true);
    let purse = tk::install_rig(
        &mut vm,
        tk::hosted_credit_source("Cyberfeeder-like", jinteki_cr::object::CardType::Hardware),
    );
    tk::place_counters(&mut vm, purse, CounterKind::Credit, 3);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Hq).stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterBegan { .. })),
        "the Runner encountered the ice"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { .. })),
        "the subroutine resolved: {}",
        t.tail(4)
    );
    assert_eq!(vm.st.runner.credits, 0, "the credit pool is emptied as far as it goes");
    assert_eq!(
        vm.st.objects[&purse].counter(CounterKind::Credit),
        3,
        "1.13.3: the Runner cannot lose credits hosted on their cards, even with \
         fewer than 3 in their credit pool"
    );
}

/// example_rule_hosted_counters_not_on_player_2 (1.13.3): the Corp has a
/// Superdeep-Borehole-class card with hosted bad publicity counters. When
/// the Runner fills their bad publicity fund at step 6.9.1b, the hosted
/// counters are not counted.
#[test]
fn example_rule_hosted_counters_not_on_player_2() {
    let mut vm = Vm::empty(521);
    let borehole =
        tk::install_root(&mut vm, tk::vanilla_asset("SuperdeepBorehole-like", 0, 3), ServerId::Remote(1), true);
    tk::place_counters(&mut vm, borehole, CounterKind::BadPublicity, 2);
    vm.st.corp.bad_publicity = 1;
    // A 1[credit] paid ability and an empty credit pool: the Runner can only
    // be offered it out of the bad publicity fund (10.6.2), so the window
    // itself witnesses what the fund holds.
    tk::install_rig(&mut vm, tk::pump_breaker("Breaker-like", 1));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    // Halt at the first paid ability window of the run — step 6.9.1e,
    // immediately after the fund is filled at 6.9.1b.
    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_initiation_paw"), Reply::Halt)
            .stop_at_action(),
    );
    let t = script.run(&mut vm).clone();
    assert!(t.halted, "stopped inside the run's initiation phase:\n{}", t.tail(10));
    assert_eq!(
        vm.st.bp_fund, 1,
        "1.13.3: 1 credit for the bad publicity the Corp has — the 2 counters \
         hosted on a card are not on the Corp and add nothing"
    );
    assert_eq!(
        vm.st.objects[&borehole].counter(CounterKind::BadPublicity),
        2,
        "the hosted counters are untouched"
    );
}

/// example_rule_hosted_counters_not_on_player_3 (1.13.3): the Corp plays a
/// Scapegoat-class operation to remove 2 bad publicity. It removes what the
/// Corp has and cannot reach the bad publicity counters hosted on a card.
#[test]
fn example_rule_hosted_counters_not_on_player_3() {
    let mut vm = Vm::empty(522);
    let borehole =
        tk::install_root(&mut vm, tk::vanilla_asset("SuperdeepBorehole-like", 0, 3), ServerId::Remote(1), true);
    tk::place_counters(&mut vm, borehole, CounterKind::BadPublicity, 2);
    vm.st.corp.bad_publicity = 1;
    let scapegoat = vm.new_object(tk::remove_bad_pub_operation("Scapegoat-like", 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(scapegoat);
    tk::install_root(
        &mut vm,
        tk::play_operation_button("Play-Button", scapegoat),
        ServerId::Remote(1),
        true,
    );
    vm.st.corp.credits = 5;
    tk::fill_deck(&mut vm, Side::Corp, 3);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("play-op")).stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("play-op"), "the operation was played");
    assert_eq!(vm.st.corp.bad_publicity, 0, "the Corp's own bad publicity is removed");
    assert_eq!(
        vm.st.objects[&borehole].counter(CounterKind::BadPublicity),
        2,
        "1.13.3: bad publicity counters hosted on a card cannot be removed by an \
         ability that removes counters from a player"
    );
}

/// example_rule_hosted_counter_used_condition_1 (9.1.6c): the Runner spends
/// the credit hosted on a Cyberfeeder-class card to pay the trigger cost of
/// a Mimic-class paid ability. Both cards have been used — the one whose
/// ability was triggered, and the one whose ability allowed the credit to be
/// spent.
#[test]
fn example_rule_hosted_counter_used_condition_1() {
    let mut vm = Vm::empty(523);
    let feeder = tk::install_rig(
        &mut vm,
        tk::hosted_credit_source("Cyberfeeder-like", jinteki_cr::object::CardType::Hardware),
    );
    tk::place_counters(&mut vm, feeder, CounterKind::Credit, 1);
    let mimic = tk::install_rig(&mut vm, tk::credit_cost_program("Mimic-like"));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("mimic")).stop_at_action(),
    );
    assert!(
        t.took("mimic"),
        "1.10.3c: the hosted credit makes the 1[credit] trigger cost payable"
    );
    assert_eq!(vm.st.runner.credits, 0, "nothing came out of the credit pool");
    assert_eq!(
        vm.st.objects[&feeder].counter(CounterKind::Credit),
        0,
        "1.13.11: the hosted credit was spent from its host"
    );
    let used: Vec<jinteki_cr::ObjectId> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::AbilityUsed { source } => Some(*source),
            _ => None,
        })
        .collect();
    assert!(
        used.contains(&mimic),
        "9.1.6a: the paid ability is used once its trigger cost is paid"
    );
    assert!(
        used.contains(&feeder),
        "9.1.6c: the card whose ability allowed the counter to be spent has been \
         used too, though the cost was paid for another card's ability"
    );
}


// ===========================================================================
// §1.14 — control, and who carries an effect out
// ===========================================================================

/// example_rule_controller_choices_1 (1.14.5): a Rototurret-class subroutine
/// reads "Trash 1 installed program." — its controller, the Corp, chooses.
/// A Bulwark-class subroutine reads "The Runner trashes 1 installed
/// program." — the Runner is specified to carry the effect out, so the
/// RUNNER chooses instead. Same effect, different chooser.
#[test]
fn example_rule_controller_choices_1() {
    let mut vm = Vm::empty(612);
    tk::install_ice(
        &mut vm,
        tk::trash_program_sub_ice("Rototurret-like", None),
        ServerId::Hq,
        true,
    );
    tk::install_ice(
        &mut vm,
        tk::trash_program_sub_ice("Bulwark-like", Some(Side::Runner)),
        ServerId::Rnd,
        true,
    );
    tk::install_rig(&mut vm, tk::program_mu("Program-a", 1));
    tk::install_rig(&mut vm, tk::program_mu("Program-b", 1));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::action().once(), Reply::run(ServerId::Hq))
            .when(Match::action().once(), Reply::run(ServerId::Rnd))
            .stop_at_action(),
    );
    let choosers: Vec<Side> = t.of_kind(Kind::Targets).iter().map(|e| e.side).collect();
    assert_eq!(
        choosers,
        vec![Side::Corp, Side::Runner],
        "the unwrapped subroutine's controller chose; the one naming the \
         Runner put the choice to the Runner: {}",
        t.tail(8)
    );
}

/// example_rule_trigger_condition_effect_by_player_1 (1.14.5a): an
/// Apocalypse-class Runner card does not specify who trashes the installed
/// Corp cards, so its controller — the Runner — is responsible, and a rezzed
/// Hostile-Infrastructure-class ability meets its trigger condition.
#[test]
fn example_rule_trigger_condition_effect_by_player_1() {
    let mut vm = Vm::empty(613);
    let asset = tk::install_root(&mut vm, tk::corp_filler("Asset-like"), ServerId::Remote(1), true);
    tk::install_root(
        &mut vm,
        tk::hostile_infra_like("HostileInfra-like"),
        ServerId::Remote(2),
        true,
    );
    tk::install_rig(&mut vm, tk::trash_set_button("Apocalypse-like", vec![asset]));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("trash the set")).stop_at_action(),
    );
    assert!(t.took("trash the set"));
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardTrashed { obj, by: Side::Runner, .. } if *obj == asset
        )),
        "the Runner carried out the trashing"
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "so the Corp's ability did 1 net damage");
}

/// example_rule_trigger_condition_effect_by_player_2 (1.14.5a): an
/// Alice-Merchant-class Runner card states that the CORP must trash a card
/// from HQ. The Corp carries that out, so the same Hostile-Infrastructure
/// class ability does NOT meet its trigger condition.
#[test]
fn example_rule_trigger_condition_effect_by_player_2() {
    let mut vm = Vm::empty(614);
    tk::install_root(
        &mut vm,
        tk::hostile_infra_like("HostileInfra-like"),
        ServerId::Remote(2),
        true,
    );
    tk::install_rig(&mut vm, tk::alice_like("Alice-like"));
    let hq = tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_hand(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("alice")).stop_at_action(),
    );
    assert!(t.took("alice"));
    let chooser = t.of_kind(Kind::Targets).first().map(|e| e.side);
    assert_eq!(chooser, Some(Side::Corp), "the named player makes the choice: {}", t.tail(6));
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::CardTrashed { obj, by: Side::Corp, .. } if hq.contains(obj)
        )),
        "the Corp carried out the trashing"
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        3,
        "the condition 'whenever the RUNNER trashes a Corp card' was not met"
    );
}

// ===========================================================================
// §1.10 — credits, spending, recurring credits
// ===========================================================================

/// example_rule_lose_credits_1 (1.10.3b): a DNA-Tracker-class subroutine
/// makes the Runner lose 2[c]. With 1[c] in the pool they lose exactly 1;
/// with an empty pool the effect does nothing. The loss is forced and takes
/// as much as the pool holds — never more, never from cards.
#[test]
fn example_rule_lose_credits_1() {
    let mut vm = Vm::empty(600);
    tk::install_ice(&mut vm, tk::whitespace_like("DNATracker-like", 2), ServerId::Hq, true);
    vm.st.runner.credits = 1;
    vm.start_turn(Side::Runner);

    // The plan: run HQ twice — once holding 1[c], once holding none.
    plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::action().times(2), Reply::run(ServerId::Hq)).stop_at_action(),
    );
    let losses: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CreditsLost { side: Side::Runner, amount } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(losses, vec![1, 0], "lost 1 of the 2 demanded, then nothing at all");
    assert_eq!(vm.st.runner.credits, 0);
}

/// example_rule_spend_credits_1 (1.10.3c): a Cyberfeeder-class card's ability
/// lets the Runner spend the credits hosted on it. With an empty credit pool
/// the 1[c] trigger cost of an Atman-class ability is still payable — and it
/// is paid from the hosted credit, not from the pool.
#[test]
fn example_rule_spend_credits_1() {
    let mut vm = Vm::empty(601);
    let feeder = tk::install_rig(
        &mut vm,
        tk::hosted_credit_source("Cyberfeeder-like", jinteki_cr::object::CardType::Hardware),
    );
    tk::place_counters(&mut vm, feeder, CounterKind::Credit, 1);
    let atman = tk::install_rig(&mut vm, tk::credit_cost_program("Atman-like"));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("mimic")).stop_at_action(),
    );
    assert!(t.took("mimic"), "the ability was offered with an empty pool: {}", t.tail(4));
    assert_eq!(vm.st.runner.credits, 0, "no credits came from the pool — it was empty");
    assert_eq!(
        vm.st.objects[&feeder].counter(CounterKind::Credit),
        0,
        "the credit was spent from the card that allows spending it"
    );
    assert!(vm.changes.log.iter().any(|c| matches!(
        c,
        GameChange::AbilityUsed { source } if *source == atman
    )));
}

/// example_rule_spend_credits_2 (1.10.3c): credits hosted on a Ghost-Runner-
/// class card can be spent secretly, when bidding for a psi ability — the
/// legal bids are capped by everything the player can spend (10.14.3), not
/// by their credit pool alone.
#[test]
fn example_rule_spend_credits_2() {
    let mut vm = Vm::empty(602);
    let ghost = tk::install_rig(&mut vm, tk::fencer_like("GhostRunner-like", 2));
    tk::place_counters(&mut vm, ghost, CounterKind::Credit, 2);
    tk::install_root(&mut vm, tk::psi_button("FuturePerfect-like"), ServerId::Remote(1), true);
    vm.st.runner.credits = 0;
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    assert_eq!(
        vm.psi_legal_bids(Side::Runner),
        vec![0, 1, 2],
        "1.10.3c: hosted credits the Runner may spend count towards a bid"
    );
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("psi"))
            .when(Match::psi_bid(), Reply::Bid(0)),
        Plan::runner().when(Match::psi_bid(), Reply::Bid(2)).stop_at_action(),
    );
    assert!(t.took("psi"), "the psi game was played: {}", t.tail(4));
    assert_eq!(vm.st.runner.credits, 0, "the pool was empty throughout");
    assert_eq!(
        vm.st.objects[&ghost].counter(CounterKind::Credit),
        0,
        "the bid was paid with the hosted credits"
    );
}

/// example_rule_spend_credits_3 (1.10.3c): the Runner must use the credits
/// hosted on a Ghost-Runner-class card when that is the only way to pay a
/// Tollbooth-class encounter cost — with an empty pool the nested cost is
/// still payable, and paying it keeps the run alive.
#[test]
fn example_rule_spend_credits_3() {
    let mut vm = Vm::empty(603);
    tk::install_ice(&mut vm, tk::toll_ice("Tollbooth-like", 3), ServerId::Hq, true);
    let ghost = tk::install_rig(&mut vm, tk::fencer_like("GhostRunner-like", 3));
    tk::place_counters(&mut vm, ghost, CounterKind::Credit, 3);
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    let costs: Vec<u32> =
        t.of_kind(Kind::NestedCost).iter().filter_map(|e| e.cost()).map(|c| c.flat_credits()).collect();
    assert_eq!(costs, vec![3], "the 3[c] was put to the Runner even with an empty pool");
    assert_eq!(
        vm.st.objects[&ghost].counter(CounterKind::Credit),
        0,
        "the only credits that could pay it were the hosted ones"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "paying kept the run going: {}",
        t.tail(6)
    );
}

/// example_rule_recurring_credits_do_not_accumulate_1 (1.10.5d): a
/// Spinal-Modem-class card with 2[recurring] gets 2 credits when it becomes
/// active (1.10.5b). The Runner spends 1 later in the turn; at the start of
/// their next turn the card is refilled UP TO 2 — one credit is placed, not
/// two, and the total never becomes 3.
#[test]
fn example_rule_recurring_credits_do_not_accumulate_1() {
    let mut vm = Vm::empty(604);
    tk::fill_deck(&mut vm, Side::Corp, 6);
    tk::fill_deck(&mut vm, Side::Runner, 6);
    let button = tk::install_rig(&mut vm, tk::runner_install_button("Install-button", 1));
    let modem = vm.new_object(tk::recurring_card("SpinalModem-like", 2), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(modem);
    let pump = tk::install_rig(&mut vm, tk::credit_cost_program("Pump-like"));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);
    let _ = (button, pump);

    // The plan: install the modem, then spend one of its credits on a 1[c]
    // paid ability, then stop.
    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .when(
                Match::paid().at_step("step_runner_turn_action_phase_paw").once(),
                Reply::take("install-button"),
            )
            .when(Match::targets().once(), Reply::target(modem))
            // AFTER the 5.7.1c refill step: the modem already holds its 2, so
            // the refill places nothing (1.10.5d) and the spend is the only
            // change to its counters this turn.
            .when(
                Match::paid().at_step("step_runner_turn_loop_paw").once(),
                Reply::take("mimic"),
            )
            .when(Match::action().nth(1), Reply::Halt)
            .when(Match::action().times(4), Reply::credit())
            .when(Match::action(), Reply::Halt),
    );
    script.run(&mut vm);
    assert_eq!(
        vm.st.objects[&modem].counter(CounterKind::Credit),
        1,
        "1.10.5b placed 2 on becoming active; one was just spent"
    );

    // Play out this turn and the Corp's; the Runner's next turn begins with
    // the 5.7.1c refill step.
    script.run(&mut vm);
    assert_eq!(
        vm.st.objects[&modem].counter(CounterKind::Credit),
        2,
        "1.10.5d: refilled only up to the printed number, never past it"
    );
}

// ===========================================================================
// §1.17 / §1.19 / §1.20 — score, threat, [trash], memory
// ===========================================================================

/// example_rule_threat_level_1 (1.17.1a): with the Runner on 4 agenda points
/// and the Corp on 3, the threat level is 4 — the GREATEST score, not the
/// sum and not the active player's. A "threat 4" ability is therefore active
/// (9.3.6f) while a "threat 5" ability is not.
#[test]
fn example_rule_threat_level_1() {
    let mut vm = Vm::empty(605);
    tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Runner-agenda", 3, 4), Side::Runner);
    tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Corp-agenda", 3, 3), Side::Corp);
    tk::install_root(&mut vm, tk::threat_button("Threat4-like", 4, "t4"), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::threat_button("Threat5-like", 5, "t5"), ServerId::Remote(2), true);
    vm.start_turn(Side::Corp);

    assert_eq!(vm.score(Side::Runner), 4);
    assert_eq!(vm.score(Side::Corp), 3);
    assert_eq!(vm.threat_level(), 4, "the greatest score of any player");

    let t = plan::play(&mut vm, Plan::corp().stop_at_action(), Plan::runner());
    assert!(t.ever_offered("t4"), "threat 4 ≤ 4: the ability is active");
    assert!(!t.ever_offered("t5"), "threat 5 > 4: the ability is not active");
}

/// example_rule_trash_symbol_1 (1.19.4): Fall Guy's "[trash]: Gain 2[credit]"
/// — the [trash] symbol IS the trigger cost, so using the ability trashes the
/// source as payment (before the effect resolves, 9.5.7).
#[test]
fn example_rule_trash_symbol_1() {
    let mut vm = Vm::empty(606);
    let guy = tk::install_rig(&mut vm, tk::fall_guy_like("FallGuy-like"));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("fall-guy")).stop_at_action(),
    );
    assert!(t.took("fall-guy"));
    assert_eq!(
        vm.st.objects[&guy].zone,
        Zone::Discard(Side::Runner),
        "the cost was paid by trashing the card the symbol appears on"
    );
    assert_eq!(vm.st.runner.credits, 2, "and then the effect resolved");
}

/// example_rule_memory_limit_1 (1.20.2): a starting memory limit of 4[mu]
/// plus a T400-class "+1[mu]" lets the Runner keep 5[mu] of programs
/// installed. Without the hardware the fifth point of memory is a
/// restriction violation and the minimal set is trashed at the checkpoint.
#[test]
fn example_rule_memory_limit_1() {
    let mut with_chip = Vm::empty(607);
    tk::install_rig(&mut with_chip, tk::mem_chip_like("T400-like", 1));
    let big = tk::install_rig(&mut with_chip, tk::program_mu("Program-4mu", 4));
    let small = tk::install_rig(&mut with_chip, tk::program_mu("Program-1mu", 1));
    with_chip.start_turn(Side::Runner);
    assert_eq!(with_chip.memory_limit(), 5, "4 base + 1 from the hardware");
    plan::play(&mut with_chip, Plan::corp(), Plan::runner().stop_at_action());
    assert_eq!(with_chip.st.objects[&big].zone, Zone::Rig);
    assert_eq!(with_chip.st.objects[&small].zone, Zone::Rig, "5[mu] of programs fit under 5[mu]");

    let mut without = Vm::empty(608);
    let big2 = tk::install_rig(&mut without, tk::program_mu("Program-4mu", 4));
    let small2 = tk::install_rig(&mut without, tk::program_mu("Program-1mu", 1));
    without.start_turn(Side::Runner);
    assert_eq!(without.memory_limit(), 4);
    plan::play(&mut without, Plan::corp(), Plan::runner().stop_at_action());
    assert!(
        without.st.objects[&big2].zone == Zone::Discard(Side::Runner)
            || without.st.objects[&small2].zone == Zone::Discard(Side::Runner),
        "over the limit, a program is trashed"
    );
}

// ===========================================================================
// §10.4 — damage
// ===========================================================================

/// example_rule_suffer_or_take_damage_1 (10.4.1): an Argus-class ability lets
/// the Runner choose to SUFFER 2 meat damage. The Cleaners only adds to
/// damage the Corp DOES, so the suffered damage stays at 2 — while the same
/// Cleaners raises a Corp-done 2 to 3.
#[test]
fn example_rule_suffer_or_take_damage_1() {
    let mut vm = Vm::empty(609);
    tk::install_identity(&mut vm, tk::argus_like("Argus-like"), Side::Corp);
    tk::put_in_score_area(&mut vm, tk::cleaners_static_like("Cleaners-like"), Side::Corp);
    let steal = vm.new_object(tk::vanilla_agenda("HostileTakeover-like", 2, 1), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(steal);
    tk::fill_hand(&mut vm, Side::Runner, 8);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Rnd)
            .when(Match::options(), Reply::ChooseNamed("suffer"))
            .stop_at_action(),
    );
    assert!(vm.changes.log.iter().any(|c| matches!(c, GameChange::AgendaStolen { .. })));
    let suffered: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::DamageSuffered { amount, .. } => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(
        suffered,
        vec![2],
        "the Runner is responsible for damage they suffer, so The Cleaners' \
         'damage done by the Corp' bonus does not apply: {}",
        t.tail(6)
    );
    assert_eq!(vm.st.runner.tags, 0, "the other option was not taken");

    // Control: the same Cleaners raises damage the CORP does from 2 to 3.
    let mut done = Vm::empty(610);
    tk::put_in_score_area(&mut done, tk::cleaners_static_like("Cleaners-like"), Side::Corp);
    tk::install_root(&mut done, tk::meat_damage_button("Scorch-like", 2), ServerId::Remote(1), true);
    tk::fill_hand(&mut done, Side::Runner, 8);
    done.start_turn(Side::Corp);
    plan::play(
        &mut done,
        Plan::corp().when(Match::paid().once(), Reply::take("do meat damage")).stop_at_action(),
        Plan::runner(),
    );
    assert!(done.changes.log.iter().any(
        |c| matches!(c, GameChange::DamageSuffered { amount: 3, .. })
    ));
}

/// example_rule_multiple_damage_taken_simultaneously_1 (10.4.3): a BOOM!-class
/// 7 meat damage against a 4-card grip. The cards are trashed randomly and
/// simultaneously — ONE occurrence — and because the Runner suffered more
/// damage than they had cards, they flatline.
#[test]
fn example_rule_multiple_damage_taken_simultaneously_1() {
    let mut vm = Vm::empty(611);
    tk::install_root(&mut vm, tk::meat_damage_button("BOOM-like", 7), ServerId::Remote(1), true);
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.st.runner.tags = 2;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("do meat damage")).stop_at_action(),
        Plan::runner(),
    );
    let events: Vec<&GameChange> = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .collect();
    assert_eq!(events.len(), 1, "one damage occurrence, not seven");
    match events[0] {
        GameChange::DamageSuffered { amount, cards, .. } => {
            assert_eq!(*amount, 7);
            assert_eq!(cards.len(), 4, "the whole grip went at once");
        }
        _ => unreachable!(),
    }
    assert_eq!(
        t.result,
        Some(jinteki_cr::decision::GameResult::Flatline),
        "more damage suffered than cards in the grip"
    );
}

// ===========================================================================
// §3.9.5 / §9.10 — strength modifications and their durations
// ===========================================================================

/// example_rule_icebreaker_strength_increase_implicit_1 (3.9.5b): Corroder's
/// "1[credit]: +1 strength" states no duration, so the increase lasts for
/// the remainder of the current encounter — and no longer.
#[test]
fn example_rule_icebreaker_strength_increase_implicit_1() {
    let mut vm = Vm::empty(620);
    tk::install_ice(&mut vm, tk::vanilla_ice("Ice-a", 0, 3), ServerId::Hq, true);
    let breaker = tk::install_rig(&mut vm, tk::implicit_pump_breaker("Corroder-like", 2));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("corroder"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(breaker),
        Some(3),
        "inside the encounter the +1 applies: {}",
        script.transcript().tail(5)
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(breaker),
        Some(2),
        "the implicit 'remainder of the current encounter' duration expired"
    );
}

/// example_rule_icebreaker_strength_increase_specified_1 (3.9.5c): Gordian
/// Blade's "+1 strength for the remainder of this run" is triggered during
/// an encounter and still applies after that encounter ends, because the
/// stated run duration outlives the implicit encounter one. It expires when
/// the run does.
#[test]
fn example_rule_icebreaker_strength_increase_specified_1() {
    let mut vm = Vm::empty(621);
    tk::install_ice(&mut vm, tk::vanilla_ice("Ice-a", 0, 3), ServerId::Hq, true);
    let breaker = tk::install_rig(&mut vm, tk::run_pump_breaker("Gordian-like", 2));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("gordian"))
            .when(Match::jack_out().once(), Reply::Halt)
            .stop_at_action(),
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(breaker),
        Some(3),
        "the encounter is over, but the run is not: {}",
        script.transcript().tail(5)
    );
    script.run(&mut vm);
    assert_eq!(vm.effective_strength(breaker), Some(2), "the run ended and the increase went");
}

/// example_rule_icebreaker_strength_increase_outside_of_encounter_1 (3.9.5d):
/// the same Corroder ability used with no encounter in progress states no
/// applicable duration, so the modification expires during the very next
/// checkpoint — the credit is spent and nothing lasts.
#[test]
fn example_rule_icebreaker_strength_increase_outside_of_encounter_1() {
    let mut vm = Vm::empty(622);
    let breaker = tk::install_rig(&mut vm, tk::implicit_pump_breaker("Corroder-like", 2));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().when(Match::paid().once(), Reply::take("corroder")).stop_at_action(),
    );
    assert!(t.took("corroder"), "the ability really was used: {}", t.tail(4));
    assert_eq!(vm.st.runner.credits, 4, "and paid for");
    assert!(vm.lingering.is_empty(), "the modification did not survive the next checkpoint");
    assert_eq!(vm.effective_strength(breaker), Some(2));
}

/// example_rule_modify_duration_of_lingering_effect_1 (9.10.5): Na'Not'K's
/// static ability sets its strength from the ice protecting the attacked
/// server, and its paid ability adds +2 during an encounter. With a
/// Gebrselassie-class card hosted on it, the +2 lingering effect is kept
/// alive to the end of the turn; the static contribution simply lapses with
/// the run, because static abilities have no durations to modify.
#[test]
fn example_rule_modify_duration_of_lingering_effect_1() {
    let mut vm = Vm::empty(623);
    for i in 0..3 {
        let name: &'static str = Box::leak(format!("Ice-{i}").into_boxed_str());
        tk::install_ice(&mut vm, tk::vanilla_ice(name, 0, 3), ServerId::Hq, true);
    }
    let breaker = tk::install_rig(&mut vm, tk::attacked_server_breaker("NaNotK-like"));
    let geb = tk::install_rig(&mut vm, tk::duration_extender("Gebrselassie-like"));
    tk::host_on(&mut vm, geb, breaker);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("nanotk"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(breaker),
        Some(5),
        "3 ice protecting the attacked server, plus the paid +2: {}",
        script.transcript().tail(5)
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(breaker),
        Some(2),
        "the run ended: the static ability provides nothing, but the +2 \
         lingering effect was kept alive to the end of the turn"
    );
}

/// example_rule_static_no_lingering_effects_1 (9.4.4): a Puffer-class
/// icebreaker's strength comes from a STATIC ability, which creates no
/// lingering effect and has no duration — so the Gebrselassie-class card
/// hosted on it has nothing to modify there. Only the paid pump makes a
/// lingering effect, and only that one gets its duration replaced.
#[test]
fn example_rule_static_no_lingering_effects_1() {
    let mut vm = Vm::empty(624);
    tk::install_ice(&mut vm, tk::vanilla_ice("Ice-a", 0, 3), ServerId::Hq, true);
    let puffer = tk::install_rig(&mut vm, tk::counter_strength_breaker("Puffer-like"));
    tk::place_counters(&mut vm, puffer, CounterKind::Virus, 2);
    let geb = tk::install_rig(&mut vm, tk::duration_extender("Gebrselassie-like"));
    tk::host_on(&mut vm, geb, puffer);
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("puffer"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    script.run(&mut vm);
    assert_eq!(vm.effective_strength(puffer), Some(4), "1 + 2 virus counters + the paid 1");
    let strength_effects = vm
        .lingering
        .iter()
        .filter(|l| matches!(l.payload, jinteki_cr::lingering::Payload::StrengthMod { .. }))
        .count();
    assert_eq!(
        strength_effects, 1,
        "9.4.4: only the PAID ability created a lingering effect; the static \
         ability contributes through the characteristics pipeline instead"
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(puffer),
        Some(4),
        "the paid increase was extended to the turn; the static contribution \
         never had a duration to extend and is still 1 + 2"
    );
}

/// example_rule_replacement_on_static_ability_must_remain_active_1 (9.9.9a):
/// the duration replacement applies only while its own source is active. If
/// the Gebrselassie-class card is trashed before the modified effect's
/// original duration runs out, the effect reverts to that duration and
/// expires on time.
#[test]
fn example_rule_replacement_on_static_ability_must_remain_active_1() {
    let mut vm = Vm::empty(625);
    tk::install_ice(&mut vm, tk::vanilla_ice("Ice-a", 0, 3), ServerId::Hq, true);
    let breaker = tk::install_rig(&mut vm, tk::implicit_pump_breaker("Corroder-like", 2));
    let geb = tk::install_rig(&mut vm, tk::duration_extender("Gebrselassie-like"));
    tk::host_on(&mut vm, geb, breaker);
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Skorpios-like", vec![geb]),
        ServerId::Remote(1),
        true,
    );
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        // The Corp trashes the duration-modifier during the encounter,
        // before the pump's own duration runs out.
        Plan::corp().when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("corp-trash")),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("corroder"))
            .stop_at_action(),
    );
    assert_eq!(
        vm.st.objects[&geb].zone,
        Zone::Discard(Side::Runner),
        "the replacement's source left play: {}",
        t.tail(6)
    );
    assert_eq!(
        vm.effective_strength(breaker),
        Some(2),
        "with the replacement gone, the increase expired at the end of the \
         encounter as originally stated"
    );
}

// ===========================================================================
// §10.12 — sabotage
// ===========================================================================

/// example_rule_sabotage_all_remaining_cards_1 (10.12.3b, the plentiful
/// case): sabotage 2 against 2 cards in HQ and 30 in R&D. Nothing is forced:
/// the Corp may take both off R&D, one from each, or both from HQ — the
/// choice put to them spans 0..2 from HQ.
#[test]
fn example_rule_sabotage_all_remaining_cards_1() {
    let mut vm = Vm::empty(630);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 30);
    tk::install_rig(&mut vm, tk::sabotage_button("Esa-like", 2));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::targets().once(), Reply::Targets(vec![])),
        Plan::runner().when(Match::paid().once(), Reply::take("sabotage")).stop_at_action(),
    );
    let ask = t.first_window(Kind::Targets, Side::Corp);
    match ask.spec {
        DecisionSpec::ChooseTargets { count, min, .. } => {
            assert_eq!((count, min), (2, 0), "up to 2 from HQ, none of them forced");
        }
        _ => unreachable!(),
    }
    assert_eq!(vm.st.hand[&Side::Corp].len(), 2, "the Corp chose none from HQ");
    assert_eq!(vm.st.deck[&Side::Corp].len(), 28, "so both came off the top of R&D");
    assert_eq!(vm.st.discard[&Side::Corp].len(), 2);
    assert!(
        vm.st.discard[&Side::Corp].iter().all(|c| !vm.st.objects[c].faceup),
        "10.12.2a: cards trashed by a sabotage enter Archives facedown"
    );
}

/// example_rule_sabotage_all_remaining_cards_2 (10.12.3a): sabotage 4 with 5
/// cards in HQ and 2 in R&D. R&D cannot supply more than 2, so the Corp must
/// choose AT LEAST 2 cards in HQ.
#[test]
fn example_rule_sabotage_all_remaining_cards_2() {
    let mut vm = Vm::empty(631);
    tk::fill_hand(&mut vm, Side::Corp, 5);
    tk::fill_deck(&mut vm, Side::Corp, 2);
    tk::install_rig(&mut vm, tk::sabotage_button("Chastushka-like", 4));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        // The Corp tries to give up nothing from HQ; the floor still applies.
        Plan::corp().when(Match::targets().once(), Reply::Targets(vec![])),
        Plan::runner().when(Match::paid().once(), Reply::take("sabotage")).stop_at_action(),
    );
    let ask = t.first_window(Kind::Targets, Side::Corp);
    match ask.spec {
        DecisionSpec::ChooseTargets { count, min, .. } => {
            assert_eq!((count, min), (4, 2), "at least 2 of the 4 must come from HQ");
        }
        _ => unreachable!(),
    }
    assert_eq!(vm.st.hand[&Side::Corp].len(), 3, "2 of the 5 went");
    assert_eq!(vm.st.deck[&Side::Corp].len(), 0, "and all of R&D");
    assert_eq!(vm.st.discard[&Side::Corp].len(), 4, "4 cards trashed in total");
}

/// example_rule_sabotage_all_remaining_cards_3 (10.12.3b): sabotage 4 with 2
/// cards in HQ and 1 in R&D — fewer than 4 between them, so the Corp trashes
/// all of both zones and there is no choice left to make.
#[test]
fn example_rule_sabotage_all_remaining_cards_3() {
    let mut vm = Vm::empty(632);
    tk::fill_hand(&mut vm, Side::Corp, 2);
    tk::fill_deck(&mut vm, Side::Corp, 1);
    tk::install_rig(&mut vm, tk::sabotage_button("Chastushka-like", 4));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::targets().once(), Reply::Targets(vec![])),
        Plan::runner().when(Match::paid().once(), Reply::take("sabotage")).stop_at_action(),
    );
    let ask = t.first_window(Kind::Targets, Side::Corp);
    match ask.spec {
        DecisionSpec::ChooseTargets { count, min, .. } => {
            assert_eq!((count, min), (2, 2), "both cards in HQ, forced");
        }
        _ => unreachable!(),
    }
    assert!(vm.st.hand[&Side::Corp].is_empty(), "all of HQ");
    assert!(vm.st.deck[&Side::Corp].is_empty(), "and all of R&D");
    assert_eq!(vm.st.discard[&Side::Corp].len(), 3);
}

/// example_rule_static_modification_keep_restrictions_1 (9.4.5): a
/// Flare-class effect does 2 meat damage that cannot be prevented. The
/// Cleaners' static ability adds 1 to the amount — and the added point
/// carries the SAME restriction, so a Plascrete-class preventer stops none
/// of the 3.
#[test]
fn example_rule_static_modification_keep_restrictions_1() {
    let mut vm = Vm::empty(633);
    tk::put_in_score_area(&mut vm, tk::cleaners_static_like("Cleaners-like"), Side::Corp);
    tk::install_root(&mut vm, tk::flare_like("Flare-like"), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::biometric_like("Plascrete-like", DamageKind::Meat));
    tk::fill_hand(&mut vm, Side::Runner, 6);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().once(), Reply::take("flare")).stop_at_action(),
        Plan::runner().always_uses("biometric"),
    );
    assert!(t.took("flare"));
    assert!(
        vm.changes.log.iter().any(|c| matches!(
            c,
            GameChange::DamageSuffered { amount: 3, kind: DamageKind::Meat, .. }
        )),
        "2 printed + 1 from the static ability: {}",
        t.tail(6)
    );
    assert_eq!(
        vm.st.hand[&Side::Runner].len(),
        3,
        "all 3 points are unpreventable — the modification kept the \
         restriction the original value carried"
    );
}

/// example_rule_paid_ability_refers_to_encountered_ice_2 (9.5.6c / 9.3.6c):
/// an Abagnale-class icebreaker's [trash] ability is usable during any
/// encounter with a code gate; its [interface] ability additionally requires
/// the breaker's strength to reach the encountered ice's.
#[test]
fn example_rule_paid_ability_refers_to_encountered_ice_2() {
    // Strength 2 against a strength-4 code gate: only the [trash] ability.
    let mut weak = Vm::empty(634);
    tk::install_ice(
        &mut weak,
        tk::subtyped_etr_ice("CodeGate-like", "code gate", 0, 4),
        ServerId::Hq,
        true,
    );
    tk::install_rig(&mut weak, tk::abagnale_like("Abagnale-like", 2));
    weak.st.runner.credits = 5;
    weak.start_turn(Side::Runner);
    let t = plan::play(
        &mut weak,
        Plan::corp(),
        Plan::runner().runs(ServerId::Hq).when(Match::jack_out().once(), Reply::Halt),
    );
    let enc: Vec<&plan::Entry> = t
        .entries
        .iter()
        .filter(|e| e.step.as_deref() == Some("step_encounter_paw") && e.side == Side::Runner)
        .collect();
    assert!(!enc.is_empty(), "the Runner had priority during the encounter: {}", t.tail(6));
    assert!(
        enc.iter().any(|e| e.offered("[trash] bypass")),
        "the [trash] ability is usable during an encounter with a code gate"
    );
    assert!(
        !enc.iter().any(|e| e.offered("interface break")),
        "the interface ability is not — strength 2 cannot interface with 4"
    );

    // Strength 4 against the same ice: both.
    let mut strong = Vm::empty(635);
    tk::install_ice(
        &mut strong,
        tk::subtyped_etr_ice("CodeGate-like", "code gate", 0, 4),
        ServerId::Hq,
        true,
    );
    tk::install_rig(&mut strong, tk::abagnale_like("Abagnale-like", 4));
    strong.st.runner.credits = 5;
    strong.start_turn(Side::Runner);
    let t2 = plan::play(
        &mut strong,
        Plan::corp(),
        Plan::runner().runs(ServerId::Hq).when(Match::jack_out().once(), Reply::Halt),
    );
    assert!(t2.ever_offered_to(Side::Runner, "interface break"));

    // A sentry instead of a code gate: neither ability refers to it.
    let mut wrong = Vm::empty(636);
    tk::install_ice(
        &mut wrong,
        tk::subtyped_etr_ice("Sentry-like", "sentry", 0, 1),
        ServerId::Hq,
        true,
    );
    tk::install_rig(&mut wrong, tk::abagnale_like("Abagnale-like", 4));
    wrong.st.runner.credits = 5;
    wrong.start_turn(Side::Runner);
    let t3 = plan::play(
        &mut wrong,
        Plan::corp(),
        Plan::runner().runs(ServerId::Hq).when(Match::jack_out().once(), Reply::Halt),
    );
    assert!(!t3.ever_offered("abagnale"), "9.5.6c: the ice must meet every stipulation");
}

// ===========================================================================
// §1.15 — targets: what can be announced, and how many (W7a)
// ===========================================================================

/// example_rule_targets_must_be_in_play_area_1 (1.15.2c): a subroutine reads
/// "The Runner trashes 1 program." — it names no zone, so only the Runner's
/// INSTALLED programs are valid targets; the ones in the grip and the stack
/// are not offered.
#[test]
fn example_rule_targets_must_be_in_play_area_1() {
    let mut vm = Vm::empty(701);
    tk::install_ice(
        &mut vm,
        tk::trash_program_sub_ice("TrashProgram-Ice", Some(Side::Runner)),
        ServerId::Hq,
        true,
    );
    let rigged = tk::install_rig(&mut vm, tk::program_mu("Rig-Program", 1));
    let in_grip = vm.new_object(tk::program_mu("Grip-Program", 1), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(in_grip);
    let in_stack = vm.new_object(tk::program_mu("Stack-Program", 1), Zone::Deck(Side::Runner));
    vm.st.deck.get_mut(&Side::Runner).unwrap().push(in_stack);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Hq).stop_at_action(),
    );
    let announce = t.of_kind(Kind::Targets);
    assert_eq!(announce.len(), 1, "one announcement: {}", t.tail(8));
    assert_eq!(
        announce[0].candidates(),
        &[rigged],
        "1.15.2c: the grip and the stack are not the play area"
    );
    assert_eq!(vm.st.objects[&rigged].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.objects[&in_grip].zone, Zone::Hand(Side::Runner));
    assert_eq!(vm.st.objects[&in_stack].zone, Zone::Deck(Side::Runner));
}

/// example_rule_distinct_targets_1 (1.15.2e): the Runner accesses an
/// Aggressive Secretary with three advancement counters, but has only two
/// installed programs. The Corp announces as many distinct targets as
/// possible — both programs, in ONE announcement — and they are trashed
/// simultaneously when the instruction resolves.
#[test]
fn example_rule_distinct_targets_1() {
    let mut vm = Vm::empty(702);
    let sec = tk::install_root(
        &mut vm,
        tk::aggressive_secretary_like("AggressiveSecretary-like"),
        ServerId::Remote(1),
        true,
    );
    tk::place_counters(&mut vm, sec, CounterKind::Advancement, 3);
    let pa = tk::install_rig(&mut vm, tk::program_mu("Program-a", 1));
    let pb = tk::install_rig(&mut vm, tk::program_mu("Program-b", 1));
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .always_uses("secretary")
            .when(Match::nested_cost(), Reply::PayCost(true)),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    let announce = t.of_kind(Kind::Targets);
    assert_eq!(
        announce.len(),
        1,
        "1.15.2d: one announcement chooses the whole set: {}",
        t.tail(10)
    );
    assert_eq!(announce[0].side, Side::Corp, "the ability's controller chooses");
    assert!(
        matches!(
            announce[0].spec,
            DecisionSpec::ChooseTargets { count: 2, min: 2, up_to: false, .. }
        ),
        "1.15.2e: X is 3 but only 2 distinct targets exist, and both must be \
         chosen: {:?}",
        announce[0].spec
    );
    assert_eq!(announce[0].candidates(), &[pa, pb]);
    assert_eq!(vm.st.objects[&pa].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.objects[&pb].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.corp.credits, 3, "the 2 credits were paid");
}

/// example_rule_target_2 (1.15.1): a Colossus-class subroutine reads "Trash 1
/// installed program and 1 installed resource." — ONE instruction that
/// requires TWO announcements (1.15.2), whose targets are the two cards that
/// will be trashed. Nothing intervenes between the announcements: the
/// instruction becomes imminent only once both are made.
#[test]
fn example_rule_target_2() {
    let mut vm = Vm::empty(703);
    tk::install_ice(&mut vm, tk::colossus_like("Colossus-like"), ServerId::Hq, true);
    let prog = tk::install_rig(&mut vm, tk::program_mu("Program-a", 1));
    let res = tk::install_rig(&mut vm, tk::vanilla_runner_card("Resource-a", jinteki_cr::object::CardType::Resource));
    let other = tk::install_rig(&mut vm, tk::program_mu("Program-b", 1));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::targets().nth(1), Reply::target(prog)),
        Plan::runner().runs(ServerId::Hq).stop_at_action(),
    );
    let announce = t.of_kind(Kind::Targets);
    assert_eq!(announce.len(), 2, "1.15.2: one announcement per choice: {}", t.tail(8));
    assert_eq!(
        announce[1].seq,
        announce[0].seq + 1,
        "both announcements happen before the instruction becomes imminent"
    );
    assert!(
        announce[0].candidates().contains(&prog) && !announce[0].candidates().contains(&res),
        "the program announcement offers programs"
    );
    assert_eq!(
        announce[1].candidates(),
        &[res],
        "the resource announcement offers resources"
    );
    assert_eq!(vm.st.objects[&prog].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.objects[&res].zone, Zone::Discard(Side::Runner));
    assert_eq!(vm.st.objects[&other].zone, Zone::Rig, "only the announced program");
}

/// example_rule_target_4 (1.15.1): the Runner encounters a barrier and uses a
/// Cleaver-class interface ability to break subroutines. The targets are the
/// 1 or 2 subroutines it will break — subroutines are targets, and 9.8.6
/// offers only the unbroken ones.
#[test]
fn example_rule_target_4() {
    let mut vm = Vm::empty(704);
    tk::install_ice(&mut vm, tk::heimdall_like("Heimdall-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::cleaver_like("Cleaver-like", 6));
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::any().once(), Reply::take("cleaver"))
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("end the run (a)"))
            .stop_at_action(),
    );
    let announce = t.of_kind(Kind::SubTargets);
    assert_eq!(announce.len(), 1, "the break ability announces once: {}", t.tail(8));
    let offered: Vec<&str> = announce[0].subroutines().iter().map(|(_, l)| *l).collect();
    assert_eq!(
        offered,
        vec!["[sub] do 1 core damage", "[sub] end the run (a)", "[sub] end the run (b)"],
        "9.8.6: every unbroken subroutine is a candidate"
    );
    assert!(
        matches!(announce[0].spec, DecisionSpec::ChooseSubroutines { count: 2, up_to: true, .. }),
        "'break up to 2' asks for up to 2: {:?}",
        announce[0].spec
    );
    // Announcing 1 of the 2 breaks exactly that one — "the 1 or 2
    // subroutines that it will break". The other two resolve: 1 core damage
    // and the run ends.
    assert_eq!(vm.st.runner.credits, 8, "the 2 credits were paid");
    assert_eq!(vm.st.runner.core_damage, 1, "the unbroken core-damage sub resolved");
    let resolved = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::SubroutineResolved { .. }))
        .count();
    assert_eq!(resolved, 2, "the announced subroutine was broken, the other two resolved");
}

/// example_rule_break_all_but_x_subroutines_targets_1 (9.8.6b): two Grappling
/// Hooks on Heimdall 1.0. The first targets the unbroken core-damage
/// subroutine and breaks both "End the run" subroutines. The second targets an
/// ALREADY-BROKEN "End the run" — legal, because the ability will not attempt
/// to break its target — and breaks the core-damage subroutine, doing nothing
/// to the other, already-broken one.
#[test]
fn example_rule_break_all_but_x_subroutines_targets_1() {
    let mut vm = Vm::empty(705);
    let ice = tk::install_ice(&mut vm, tk::heimdall_like("Heimdall-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::grappling_hook_like("Hook-a"));
    tk::install_rig(&mut vm, tk::grappling_hook_like("Hook-b"));
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().once(), Reply::take("hook"))
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("core damage"))
            .when(Match::paid().once(), Reply::take("hook"))
            .when(Match::sub_targets().once(), Reply::SubroutineNamed("end the run (a)"))
            .when(Match::jack_out().once(), Reply::Halt),
    );
    let announce = t.of_kind(Kind::SubTargets);
    assert_eq!(announce.len(), 2, "one announcement per Hook: {}", t.tail(10));
    let second: Vec<&str> = announce[1].subroutines().iter().map(|(_, l)| *l).collect();
    assert_eq!(
        second.len(),
        3,
        "9.8.6b: the broken subroutines are still valid targets — the ability \
         will not attempt to break the chosen one: {second:?}"
    );
    // Every subroutine ended up broken: the first Hook broke both "End the
    // run" subroutines, the second broke the core-damage one. Nothing
    // resolved, so the run was not ended and no core damage was suffered.
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::AllSubsBroken { ice: i } if *i == ice)),
        "9.12.2d: all three subroutines were broken"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { .. })),
        "no subroutine resolved"
    );
    assert_eq!(vm.st.runner.core_damage, 0, "the core-damage subroutine was broken");
}

/// example_rule_target_beyond_move_1 (1.15.4): a Howler-class ability targets
/// a card in HQ. Its first instruction installs that card; its second creates
/// a delayed conditional ability that refers to it in the play area. The
/// later ability finds and acts on the card without a second announcement.
#[test]
fn example_rule_target_beyond_move_1() {
    let mut vm = Vm::empty(706);
    tk::install_ice(&mut vm, tk::howler_like("Howler-like", ServerId::Hq), ServerId::Hq, true);
    let hq_ice = vm.new_object(tk::vanilla_ice("HQ-Ice", 4, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hq_ice);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Hq).stop_at_action(),
    );
    // ONE announcement — the card in HQ. The second instruction refers to it.
    let announce = t.of_kind(Kind::Targets);
    assert_eq!(announce.len(), 1, "1.15.4: no second announcement: {}", t.tail(10));
    assert_eq!(announce[0].candidates(), &[hq_ice]);
    // The delayed ability found and acted on the card it never re-selected:
    // installed during the encounter, trashed when the encounter ended.
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CardInstalled { obj, .. } if *obj == hq_ice)),
        "the announced card was installed"
    );
    assert_eq!(
        vm.st.objects[&hq_ice].zone,
        Zone::Discard(Side::Corp),
        "the delayed conditional acted on the target beyond its move"
    );
}

/// example_rule_target_1 (1.15.1): a Top-Hat-class instruction reads "you may
/// choose 1 of the top 5 cards of R&D and access it." The target is the card
/// in R&D the Runner chooses — the instruction names the zone, so 1.15.2c
/// does not confine the choice to the play area, and only the top 5 are
/// offered.
#[test]
fn example_rule_target_1() {
    let mut vm = Vm::empty(707);
    tk::install_rig(&mut vm, tk::top_hat_like("TopHat-like", 5));
    let rnd = tk::fill_deck(&mut vm, Side::Corp, 6);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .uses("top-hat")
            .when(Match::targets().once(), Reply::target(rnd[2]))
            .stop_at_action(),
    );
    let announce = t.of_kind(Kind::Targets);
    assert_eq!(announce.len(), 1, "one announcement: {}", t.tail(8));
    assert_eq!(
        announce[0].candidates(),
        &rnd[..5],
        "only the top 5 of R&D are valid targets"
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == rnd[2])),
        "the announced card was accessed: {:?}",
        vm.changes.log
    );
}

// ===========================================================================
// §9.11 — identifying instructions (W7d), and subtypes as characteristics
// ===========================================================================

/// example_rule_choose_instruction_1 (9.11.4c) and
/// example_rule_add_remove_subtypes_1 (2.16.5): a Tinkering-class play
/// ability reads "Choose a piece of ice. That ice gains sentry, code gate,
/// and barrier until the end of the turn." The first sentence only directs
/// the player to select a target, so the two sentences form ONE instruction:
/// the target is announced as it becomes imminent and the subtypes are gained
/// when it resolves. Played on a Lycan-class morph ice, which prints sentry
/// and removes one instance of sentry with its own ability, the counting of
/// 2.16.5 leaves the ice a sentry.
#[test]
fn example_rule_choose_instruction_1() {
    let mut vm = Vm::empty(708);
    let lycan = tk::install_ice(&mut vm, tk::morph_ice("Lycan-like", "sentry", "sentry"), ServerId::Hq, true);
    let other = tk::install_ice(&mut vm, tk::vanilla_ice("Plain-Ice", 1, 1), ServerId::Rnd, true);
    let tinkering = vm.new_object(tk::tinkering_like("Tinkering-like"), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(tinkering);
    tk::install_rig(&mut vm, tk::play_event_action("Play-Button", tinkering));
    vm.start_turn(Side::Runner);

    // Before: the morph ice's printed sentry is cancelled by its own ability.
    assert!(!vm.has_subtype(lycan, "sentry"), "printed once, removed once");

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .uses("play-event-action")
            .when(Match::targets().once(), Reply::target(lycan))
            .stop_at_action(),
    );
    let announce = t.of_kind(Kind::Targets);
    assert_eq!(
        announce.len(),
        1,
        "9.11.4c: choosing and modifying are ONE instruction, one announcement: {}",
        t.tail(10)
    );
    assert!(announce[0].candidates().contains(&lycan) && announce[0].candidates().contains(&other));
    // 2.16.5: printed once + gained once - removed once = still a sentry.
    assert!(vm.has_subtype(lycan, "sentry"), "two instances added, one removed");
    assert!(vm.has_subtype(lycan, "code gate"));
    assert!(vm.has_subtype(lycan, "barrier"));
    assert!(!vm.has_subtype(other, "barrier"), "only the announced target");
}

/// example_rule_add_remove_subtypes_1 (2.16.5) — asserted above, from the
/// other side: the morph ice's own removal cancels the printed instance, so
/// the subtype is present exactly while the adds outnumber the removals.
#[test]
fn example_rule_add_remove_subtypes_1() {
    let mut vm = Vm::empty(709);
    let lycan = tk::install_ice(&mut vm, tk::morph_ice("Lycan-like", "sentry", "sentry"), ServerId::Hq, true);
    assert!(!vm.has_subtype(lycan, "sentry"), "1 printed - 1 removed = not a sentry");
    let plain = tk::install_ice(&mut vm, tk::morph_ice("Plain-Morph", "sentry", "code gate"), ServerId::Rnd, true);
    assert!(vm.has_subtype(plain, "sentry"), "removing an absent subtype changes nothing");
    assert!(!vm.has_subtype(plain, "code gate"));
}

/// example_rule_choice_instruction_1 (9.11.4g): a Data-Raven-class ability
/// forces the Runner to resolve either "take 1 tag" or "end the run". Making
/// the choice ENDS the first instruction; the chosen option becomes the next
/// instruction and becomes imminent after a checkpoint — so it can still be
/// interrupted in its own right.
#[test]
fn example_rule_choice_instruction_1() {
    let mut vm = Vm::empty(710);
    tk::install_ice(&mut vm, tk::data_raven_like("DataRaven-like"), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::decoy_like("Decoy-like"));
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::options().once(), Reply::ChooseNamed("take 1 tag"))
            .when(Match::interrupt().once(), Reply::take("decoy"))
            .stop_at_action(),
    );
    let choice = t.of_kind(Kind::Options);
    assert_eq!(choice.len(), 1, "one choice instruction: {}", t.tail(10));
    assert_eq!(choice[0].choices(), &["take 1 tag", "end the run"]);
    // The chosen option became a SEPARATE instruction, imminent after a
    // checkpoint — which is exactly what let the interrupt reach it.
    assert!(t.took("decoy"), "the chosen effect became imminent in its own right");
    assert_eq!(vm.st.runner.tags, 0, "the tag was avoided");
    assert!(vm.current_run.is_none() || vm.st.runner.tags == 0);
}

/// example_rule_split_up_instruction_1 (9.11.4b): a Shipment-from-MirrorMorph
/// -class single sentence directing the player to install up to 3 cards is
/// treated as three separate instructions — three announcements, three
/// installs, each with its own checkpoint.
#[test]
fn example_rule_split_up_instruction_1() {
    let mut vm = Vm::empty(711);
    let a = vm.new_object(tk::program_cost("Prog-A", 0), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(a);
    let b = vm.new_object(tk::program_cost("Prog-B", 0), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(b);
    let c = vm.new_object(tk::program_cost("Prog-C", 0), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(c);
    tk::install_rig(&mut vm, tk::mass_install_button("Shipment-like", 3));
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().uses("mass-install").stop_at_action(),
    );
    // Three separate announcements, each offering only the cards still in
    // hand — "You may install a card from HQ." three times, not one
    // announcement of three cards.
    let announce = t.of_kind(Kind::Targets);
    assert!(announce.len() >= 3, "one announcement per install: {}", t.tail(14));
    assert_eq!(announce[0].candidates().len(), 3);
    assert!(
        announce.iter().all(|e| matches!(
            e.spec,
            DecisionSpec::ChooseTargets { count: 1, up_to: true, .. }
        )),
        "each is its own optional install"
    );
    for id in [a, b, c] {
        assert_eq!(vm.st.objects[&id].zone, Zone::Rig, "all three were installed");
    }
}

/// example_rule_step_sequences_2 (9.11.2a): the steps of a checkpoint are not
/// instructions, and no checkpoint takes place in the middle of another
/// checkpoint. One instruction trashes the top of a 3-deep hosting chain; the
/// checkpoint repeats steps 10.3.1f/g to a fixpoint INSIDE itself, and its
/// reaction window opens only afterwards (10.3.2) — so the hosted cards are
/// already gone by the time the window's ability resolves.
#[test]
fn example_rule_step_sequences_2() {
    let mut vm = Vm::empty(712);
    let host = tk::install_root(&mut vm, tk::corp_filler("Host-Asset"), ServerId::Remote(1), true);
    let guest = tk::install_root(&mut vm, tk::corp_filler("Guest-Asset"), ServerId::Remote(1), true);
    let grand = tk::install_root(&mut vm, tk::corp_filler("Grand-Asset"), ServerId::Remote(1), true);
    tk::host_on(&mut vm, guest, host);
    tk::host_on(&mut vm, grand, guest);
    tk::install_root(
        &mut vm,
        tk::hostile_infra_like("HostileInfra-like"),
        ServerId::Remote(2),
        true,
    );
    tk::install_rig(&mut vm, tk::trash_set_button("Apocalypse-like", vec![host]));
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().uses("trash the set").stop_at_action(),
    );
    // The orphan cascade ran to a fixpoint inside ONE checkpoint.
    for id in [host, guest, grand] {
        assert_eq!(vm.st.objects[&id].zone, Zone::Discard(Side::Corp));
    }
    assert_eq!(
        t.of_kind(Kind::Reaction).len(),
        1,
        "one reaction window, opened after the checkpoint completed: {}",
        t.tail(12)
    );
    let log = &vm.changes.log;
    let last_trash = log
        .iter()
        .rposition(|c| matches!(c, GameChange::CardTrashed { obj, .. } if *obj == grand))
        .expect("the deepest guest was trashed");
    let damage = log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("the reaction resolved");
    assert!(
        last_trash < damage,
        "10.3.2: the checkpoint finished — including its repeated 10.3.1f/g \
         steps — before its reaction window's ability resolved"
    );
}

// ===========================================================================
// §1.4 / §5.2 / §6.1 / §7.4 — deck construction, actions, stray run rules
// ===========================================================================

/// example_rule_influence_by_copy_1 (1.4.5a): the total influence cost of
/// out-of-faction cards is counted BY COPY, not by name — one copy of a
/// 2-influence card adds 2, two copies add 4.
#[test]
fn example_rule_influence_by_copy_1() {
    use jinteki_cr::deck::total_influence;
    let one = [(Some("anarch"), Some(2))];
    let two = [(Some("anarch"), Some(2)), (Some("anarch"), Some(2))];
    assert_eq!(total_influence(&one, "shaper"), 2);
    assert_eq!(total_influence(&two, "shaper"), 4);
    // In faction, the same cards cost nothing.
    assert_eq!(total_influence(&two, "anarch"), 0);
}

/// example_rule_54+_1 (1.4.6d): a 66-card deck requires 6 additional agenda
/// points — 3 full sets of 5 cards beyond 50 — giving 28 or 29.
#[test]
fn example_rule_54_1() {
    use jinteki_cr::deck::agenda_points_required;
    assert_eq!(agenda_points_required(66), (28, 29));
    // The banded cases the same rule sits on top of.
    assert_eq!(agenda_points_required(44), (18, 19));
    assert_eq!(agenda_points_required(45), (20, 21));
    assert_eq!(agenda_points_required(54), (22, 23));
    assert_eq!(agenda_points_required(55), (24, 25), "55 is one full set of 5 over 50");
}

/// example_rule_costs_with_click_1 (5.2.1a): a "[click]: Gain 1[credit] and
/// draw 1 card." ability is an ACTION and is offered in the action window; a
/// "Lose [click]: Break 1 subroutine on this ice." ability is not an action
/// and is used during a paid ability window.
#[test]
fn example_rule_costs_with_click_1() {
    let mut vm = Vm::empty(713);
    tk::install_ice(&mut vm, tk::etr_ice("Plain-Ice", 0, 1), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::lose_click_break_program("LoseClick-Breaker"));
    tk::install_rig(&mut vm, tk::click_action_card("ProCon-like"));
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().offering("lose-click").once(), Reply::take("lose-click"))
            .when(Match::sub_targets().once(), Reply::Default)
            .stop_at_action(),
    );
    // The [click] ability is an action; the Lose-[click] one never is.
    let actions = t.of_kind(Kind::Action);
    assert!(
        actions.iter().any(|e| e.actions().iter().any(|a| matches!(
            a,
            ActionOption::CardAction { label, .. } if label.contains("procon")
        ))),
        "5.2.1a: a [click] cost makes the ability an action: {}",
        t.tail(10)
    );
    assert!(
        actions
            .iter()
            .all(|e| !e.actions().iter().any(|a| matches!(
                a,
                ActionOption::CardAction { label, .. } if label.contains("eli")
            ))),
        "a Lose-[click] ability is not an action"
    );
    assert!(t.took("lose-click"), "it was used from a paid ability window instead");
    assert_eq!(vm.st.runner.clicks, 2, "the run took 1 click, losing [click] took another");
}

/// example_rule_action_timing_structure_completion_1 (5.2.2b): the Runner
/// takes the "play an event" action with a Stimhack-class event. The action
/// is not complete until the run it initiated ends, the core damage is
/// suffered and the event is trashed following its resolution.
#[test]
fn example_rule_action_timing_structure_completion_1() {
    let mut vm = Vm::empty(714);
    let stim = vm.new_object(tk::stimhack_like("Stimhack-like", ServerId::Hq), Zone::Hand(Side::Runner));
    vm.st.hand.get_mut(&Side::Runner).unwrap().push(stim);
    tk::install_rig(&mut vm, tk::play_event_action("Play-Button", stim));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().uses("play-event-action").stop_at_action_nth(2),
    );
    let log = &vm.changes.log;
    let run_ended = log
        .iter()
        .position(|c| matches!(c, GameChange::RunEnded { .. }))
        .expect("the run ended");
    let damaged = log
        .iter()
        .position(|c| matches!(c, GameChange::DamageSuffered { .. }))
        .expect("core damage suffered");
    let trashed = log
        .iter()
        .position(|c| matches!(c, GameChange::CardTrashed { obj, .. } if *obj == stim))
        .expect("the event was trashed after resolving");
    assert!(run_ended < damaged && damaged < trashed, "5.2.2b order: {log:?}");
    assert_eq!(vm.st.runner.core_damage, 1);
    // Only THEN does the next action window open.
    let last = t.last().expect("a decision");
    assert_eq!(last.kind(), Kind::Action, "the action completed first: {}", t.tail(6));
    assert!(last.seq > 1);
}

/// example_rule_end_run_no_run_or_encounter_1 (6.1.4c): a Lycian-class
/// ability gains 1 credit and ends the run. Used with no run and no encounter
/// in progress, the Corp gains the credit and nothing else happens.
#[test]
fn example_rule_end_run_no_run_or_encounter_1() {
    let mut vm = Vm::empty(715);
    tk::install_root(
        &mut vm,
        tk::gain_and_etr_button("Munition-like"),
        ServerId::Remote(1),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().uses("munition").stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("munition"), "the ability resolved: {}", t.tail(6));
    assert_eq!(vm.st.corp.credits, 1, "the credit was gained");
    assert!(vm.current_run.is_none() && vm.st.encounter.is_none());
    assert!(vm.game_over.is_none(), "no further effect");
}

/// example_rule_candidates_leaving_server_1 (7.4.5): the Runner trashes an
/// installed upgrade during a breach of Archives. The card moves to the
/// Corp's discard pile, and the NEW object for it there becomes a candidate,
/// so the Runner accesses the same physical card twice in one breach.
#[test]
fn example_rule_candidates_leaving_server_1() {
    let mut vm = Vm::empty(716);
    let mut up = tk::vanilla_upgrade("Archives-Upgrade", 0);
    up.trash_cost = Some(1);
    let upgrade = tk::install_root(&mut vm, up, ServerId::Archives, true);
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Archives).trashes_on_access().stop_at_action(),
    );
    let accesses = vm
        .changes
        .log
        .iter()
        .filter(|c| matches!(c, GameChange::CardAccessed { obj } if *obj == upgrade))
        .count();
    assert_eq!(
        accesses, 2,
        "7.4.5/7.4.6d: the discard-pile object is derived continuously, so the \
         trashed upgrade becomes a candidate again: {}",
        t.tail(12)
    );
    assert_eq!(vm.st.objects[&upgrade].zone, Zone::Discard(Side::Corp));
}

// ===========================================================================
// §1.12 — object identity across moves (W7f)
// ===========================================================================

/// example_rule_no_memory_1 (1.12.2): the Corp uses a once-per-turn ability
/// on a card, the card is trashed, and the same physical card is reinstalled
/// from Archives. It is a NEW object, so its once-per-turn ability is
/// available again this turn.
#[test]
fn example_rule_no_memory_1() {
    let mut vm = Vm::empty(717);
    let vf = tk::install_root(
        &mut vm,
        tk::once_per_turn_asset("Vaporframe-like"),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(
        &mut vm,
        tk::corp_trash_button("Trash-Button", vec![vf]),
        ServerId::Remote(2),
        true,
    );
    tk::install_root(
        &mut vm,
        tk::corp_install_button(
            "Restore-like",
            vf,
            jinteki_cr::instr::InstallDest::Root(ServerId::Remote(3)),
        ),
        ServerId::Remote(2),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .uses("vaporframe")
            .uses("corp-trash")
            .uses("corp-install")
            .when(Match::paid().offering_pick(Pick::Rez(vf)).once(), Reply::rez(vf))
            .when(Match::paid().offering("vaporframe").once(), Reply::take("vaporframe"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        t.entries.iter().filter(|e| e.took("vaporframe")).count(),
        2,
        "1.12.2: the reinstalled card is a new object, so its once-per-turn \
         ability is available again: {}",
        t.tail(12)
    );
    assert_eq!(vm.st.objects[&vf].zone, Zone::Root(ServerId::Remote(3)));
}

/// example_rule_object_turn_faceup_facedown_1 (1.12.5): the same card is
/// derezzed and rezzed again. Turning a card faceup or facedown does not
/// change its zone, so it is still the same object and its once-per-turn
/// ability cannot be used again this turn.
#[test]
fn example_rule_object_turn_faceup_facedown_1() {
    let mut vm = Vm::empty(718);
    let vf = tk::install_root(
        &mut vm,
        tk::once_per_turn_asset("Vaporframe-like"),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(&mut vm, tk::derez_button("Divert-like", vf), ServerId::Remote(2), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .uses("vaporframe")
            .uses("divert")
            .when(Match::paid().offering_pick(Pick::Rez(vf)).once(), Reply::rez(vf))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.entries.iter().any(|e| matches!(
        &e.answer,
        Some(DecisionAnswer::Take(WindowOption::Rez { card })) if *card == vf
    )), "the card was rezzed again: {}", t.tail(12));
    assert_eq!(
        t.entries.iter().filter(|e| e.took("vaporframe")).count(),
        1,
        "1.12.5: still the same object, so the once-per-turn ability is spent"
    );
    assert!(
        !t.entries.iter().skip_while(|e| !e.took("divert")).any(|e| e.offered("vaporframe")),
        "and it is not even offered after the re-rez: {}",
        t.tail(12)
    );
}

/// example_rule_identify_object_after_move_1 (1.12.2a): a Priority-
/// Construction-class operation installs a piece of ice from HQ. The ice is a
/// new object in the play area, but the operation's SECOND instruction can
/// still find it and place advancement counters on it (1.15.4).
#[test]
fn example_rule_identify_object_after_move_1() {
    let mut vm = Vm::empty(719);
    let ice = vm.new_object(tk::vanilla_ice("HQ-Ice", 3, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ice);
    let op = vm.new_object(
        tk::priority_construction_like("PriorityConstruction-like", ServerId::Hq),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(op);
    tk::install_root(&mut vm, tk::play_operation_button("Play-Button", op), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().uses("play-op").stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(vm.st.objects[&ice].zone, Zone::Ice(ServerId::Hq), "the ice was installed");
    assert_eq!(
        vm.st.objects[&ice].counter(CounterKind::Advancement),
        3,
        "1.12.2a: the second instruction found the new object: {}",
        t.tail(12)
    );
    assert_eq!(
        t.of_kind(Kind::Targets).len(),
        1,
        "and did not need to select it again"
    );
}

/// example_rule_previous_object_2 (1.12.6): the Runner trashes the top card
/// of R&D while accessing it. The object for that card on top of R&D ceases
/// to exist, but it still counts against the number of cards the Runner can
/// access from R&D during this breach.
#[test]
fn example_rule_previous_object_2() {
    let mut vm = Vm::empty(720);
    let rnd = tk::fill_deck(&mut vm, Side::Corp, 4);
    for c in &rnd {
        vm.st.objects.get_mut(c).unwrap().printed.trash_cost = Some(0);
    }
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Rnd).trashes_on_access().stop_at_action(),
    );
    let accessed: Vec<jinteki_cr::object::ObjectId> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CardAccessed { obj } => Some(*obj),
            _ => None,
        })
        .collect();
    assert_eq!(
        accessed.len(),
        1,
        "1.12.6: the trashed object still counts against the R&D access \
         limit, so the breach is over: {}",
        t.tail(12)
    );
    assert_eq!(accessed[0], rnd[0]);
    assert_eq!(vm.st.objects[&rnd[0]].zone, Zone::Discard(Side::Corp));
}

// ===========================================================================
// §6.2 — positions (W8a): the Runner's position is an ELEMENT, not an index
// ===========================================================================

/// example_rule_ice_change_during_movement_1 (6.2.7e): a run begins, and
/// before the Runner approaches the outermost ice the Corp swaps that ice
/// with another. The Runner is not moved and the timing step does not
/// change — they approach whatever now occupies their position, which is the
/// new ice. 6.2.2f: the swap creates no position, so the position id is the
/// one the run entered.
#[test]
fn example_rule_ice_change_during_movement_1() {
    let mut vm = Vm::empty(730);
    let outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 1), ServerId::Remote(1), false);
    let other = tk::install_ice(&mut vm, tk::vanilla_ice("Other", 0, 1), ServerId::Remote(2), false);
    tk::install_root(&mut vm, tk::ice_swap_button("Yagi-like", outer, other), ServerId::Remote(3), true);
    let pos = vm.positions_at(ServerId::Remote(1))[0].id;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().at_step("step_initiation_paw").once(), Reply::take("yagi")),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    assert!(t.took("yagi"), "the swap happened during the Initiation Phase");
    assert_eq!(vm.st.objects[&other].zone, Zone::Ice(ServerId::Remote(1)));
    assert_eq!(vm.st.objects[&outer].zone, Zone::Ice(ServerId::Remote(2)));
    assert_eq!(
        vm.positions_at(ServerId::Remote(1)).iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![pos],
        "6.2.2f: a swap creates no new position"
    );
    let approached: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IceApproached { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    assert_eq!(approached, vec![other], "the Runner approaches the new ice");
}

/// example_rule_ice_change_during_movement_2 (6.2.7e): the Runner passes a
/// piece of ice and the Corp swaps it away in step 6.9.4b. The Runner has
/// still passed their POSITION — they do not pass, approach or encounter the
/// new ice — and proceed inward in step 6.9.4d.
#[test]
fn example_rule_ice_change_during_movement_2() {
    let mut vm = Vm::empty(731);
    let inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 1), ServerId::Remote(1), false);
    let outer = tk::install_ice(&mut vm, tk::vanilla_ice("Outer", 0, 1), ServerId::Remote(1), false);
    let other = tk::install_ice(&mut vm, tk::vanilla_ice("Other", 0, 1), ServerId::Remote(2), false);
    tk::install_root(&mut vm, tk::ice_swap_button("Yagi-like", outer, other), ServerId::Remote(3), true);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().at_step("step_before_jack_out_paw").once(), Reply::take("yagi")),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    assert!(t.took("yagi"), "the swap happened in step 6.9.4b");
    let approached: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IceApproached { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    let passed: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IcePassed { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    assert_eq!(approached, vec![outer, inner], "never the new ice occupying that position");
    assert_eq!(passed, vec![outer, inner], "the position they passed was the old ice's");
    assert_eq!(
        vm.ice_at(ServerId::Remote(1)),
        vec![inner, other],
        "the new ice took the outermost position"
    );
}

/// example_rule_ice_change_outward_1 (6.2.6a): the Runner is encountering the
/// outermost of 2 ice; its subroutine installs a new piece of ice protecting
/// the attacked server and trashes both of the others. The new ice occupies a
/// position OUTWARD from the Runner's, so they never approach it — even
/// though fewer positions now lie between it and the server than lay between
/// the Runner and the server a moment ago.
#[test]
fn example_rule_ice_change_outward_1() {
    let mut vm = Vm::empty(732);
    let inner = tk::install_ice(&mut vm, tk::vanilla_ice("Inner", 0, 1), ServerId::Remote(1), false);
    let fresh = vm.new_object(tk::vanilla_ice("Fresh", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(fresh);
    let drafter = tk::install_ice(
        &mut vm,
        tk::drafter_like("Drafter-like", fresh, ServerId::Remote(1)),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 3), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::targets(), Reply::Targets(vec![inner, drafter])),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    assert_eq!(vm.st.objects[&fresh].zone, Zone::Ice(ServerId::Remote(1)), "{}", t.tail(8));
    assert_eq!(vm.st.objects[&inner].zone, Zone::Discard(Side::Corp));
    assert_eq!(vm.st.objects[&drafter].zone, Zone::Discard(Side::Corp));
    let approached: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IceApproached { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    assert_eq!(approached, vec![drafter], "6.2.6a: the new outermost ice is never approached");
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "the run reached the server: {}",
        t.tail(8)
    );
    assert_eq!(
        vm.ice_at(ServerId::Remote(1)),
        vec![fresh],
        "10.3.1i: the vacated positions ceased once the Runner left them"
    );
}

/// example_rule_ice_change_inward_1 (6.2.6b): a Brân-class subroutine
/// installs a new piece of ice in the next INWARD position while the Runner
/// is encountering it. That position is inward from the Runner's, so they
/// approach the new ice later in the same run.
#[test]
fn example_rule_ice_change_inward_1() {
    let mut vm = Vm::empty(733);
    let fresh = vm.new_object(tk::vanilla_ice("Fresh", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(fresh);
    let bran = tk::install_ice(&mut vm, tk::bran_like("Bran-like", fresh), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 3), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    assert_eq!(
        vm.ice_at(ServerId::Remote(1)),
        vec![fresh, bran],
        "6.2.2c: the new position is inward from Brân's"
    );
    let approached: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IceApproached { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    assert_eq!(
        approached,
        vec![bran, fresh],
        "6.2.6b: the Runner approaches the new ice later in this run: {}",
        t.tail(8)
    );
}

/// example_rule_count_positions_1 (6.2.3): a Rook-class program hosted on the
/// outermost of 2 pieces of ice can move to the 2nd piece of ice protecting
/// ANY server, counted from the innermost outward — and cannot move to a
/// server protected by only 1 piece of ice.
#[test]
fn example_rule_count_positions_1() {
    let mut vm = Vm::empty(734);
    let a_inner = tk::install_ice(&mut vm, tk::vanilla_ice("A-in", 0, 1), ServerId::Remote(1), false);
    let a_outer = tk::install_ice(&mut vm, tk::vanilla_ice("A-out", 0, 1), ServerId::Remote(1), false);
    let b_inner = tk::install_ice(&mut vm, tk::vanilla_ice("B-in", 0, 1), ServerId::Remote(2), false);
    let b_outer = tk::install_ice(&mut vm, tk::vanilla_ice("B-out", 0, 1), ServerId::Remote(2), false);
    let c_only = tk::install_ice(&mut vm, tk::vanilla_ice("C-only", 0, 1), ServerId::Remote(3), false);
    let rook = tk::install_rig(&mut vm, tk::rook_like("Rook-like"));
    tk::host_on(&mut vm, rook, a_outer);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("rook"))
            .when(Match::targets().once(), Reply::Targets(vec![b_outer]))
            .stop_at_action(),
    );
    let offer = t.first_window(Kind::Targets, Side::Runner).candidates().to_vec();
    assert!(offer.contains(&b_outer), "the 2nd piece of ice protecting another server");
    assert!(!offer.contains(&c_only), "a server protected by only 1 piece of ice has no 2nd position");
    assert!(!offer.contains(&a_inner) && !offer.contains(&b_inner), "the innermost positions differ");
    assert_eq!(vm.st.objects[&rook].host, Some(b_outer));
    assert_eq!(vm.st.objects[&rook].zone, Zone::Ice(ServerId::Remote(2)), "1.13.12");
}

/// example_rule_count_positions_2 (6.2.3 / 6.2.8a): the Runner passes the ice
/// in the innermost position protecting a server and moves to the "same
/// position" protecting another server. Positions are compared from the
/// innermost, so the innermost ice of any server qualifies however many
/// pieces of ice are outward from it — and the run continues on that server.
#[test]
fn example_rule_count_positions_2() {
    let mut vm = Vm::empty(735);
    let a_only = tk::install_ice(&mut vm, tk::vanilla_ice("A-only", 0, 1), ServerId::Remote(1), false);
    let b0 = tk::install_ice(&mut vm, tk::vanilla_ice("B-0", 0, 1), ServerId::Remote(2), false);
    let b1 = tk::install_ice(&mut vm, tk::vanilla_ice("B-1", 0, 1), ServerId::Remote(2), false);
    let b2 = tk::install_ice(&mut vm, tk::vanilla_ice("B-2", 0, 1), ServerId::Remote(2), false);
    tk::install_rig(&mut vm, tk::slipstream_like("Slipstream-like"));
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 3), ServerId::Remote(2), false);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Remote(1))
            .when(
                Match::paid().at_step("step_before_jack_out_paw").once(),
                Reply::take("slipstream"),
            )
            .when(Match::targets().once(), Reply::Targets(vec![b0]))
            .stop_at_action(),
    );
    let offer = t.first_window(Kind::Targets, Side::Runner).candidates().to_vec();
    assert!(offer.contains(&b0), "the innermost ice of another server is the same position");
    assert!(
        !offer.contains(&b1) && !offer.contains(&b2),
        "however many pieces of ice are outward from it"
    );
    let approached: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IceApproached { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    assert_eq!(approached, vec![a_only, b0], "6.2.8a: the Runner approaches the ice moved to");
    assert!(
        vm.changes.log.iter().any(
            |c| matches!(c, GameChange::RunDeclaredSuccessful { server } if *server == ServerId::Remote(2))
        ),
        "6.2.8a: that server became the attacked server: {}",
        t.tail(10)
    );
}

/// example_rule_ice_change_encounter_move_swap_1 (6.2.7d): a Bullfrog-class
/// subroutine moves the ice being encountered to the outermost position
/// protecting another server. The Runner stays WITH the ice: that server
/// becomes the attacked server and the run continues from the ice's new
/// position, working inward from it.
#[test]
fn example_rule_ice_change_encounter_move_swap_1() {
    let mut vm = Vm::empty(736);
    let arch = tk::install_ice(&mut vm, tk::vanilla_ice("Arch-ice", 0, 1), ServerId::Archives, false);
    let frog = tk::install_ice(
        &mut vm,
        tk::bullfrog_like("Bullfrog-like", ServerId::Archives),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 3), ServerId::Remote(1), false);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    assert_eq!(
        vm.ice_at(ServerId::Archives),
        vec![arch, frog],
        "moved to the outermost position protecting Archives"
    );
    let approached: Vec<_> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::IceApproached { ice } => Some(*ice),
            _ => None,
        })
        .collect();
    assert_eq!(
        approached,
        vec![frog, arch],
        "the run continued inward from Bullfrog's new position: {}",
        t.tail(10)
    );
    assert!(
        vm.changes.log.iter().any(
            |c| matches!(c, GameChange::RunDeclaredSuccessful { server } if *server == ServerId::Archives)
        ),
        "the Runner is now running on Archives"
    );
}

// ===========================================================================
// §8.8 — swapping cards (W8b)
// ===========================================================================

/// example_rule_swap_installed_cards_preserves_hosting_1 (8.8.3a): a
/// Thimblerig-class ice swaps itself with another piece of ice that is
/// hosting a Runner program. The program remains hosted on its host
/// throughout, and follows it to its new position (1.13.12).
#[test]
fn example_rule_swap_installed_cards_preserves_hosting_1() {
    let mut vm = Vm::empty(737);
    let palisade = tk::install_ice(&mut vm, tk::vanilla_ice("Palisade", 0, 1), ServerId::Remote(1), true);
    let thimble = tk::install_ice(&mut vm, tk::thimblerig_like("Thimblerig-like"), ServerId::Remote(2), true);
    let botulus = tk::install_rig(&mut vm, tk::virus_program("Botulus-like", 0));
    tk::host_on(&mut vm, botulus, palisade);
    tk::place_counters(&mut vm, botulus, CounterKind::Virus, 3);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("thimblerig"))
            .when(Match::targets().once(), Reply::Targets(vec![palisade]))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("thimblerig"), "{}", t.tail(6));
    assert_eq!(vm.st.objects[&palisade].zone, Zone::Ice(ServerId::Remote(2)));
    assert_eq!(vm.st.objects[&thimble].zone, Zone::Ice(ServerId::Remote(1)));
    assert_eq!(
        vm.st.objects[&botulus].host,
        Some(palisade),
        "8.8.3a: the hosting relationship is maintained"
    );
    assert_eq!(
        vm.st.objects[&botulus].zone,
        Zone::Ice(ServerId::Remote(2)),
        "1.13.12: the hosted object is in its host's zone"
    );
    assert_eq!(vm.st.objects[&botulus].counter(CounterKind::Virus), 3);
    assert!(
        !vm.changes.log.iter().any(
            |c| matches!(c, GameChange::CardTrashed { obj, .. } if *obj == botulus)
        ),
        "nothing hosted was trashed: both cards stayed installed (8.8.3)"
    );
}

/// example_rule_swap_only_to_valid_location_1 (8.8.2): a Metamorph-class
/// subroutine swaps 2 installed cards. Having announced an agenda, the Corp
/// cannot choose an upgrade that would put that agenda in the same remote
/// server as an asset — that upgrade is not a legal exchange, and is not
/// offered.
#[test]
fn example_rule_swap_only_to_valid_location_1() {
    let mut vm = Vm::empty(738);
    let agenda = tk::install_root(&mut vm, tk::vanilla_agenda("Agenda", 3, 2), ServerId::Remote(1), false);
    let upg_with_asset =
        tk::install_root(&mut vm, tk::vanilla_upgrade("Upg-A", 0), ServerId::Remote(2), false);
    let asset = tk::install_root(&mut vm, tk::vanilla_asset("Asset", 0, 3), ServerId::Remote(2), false);
    let upg_alone =
        tk::install_root(&mut vm, tk::vanilla_upgrade("Upg-B", 0), ServerId::Remote(3), false);
    tk::install_ice(&mut vm, tk::metamorph_like("Metamorph-like"), ServerId::Hq, true);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::targets().once(), Reply::Targets(vec![agenda]))
            .when(Match::targets().once(), Reply::Targets(vec![upg_alone])),
        Plan::runner().runs(ServerId::Hq).stop_at_action(),
    );
    let second = t.nth_window(Kind::Targets, Side::Corp, 2).candidates().to_vec();
    assert!(
        !second.contains(&upg_with_asset),
        "8.8.2: the agenda would end up in the same remote as an asset"
    );
    assert!(second.contains(&upg_alone), "a legal exchange is still offered: {second:?}");
    assert_eq!(vm.st.objects[&agenda].zone, Zone::Root(ServerId::Remote(3)));
    assert_eq!(vm.st.objects[&upg_alone].zone, Zone::Root(ServerId::Remote(1)));
    assert_eq!(vm.st.objects[&asset].zone, Zone::Root(ServerId::Remote(2)), "untouched");
}

/// example_rule_swap_become_installed_1 (8.8.4b): the Runner passes a
/// Tatu-Bola-class ice, whose ability swaps it with a piece of ice from HQ.
/// The HQ card becomes installed in the exact position the first occupied,
/// without the install procedure; the trigger conditions of installing it are
/// met at the next checkpoint, so an A-Teia-class "whenever you install"
/// ability fires — and the swapping ability then goes on resolving its next
/// instruction.
#[test]
fn example_rule_swap_become_installed_1() {
    let mut vm = Vm::empty(739);
    let hq_ice = vm.new_object(tk::vanilla_ice("HQ-Ice", 0, 1), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(hq_ice);
    let teia_card = vm.new_object(tk::vanilla_upgrade("Teia-Installee", 0), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(teia_card);
    tk::install_identity(
        &mut vm,
        tk::a_teia_like("A-Teia-like", teia_card, ServerId::Remote(2)),
        Side::Corp,
    );
    let tatu = tk::install_ice(
        &mut vm,
        tk::tatu_bola_like("Tatu-Bola-like", hq_ice),
        ServerId::Remote(1),
        true,
    );
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 3), ServerId::Remote(1), false);
    tk::install_root(&mut vm, tk::vanilla_upgrade("Anchor", 0), ServerId::Remote(2), false);
    let pos = vm.positions_at(ServerId::Remote(1))[0].id;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().uses("a-teia"),
        Plan::runner().runs(ServerId::Remote(1)).stop_at_action(),
    );
    assert_eq!(vm.st.objects[&hq_ice].zone, Zone::Ice(ServerId::Remote(1)), "{}", t.tail(10));
    assert_eq!(
        vm.positions_at(ServerId::Remote(1)).iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![pos],
        "installed in the exact position the first card occupied"
    );
    assert!(!vm.st.objects[&hq_ice].faceup, "8.8.4a: a Corp card enters the play area unrezzed");
    assert_eq!(vm.st.objects[&tatu].zone, Zone::Hand(Side::Corp), "and became uninstalled");
    assert_eq!(
        vm.st.objects[&teia_card].zone,
        Zone::Root(ServerId::Remote(2)),
        "the install trigger condition was met at the next checkpoint: {}",
        t.tail(10)
    );
    assert_eq!(vm.st.corp.credits, 4, "then the next instruction of the ability resolved");
    let teia_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CardEnteredRoot { obj, .. } if *obj == teia_card))
        .expect("A Teia installed");
    let gain_at = vm
        .changes
        .log
        .iter()
        .position(|c| matches!(c, GameChange::CreditsGained { side: Side::Corp, amount: 4 }))
        .expect("gained 4");
    assert!(teia_at < gain_at, "the reaction resolved before the next instruction (9.1.2a)");
}

// ===========================================================================
// §1.16 — costs, continued (W8c)
// ===========================================================================

/// example_rule_install_and_rez_reducing_total_1 (1.16.2f): a Tucana-class
/// ability installs and rezzes a piece of ice "paying a total of 3[credit]
/// less". The server already has 1 piece of ice, so the install cost is
/// 1[credit] and the rez cost 6[credit]. The Corp declares the split —
/// 1[credit] off the install, 2[credit] off the rez — leaving 0 and 4.
#[test]
fn example_rule_install_and_rez_reducing_total_1() {
    let mut vm = Vm::empty(740);
    tk::install_ice(&mut vm, tk::vanilla_ice("Guard", 0, 1), ServerId::Remote(1), false);
    let logjam = vm.new_object(tk::vanilla_ice("Logjam", 6, 4), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(logjam);
    tk::install_root(
        &mut vm,
        tk::total_discount_install_rez("Tucana-total", logjam, ServerId::Remote(1), 3),
        ServerId::Remote(2),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 4;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("tucana-total"))
            // 1[credit] of the 3 goes to the install cost, 2 to the rez cost.
            .when(Match::cost_division().once(), Reply::Divide(1))
            .stop_at_action(),
        Plan::runner(),
    );
    let div = t.first_window(Kind::CostDivision, Side::Corp);
    assert!(
        matches!(div.spec, DecisionSpec::DivideCostReduction { total: 3 }),
        "the whole modifier is declared at once: {:?}",
        div.spec
    );
    assert_eq!(vm.st.objects[&logjam].zone, Zone::Ice(ServerId::Remote(1)));
    assert!(vm.st.objects[&logjam].faceup, "installed AND rezzed: {}", t.tail(8));
    assert_eq!(
        vm.st.corp.credits,
        0,
        "install cost 1-1 = 0, rez cost 6-2 = 4, paid out of 4 credits"
    );
    let paid: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CostPaid { side: Side::Corp, credits, .. } => Some(*credits),
            _ => None,
        })
        .collect();
    assert!(
        paid.contains(&0) && paid.contains(&4),
        "one 0-credit install payment and one 4-credit rez payment: {paid:?}"
    );
}

/// example_rule_cost_quantities_1 (1.16.2b): a Cayambe-class nested cost of
/// "2[credit] for each piece of ice protecting the attacked server" is ONE
/// payment of 6[credit] against 3 pieces of ice, not 3 payments of 2 — so a
/// GameNET-class "whenever the Runner spends credits" ability meets its
/// trigger condition exactly once.
#[test]
fn example_rule_cost_quantities_1() {
    let mut vm = Vm::empty(741);
    for i in 0..3 {
        let name: &'static str =
            Box::leak(format!("Ice-{i}").into_boxed_str());
        tk::install_ice(&mut vm, tk::vanilla_ice(name, 0, 1), ServerId::Remote(1), i == 2);
    }
    tk::install_root(&mut vm, tk::cayambe_like("Cayambe-like"), ServerId::Remote(1), true);
    tk::install_identity(&mut vm, tk::gamenet_like("GameNET-like"), Side::Corp);
    vm.st.runner.credits = 10;
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().uses("gamenet"),
        Plan::runner()
            .runs(ServerId::Remote(1))
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert_eq!(vm.st.runner.credits, 4, "one payment of 6, not three of 2: {}", t.tail(10));
    let payments: Vec<u32> = vm
        .changes
        .log
        .iter()
        .filter_map(|c| match c {
            GameChange::CostPaid { side: Side::Runner, credits, .. } if *credits > 0 => {
                Some(*credits)
            }
            _ => None,
        })
        .collect();
    assert_eq!(payments, vec![6], "1.16.2b: the calculation is taken as an aggregate");
    assert_eq!(
        t.offers("gamenet"),
        1,
        "so only 1 instance of the credit-spending ability ever pends"
    );
    assert_eq!(vm.st.corp.credits, 1, "and it resolved once");
}

/// example_rule_cost_interrupt_static_mandatory_1 (1.16.1b): the Runner
/// cannot pay Obokata's additional steal cost of 4 net damage while a
/// Guru-Davinder-class card is installed — its MANDATORY interrupt would
/// prevent that damage, so the cost cannot be paid, and the choice is never
/// put to them.
#[test]
fn example_rule_cost_interrupt_static_mandatory_1() {
    let mut vm = Vm::empty(742);
    let obokata = tk::install_root(
        &mut vm,
        tk::obokata_like("Obokata-like", 3),
        ServerId::Remote(1),
        false,
    );
    tk::install_rig(&mut vm, tk::guru_like("Guru-like"));
    tk::fill_hand(&mut vm, Side::Runner, 5);
    vm.st.runner.credits = 10;
    vm.start_turn(Side::Runner);

    // The plan: run the remote, access Obokata, and pay any cost offered.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Remote(1))
            .when(Match::nested_cost(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert!(
        t.of_kind(Kind::NestedCost).is_empty(),
        "1.16.1b: an unpayable cost is never offered: {}",
        t.tail(8)
    );
    assert_eq!(vm.st.objects[&obokata].zone, Zone::Root(ServerId::Remote(1)), "not stolen");
    assert_eq!(vm.st.hand[&Side::Runner].len(), 5, "and no damage was suffered");
    assert_eq!(vm.st.runner.credits, 10);
}


// ===========================================================================
// §6.5.9 / §6.1.4b / §6.5.8c — encounters as a timing structure (W9a)
// ===========================================================================

/// example_rule_forced_encounter_1 (6.5.9a): a Shiro-class subroutine causes
/// a Chrysalis-class card to be accessed; that card's access ability forces
/// an encounter with itself. The Encounter Ice Phase is resolved on its own
/// — the Runner's position never changes — and when it is over, resolution
/// returns to Shiro's remaining subroutines.
#[test]
fn example_rule_forced_encounter_1() {
    let mut vm = Vm::empty(900);
    let shiro = tk::install_ice(&mut vm, tk::shiro_like("Shiro-like"), ServerId::Rnd, true);
    let chrysalis = vm.new_object(tk::accessed_encounter_ice("Chrysalis-like", 2, 2), Zone::Deck(Side::Corp));
    vm.st.deck.get_mut(&Side::Corp).unwrap().push(chrysalis);
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.start_turn(Side::Runner);

    // The plan: run R&D and let both of Shiro's subroutines resolve. The
    // Corp's neutral policy discharges the mandatory forced-encounter
    // instance the access pends.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Rnd)
            .when(Match::jack_out().once(), Reply::JackOut(true))
            .stop_at_action(),
    );
    let log = &vm.changes.log;
    let pos = |pred: &dyn Fn(&GameChange) -> bool| log.iter().position(pred);
    let accessed = pos(&|c| matches!(c, GameChange::CardAccessed { obj } if *obj == chrysalis))
        .expect("Shiro's first subroutine caused the access");
    let began = pos(&|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == chrysalis))
        .expect("the accessed card forced an encounter with itself");
    let ended = pos(&|c| matches!(c, GameChange::EncounterEnded { ice, .. } if *ice == chrysalis))
        .expect("the forced encounter completed");
    let gained = pos(&|c| matches!(c, GameChange::CreditsGained { side: Side::Corp, amount: 2 }))
        .expect("Shiro's second subroutine resolved");
    assert!(accessed < began && began < ended && ended < gained,
        "6.5.9a: encounter the accessed card, then return to resolving subroutines: {:?}", log);
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "the forced encounter's subroutine resolved");
    // The forced encounter interrupted the encounter with Shiro (6.5.9a);
    // that one resumed and ran to its own end afterwards.
    assert!(
        log.iter()
            .position(|c| matches!(c, GameChange::EncounterEnded { ice, .. } if *ice == shiro))
            .expect("the Shiro encounter ended too")
            > ended,
        "the interrupted encounter was still in progress and ended after the forced one: {}",
        t.tail(4)
    );
}

/// example_rule_active_exception_encounter_not_installed_1 (9.1.8h): the
/// Runner accesses an Archangel-class card in HQ and is forced to encounter
/// it. The card is not installed and therefore inactive — but 9.1.8h makes
/// its subroutines active for that encounter, so the subroutine resolves.
#[test]
fn example_rule_active_exception_encounter_not_installed_1() {
    let mut vm = Vm::empty(901);
    let archangel = vm.new_object(tk::accessed_encounter_ice("Archangel-like", 6, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(archangel);
    tk::install_rig(&mut vm, tk::hq_access_button("GangSign-like"));
    tk::fill_hand(&mut vm, Side::Runner, 4);
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().uses("access-hq").stop_at_action(),
    );
    assert!(t.took("access-hq"), "the access happened: {}", t.tail(6));
    assert!(!vm.st.objects[&archangel].zone.is_installed(), "the encountered card is in HQ");
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == archangel)),
        "the accessed card was encountered while not installed"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { ice, .. } if *ice == archangel)),
        "9.1.8h: its subroutine was active during that encounter: {:?}",
        vm.changes.log
    );
    assert_eq!(vm.st.hand[&Side::Runner].len(), 2, "and it did its 2 net damage");
}

/// example_rule_end_encounter_outside_run_1 (6.1.4b): with no run in
/// progress, the Runner accesses a card in HQ; a Ganked!-class ability forces
/// an encounter with an installed Loot-Box-class piece of ice. Its first
/// subroutine tries to end the run — there is no run, but the ENCOUNTER ends,
/// so the second subroutine never resolves.
#[test]
fn example_rule_end_encounter_outside_run_1() {
    let mut vm = Vm::empty(902);
    let loot = tk::install_ice(&mut vm, tk::loot_box_like("LootBox-like"), ServerId::Remote(1), true);
    let ganked = vm.new_object(tk::ganked_encounter_like("Ganked-like"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(ganked);
    tk::install_rig(&mut vm, tk::hq_access_button("Detente-like"));
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner().uses("access-hq").stop_at_action(),
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == loot)),
        "the forced encounter began outside any run: {}",
        t.tail(8)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterEnded { ice, .. } if *ice == loot)),
        "6.1.4b: 'end the run' with no run ended the encounter instead"
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::SubroutineResolved { ice, .. } if *ice == loot))
            .count(),
        1,
        "only the first subroutine resolved: {:?}",
        vm.changes.log
    );
    assert_eq!(vm.st.corp.credits, 0, "the second subroutine never resolved");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "6.1.4b: no step of the Run Ends Phase ran — there was no run"
    );
}

/// example_rule_forced_encounter_during_run_1 (6.5.9b): a Twins-class ability
/// forces the Runner to encounter a piece of ice again when they pass it. A
/// subroutine of that extra encounter ends the run: both the forced encounter
/// AND the Movement Phase it was initiated from are aborted, and the game
/// proceeds to the Run Ends Phase.
#[test]
fn example_rule_forced_encounter_during_run_1() {
    let mut vm = Vm::empty(903);
    let twins = tk::install_ice(&mut vm, tk::twins_ice("Twins-Ice", 1), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::break_button("Breaker"));
    vm.start_turn(Side::Runner);

    // The plan: run the remote, break the ETR subroutine in the FIRST
    // encounter so the ice is passed, then let the forced re-encounter
    // resolve it — 6.5.3 gives every encounter its own unbroken statuses.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Remote(1))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("break"))
            .stop_at_action(),
    );
    assert_eq!(
        vm.changes
            .log
            .iter()
            .filter(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == twins))
            .count(),
        2,
        "the passed ice was encountered a second time: {}",
        t.tail(10)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::SubroutineResolved { ice, .. } if *ice == twins)),
        "the unbroken subroutine resolved in the extra encounter"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { .. })),
        "6.5.9b: the Movement Phase was aborted too — the run never reached the Success Phase"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "the game proceeded to the Run Ends Phase: {:?}",
        vm.changes.log
    );
    assert!(vm.st.encounter.is_none(), "no encounter survived the run");
}

/// example_rule_bypass_during_encounter_1 (6.5.8c): the Runner plays a
/// Forked-class effect and encounters a Troll-class piece of ice — zero
/// subroutines — which they bypass with a Femme-class ability. Step 6.9.3b
/// is never reached, so the vacuous "all subroutines broken" of 9.12.2d is
/// never noted, Forked's trigger condition is not met, and the ice is not
/// trashed.
#[test]
fn example_rule_bypass_during_encounter_1() {
    let mut vm = Vm::empty(904);
    let troll = tk::install_ice(&mut vm, tk::troll_like("Troll-like"), ServerId::Remote(1), true);
    tk::install_rig(&mut vm, tk::forked_button("Forked-like", ServerId::Remote(1)));
    tk::install_rig(&mut vm, tk::femme_like("Femme-like"));
    vm.st.runner.credits = 5;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("forked"))
            .always_uses("femme")
            .when(Match::nested_cost().once(), Reply::PayCost(true))
            .stop_at_action(),
    );
    assert!(t.took("forked") && t.took("femme"), "both abilities were used: {}", t.tail(10));
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterEnded { ice, .. } if *ice == troll)),
        "the encounter ended when the ice was bypassed"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::AllSubsBroken { ice } if *ice == troll)),
        "6.5.8c: step 6.9.3b never occurred, so nothing was vacuously all-broken: {:?}",
        vm.changes.log
    );
    assert_eq!(
        vm.st.objects[&troll].zone,
        Zone::Ice(ServerId::Remote(1)),
        "the Forked-class condition was not met and the ice was not trashed"
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::IcePassed { ice } if *ice == troll)),
        "6.5.8a: the Runner immediately proceeded to pass that ice"
    );
}

/// example_rule_no_position_after_approach_server_1 (6.2.5d): a Ganked!-class
/// ability forces the Runner to encounter a Cell-Portal-class piece of ice in
/// the middle of breaching a server. The run is already in its Success Phase,
/// where the Runner has no position and cannot move to one — so the
/// subroutine's move does nothing and only the derez happens.
#[test]
fn example_rule_no_position_after_approach_server_1() {
    let mut vm = Vm::empty(905);
    let portal = tk::install_ice(&mut vm, tk::cell_portal_like("CellPortal-like"), ServerId::Remote(2), true);
    tk::install_root(&mut vm, tk::ganked_encounter_like("Ganked-like"), ServerId::Remote(1), false);
    tk::install_rig(&mut vm, tk::break_button("Breaker"));
    vm.start_turn(Side::Runner);

    // The plan: run the undefended remote; the breach accesses Ganked!, whose
    // ability forces the encounter. Halt in the forced encounter's paid
    // window, while the Success Phase is in progress.
    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Remote(1))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    script.run(&mut vm);
    assert_eq!(
        vm.st.encounter.as_ref().map(|e| e.ice),
        Some(portal),
        "the forced encounter is in progress: {}",
        script.transcript().tail(8)
    );
    assert!(
        vm.run_ctx().expect("the run is still in progress").position.is_none(),
        "6.2.5d: during the Success Phase the Runner has no position"
    );
    script.run(&mut vm);
    assert!(!vm.st.objects[&portal].faceup, "the subroutine derezzed the ice");
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::IceApproached { ice } if *ice == portal)),
        "6.2.5d: the Runner could not move to that position: {:?}",
        vm.changes.log
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunDeclaredSuccessful { server } if *server == ServerId::Remote(1))),
        "the attacked server was never changed by the refused move"
    );
}

/// example_rule_ice_strength_modification_duration_1 (3.4.4a): the Runner
/// accesses an Archangel-class card in HQ with a Gang-Sign-class ability and
/// is forced to encounter it. A Devil-Charm-class ability lowers the
/// encountered ice's strength "for the remainder of the run" — but no run is
/// in progress, so the modification lasts for the remainder of the ENCOUNTER
/// instead.
#[test]
fn example_rule_ice_strength_modification_duration_1() {
    let mut vm = Vm::empty(906);
    let archangel = vm.new_object(tk::accessed_encounter_ice("Archangel-like", 6, 0), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(archangel);
    tk::install_rig(&mut vm, tk::hq_access_button("GangSign-like"));
    tk::install_rig(&mut vm, tk::devil_charm_like("DevilCharm-like", 3));
    tk::fill_hand(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    let mut script = plan::Script::new(
        Plan::corp(),
        Plan::runner()
            .uses("access-hq")
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("devil-charm"))
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::Halt)
            .stop_at_action(),
    );
    script.run(&mut vm);
    assert!(
        vm.st.encounter.is_some() && vm.current_run.is_none(),
        "an encounter with no run in progress: {}",
        script.transcript().tail(8)
    );
    assert_eq!(
        vm.effective_strength(archangel),
        Some(3),
        "the strength reduction applies during the encounter"
    );
    script.run(&mut vm);
    assert_eq!(
        vm.effective_strength(archangel),
        Some(6),
        "3.4.4a: with no run to outlive, the reduction lasted for the encounter only"
    );
}


// ===========================================================================
// §1.18 advancement and §10.13 dividends (W9b)
// ===========================================================================

/// example_rule_placing_advancement_counter_1 (1.18.2): a Mushin-No-Shin-class
/// operation installs an Oaktown-Renovation-class agenda and PLACES 3
/// advancement counters on it. Placing counters directly is not advancing, so
/// Oaktown's "whenever you advance this card" ability gains the Corp nothing.
/// Advancing the same card afterwards does meet the condition.
#[test]
fn example_rule_placing_advancement_counter_1() {
    let mut vm = Vm::empty(910);
    let oaktown = vm.new_object(tk::vanilla_agenda("Oaktown-like", 4, 2), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(oaktown);
    tk::install_root(&mut vm, tk::advance_watcher("Oaktown-Ability"), ServerId::Remote(4), true);
    let mushin = vm.new_object(
        tk::mushin_like("Mushin-like", oaktown, ServerId::Remote(1)),
        Zone::Hand(Side::Corp),
    );
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(mushin);
    tk::install_root(&mut vm, tk::play_operation_button("Play-Op", mushin), ServerId::Remote(2), true);
    tk::install_root(&mut vm, tk::advance_button_card("Advancer", oaktown), ServerId::Remote(3), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let mut script = plan::Script::new(
        Plan::corp()
            .uses("play-op")
            .when(Match::action().once(), Reply::Halt)
            .when(Match::action().once(), Reply::take("advance target"))
            .stop_at_action(),
        Plan::runner(),
    );
    script.run(&mut vm);
    assert_eq!(
        vm.st.objects[&oaktown].counter(CounterKind::Advancement),
        3,
        "the three counters were placed on the installed agenda: {}",
        script.transcript().tail(8)
    );
    assert_eq!(
        vm.st.corp.credits, 5,
        "1.18.2: placing advancement counters directly is not advancing"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardAdvanced { .. })),
        "nothing was advanced: {:?}",
        vm.changes.log
    );
    // The discrimination: an ADVANCE of the same card does meet the condition.
    script.run(&mut vm);
    assert_eq!(vm.st.objects[&oaktown].counter(CounterKind::Advancement), 4);
    assert_eq!(vm.st.corp.credits, 7, "advancing meets 'whenever you advance this card'");
}

/// example_rule_dividends_1 (10.13.1): an Embedded-Reporting-class agenda with
/// "dividends 2" is scored with 5 advancement counters and an advancement
/// requirement of 3 — 2 excess counters, so 4 agenda counters are placed on it
/// in the Corp's score area.
#[test]
fn example_rule_dividends_1() {
    let mut vm = Vm::empty(911);
    let agenda = tk::install_root(
        &mut vm,
        tk::dividends_agenda("EmbeddedReporting-like", 3, 1, 2),
        ServerId::Remote(1),
        false,
    );
    tk::place_counters(&mut vm, agenda, CounterKind::Advancement, 5);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(Match::paid().can_score().once(), Reply::score(agenda)).stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Corp),
        "the agenda was scored: {}",
        t.tail(6)
    );
    assert_eq!(
        vm.st.objects[&agenda].counter(CounterKind::Advancement),
        0,
        "1.17.5: the advancement counters went back to the bank with the move"
    );
    assert_eq!(
        vm.st.objects[&agenda].counter(CounterKind::Agenda),
        4,
        "10.13.1: 2 agenda counters for each of the 2 excess advancement counters"
    );
}

/// example_rule_dividends_timing_1 (10.13.2): a Project-Ingatan-class agenda
/// with "dividends 1" is advanced 3 times in a server with a SanSan-class
/// upgrade, whose declaration lowers its advancement requirement to 2. Once it
/// is scored, its counters are gone and the upgrade no longer applies to it —
/// but the dividends ability reads the values as of the moment it began to be
/// scored, so 1 agenda counter is placed.
#[test]
fn example_rule_dividends_timing_1() {
    let mut vm = Vm::empty(912);
    let agenda = tk::install_root(
        &mut vm,
        tk::dividends_agenda("ProjectIngatan-like", 3, 1, 1),
        ServerId::Remote(1),
        false,
    );
    tk::install_root(&mut vm, tk::sansan_like("SanSan-like", 1), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::advance_button_card("Advancer", agenda), ServerId::Remote(2), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 5;
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::action().times(3), Reply::take("advance target"))
            // The (S) option appears as soon as the SanSan-class requirement of
            // 2 is met; the Corp waits for the third advancement before scoring.
            .when(Match::paid().can_score().nth(2), Reply::score(agenda))
            .stop_at_action(),
        Plan::runner(),
    );
    assert_eq!(t.times_taken("advance target"), 3, "the agenda was advanced 3 times");
    assert_eq!(
        vm.st.objects[&agenda].zone,
        Zone::ScoreArea(Side::Corp),
        "1.17.3a: the SanSan-class declaration let it be scored with 3: {}",
        t.tail(8)
    );
    assert_eq!(
        vm.st.objects[&agenda].counter(CounterKind::Agenda),
        1,
        "10.13.2: requirement 2 and 3 counters AS IT BEGAN TO BE SCORED — 1 excess"
    );
}


// ===========================================================================
// §6.1.2d / §6.3.2a — the attacked server; §9.3.3f / §9.11.4a — restrictions
// ===========================================================================

/// example_rule_change_attacked_server_directly_1 (6.1.2d): a Sneakdoor-Beta-
/// class ability changes the attacked server from Archives to HQ without
/// referring to the Runner's position. The run's current timing step does not
/// change, so the Runner does not approach or encounter the ice protecting
/// HQ — and the breach at step 6.9.5b is a breach of HQ.
#[test]
fn example_rule_change_attacked_server_directly_1() {
    let mut vm = Vm::empty(920);
    let hq_ice = tk::install_ice(&mut vm, tk::etr_ice("Ice-Wall", 0, 1), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::sneakdoor_like("Sneakdoor-like", ServerId::Hq));
    tk::fill_hand(&mut vm, Side::Corp, 2);
    vm.start_turn(Side::Runner);

    // The plan: run Archives (undefended), then change the attacked server
    // from inside the run.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Archives)
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("sneakdoor"),
            )
            .stop_at_action(),
    );
    assert!(t.took("sneakdoor"), "the attacked server was changed mid-run: {}", t.tail(8));
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::IceApproached { ice } if *ice == hq_ice)),
        "6.1.2d: the timing step did not change — HQ's ice was never approached: {:?}",
        vm.changes.log
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::EncounterBegan { ice, .. } if *ice == hq_ice)),
        "…nor encountered"
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::BreachBegan { server } if *server == ServerId::Hq)),
        "the run continued from where it was, against its new attacked server"
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunEnded { server, .. } if *server == ServerId::Hq)),
        "and ended as a run on HQ"
    );
}

/// example_rule_cannot_run_abilities_1 (6.3.2a): an Off-the-Grid-class
/// declaration prohibits ANNOUNCING its server as the attacked server — the
/// basic run action is not offered for it. It says nothing about a run that
/// is already in progress: an ability that changes the attacked server to
/// that server is unaffected.
#[test]
fn example_rule_cannot_run_abilities_1() {
    let mut vm = Vm::empty(921);
    tk::install_root(&mut vm, tk::off_the_grid_like("OffTheGrid-like"), ServerId::Remote(1), true);
    tk::install_root(&mut vm, tk::vanilla_asset("Bait", 0, 0), ServerId::Remote(2), false);
    tk::install_rig(
        &mut vm,
        tk::sneakdoor_like("Sneakdoor-like", ServerId::Remote(1)),
    );
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Remote(2))
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("sneakdoor"),
            )
            .stop_at_action(),
    );
    let first = t.first_window(Kind::Action, Side::Runner);
    assert!(
        !first
            .actions()
            .iter()
            .any(|a| matches!(a, ActionOption::BasicRun { server } if *server == ServerId::Remote(1))),
        "6.3.2a: the protected server cannot be announced as the attacked server: {:?}",
        first.actions()
    );
    assert!(
        first
            .actions()
            .iter()
            .any(|a| matches!(a, ActionOption::BasicRun { server } if *server == ServerId::Remote(2))),
        "every other server is still runnable"
    );
    assert!(
        vm.changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::RunEnded { server, .. } if *server == ServerId::Remote(1))),
        "…and the prohibition did not interfere with the run being moved there: {}",
        t.tail(8)
    );
}

/// example_rule_use_restrictions_1 (9.11.4a): a sentence that only restricts
/// when an ability can be used is not an instruction and not part of one. The
/// kernel keeps such a sentence as a restriction on the ability, so it gates
/// the ability's availability and contributes nothing to its resolution.
#[test]
fn example_rule_use_restrictions_1() {
    let mut vm = Vm::empty(922);
    tk::install_ice(&mut vm, tk::vanilla_ice("Ice-a", 0, 1), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::arruaceiras_like("Arruaceiras-like"));
    vm.start_turn(Side::Runner);

    // The restricted ability is offered only where its restriction allows —
    // never in the action-phase paid windows before the run.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::paid().at_step("step_encounter_paw").once(), Reply::take("arruaceiras"))
            .stop_at_action(),
    );
    let offers: Vec<&plan::Entry> =
        t.entries.iter().filter(|e| e.offered("arruaceiras")).collect();
    assert!(!offers.is_empty(), "the ability was offered somewhere: {}", t.tail(10));
    assert!(
        offers.iter().all(|e| e.step.as_deref() == Some("step_encounter_paw")),
        "9.11.4a: the restriction sentence gates the whole ability: {:?}",
        offers.iter().map(|e| e.step.clone()).collect::<Vec<_>>()
    );
    // The restriction is not an instruction: resolving the ability performed
    // exactly the one effect its instruction list holds.
    assert_eq!(vm.st.runner.tags, 1, "one instruction, one effect");
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::TagsTaken { .. })).count(),
        1
    );
}

/// example_rule_variable_restriction_1 (9.3.3f): a definition for a variable
/// ("X is the number of ice protecting this server") is a RESTRICTION, not an
/// instruction — it lives in a static ability that never resolves (9.4.1) and
/// only constrains the value X takes elsewhere. Remove the ability and X is
/// treated as 0 (9.12.2e).
#[test]
fn example_rule_variable_restriction_1() {
    let mut vm = Vm::empty(923);
    let surveyor = tk::install_ice(&mut vm, tk::surveyor_like("Surveyor-like"), ServerId::Remote(1), true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Ice-b", 0, 1), ServerId::Remote(1), true);
    vm.start_turn(Side::Runner);

    let defining: Vec<&jinteki_cr::ability::AbilityDef> = vm.st.objects[&surveyor]
        .printed
        .abilities
        .iter()
        .filter(|a| !a.statics.is_empty())
        .collect();
    assert!(!defining.is_empty(), "the X definition is carried by a static ability");
    assert!(
        defining.iter().all(|a| a.instructions.is_empty()),
        "9.3.3f: the definition is a restriction — it is not an instruction"
    );
    // And it does what a restriction does: it constrains the value X takes.
    assert_eq!(
        vm.effective_strength(surveyor),
        Some(4),
        "X = 2 per piece of ice protecting this server"
    );
    let t = plan::play(&mut vm, Plan::corp(), Plan::runner().stop_at_action());
    assert!(t.of_kind(Kind::Targets).is_empty(), "a static ability resolves nothing");
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

// ===========================================================================
// W11a — resolving an ability by class (§9.6.14d, §9.8) and "this server"
// ===========================================================================

/// example_rule_this_server_3 (4.6.6i): a Nanisivik-Grid-class ability turns a
/// facedown Border Control in Archives faceup and resolves its first
/// subroutine. Border Control was not moved between servers, so "this server"
/// in that subroutine refers to Archives — the server the ICE is in, not the
/// server the resolving card is in. The grid sits in a remote protected by
/// three pieces of ice; Archives is protected by two, so the count the
/// subroutine produces says which server was read.
#[test]
fn example_rule_this_server_3() {
    let mut vm = Vm::empty(930);
    let bc = tk::install_ice(
        &mut vm,
        tk::border_control_like("BorderControl-like"),
        ServerId::Archives,
        false,
    );
    tk::install_ice(&mut vm, tk::vanilla_ice("Archives-Ice", 0, 1), ServerId::Archives, true);
    tk::install_root(&mut vm, tk::nanisivik_like("Nanisivik-like"), ServerId::Remote(1), true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Remote-Ice-1", 0, 1), ServerId::Remote(1), true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Remote-Ice-2", 0, 1), ServerId::Remote(1), true);
    tk::install_ice(&mut vm, tk::vanilla_ice("Remote-Ice-3", 0, 1), ServerId::Remote(1), true);
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    // The plan: the Corp trashes the grid, announcing the facedown ice.
    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("nanisivik"))
            .when(Match::targets().once(), Reply::target(bc))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("nanisivik"), "the grid's ability was offered and used: {}", t.tail(8));
    assert!(vm.st.objects[&bc].faceup, "8.1.2: rezzing it ignoring costs turned it faceup");
    assert_eq!(
        vm.st.corp.credits, 2,
        "4.6.6i: 'this server' in the resolved subroutine is ARCHIVES (2 ice), \
         not the remote the grid was in (3 ice)"
    );
}

/// example_rule_instructed_to_resolve_conditional_ability_1 (9.6.14d): a
/// 24/7-News-Cycle-class operation forfeits an agenda and resolves the "when
/// scored" ability of an agenda already in the score area. Nothing was scored,
/// so the ability is marked PENDING as though the stipulation had occurred —
/// and it resolves from the ordinary reaction window, placing its counter.
#[test]
fn example_rule_instructed_to_resolve_conditional_ability_1() {
    let mut vm = Vm::empty(931);
    let fodder = tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Fodder", 3, 1), Side::Corp);
    let astro =
        tk::put_in_score_area(&mut vm, tk::when_scored_agenda("Astro-like", 3, 3, false), Side::Corp);
    let market = tk::put_in_score_area(
        &mut vm,
        tk::when_scored_agenda("MarketResearch-like", 4, 2, true),
        Side::Corp,
    );
    let cycle = vm.new_object(tk::news_cycle_like("NewsCycle-like"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(cycle);
    tk::install_root(
        &mut vm,
        tk::play_operation_button("Play-Button", cycle),
        ServerId::Remote(1),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("play-op"))
            .when(Match::targets().once(), Reply::target(astro))
            .when(Match::reaction(), Reply::take("when scored"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("play-op"), "the operation was played: {}", t.tail(10));
    assert_eq!(
        vm.st.objects[&fodder].zone,
        Zone::RemovedFromGame,
        "8.2.5: the additional cost forfeited an agenda"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::AgendaScored { .. })),
        "9.6.14d: nothing was actually scored"
    );
    let reaction = t
        .windows(Kind::Reaction, Side::Corp)
        .into_iter()
        .find(|e| e.offered("when scored"))
        .expect("9.6.14d: the ability is marked PENDING, so it is offered in a reaction window");
    assert!(reaction.took("when scored"), "…and the Corp resolved it from that window");
    assert_eq!(
        vm.st.objects[&astro].counter(CounterKind::Agenda),
        1,
        "the 'when scored' ability resolved and placed its counter"
    );
    assert_eq!(
        vm.st.objects[&market].counter(CounterKind::Agenda),
        0,
        "…on the chosen agenda only"
    );
}

/// example_rule_instructed_to_resolve_conditional_ability_1, second half
/// (9.6.14d): "Any additional requirements of the trigger condition in
/// question must still be met by the game state." With the Runner UNTAGGED,
/// naming the agenda whose "when scored" condition requires a tag creates no
/// pending instance at all — nothing is offered and nothing resolves. Tag the
/// Runner and the same choice places the counter.
#[test]
fn example_rule_instructed_to_resolve_conditional_ability_1_requirement() {
    for tagged in [false, true] {
        let mut vm = Vm::empty(932);
        tk::put_in_score_area(&mut vm, tk::vanilla_agenda("Fodder", 3, 1), Side::Corp);
        let market = tk::put_in_score_area(
            &mut vm,
            tk::when_scored_agenda("MarketResearch-like", 4, 2, true),
            Side::Corp,
        );
        let cycle = vm.new_object(tk::news_cycle_like("NewsCycle-like"), Zone::Hand(Side::Corp));
        vm.st.hand.get_mut(&Side::Corp).unwrap().push(cycle);
        tk::install_root(
            &mut vm,
            tk::play_operation_button("Play-Button", cycle),
            ServerId::Remote(1),
            true,
        );
        tk::fill_deck(&mut vm, Side::Corp, 5);
        if tagged {
            vm.st.runner.tags = 1;
        }
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::paid().once(), Reply::take("play-op"))
                .when(Match::targets().once(), Reply::target(market))
                .when(Match::reaction(), Reply::take("when scored"))
                .stop_at_action(),
            Plan::runner(),
        );
        assert!(t.took("play-op"), "the operation was played: {}", t.tail(10));
        if tagged {
            assert!(t.ever_offered("when scored"), "9.6.5c: the requirement is met");
            assert_eq!(
                vm.st.objects[&market].counter(CounterKind::Agenda),
                1,
                "…so the ability became pending and resolved"
            );
        } else {
            assert!(
                !t.ever_offered("when scored"),
                "9.6.14d: the additional requirement is unmet, so the ability \
                 cannot even become pending: {}",
                t.tail(10)
            );
            assert_eq!(
                vm.st.objects[&market].counter(CounterKind::Agenda),
                0,
                "…and nothing resolved"
            );
        }
    }
}

// ===========================================================================
// W11b — §9.1: what "is resolving" scopes, and resolution independence
// ===========================================================================

/// example_rule_abilities_resolution_independent_1 (9.1.4): a Compile-class
/// ability and a Mayfly-class ability both arm "when this run ends" delayed
/// conditionals. When the run ends both pend; the Runner resolves Compile's
/// first, adding the program to the bottom of the stack. That zone change
/// makes the card a NEW object (1.12.3), so the ability from Mayfly — which
/// became independent of its source before the move — has nothing to trash.
#[test]
fn example_rule_abilities_resolution_independent_1() {
    let mut vm = Vm::empty(933);
    let mayfly = tk::install_rig(&mut vm, tk::mayfly_button("Mayfly-like"));
    tk::install_rig(&mut vm, tk::compile_like("Compile-like", mayfly));
    tk::fill_deck(&mut vm, Side::Runner, 3);
    vm.start_turn(Side::Runner);

    // The plan: run, arm both delayed conditionals inside the run (9.6.13d),
    // then at the run-end reaction window resolve Compile's ability FIRST.
    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .runs(ServerId::Archives)
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("compile"),
            )
            .when(
                Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
                Reply::take("mayfly"),
            )
            .when(Match::reaction().once(), Reply::take("compile-delayed"))
            .when(Match::reaction().once(), Reply::take("mayfly-delayed"))
            .stop_at_action(),
    );
    assert!(t.took("compile") && t.took("mayfly"), "both abilities armed: {}", t.tail(12));
    assert!(t.took("compile-delayed"), "the Runner resolved Compile first: {}", t.tail(30));
    assert!(t.took("mayfly-delayed"), "…and then the ability from Mayfly");
    assert_eq!(
        vm.st.objects[&mayfly].zone,
        Zone::Deck(Side::Runner),
        "the program went to the bottom of the stack"
    );
    assert!(
        !vm.changes.log.iter().any(|c| matches!(c, GameChange::CardTrashed { obj, .. } if *obj == mayfly)),
        "9.1.4 / 1.12.3: the copy on the stack is a NEW object — with nothing \
         to trash, the ability from Mayfly did nothing: {:?}",
        vm.changes.log.iter().rev().take(6).collect::<Vec<_>>()
    );
}

/// example_rule_is_resolving_1 (9.1.2b): an Attini-class declaration forbids
/// the Runner from spending credits while an ability of its source is
/// resolving. A subroutine "is resolving" during the interrupt window for its
/// instruction, so a Caldera-class credit-costed prevention is not even
/// offered there — while the same prevention IS offered against damage from a
/// piece of ice with no such declaration.
#[test]
fn example_rule_is_resolving_1() {
    for attini in [true, false] {
        let mut vm = Vm::empty(934);
        let ice = if attini {
            tk::attini_like("Attini-like")
        } else {
            let mut c = tk::vanilla_ice("Plain-Ice", 0, 3);
            c.abilities = vec![jinteki_cr::ability::AbilityDef::subroutine(vec![
                jinteki_cr::instr::Instruction::Damage {
                    kind: DamageKind::Net,
                    amount: Quantity::c(1),
                    responsible: Side::Corp,
                },
            ])
            .labeled("[sub] do 1 net damage")];
            c
        };
        tk::install_ice(&mut vm, ice, ServerId::Hq, true);
        tk::install_rig(&mut vm, tk::caldera_like("Caldera-like"));
        tk::fill_hand(&mut vm, Side::Runner, 4);
        vm.st.runner.credits = 5;
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .runs(ServerId::Hq)
                .when(Match::interrupt(), Reply::take("caldera"))
                .when(Match::jack_out(), Reply::JackOut(true))
                .stop_at_action(),
        );
        let suffered = vm
            .changes
            .log
            .iter()
            .any(|c| matches!(c, GameChange::DamageSuffered { kind: DamageKind::Net, .. }));
        if attini {
            assert!(
                !t.ever_offered("caldera"),
                "9.1.2b: the subroutine is resolving during its instruction's \
                 interrupt window, so the Runner cannot spend credits: {}",
                t.tail(12)
            );
            assert!(suffered, "…and the net damage landed");
            assert_eq!(vm.st.runner.credits, 5, "no credits were spent");
        } else {
            assert!(t.took("caldera"), "the same prevention IS offered otherwise: {}", t.tail(12));
            assert!(!suffered, "…and it prevented the damage");
        }
    }
}

/// example_rule_is_resolving_2 (9.1.2b): a Direct-Access-class event runs a
/// server and declares that identity cards do not have abilities. The
/// identity's "when a run ends" ability would meet its condition immediately
/// after the "Run any server." instruction — a reaction window OUTSIDE the
/// run, but still inside step 8.6.7f of playing the event — so the ability is
/// not present at the needed time and cannot be triggered or resolved.
#[test]
fn example_rule_is_resolving_2() {
    for via_direct_access in [true, false] {
        let mut vm = Vm::empty(935);
        tk::install_identity(&mut vm, tk::run_end_identity("Zahya-like"), Side::Runner);
        let ev = vm.new_object(
            tk::direct_access_like("DirectAccess-like", ServerId::Hq),
            Zone::Hand(Side::Runner),
        );
        vm.st.hand.get_mut(&Side::Runner).unwrap().push(ev);
        tk::install_rig(&mut vm, tk::play_event_button("Play-Button", ev));
        tk::fill_hand(&mut vm, Side::Corp, 2);
        vm.st.runner.credits = 0;
        vm.start_turn(Side::Runner);

        let runner = if via_direct_access {
            Plan::runner()
                .when(Match::paid().once(), Reply::take("play-event"))
                .when(Match::reaction(), Reply::take("identity"))
                .stop_at_action()
        } else {
            Plan::runner()
                .runs(ServerId::Hq)
                .when(Match::reaction(), Reply::take("identity"))
                .stop_at_action()
        };
        let t = plan::play(&mut vm, Plan::corp(), runner);
        assert!(
            vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
            "a run on HQ happened either way: {}",
            t.tail(12)
        );
        if via_direct_access {
            assert!(
                !t.ever_offered("identity"),
                "9.1.2b: the reaction window is still part of the event resolving, \
                 so the identity's ability is not present: {}",
                t.tail(12)
            );
            assert_eq!(vm.st.runner.credits, 0, "…and nothing resolved");
        } else {
            assert!(t.took("identity"), "without the declaration it resolves: {}", t.tail(12));
            assert_eq!(vm.st.runner.credits, 1);
        }
    }
}

// ===========================================================================
// W11c — exposing (1.21.4/9.6.4b), trashes that did not happen (8.2.2a),
// ending the run from a paid window (6.8.2a)
// ===========================================================================

/// example_rule_condition_met_multiple_times_2 (9.6.4b): a Satellite-Uplink-
/// class instruction exposes 2 cards. Exposing is not one of 9.12.2c's
/// aggregated effect classes, so the condition of a Blackguard-class ability
/// is met once per exposed card and TWO instances become pending in the next
/// checkpoint.
#[test]
fn example_rule_condition_met_multiple_times_2() {
    let mut vm = Vm::empty(936);
    let a = tk::install_root(&mut vm, tk::vanilla_asset("Facedown-A", 0, 3), ServerId::Remote(1), false);
    let b = tk::install_root(&mut vm, tk::vanilla_asset("Facedown-B", 0, 3), ServerId::Remote(2), false);
    tk::install_rig(&mut vm, tk::blackguard_like("Blackguard-like"));
    tk::install_rig(&mut vm, tk::satellite_uplink_like("SatelliteUplink-like", 2));
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp(),
        Plan::runner()
            .when(Match::paid().once(), Reply::take("satellite-uplink"))
            .when(Match::targets().once(), Reply::Targets(vec![a, b]))
            .when(Match::reaction(), Reply::take("blackguard"))
            .stop_at_action(),
    );
    assert!(t.took("satellite-uplink"), "the expose instruction resolved: {}", t.tail(10));
    assert_eq!(
        vm.changes.log.iter().filter(|c| matches!(c, GameChange::CardExposed { .. })).count(),
        2,
        "1.21.4: both installed unrezzed cards were exposed"
    );
    assert_eq!(
        t.first_window(Kind::Reaction, Side::Runner).count("blackguard"),
        2,
        "9.6.4b: two instances, one per exposed card: {}",
        t.tail(10)
    );
    assert_eq!(vm.st.runner.credits, 2, "…and both resolved");
}

/// example_rule_cancelled_movement_1 (8.2.2a): a Rototurret-class subroutine
/// would trash an installed program; the Runner uses a Sacrificial-Construct-
/// class prevention. Trashing did not occur, so a District-99-class "whenever
/// an installed Runner card is trashed" ability does not meet its trigger
/// condition — and is still able to meet it later.
#[test]
fn example_rule_cancelled_movement_1() {
    for prevented in [true, false] {
        let mut vm = Vm::empty(937);
        tk::install_ice(&mut vm, tk::rototurret_like("Rototurret-like"), ServerId::Hq, true);
        let prog = tk::install_rig(&mut vm, tk::program_cost("Program", 0));
        let d99 = tk::install_rig(&mut vm, tk::trash_counter_like("District99-like", Side::Runner));
        if prevented {
            tk::install_rig(&mut vm, tk::sac_con_like("SacCon-like", prog));
        }
        tk::fill_hand(&mut vm, Side::Corp, 2);
        vm.start_turn(Side::Runner);

        let t = plan::play(
            &mut vm,
            Plan::corp(),
            Plan::runner()
                .runs(ServerId::Hq)
                .when(Match::interrupt(), Reply::take("sac-con"))
                .when(Match::reaction(), Reply::take("district99"))
                .when(Match::jack_out(), Reply::JackOut(true))
                .stop_at_action(),
        );
        if prevented {
            assert!(t.took("sac-con"), "the prevention was used: {}", t.tail(12));
            assert_eq!(vm.st.objects[&prog].zone, Zone::Rig, "the program is still installed");
            assert_eq!(
                vm.st.objects[&d99].counter(CounterKind::Power),
                0,
                "8.2.2a: trashing did not occur, so the condition was not met: {}",
                t.tail(12)
            );
            assert!(
                !t.ever_offered("district99"),
                "…and no instance was ever pending"
            );
        } else {
            assert_eq!(vm.st.objects[&prog].zone, Zone::Discard(Side::Runner));
            assert_eq!(
                vm.st.objects[&d99].counter(CounterKind::Power),
                1,
                "…while a trash that DID occur meets it: {}",
                t.tail(12)
            );
        }
    }
}

/// example_rule_run_ends_close_paws_1 (6.8.2a): the Corp spends a counter from
/// a scored Nisei-MK-II-class agenda to end the run, from inside a paid ability
/// window. That window closes: the Runner gets no further opportunity in it to
/// spend the bad publicity credits from this run.
#[test]
fn example_rule_run_ends_close_paws_1() {
    let mut vm = Vm::empty(938);
    let nisei = tk::put_in_score_area(&mut vm, tk::nisei_like("Nisei-like", 3, 2), Side::Corp);
    tk::place_counters(&mut vm, nisei, CounterKind::Agenda, 1);
    tk::install_ice(&mut vm, tk::vanilla_ice("HQ-Ice", 0, 1), ServerId::Hq, true);
    tk::install_rig(&mut vm, tk::smc_credit_button("SMC-like", 2));
    tk::fill_hand(&mut vm, Side::Corp, 2);
    // The run's bad publicity fund is the only thing the Runner could pay with
    // (6.4.2: 1 credit per bad publicity, for this run only).
    vm.st.corp.bad_publicity = 2;
    vm.st.runner.credits = 0;
    vm.start_turn(Side::Runner);

    let t = plan::play(
        &mut vm,
        Plan::corp().when(
            Match::paid().during(jinteki_cr::timing::StructKind::Run).once(),
            Reply::take("nisei"),
        ),
        Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::jack_out(), Reply::JackOut(true))
            .stop_at_action(),
    );
    let nisei_at = t
        .entries
        .iter()
        .find(|e| e.took("nisei"))
        .expect("the Corp ended the run from a paid window")
        .seq;
    assert!(
        t.entries.iter().any(|e| e.seq < nisei_at && e.offered("smc")),
        "the Runner could spend the fund BEFORE the run was ended: {}",
        t.tail(14)
    );
    assert!(
        t.entries.iter().all(|e| e.seq <= nisei_at || !e.offered("smc")),
        "6.8.2a: the paid window that was open when the run ended closes — the \
         Runner gets no further opportunity to spend the fund: {}",
        t.tail(14)
    );
    assert!(
        vm.changes.log.iter().any(|c| matches!(c, GameChange::RunEnded { .. })),
        "the run ended"
    );
    assert_eq!(vm.st.bp_fund, 0, "6.9.6b: the fund is emptied when the run ends");
    assert_eq!(vm.st.runner.credits, 0, "…and nothing was spent from it");
}

/// example_rule_prevent_as_trigger_condition_1 (9.9.7f): 2 meat damage is
/// imminent. A Plascrete-class interrupt prevents 2, leaving the expected
/// effect at 0 — a value The Cleaners could still modify. Then a
/// Guru-Davinder-class interrupt removes the damage effect altogether. Since
/// the value was already 0, Guru Davinder's conditional ability does NOT meet
/// its trigger condition, and with no imminent damage effect left the Corp
/// cannot use The Cleaners.
#[test]
fn example_rule_prevent_as_trigger_condition_1() {
    for plascrete_first in [true, false] {
        let mut vm = Vm::empty(939);
        tk::put_in_score_area(&mut vm, tk::cleaners_like("Cleaners-like"), Side::Corp);
        tk::install_root(
            &mut vm,
            tk::meat_damage_button("Angelique-like", 2),
            ServerId::Remote(1),
            true,
        );
        let guru = tk::install_rig(&mut vm, tk::guru_davinder_like("Guru-like", DamageKind::Meat));
        if plascrete_first {
            tk::install_rig(&mut vm, tk::biometric_like("Plascrete-like", DamageKind::Meat));
        }
        tk::fill_hand(&mut vm, Side::Runner, 5);
        vm.st.runner.credits = 4;
        vm.start_turn(Side::Corp);

        let t = plan::play(
            &mut vm,
            Plan::corp()
                .when(Match::paid().once(), Reply::take("do meat damage"))
                .stop_at_action(),
            Plan::runner()
                .when(Match::interrupt(), Reply::take("biometric"))
                .when(Match::interrupt(), Reply::take("guru: prevent"))
                .when(Match::nested_cost(), Reply::PayCost(true)),
        );
        let guru_at = t
            .entries
            .iter()
            .find(|e| e.took("guru: prevent"))
            .map(|e| e.seq)
            .unwrap_or(usize::MAX);
        assert!(
            t.entries.iter().any(|e| e.seq < guru_at && e.offered("cleaners")),
            "9.9.7b: a 0 value is still a value The Cleaners could modify: {}",
            t.tail(14)
        );
        assert!(
            t.entries.iter().all(|e| e.seq <= guru_at || !e.offered("cleaners")),
            "…and once the damage effect is removed entirely there is nothing \
             left for the Corp to modify: {}",
            t.tail(14)
        );
        assert!(t.took("guru: prevent"), "the prevent-all interrupt resolved: {}", t.tail(14));
        assert!(
            !vm.changes.log.iter().any(|c| matches!(
                c,
                GameChange::DamageSuffered { kind: DamageKind::Meat, .. }
            )),
            "no meat damage was suffered either way"
        );
        if plascrete_first {
            assert!(t.took("biometric"), "…after the value was already reduced to 0");
            assert!(
                !t.ever_offered("guru: pay 4"),
                "9.9.7f: the damage value was 0, so removing the effect prevented \
                 nothing and the conditional ability was never pending: {}",
                t.tail(14)
            );
            assert_eq!(vm.st.runner.credits, 4, "…and nothing was paid");
            assert_eq!(vm.st.objects[&guru].zone, Zone::Rig, "…and nothing was trashed");
        } else {
            assert!(
                t.took("guru: pay 4"),
                "with a value above 0 the same removal DOES meet the condition: {}",
                t.tail(14)
            );
            assert_eq!(vm.st.runner.credits, 0, "…and the Runner paid 4");
        }
    }
}

/// example_rule_look_reveal_instruction_1 (9.11.4e): an Architect-class
/// subroutine looks at the top 5 cards of R&D and installs one of them, in one
/// printed sentence. Making the cards visible ENDS an instruction: the Corp
/// sees those 5 cards before the second instruction's target announcement, and
/// the announcement offers exactly them. The install is optional, so declining
/// leaves the second instruction with no effect.
#[test]
fn example_rule_look_reveal_instruction_1() {
    for install in [true, false] {
        let mut vm = Vm::empty(940);
        tk::install_ice(
            &mut vm,
            tk::architect_look_install("Architect-like", 5, ServerId::Remote(1)),
            ServerId::Hq,
            true,
        );
        let deck = tk::fill_deck(&mut vm, Side::Corp, 7);
        vm.start_turn(Side::Runner);

        let runner = Plan::runner()
            .runs(ServerId::Hq)
            .when(Match::jack_out(), Reply::JackOut(true))
            .stop_at_action();
        let corp = if install {
            Plan::corp()
                .when(Match::optional(), Reply::Optional(true))
                .when(Match::targets().once(), Reply::target(deck[2]))
        } else {
            Plan::corp().when(Match::optional(), Reply::Optional(false))
        };
        let t = plan::play(&mut vm, corp, runner);

        let looked: Vec<ObjectIdShim> = vm
            .changes
            .log
            .iter()
            .filter_map(|c| match c {
                GameChange::CardLookedAt { obj, .. } => Some(ObjectIdShim(*obj)),
                _ => None,
            })
            .collect();
        assert_eq!(looked.len(), 5, "1.21.2: the Corp looked at the top 5 cards of R&D");
        if install {
            let ann = t
                .of_kind(Kind::Targets)
                .into_iter()
                .find(|e| e.side == Side::Corp)
                .unwrap_or_else(|| panic!("the second instruction announced a target: {}", t.tail(20)));
            assert_eq!(
                ann.candidates().len(),
                5,
                "9.11.4e: the choice is made among the cards now visible: {:?}",
                ann.candidates()
            );
            assert_eq!(
                vm.st.objects[&deck[2]].zone,
                Zone::Root(ServerId::Remote(1)),
                "…and the chosen card was installed"
            );
        } else {
            assert!(
                t.of_kind(Kind::Targets).iter().all(|e| e.side != Side::Corp),
                "9.6.9c: declining the optional install means no target is chosen"
            );
            assert!(
                !vm.changes.log.iter().any(|c| matches!(
                    c,
                    GameChange::CardInstalled { side: Side::Corp, .. }
                )),
                "…and the second instruction had no effect"
            );
        }
    }
}

/// Newtype so the look-log assertion above reads as a count, not as ids.
#[derive(Debug, PartialEq, Eq)]
struct ObjectIdShim(jinteki_cr::object::ObjectId);

/// example_rule_play_ability_1 (9.7.1): an Oppo-Research-class operation
/// carries four abilities of three types. The first is a static ability that
/// is nothing but a restriction — no declarations, no instructions. The second
/// is a conditional ability that meets its trigger condition and resolves
/// AFTER the Corp finishes playing and resolving the operation (8.6.7h). The
/// third and fourth have instructions with no trigger condition and no paid
/// trigger cost, so they are both PLAY abilities, and they resolve in sequence
/// while the operation is being played.
#[test]
fn example_rule_play_ability_1() {
    use jinteki_cr::ability::AbilityKind;
    let mut vm = Vm::empty(941);
    let oppo = vm.new_object(tk::oppo_research_like("Oppo-like"), Zone::Hand(Side::Corp));
    vm.st.hand.get_mut(&Side::Corp).unwrap().push(oppo);
    tk::install_root(
        &mut vm,
        tk::play_operation_button("Play-Button", oppo),
        ServerId::Remote(1),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.st.corp.credits = 0;
    vm.start_turn(Side::Corp);

    // 9.7.1's classification, read straight off the printed card.
    let kinds: Vec<AbilityKind> =
        vm.st.objects[&oppo].printed.abilities.iter().map(|a| a.kind).collect();
    assert_eq!(
        kinds,
        vec![AbilityKind::Static, AbilityKind::Conditional, AbilityKind::Play, AbilityKind::Play],
        "9.7.1: a restriction-only static, a conditional, and two play abilities"
    );
    assert!(
        vm.st.objects[&oppo].printed.abilities[0].statics.is_empty()
            && vm.st.objects[&oppo].printed.abilities[0].instructions.is_empty(),
        "9.3.4/9.11.4a: the first ability contains a restriction and no declarations"
    );

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("play-op"))
            .when(Match::reaction(), Reply::take("action phase ends"))
            .stop_at_action(),
        Plan::runner(),
    );
    assert!(t.took("play-op"), "the operation was played: {}", t.tail(10));
    let gained = change_at(&vm, |c| matches!(c, GameChange::CreditsGained { side: Side::Corp, .. }));
    let tagged = change_at(&vm, |c| matches!(c, GameChange::TagsTaken { .. }));
    let resolved = change_at(&vm, |c| matches!(c, GameChange::CardPlayResolved { obj } if *obj == oppo));
    assert!(gained < tagged, "the two play abilities resolved in sequence");
    assert!(
        tagged < resolved,
        "…both while the operation was being played (8.6.7f), before 8.6.7h"
    );
    assert!(
        t.took("action phase ends"),
        "8.6.7h: the conditional ability meets its condition after that: {}",
        t.tail(10)
    );
    let lost = change_at(&vm, |c| {
        matches!(c, GameChange::ClicksLost { side: Side::Corp, amount } if *amount > 0)
    });
    assert!(resolved < lost, "…and ended the Corp's action phase, after all of that");
    assert_eq!(vm.st.corp.credits, 1);
    assert_eq!(vm.st.runner.tags, 1);
}

/// example_rule_reveal_from_hidden_1 (4.1.2a): a Clone-Suffrage-Movement-class
/// ability adds an operation from Archives to HQ. The chosen card is facedown
/// in Archives, and the ability stipulates that it be an operation, so it must
/// be REVEALED before it is added — otherwise nothing would demonstrate that
/// the requirement was met.
#[test]
fn example_rule_reveal_from_hidden_1() {
    let mut vm = Vm::empty(942);
    let op = vm.new_object(tk::operation("FastBreak-like", 0, vec![]), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(op);
    let asset = vm.new_object(tk::vanilla_asset("Bait", 0, 3), Zone::Discard(Side::Corp));
    vm.st.discard.get_mut(&Side::Corp).unwrap().push(asset);
    tk::install_root(
        &mut vm,
        tk::clone_suffrage_like("CloneSuffrage-like"),
        ServerId::Remote(1),
        true,
    );
    tk::fill_deck(&mut vm, Side::Corp, 5);
    vm.start_turn(Side::Corp);

    let t = plan::play(
        &mut vm,
        Plan::corp()
            .when(Match::paid().once(), Reply::take("clone-suffrage"))
            .when(Match::targets().once(), Reply::target(op))
            .stop_at_action(),
        Plan::runner(),
    );
    let ann = t
        .of_kind(Kind::Targets)
        .into_iter()
        .next()
        .expect("the ability announced its target");
    assert_eq!(
        ann.candidates(),
        &[op],
        "1.15.2c: the criteria name the zone, and only the operation matches"
    );
    let revealed = change_at(&vm, |c| matches!(c, GameChange::CardRevealed { obj } if *obj == op));
    let moved = change_at(&vm, |c| {
        matches!(c, GameChange::CardMoved { obj, to: Zone::Hand(Side::Corp), .. } if *obj == op)
    });
    assert!(
        revealed < moved,
        "4.1.2a: the card is revealed BEFORE it is added, to demonstrate that it \
         meets the ability's stipulation: {}",
        t.tail(10)
    );
    assert_eq!(vm.st.objects[&op].zone, Zone::Hand(Side::Corp));
    assert_eq!(vm.st.objects[&asset].zone, Zone::Discard(Side::Corp));
}
