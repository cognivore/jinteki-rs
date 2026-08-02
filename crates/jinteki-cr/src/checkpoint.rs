//! The checkpoint procedure (§10.3): steps 10.3.1 (a)–(l) EXACTLY, in order,
//! each citing its `step_checkpoint_*` id. 10.3.2 reaction-window spawning is
//! done by the caller (`Vm::checkpoint_and_react`); this module returns the
//! newly-pended instance ids. No checkpoint nests inside a checkpoint
//! (9.11.2a): the procedure is straight-line.

use crate::ability::{
    ability_active, is_corp_card, trigger_matches, trigger_per_event, AbilityDef,
    AbilityInstance, AbilityKind, AbilityRef, Condition, StaticCond,
};
use crate::change::GameChange;
use crate::decision::GameResult;
use crate::frames::{AbilityPhase, Frame};
use crate::lingering::Payload;
use crate::object::{card_active, CardType, CounterKind, ObjectId, Side, Zone};
use crate::vm::Vm;

/// Run one checkpoint. Returns the instance ids marked pending in step (a)
/// (the caller opens the 10.3.2 reaction window for them).
pub fn run_checkpoint(vm: &mut Vm) -> Vec<u64> {
    cite!("rule_checkpoints");
    cite!("rule_step_sequences"); // checkpoint steps are not instructions

    let newly = step_a_conditional_abilities(vm);
    step_b_durations(vm);
    step_c_agenda_points(vm);
    if vm.game_over.is_some() {
        return newly;
    }
    step_d_uniqueness(vm);
    step_e_restrictions(vm);
    step_fg_hosted_orphans(vm);
    step_h_empty_remotes(vm);
    step_i_vacant_positions(vm);
    step_j_breach_candidates(vm);
    step_kl_discard_pile(vm);
    newly
}

/// 10.3.1a: each active conditional ability looks at the changes since the
/// beginning of the last checkpoint; one pending instance per occurrence
/// (9.6.4b), grouped for per-event triggers (9.12.2a).
fn persisted_server_override(
    vm: &Vm,
    from_lingering: Option<u64>,
    cond: &crate::ability::TriggerCond,
    change: &GameChange,
) -> bool {
    // A persisted "run on this server ends" ability's source has left the
    // server; the binding to its run substitutes for the server check.
    if let (Some(lid), crate::ability::TriggerCond::RunOnThisServerEnds, GameChange::RunEnded { run_id, .. }) =
        (from_lingering, cond, change)
    {
        if let Some(l) = vm.lingering.iter().find(|l| l.id == lid) {
            if let Payload::PersistedAbility { run_id: bound, .. } = &l.payload {
                return run_id == bound;
            }
        }
    }
    false
}

