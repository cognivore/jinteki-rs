//! The checkpoint procedure (§10.3): steps 10.3.1 (a)–(l) EXACTLY, in order,
//! each citing its `step_checkpoint_*` id. 10.3.2 reaction-window spawning is
//! done by the caller (`Vm::checkpoint_and_react`); this module returns the
//! newly-pended instance ids. No checkpoint nests inside a checkpoint
//! (9.11.2a): the procedure is straight-line.

use crate::ability::{
    ability_active, is_corp_card, trigger_matches, trigger_per_event, AbilityDef,
    AbilityInstance, AbilityKind, AbilityRef, Condition,
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

    // 6.2.7: not a step of this procedure — a consequence the run draws from
    // what has happened to the ice in the Runner's position. It runs first so
    // that an encounter ending here is visible to step (a)'s scan window and
    // so that a position the Runner has just left is vacant for step (i).
    vm.apply_ice_change_to_run();

    let mut newly = step_a_conditional_abilities(vm);
    // CR 9.6.14d: instances an effect marked pending are pending exactly like
    // the ones step (a) just created — they were simply not created BY the
    // scan. Draining them here is what makes 10.3.2 open a reaction window
    // for them.
    if !vm.pending_from_effect.is_empty() {
        cite!("rule_instructed_to_resolve_conditional_ability");
        let forced = std::mem::take(&mut vm.pending_from_effect);
        newly.extend(forced.into_iter().filter(|id| vm.instances.contains_key(id)));
    }
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

    // Gather the scan window first (immutable borrow). Its absolute start in
    // the log is what lets a condition ask how many times it has already been
    // met earlier in the turn (9.6.5c ordinals).
    let window_start = vm.changes.last_checkpoint_start;
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
                    vm.threat_level(),
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
        for (i, a) in o.face().abilities.iter().enumerate() {
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
        // CR 9.1.9: an ability the card no longer HAS cannot meet a condition.
        // (`usize::MAX` indices are abilities carried by a lingering effect,
        // not printed on the card, so 9.1.9's gains/losses do not address
        // them.)
        if index < usize::MAX - 1 && !vm.ability_present(obj_id, index) {
            cite!("rule_lose_ability");
            continue;
        }
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
                        vm.threat_level(),
                    );
                // 9.1.8g: "if an ACTIVE card moves to a zone where it is
                // inactive…". The card must have been active before the move —
                // a card trashed out of a hand or a deck was never active, so
                // nothing of it lingers, and 9.1.8b is then the only rule that
                // can keep such an ability active (in the zone it lands in).
                let was_active = vm
                    .snapshot
                    .as_ref()
                    .and_then(|st| st.objects.get(&obj_id))
                    .map(crate::object::card_active)
                    .unwrap_or(false);
                let moved_to_inactive_in_window = was_active
                    && window.iter().any(|(c, _)| match c {
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
                    && vm.once_per_turn_used.contains(&(aref, source_obj.generation))
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
                for (offset, (c, group)) in window.iter().enumerate() {
                    // 4.6.6i: "this server" in a trigger condition is read
                    // through `Vm::this_server`, so a condition met BY the
                    // source leaving its server still names the server it
                    // left (Warroid Tracker class).
                    if !trigger_matches(
                        cond,
                        c,
                        source_obj,
                        vm.this_server(obj_id),
                        is_corp,
                        |o| vm.st.objects.get(&o).map(|x| x.printed.card_type),
                        |o, s| vm.has_subtype(o, s),
                        |o, k| vm.object_matches_maintained_choice(o, obj_id, k),
                        |o, cr| vm.object_matches_criteria(o, cr, Some(obj_id)),
                    ) && !persisted_server_override(vm, from_lingering, cond, c)
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
                        let sv = vm.this_server(obj_id);
                        if sv.is_none() || vm.server_of(*obj) != sv {
                            continue;
                        }
                    }
                    // 5.2.5b: the actions taken this turn are all different.
                    if let crate::ability::TriggerCond::DifferentActionsThisTurn {
                        count, ..
                    } = cond
                    {
                        cite!("rule_same_actions");
                        cite!("rule_defferent_actions");
                        let taken: Vec<crate::change::ActionIdentity> = vm.changes.log
                            [vm.st.turn_log_start..]
                            .iter()
                            .filter_map(|x| match x {
                                GameChange::ActionTaken { action, .. } => Some(*action),
                                _ => None,
                            })
                            .collect();
                        let all_different = taken
                            .iter()
                            .enumerate()
                            .all(|(i, a)| !taken[..i].contains(a));
                        if taken.len() != *count || !all_different {
                            continue;
                        }
                    }
                    // 1.16.4d: every [click] spent to TAKE the action counts,
                    // including one paid several steps into its resolution.
                    if let crate::ability::TriggerCond::ClicksSpentOnAction { count, .. } = cond {
                        cite!("rule_inherent_cost_aggregates");
                        match vm.st.current_action {
                            Some((_, spent)) if spent >= *count => {}
                            _ => continue,
                        }
                    }
                    // 6.3.4: "during a run" is a game-state test the scan can
                    // make, and the run is in progress only once it has
                    // formally begun — the clicks and credits paid to MAKE
                    // the run are spent before that.
                    if let crate::ability::TriggerCond::PlayerSpendsClick {
                        during_run: true, ..
                    } = cond
                    {
                        cite!("rule_abilities_during_a_run");
                        if vm.current_run.is_none() {
                            continue;
                        }
                    }
                    // 10.11.5: the run must be on the MARK, and the "first
                    // time each turn" ordinal is counted only from the moment
                    // that server was designated — an earlier successful run
                    // on the same server, before it was the mark, is not one
                    // of the times this condition counts.
                    if let (
                        crate::ability::TriggerCond::SuccessfulRunOnMark { first_each_turn },
                        GameChange::RunDeclaredSuccessful { server },
                    ) = (cond, c)
                    {
                        cite!("rule_mark_designated_condition_check");
                        let Some((mark, since)) = vm.mark() else { continue };
                        if mark != *server {
                            continue;
                        }
                        // The successful runs on the mark SINCE the
                        // designation. Zero means the run that met this
                        // condition happened before the server was the mark,
                        // and 10.11.5 does not let the condition see it at
                        // all; more than one means this is not the first.
                        let so_far = vm.changes.log[since..]
                            .iter()
                            .filter(|x| {
                                matches!(x, GameChange::RunDeclaredSuccessful { server: s }
                                         if *s == mark)
                            })
                            .count();
                        if so_far == 0 || (*first_each_turn && so_far > 1) {
                            continue;
                        }
                    }
                    // 10.9.1/10.9.2: a card is EMPTY only when it holds no
                    // counters of a kind it was previously LOADED with. A
                    // card that was never loaded cannot become empty, and
                    // counters of an unloaded kind coming off do nothing
                    // (10.9.3).
                    if let crate::ability::TriggerCond::SelfEmpty { kind } = cond {
                        cite!("rule_empty_requires_loading");
                        cite!("rule_meeting_empty_condition");
                        let loaded = source_obj.loaded_kinds.contains(kind);
                        let left = source_obj.counters.get(kind).copied().unwrap_or(0);
                        if !loaded || left > 0 {
                            continue;
                        }
                    }
                    // 9.10.3b: Security Testing class — the successful run
                    // must be on the server the source chose THIS turn. No
                    // choice maintained means the condition is never met.
                    if let (
                        crate::ability::TriggerCond::SuccessfulRunOnChosenServer { key },
                        GameChange::RunDeclaredSuccessful { server },
                    ) = (cond, c)
                    {
                        let chosen = vm.maintained_choice(obj_id, key);
                        if chosen != Some(crate::lingering::ChoiceValue::Server(*server)) {
                            continue;
                        }
                    }
                    // Warroid Tracker class: the trashed card must have been
                    // in the source's server — which for a card trashed FROM
                    // that server is the server it left (4.6.6i again).
                    if let (
                        crate::ability::TriggerCond::RunnerTrashesAtLeastOneCorpCard {
                            in_this_server: true,
                        },
                        GameChange::CardTrashed { obj, .. },
                    ) = (cond, c)
                    {
                        let sv = vm.this_server(obj_id);
                        if sv.is_none() || vm.this_server(*obj) != sv {
                            continue;
                        }
                    }
                    // The District 99 class narrows the description to some
                    // card types; the scan has the state access to read the
                    // trashed card's type.
                    if let (
                        crate::ability::TriggerCond::InstalledCardTrashed { of_types, .. },
                        GameChange::CardTrashed { obj, .. },
                    ) = (cond, c)
                    {
                        if !of_types.is_empty() {
                            let ty = vm.st.objects.get(obj).map(|o| o.printed.card_type);
                            if !ty.map(|t| of_types.contains(&t)).unwrap_or(false) {
                                continue;
                            }
                        }
                    }
                    // 9.6.5c again, for the ordinal a sentence states about
                    // the occurrence itself: "**the first time each turn**
                    // <the condition>". The stipulation has to hold when the
                    // condition would occur, so this occurrence meets it only
                    // if no earlier change THIS TURN matched the same
                    // condition — read from the change log, which 10.2.1
                    // makes open information.
                    //
                    // Deliberately not 9.3.6g's flag, and this is the whole
                    // difference: the flag is spent by USING an ability, 9.1.6
                    // says players never use an entirely mandatory one, and
                    // 1.12.2's Vaporframe Fabricator example makes the flag
                    // per OBJECT — so a mandatory "first time each turn"
                    // written as the flag comes back fresh the moment its
                    // card is reinstalled, which the printed sentence does
                    // not say. One stipulation, on the condition, for every
                    // condition (§12 rule 2).
                    if def.first_each_turn {
                        cite!("rule_condition_requirements_part_of_condition");
                        cite!("rule_hidden_or_open_information");
                        let here = window_start + offset;
                        let from = vm.st.turn_log_start.min(here);
                        let earlier = vm.changes.log[from..here].iter().any(|x| {
                            trigger_matches(
                                cond,
                                x,
                                source_obj,
                                vm.this_server(obj_id),
                                is_corp,
                                |o| vm.st.objects.get(&o).map(|x| x.printed.card_type),
                                |o, s| vm.has_subtype(o, s),
                                |o, k| vm.object_matches_maintained_choice(o, obj_id, k),
                                |o, cr| vm.object_matches_criteria(o, cr, Some(obj_id)),
                            )
                        });
                        if earlier {
                            continue;
                        }
                    }
                    // 9.6.5c: requirements listed in the trigger condition
                    // must hold when the condition would occur (QPM class:
                    // the Runner must already be tagged).
                    if !vm.trigger_requirements_met_for(cond, Some(obj_id)) {
                        cite!("rule_condition_requirements_part_of_condition");
                        continue;
                    }
                    // 9.3.6g, as the CR's own example fixes the reading: the
                    // flag is spent when the ability is USED, not when its
                    // condition occurs — an instance that never resolved does
                    // not stop the ability pending again later the same turn
                    // (example_rule_once_per_turn_flag_1). So creation is
                    // gated on used-ness, and marking happens at resolution.
                    if def.has_flag(crate::ability::AbilityFlag::OncePerTurn)
                        && vm
                            .once_per_turn_used
                            .contains(&(aref, vm.generation(obj_id)))
                    {
                        cite!("rule_once_per_turn_flag");
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
                        if let GameChange::CardAdvanced { obj } = c {
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
                            source_generation: vm.generation(obj_id),
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
                let true_now = vm.static_cond_holds(obj_id, sc);
                if !true_now {
                    continue;
                }
                if !ability_active(
                    source_obj,
                    &def,
                    vm.st.encounter.as_ref().map(|e| e.ice),
                    vm.st.accessed,
                    vm.threat_level(),
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
                        source_generation: vm.generation(obj_id),
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
    // as one of the indicated effects occurs; the card is then trashed as if
    // completing its resolution. 3.5.1b/3.7.1b list them for the current
    // class — another current operation or event being played, and the
    // opponent putting an agenda in their score area — and the shield carries
    // whichever its declaration named, evaluated through the same
    // `trigger_matches` a conditional ability's condition goes through, with
    // the shielded card as the source (so "another" excludes its own play).
    let shields: Vec<(u64, crate::object::ObjectId, Vec<crate::ability::TriggerCond>)> = vm
        .lingering
        .iter()
        .filter_map(|l| match &l.payload {
            Payload::PlayedTrashShield { card, until } => Some((l.id, *card, until.clone())),
            _ => None,
        })
        .collect();
    let mut expired: Vec<(u64, crate::object::ObjectId)> = Vec::new();
    for (lid, card, until) in shields {
        let Some(source_obj) = vm.st.objects.get(&card) else { continue };
        let occurred = vm.last_scan_window.iter().any(|(c, _)| {
            until.iter().any(|cond| {
                trigger_matches(
                    cond,
                    c,
                    source_obj,
                    vm.this_server(card),
                    |o| vm.st.objects.get(&o).is_some_and(|x| is_corp_card(x.printed.card_type)),
                    |o| vm.st.objects.get(&o).map(|x| x.printed.card_type),
                    |o, s| vm.has_subtype(o, s),
                    |o, k| vm.object_matches_maintained_choice(o, card, k),
                    |o, cr| vm.object_matches_criteria(o, cr, Some(card)),
                )
            })
        });
        if occurred {
            expired.push((lid, card));
        }
    }
    if !expired.is_empty() {
        cite!("rule_play_not_trashed_until");
        cite!("rule_operation_current");
        cite!("rule_event_current");
    }
    for (lid, card) in expired {
        vm.lingering.retain(|l| l.id != lid);
        if matches!(vm.st.objects.get(&card).map(|o| o.zone), Some(Zone::PlayArea(_))) {
            let owner = vm.st.objects[&card].owner;
            vm.trash_card(card, owner);
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
    // CR 9.10.5 + 9.9.9a: a duration-modifying ability keeps the lingering
    // effects it applies to alive until an ADDITIONAL duration expires. It is
    // a replacement effect on a duration, so it applies only while it is
    // itself active — which is checked here, at the moment the original
    // duration runs out (9.9.9a: once the replacement or the effect it
    // modifies goes inactive, the modification no longer applies). Static
    // abilities are untouched: their effects have no durations and create no
    // lingering effects at all (9.4.4).
    cite!("rule_modify_duration_of_lingering_effect");
    cite!("rule_replacement_on_static_ability_must_remain_active");
    cite!("rule_static_no_lingering_effects");
    let extenders: Vec<(ObjectId, crate::lingering::WantedDuration)> = vm
        .active_statics()
        .into_iter()
        .filter_map(|(src, d)| match d {
            crate::ability::StaticDecl::ExtendStrengthDurations { target_host, until } => {
                let target = if target_host {
                    vm.st.objects.get(&src).and_then(|o| o.host)?
                } else {
                    src
                };
                Some((target, until))
            }
            _ => None,
        })
        .collect();
    let mut extend: Vec<(u64, crate::lingering::Duration)> = Vec::new();
    for l in &vm.lingering {
        if l.duration_extended {
            continue;
        }
        let Payload::StrengthMod { target, .. } = l.payload else { continue };
        let source_active = vm.st.objects.get(&l.source).map(card_active).unwrap_or(false);
        if !l.expired(current_encounter, current_run, current_turn, source_active) {
            continue;
        }
        if let Some((_, until)) = extenders.iter().find(|(t, _)| *t == target) {
            extend.push((
                l.id,
                crate::lingering::bind_duration(
                    *until,
                    current_encounter,
                    current_run,
                    current_turn,
                ),
            ));
        }
    }
    for (id, dur) in extend {
        if let Some(l) = vm.lingering.iter_mut().find(|l| l.id == id) {
            l.duration = dur;
            l.duration_extended = true;
        }
    }

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
    let corp = vm.score(Side::Corp) >= 7;
    let runner = vm.score(Side::Runner) >= 7;
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
                            && host.face().abilities.iter().enumerate().any(|(i, a)| {
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
    // CR 1.13.13: if a host card CHANGED ZONES since the last checkpoint —
    // except by moving from one score area to another (8.8.4c) — everything
    // hosted on it (transitively) is trashed, and its hosted counters return
    // to the bank (1.9.2). This cannot be prevented.
    let moved: Vec<ObjectId> = vm
        .last_scan_window
        .iter()
        .filter_map(|(c, _)| match c {
            GameChange::CardMoved { obj, from, to } => {
                cite!("rule_trash_hosted_objects_when_host_trashed");
                if matches!((from, to), (Zone::ScoreArea(_), Zone::ScoreArea(_))) {
                    cite!("rule_swap_score_areas");
                    None
                } else if from.zone_class() == to.zone_class() {
                    // 1.13.13 fires on a host CHANGING ZONES, and 1.12.4 says
                    // the whole play area is one zone: a card moved from one
                    // position or server to another has not changed zones, so
                    // nothing hosted on it is trashed. That is what makes
                    // 8.8.3's "the two cards remain installed throughout the
                    // process of swapping, and do not otherwise affect any
                    // other part of the game state" true, and 8.8.3a's
                    // hosted Botulus survive.
                    cite!("rule_play_area");
                    cite!("rule_swap_installed_cards");
                    None
                } else {
                    Some(*obj)
                }
            }
            _ => None,
        })
        .collect();
    for host in moved {
        // "…and all objects hosted on those objects, and so on."
        let mut stack = vec![host];
        let mut doomed: Vec<ObjectId> = Vec::new();
        let mut seen: std::collections::BTreeSet<ObjectId> = [host].into_iter().collect();
        while let Some(h) = stack.pop() {
            if !vm.st.objects.contains_key(&h) {
                continue;
            }
            bank_hosted_counters(vm, h);
            let Some(o) = vm.st.objects.get(&h) else { continue };
            for g in o.hosted.clone() {
                // CR 9.5.5: objects set aside by a [trash] trigger cost are
                // still hosted for that ability and survive its source's
                // trashing.
                if vm.st.objects.get(&g).is_some_and(|x| x.set_aside_for_ability) {
                    cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                    continue;
                }
                if !seen.insert(g) {
                    continue;
                }
                doomed.push(g);
                stack.push(g);
            }
        }
        for id in doomed {
            if vm.st.objects.contains_key(&id) {
                let owner = vm.st.objects[&id].owner;
                vm.trash_card(id, owner);
            }
        }
    }
    // The state-side of the same rule, repeated to fixpoint: a hosted object
    // whose host is no longer anywhere it could host from (1.13.1a) is
    // trashed too.
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

/// CR 1.13.13 + 1.9.2: the counters hosted on a card that changed zones are
/// no longer hosted on anything; they return to the bank.
///
/// The rule is about what the card was hosting WHEN it moved. Counters placed
/// on it AFTER the move — a Project-Vacheron-class replacement adds an agenda
/// to the score area WITH hosted counters (9.9.9c) — were never hosted on it
/// in the zone it left, so they stay. The change log is the kernel's only
/// record of that ordering, so it is what the sweep reads.
fn bank_hosted_counters(vm: &mut Vm, card: ObjectId) {
    let placed_after: Vec<(CounterKind, u32)> = {
        let moved_at = vm.last_scan_window.iter().position(|(c, _)| {
            matches!(c, GameChange::CardMoved { obj, .. } if *obj == card)
        });
        match moved_at {
            Some(i) => vm.last_scan_window[i + 1..]
                .iter()
                .filter_map(|(c, _)| match c {
                    GameChange::CounterPlaced { obj, kind, amount } if *obj == card => {
                        Some((*kind, *amount))
                    }
                    _ => None,
                })
                .collect(),
            None => Vec::new(),
        }
    };
    let counters: Vec<(CounterKind, u32)> = vm.st.objects[&card]
        .counters
        .iter()
        .map(|(k, n)| {
            let after: u32 = placed_after.iter().filter(|(x, _)| x == k).map(|(_, a)| *a).sum();
            (*k, n.saturating_sub(after))
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    if counters.is_empty() {
        return;
    }
    cite!("rule_bank");
    for (kind, amount) in &counters {
        let have = vm.st.objects[&card].counter(*kind);
        vm.st.objects.get_mut(&card).unwrap().counters.insert(*kind, have.saturating_sub(*amount));
    }
    for (kind, amount) in counters {
        vm.changes.record(crate::change::GameChange::CounterRemoved {
            obj: Some(card),
            kind,
            amount,
        });
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
            // 6.2.1: a position with no ice in it holds no card, so a server
            // whose positions are all vacant is still empty.
            vm.st.ice.get(s).map(|v| v.iter().all(|p| p.ice.is_none())).unwrap_or(true)
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
    // 6.2.4: a position a piece of ice has left ceases to exist HERE — unless
    // the Runner is standing in it, in which case it survives until they move
    // to another position or cease to have one. That exception is the whole
    // reason positions are objects rather than indices (6.2.6): the Runner
    // keeps standing where they stood while the sequence changes around them.
    //
    // The step's OTHER exception is an installation in progress: 6.2.2 makes
    // the position at step 8.5.16b and the ice only occupies it at 8.5.16e,
    // so the checkpoints in between would otherwise destroy the position the
    // install is aiming at.
    let held = vm.run_ctx().and_then(|r| r.position.map(|p| (r.server, p)));
    let installing: Vec<crate::object::ServerId> = vm
        .installs
        .iter()
        .filter_map(|p| match p.resolved_zone {
            Some(crate::object::Zone::Ice(s)) => Some(s),
            _ => None,
        })
        .collect();
    for (&server, positions) in vm.st.ice.iter_mut() {
        if installing.contains(&server) {
            continue;
        }
        positions.retain(|p| p.ice.is_some() || held == Some((server, p.id)));
    }
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