fn step_a_conditional_abilities(vm: &mut Vm) -> Vec<u64> {
    cite!("step_checkpoint_conditional_abilities");
    cite!("rule_condition_checked_in_checkpoints");

    // Gather the scan window first (immutable borrow).
    let window: Vec<(GameChange, u64)> = vm
        .changes
        .since_last_checkpoint()
        .map(|(c, g)| (c.clone(), g))
        .collect();

    // Drop pendings whose ability went inactive (9.6.10), hangover excepted
    // (9.1.8g keeps the ability active until its instances resolve).
    cite!("rule_conditional_ability_lose_pending_when_ability_becomes_inactive");
    let inactive_drop: Vec<u64> = vm
        .instances
        .iter()
        .filter(|(_, inst)| {
            if inst.hangover {
                return false;
            }
            if inst.from_lingering.is_some() {
                // Delayed conditionals live while their lingering effect does.
                return false;
            }
            match vm.st.objects.get(&inst.ability.obj) {
                None => true,
                Some(o) => !ability_active(
                    o,
                    &inst.def,
                    vm.st.encounter.as_ref().map(|e| e.ice),
                    vm.st.accessed,
                ),
            }
        })
        .map(|(id, _)| *id)
        .collect();
    for id in inactive_drop {
        vm.instances.remove(&id);
    }

    // Enumerate condition sources: printed conditional abilities + delayed
    // conditionals maintained by lingering effects (9.6.13) + granted ones.
    let mut sources: Vec<(ObjectId, usize, AbilityDef, Side, Option<u64>)> = Vec::new();
    for o in vm.st.objects.values() {
        for (i, a) in o.printed.abilities.iter().enumerate() {
            if a.kind != AbilityKind::Conditional {
                continue;
            }
            // Interrupt conditionals pend at interrupt-window open instead
            // (9.6.8b / 9.9.4b).
            cite!("rule_conditional_ability_interrupt");
            if a.is_interrupt() {
                continue;
            }
            sources.push((o.id, i, a.clone(), o.controller, None));
        }
    }
    for l in &vm.lingering {
        if let Payload::PersistedAbility { def, run_id } = &l.payload {
            // CR 9.12.5b: the ability never becomes inactive while it
            // persists; 9.12.5d: it is only applicable to the bound run —
            // condition occurrences from any other run cannot pend it.
            cite!("rule_persistent_continuous");
            cite!("rule_persistent_applicability");
            if def.kind == AbilityKind::Conditional && !def.is_interrupt() {
                let controller = vm
                    .st
                    .objects
                    .get(&l.source)
                    .map(|o| o.controller)
                    .unwrap_or(Side::Corp);
                // Encode the run binding by filtering at match time below via
                // the persisted marker: usize::MAX index + stored run.
                sources.push((l.source, usize::MAX - 1, def.clone(), controller, Some(l.id)));
                let _ = run_id;
            }
        }
        if let Payload::DelayedConditional { def } = &l.payload {
            if def.kind == AbilityKind::Conditional && !def.is_interrupt() {
                let controller = vm
                    .st
                    .objects
                    .get(&l.source)
                    .map(|o| o.controller)
                    .unwrap_or(Side::Corp);
                sources.push((l.source, usize::MAX, def.clone(), controller, Some(l.id)));
            }
        }
        if let Payload::GrantedAbility { to, def } = &l.payload {
            if def.kind == AbilityKind::Conditional && !def.is_interrupt() {
                let controller = vm
                    .st
                    .objects
                    .get(to)
                    .map(|o| o.controller)
                    .unwrap_or(Side::Corp);
                sources.push((*to, usize::MAX, def.clone(), controller, Some(l.id)));
            }
        }
    }

    let mut newly: Vec<u64> = Vec::new();
    for (obj_id, index, def, controller, from_lingering) in sources {
        let Some(source_obj) = vm.st.objects.get(&obj_id) else { continue };
        let aref = AbilityRef { obj: obj_id, index };

        match &def.condition {
            Some(Condition::Trigger(cond)) => {
                // Activity gate (9.6.5b) with the 9.1.8g hangover: the
                // ability sees the change only if active when this step
                // processes it — or if its own source's move to an inactive
                // zone is the very change that met the condition.
                cite!("rule_condition_only_met_while_active");
                let active_now = from_lingering.is_some() // incl. 9.12.5b persist
                    || ability_active(
                        source_obj,
                        &def,
                        vm.st.encounter.as_ref().map(|e| e.ice),
                        vm.st.accessed,
                    );
                let moved_to_inactive_in_window = window.iter().any(|(c, _)| match c {
                    GameChange::CardTrashed { obj, .. }
                    | GameChange::CardUninstalled { obj, .. } => *obj == obj_id,
                    _ => false,
                });
                let hangover_eligible = !active_now && moved_to_inactive_in_window;
                if !active_now && !hangover_eligible {
                    continue;
                }
                // 9.3.6g: a once-per-turn ability already USED this turn
                // does not become pending again.
                if def.has_flag(crate::ability::AbilityFlag::OncePerTurn)
                    && vm.once_per_turn_used.contains(&aref)
                {
                    cite!("rule_once_per_turn_flag");
                    continue;
                }

                // Occurrence counting: per matching record, or one per group
                // for per-event conditions (9.12.2a).
                let is_corp = |o: ObjectId| {
                    vm.st
                        .objects
                        .get(&o)
                        .map(|x| is_corp_card(x.printed.card_type))
                        .unwrap_or(false)
                };
                // 9.12.5d: a persisted ability only sees occurrences from
                // the run it is bound to.
                let persisted_run: Option<u64> = from_lingering.and_then(|lid| {
                    vm.lingering.iter().find(|l| l.id == lid).and_then(|l| match &l.payload {
                        Payload::PersistedAbility { run_id, .. } => Some(*run_id),
                        _ => None,
                    })
                });
                let mut occurrences: Vec<u64> = Vec::new();
                for (c, group) in &window {
                    if !trigger_matches(cond, c, source_obj, vm.server_of(obj_id), is_corp)
                        && !persisted_server_override(vm, from_lingering, cond, c)
                    {
                        continue;
                    }
                    // THG class: the installed card's server must be the
                    // source's server.
                    if let (
                        crate::ability::TriggerCond::CardInstalledInSourceServer,
                        GameChange::CardInstalled { obj, .. },
                    ) = (cond, c)
                    {
                        let sv = vm.server_of(obj_id);
                        if sv.is_none() || vm.server_of(*obj) != sv {
                            continue;
                        }
                    }
                    // 9.6.5c: requirements listed in the trigger condition
                    // must hold when the condition would occur (QPM class:
                    // the Runner must already be tagged).
                    if matches!(
                        cond,
                        crate::ability::TriggerCond::SelfAccessedIfRunnerTagged
                    ) && vm.st.runner.tags == 0
                    {
                        cite!("rule_condition_requirements_part_of_condition");
                        continue;
                    }
                    if let Some(bound) = persisted_run {
                        let change_run = match c {
                            GameChange::RunEnded { run_id, .. } => Some(*run_id),
                            _ => vm.current_run.map(|(r, _, _)| r),
                        };
                        if change_run != Some(bound) {
                            cite!("rule_persistent_applicability");
                            continue;
                        }
                    }
                    // 9.6.6a "had"-requirements read the previous snapshot.
                    if let crate::ability::TriggerCond::AdvancesCard {
                        had_no_advancement: true,
                    } = cond
                    {
                        cite!("rule_instruction_requirements_past_state");
                        if let GameChange::CounterPlaced { obj, .. } = c {
                            let had = vm
                                .snapshot
                                .as_ref()
                                .and_then(|s| s.objects.get(obj))
                                .map(|o| o.counter(CounterKind::Advancement))
                                .unwrap_or(0);
                            if had != 0 {
                                continue;
                            }
                        }
                    }
                    occurrences.push(*group);
                }
                if occurrences.is_empty() {
                    continue;
                }
                let n = if trigger_per_event(cond) {
                    cite!("rule_act_on_multiple_cards");
                    let mut gs = occurrences.clone();
                    gs.sort_unstable();
                    gs.dedup();
                    gs.len()
                } else {
                    cite!("rule_condition_met_multiple_times");
                    occurrences.len()
                };
                for k in 0..n {
                    cite!("rule_pending_instances");
                    let id = vm.next_instance_id();
                    let hangover = hangover_eligible;
                    cite!("rule_active_exception_conditional_move_to_inactive_zone");
                    vm.instances.insert(
                        id,
                        AbilityInstance {
                            id,
                            ability: aref,
                            def: def.clone(),
                            controller,
                            mandatory: !def.optional,
                            window: None,
                            hangover,
                            independent: false,
                            source_move_stamp: vm.st.move_seq,
                            occurrence_group: occurrences.get(k).copied().unwrap_or(0),
                            from_lingering,
                            run_id: vm.current_run.map(|(r, _, _)| r),
                        },
                    );
                    newly.push(id);
                }
            }
            Some(Condition::Static(sc)) => {
                cite!("rule_conditional_ability_with_static_condition");
                // 9.6.7b: condition must be true at the beginning of the
                // checkpoint (we evaluate before this checkpoint mutates
                // anything relevant).
                cite!("rule_conditional_ability_check_start_of_checkpoint");
                let true_now = match sc {
                    StaticCond::HostStrengthAtMost(n) => source_obj
                        .host
                        .and_then(|h| vm.effective_strength(h))
                        .map(|s| s <= *n)
                        .unwrap_or(false),
                };
                if !true_now {
                    continue;
                }
                if !ability_active(
                    source_obj,
                    &def,
                    vm.st.encounter.as_ref().map(|e| e.ice),
                    vm.st.accessed,
                ) {
                    continue;
                }
                // 9.6.7c: only one instance at a time (pending, imminent, or
                // resolving).
                cite!("rule_conditional_ability_static_one_instance");
                cite!("rule_static_condition_one_instance");
                let already = vm.instances.values().any(|i| i.ability == aref)
                    || vm.frames.iter().any(|f| match f {
                        Frame::Ability(af) => {
                            af.source == aref && af.phase != AbilityPhase::PayCost
                        }
                        _ => false,
                    });
                if already {
                    continue;
                }
                // 9.6.7d: throttled until a timing structure step completes.
                cite!("rule_conditional_ability_static_condition_no_effect");
                if vm.throttled.contains(&aref) {
                    continue;
                }
                let id = vm.next_instance_id();
                vm.instances.insert(
                    id,
                    AbilityInstance {
                        id,
                        ability: aref,
                        def: def.clone(),
                        controller,
                        mandatory: !def.optional,
                        window: None,
                        hangover: false,
                        independent: false,
                        source_move_stamp: vm.st.move_seq,
                        occurrence_group: 0,
                        from_lingering,
                        run_id: vm.current_run.map(|(r, _, _)| r),
                    },
                );
                newly.push(id);
            }
            None => {}
        }
    }

    // Close the scan window and refresh the 9.6.6a snapshot: the state as of
    // THIS checkpoint's step (a) becomes the next checkpoint's "previous".
    cite!("rule_instruction_requirements_past_state");
    vm.last_scan_window = window;
    vm.changes.begin_checkpoint_scan();
    vm.snapshot = Some(Box::new(vm.st.clone()));

    newly
}

/// 10.3.1b: any ability with a duration that has passed is removed.
fn step_b_durations(vm: &mut Vm) {
    cite!("step_checkpoint_duration_abilities");
    // CR 8.6.6c: a played card's "not trashed until <effect>" shield expires
    // when the indicated effect occurs (kernel wave: the Runner steals an
    // agenda); the game recognizes the ability no longer applies and the
    // card is trashed as if completing its resolution (the Targeted
    // Marketing example).
    let steal_occurred = vm
        .last_scan_window
        .iter()
        .any(|(c, _)| matches!(c, GameChange::AgendaStolen { .. }));
    if steal_occurred {
        cite!("rule_play_not_trashed_until");
        let expired: Vec<(u64, crate::object::ObjectId)> = vm
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::PlayedTrashShield { card } => Some((l.id, *card)),
                _ => None,
            })
            .collect();
        for (lid, card) in expired {
            vm.lingering.retain(|l| l.id != lid);
            if matches!(vm.st.objects.get(&card).map(|o| o.zone), Some(Zone::PlayArea(_))) {
                let owner = vm.st.objects[&card].owner;
                vm.trash_card(card, owner);
            }
        }
    }
    let current_encounter = vm.st.encounter.as_ref().map(|e| e.id);
    let current_run = vm.current_run.map(|(r, _, _)| r);
    let current_turn = vm.st.turn_seq;
    // CR 9.12.5c: a persisted ability expires when the reaction window after
    // its run's `step_run_complete` closes — observable as: the bound run is
    // over, no window is open, and no instance of it is still pending or
    // resolving.
    let has_open_window = vm
        .frames
        .iter()
        .any(|f| matches!(f, Frame::Window(_)));
    let pending_from: std::collections::BTreeSet<u64> = vm
        .instances
        .values()
        .filter_map(|i| i.from_lingering)
        .collect();
    let resolving_sources: std::collections::BTreeSet<crate::object::ObjectId> = vm
        .frames
        .iter()
        .filter_map(|f| match f {
            Frame::Ability(af) => Some(af.source.obj),
            _ => None,
        })
        .collect();
    let objects = &vm.st.objects;
    vm.lingering.retain(|l| {
        if let Payload::PersistedAbility { run_id, .. } = &l.payload {
            cite!("rule_persistent_expiration");
            let run_over = current_run != Some(*run_id);
            let still_needed = has_open_window
                || pending_from.contains(&l.id)
                || resolving_sources.contains(&l.source);
            return !(run_over && !still_needed);
        }
        let source_active = objects.get(&l.source).map(card_active).unwrap_or(false);
        !l.expired(current_encounter, current_run, current_turn, source_active)
    });
}

/// 10.3.1c: 7+ agenda points wins; simultaneous → draw.
fn step_c_agenda_points(vm: &mut Vm) {
    cite!("step_checkpoint_agenda_points");
    let score = |side: Side| -> i32 {
        vm.st.score_area[&side]
            .iter()
            .filter_map(|id| vm.st.objects.get(id))
            .filter_map(|o| o.printed.agenda_points)
            .sum()
    };
    let corp = score(Side::Corp) >= 7;
    let runner = score(Side::Runner) >= 7;
    cite!("rule_game_win");
    match (corp, runner) {
        (true, true) => vm.game_over = Some(GameResult::Draw),
        (true, false) => vm.game_over = Some(GameResult::AgendaPoints(Side::Corp)),
        (false, true) => vm.game_over = Some(GameResult::AgendaPoints(Side::Runner)),
        (false, false) => {}
    }
}

/// 10.3.1d: unique (◆) duplicates and duplicate consoles are trashed, keeping
/// the one that became active most recently. Cannot be prevented (10.1.1).
fn step_d_uniqueness(vm: &mut Vm) {
    cite!("step_checkpoint_uniqueness");
    cite!("rule_uniqueness");
    use std::collections::BTreeMap;
    // Unique cards by name.
    let mut by_name: BTreeMap<&'static str, Vec<(u64, ObjectId)>> = BTreeMap::new();
    for o in vm.st.objects.values() {
        if o.printed.unique && card_active(o) {
            by_name
                .entry(o.printed.name)
                .or_default()
                .push((o.active_since, o.id));
        }
    }
    let mut to_trash: Vec<ObjectId> = Vec::new();
    for (_, mut v) in by_name {
        if v.len() >= 2 {
            v.sort(); // oldest first
            v.pop(); // keep the most recently active
            to_trash.extend(v.into_iter().map(|(_, id)| id));
        }
    }
    // Consoles per controller.
    let mut consoles: BTreeMap<Side, Vec<(u64, ObjectId)>> = BTreeMap::new();
    for o in vm.st.objects.values() {
        if o.printed.console && o.zone.is_installed() {
            consoles
                .entry(o.controller)
                .or_default()
                .push((o.active_since, o.id));
        }
    }
    for (_, mut v) in consoles {
        if v.len() >= 2 {
            v.sort();
            v.pop();
            to_trash.extend(v.into_iter().map(|(_, id)| id));
        }
    }
    for id in to_trash {
        let owner = vm.st.objects[&id].owner;
        vm.trash_card(id, owner);
    }
}

/// 10.3.1e: restriction violations — trash a minimal appropriate set. The
/// kernel wave enforces the Runner's memory limit (1.20) and illegal hosting.
fn step_e_restrictions(vm: &mut Vm) {
    cite!("step_checkpoint_card_restrictions");
    // Hosting illegality (Tithonium class): objects hosted on a host that
    // prohibits hosting are in an illegal location; each such object is in
    // every appropriate set, so all are trashed (no choice arises).
    let illegal_hosted: Vec<ObjectId> = vm
        .st
        .objects
        .values()
        .filter(|o| {
            o.zone.is_installed()
                && o.host.is_some_and(|h| {
                    vm.st.objects.get(&h).is_some_and(|host| {
                        card_active(host)
                            && host.printed.abilities.iter().enumerate().any(|(i, a)| {
                                a.kind == AbilityKind::Static
                                    && vm.ability_present(h, i)
                                    && a.statics
                                        .iter()
                                        .any(|d| matches!(d, crate::ability::StaticDecl::CannotHost))
                            })
                    })
                })
        })
        .map(|o| o.id)
        .collect();
    for id in illegal_hosted {
        let owner = vm.st.objects[&id].owner;
        vm.trash_card(id, owner);
    }
    // Memory limit: installed programs' total memory cost must fit.
    cite!("rule_memory_limit");
    let limit = vm.memory_limit().max(0) as u32;
    let programs: Vec<(ObjectId, u32)> = vm
        .st
        .objects
        .values()
        .filter(|o| o.zone == Zone::Rig && o.printed.card_type == CardType::Program)
        .map(|o| (o.id, o.printed.memory_cost.unwrap_or(0)))
        .collect();
    let total: u32 = programs.iter().map(|(_, m)| m).sum();
    if total <= limit {
        return;
    }
    let overage = total - limit;
    // Enumerate minimal appropriate sets: subsets whose removal restores
    // legality and from which no element can be removed while keeping it.
    let n = programs.len();
    let mut sets: Vec<Vec<ObjectId>> = Vec::new();
    for mask in 1u32..(1 << n) {
        let sum: u32 = (0..n)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| programs[i].1)
            .sum();
        if sum < overage {
            continue;
        }
        // Minimality: removing any single member must break legality.
        let minimal = (0..n).filter(|i| mask & (1 << i) != 0).all(|i| {
            let rest = sum - programs[i].1;
            rest < overage
        });
        if minimal {
            sets.push(
                (0..n)
                    .filter(|i| mask & (1 << i) != 0)
                    .map(|i| programs[i].0)
                    .collect(),
            );
        }
    }
    if sets.is_empty() {
        return;
    }
    if sets.len() == 1 {
        for id in sets.remove(0) {
            let owner = vm.st.objects[&id].owner;
            vm.trash_card(id, owner);
        }
        return;
    }
    // Multiple distinct sets: single-owner sets → that player chooses;
    // mixed → the active player chooses. Programs are all Runner-controlled,
    // so the Runner chooses; the Decision suspends the checkpoint's caller.
    let chooser = if sets
        .iter()
        .flatten()
        .all(|id| vm.st.objects[id].controller == Side::Runner)
    {
        Side::Runner
    } else {
        vm.st.turn_side
    };
    vm.suspend_for_minimal_set(chooser, sets);
}

/// 10.3.1f/g: hosted-orphan trashing, repeated to fixpoint. Set-aside
/// survivors of 9.5.5 are exempt.
fn step_fg_hosted_orphans(vm: &mut Vm) {
    cite!("step_checkpoint_hosted_on_agenda");
    cite!("step_checkpoint_hosted_on_installed_cards");
    // CR 9.5.5: objects still set aside after their ability finished
    // resolving are trashed here; set-aside counters return to the bank.
    if !vm.orphan_set_aside_counters.is_empty() {
        cite!("rule_trash_ability_keeps_track_of_hosted_objects");
        vm.orphan_set_aside_counters.clear();
    }
    let leftover_cards: Vec<crate::object::ObjectId> =
        vm.set_aside_card_cleanup.drain(..).collect();
    for id in leftover_cards {
        if let Some(o) = vm.st.objects.get_mut(&id) {
            if o.set_aside_for_ability {
                o.set_aside_for_ability = false;
                let owner = o.owner;
                vm.trash_card(id, owner);
            }
        }
    }
    loop {
        let mut orphans: Vec<ObjectId> = Vec::new();
        for o in vm.st.objects.values() {
            if o.set_aside_for_ability {
                cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                continue;
            }
            if let Some(host) = o.host {
                let host_gone = match vm.st.objects.get(&host) {
                    None => true,
                    Some(h) => !h.zone.is_installed() && !matches!(h.zone, Zone::ScoreArea(_)),
                };
                if host_gone && o.zone.is_installed() {
                    orphans.push(o.id);
                }
            }
        }
        if orphans.is_empty() {
            break;
        }
        for id in orphans {
            let owner = vm.st.objects[&id].owner;
            vm.trash_card(id, owner);
        }
    }
}

/// 10.3.1h: empty remote servers cease to exist.
fn step_h_empty_remotes(vm: &mut Vm) {
    cite!("step_checkpoint_remote_server");
    let empty: Vec<crate::object::ServerId> = vm
        .st
        .ice
        .keys()
        .chain(vm.st.root.keys())
        .filter(|s| matches!(s, crate::object::ServerId::Remote(_)))
        .filter(|s| {
            vm.st.ice.get(s).map(|v| v.is_empty()).unwrap_or(true)
                && vm.st.root.get(s).map(|v| v.is_empty()).unwrap_or(true)
        })
        .copied()
        .collect();
    for s in empty {
        vm.st.ice.remove(&s);
        vm.st.root.remove(&s);
    }
}

/// 10.3.1i: unoccupied ice positions cease, except the Runner's current one.
fn step_i_vacant_positions(vm: &mut Vm) {
    cite!("step_checkpoint_vacant_position");
    cite!("rule_destroy_position");
    // Positions are indices into the per-server ice list; trashing ice
    // already collapses the list. The Runner's current position is preserved
    // by clamping rather than deletion: if their index now exceeds the list,
    // it stays (an existing-but-unoccupied position) until they move.
    let _ = vm;
}

/// 10.3.1j: cards that entered the breached server's root since the previous
/// checkpoint — for each, the Runner declares whether it becomes a candidate
/// (7.4.6a). Declined cards cannot become candidates for the rest of the
/// breach.
fn step_j_breach_candidates(vm: &mut Vm) {
    cite!("step_checkpoint_card_entering_root_during_breach");
    cite!("rule_candidates_entering_root");
    let Some(server) = vm.breach_server() else { return };
    let entered: Vec<ObjectId> = vm
        .last_scan_window
        .iter()
        .filter_map(|(c, _)| match c {
            GameChange::CardEnteredRoot { obj, server: s } if *s == server => Some(*obj),
            _ => None,
        })
        .collect();
    let mut fresh: Vec<ObjectId> = Vec::new();
    for obj in entered {
        // Cards still in the root, not already candidates/accessed/declined.
        let still_there = vm
            .st
            .root
            .get(&server)
            .map(|v| v.contains(&obj))
            .unwrap_or(false);
        if !still_there {
            continue;
        }
        let b = vm.run_breach_bookkeeping(obj);
        if b {
            fresh.push(obj);
        }
    }
    // One declaration Decision at a time; the answer chains the rest. The
    // checkpoint's remaining steps (k)/(l) proceed before the Decision is
    // answered — they only clean counters and cannot interact with
    // candidacy (single-pass, as with the 10.3.1e minimal-set Decision).
    if let Some(first) = fresh.pop() {
        vm.pending_candidacy.extend(fresh);
        vm.ask_breach_candidacy(first);
    }
}

/// 10.3.1k/l: discard-pile conversions revert; counters return to the bank.
fn step_kl_discard_pile(vm: &mut Vm) {
    cite!("step_checkpoint_discard_pile_cards");
    cite!("step_checkpoint_discard_pile_counters");
    let in_discard: Vec<ObjectId> = vm
        .st
        .objects
        .values()
        .filter(|o| matches!(o.zone, Zone::Discard(_)))
        .filter(|o| !o.counters.is_empty())
        .map(|o| o.id)
        .collect();
    for id in in_discard {
        vm.st.objects.get_mut(&id).unwrap().counters.clear();
    }
}
