//! The run state machine: initiation → approach ice → encounter → movement →
//! approach server → success → breach/access. Decision points are either
//! prompts (rez windows, access) or commands (continue, jack out, breaker
//! abilities); everything else auto-advances in `advance_auto`.

use crate::ir::{self, Event};
use crate::state::*;
use crate::types::*;

pub fn click_run(st: &mut GameState, side: Side, server: ServerId) -> Result<(), EngineError> {
    if side != Side::Runner {
        return Err(EngineError::InvalidCommand("only the runner runs".into()));
    }
    if st.turn_state != TurnState::Acting || st.active != side {
        return Err(EngineError::NotYourTurn);
    }
    if st.run.is_some() || st.breach.is_some() || st.any_prompt_open() {
        return Err(EngineError::InvalidCommand("cannot run now".into()));
    }
    if st.clicks(side) < 1 {
        return Err(EngineError::NoClicks);
    }
    if st.server(server).is_none() {
        return Err(EngineError::InvalidCommand("no such server".into()));
    }
    st.spend_click(side, 1);
    let disp = server.display();
    st.side_log(side, format!("spends [Click] to make a run on {disp}"));
    initiate_run(st, server, None);
    Ok(())
}

pub fn initiate_run(st: &mut GameState, server: ServerId, source: Option<Cid>) {
    let position = st.server(server).map(|s| s.ices.len()).unwrap_or(0);
    let phase = if position > 0 {
        RunPhase::ApproachIce
    } else {
        RunPhase::ApproachServer
    };
    let run_credits = st.bad_pub;
    if run_credits > 0 {
        st.side_log(
            Side::Runner,
            format!("gains {run_credits} bad publicity credits for the run"),
        );
    }
    st.run = Some(RunState {
        server,
        position,
        phase,
        successful: false,
        run_credits,
        source,
    });
    if source.is_some() {
        let disp = server.display();
        st.side_log(Side::Runner, format!("makes a run on {disp}"));
    }
}

/// The ice currently being approached/encountered.
pub fn encountered_ice(st: &GameState) -> Option<Cid> {
    let run = st.run.as_ref()?;
    let srv = st.server(run.server)?;
    if run.position == 0 || run.position > srv.ices.len() {
        return None;
    }
    Some(srv.ices[run.position - 1])
}

/// One step of decision-free progress. Returns true if progress was made.
pub fn advance_auto(st: &mut GameState) -> bool {
    if !st.pending.is_empty() {
        // Suspended effects always wait on a prompt; nothing to auto-do.
        return false;
    }
    if st.breach.is_some() {
        breach_continue(st);
        return false; // breach_continue either prompted or finished
    }
    let Some(run) = st.run.clone() else {
        return false;
    };
    match run.phase {
        RunPhase::ApproachIce => {
            let Some(ice) = encountered_ice(st) else {
                // Ice disappeared (trashed): treat as passed.
                pass_ice(st);
                return true;
            };
            if st.card(ice).rezzed {
                begin_encounter(st, ice);
                return true;
            }
            let cost = st.card(ice).def().cost as i64;
            if st.credits(Side::Corp) >= cost {
                let title_hint = "Rez the approached ice?".to_string();
                st.prompt_buttons(
                    Side::Corp,
                    title_hint,
                    &["Rez", "No action"],
                    PromptContext::RezApproached { ice },
                );
            } else {
                pass_ice(st);
            }
            true
        }
        RunPhase::EncounterIce | RunPhase::Movement => false,
        RunPhase::ApproachServer => {
            do_success(st);
            true
        }
        RunPhase::Success => false,
    }
}

pub fn begin_encounter(st: &mut GameState, ice: Cid) {
    let n = st.card(ice).def().subroutines.len();
    st.card_mut(ice).broken = vec![false; n];
    if let Some(run) = &mut st.run {
        run.phase = RunPhase::EncounterIce;
    }
    let title = st.card(ice).title().to_string();
    st.side_log(Side::Runner, format!("encounters {title}"));
    // On-encounter abilities (Data Raven's tag-or-end-the-run).
    ir::fire_event(st, Event::Encountered(ice));
}

/// Approached ice was not rezzed (or vanished): go to movement.
pub fn pass_ice(st: &mut GameState) {
    if let Some(run) = &mut st.run {
        run.phase = RunPhase::Movement;
    }
}

fn leave_encounter(st: &mut GameState) {
    for c in st.cards.iter_mut() {
        c.pump_encounter = 0;
        c.strength_mod_encounter = 0;
        c.broken.clear();
    }
    if let Some(run) = &mut st.run {
        run.phase = RunPhase::Movement;
    }
}

pub fn continue_run(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    if side != Side::Runner {
        return Err(EngineError::InvalidCommand("corp does not continue here".into()));
    }
    if st.any_prompt_open() {
        return Err(EngineError::PromptOpen);
    }
    let Some(run) = st.run.clone() else {
        return Err(EngineError::InvalidCommand("no run".into()));
    };
    match run.phase {
        RunPhase::EncounterIce => {
            let Some(ice) = encountered_ice(st) else {
                leave_encounter(st);
                return Ok(());
            };
            fire_subs_from(st, ice, 0);
            Ok(())
        }
        RunPhase::Movement => {
            if let Some(run) = &mut st.run {
                if run.position > 0 {
                    run.position -= 1;
                }
                run.phase = if run.position == 0 {
                    RunPhase::ApproachServer
                } else {
                    RunPhase::ApproachIce
                };
            }
            Ok(())
        }
        _ => Err(EngineError::InvalidCommand("nothing to continue".into())),
    }
}

pub fn jack_out(st: &mut GameState, side: Side) -> Result<(), EngineError> {
    if side != Side::Runner {
        return Err(EngineError::InvalidCommand("only the runner jacks out".into()));
    }
    if st.any_prompt_open() {
        return Err(EngineError::PromptOpen);
    }
    let Some(run) = &st.run else {
        return Err(EngineError::InvalidCommand("no run".into()));
    };
    if run.phase != RunPhase::Movement {
        return Err(EngineError::InvalidCommand("can only jack out during movement".into()));
    }
    st.side_log(Side::Runner, "jacks out".into());
    end_run(st, false);
    Ok(())
}

/// Fire unbroken subroutines starting at `start` (resume point after a
/// mid-firing prompt like Rototurret's trash-a-program or a trace).
pub fn fire_subs_from(st: &mut GameState, ice: Cid, start: usize) {
    if st.game_over() || st.run.is_none() {
        return; // the run already ended mid-firing (ETR effect, flatline)
    }
    let subs = st.card(ice).def().subroutines;
    let ice_title = st.card(ice).title().to_string();
    for i in start..subs.len() {
        if *st.card(ice).broken.get(i).unwrap_or(&false) {
            continue;
        }
        let sub = subs[i];
        let label = sub.label();
        st.side_log(Side::Corp, format!("resolves \"{label}\" on {ice_title}"));
        match sub {
            SubEffect::EndTheRun => {
                end_run(st, false);
                return;
            }
            SubEffect::RunnerLosesClick => {
                if st.clicks(Side::Runner) > 0 {
                    st.spend_click(Side::Runner, 1);
                }
            }
            SubEffect::NetDamage(n) => {
                ir::damage(st, DamageKind::Net, n);
                if st.game_over() {
                    return;
                }
            }
            SubEffect::CorpGainCredits(n) => {
                st.gain_credits(Side::Corp, n as i64);
            }
            SubEffect::TrashProgram => {
                if !st.rig.programs.is_empty() {
                    st.prompt_select(
                        Side::Corp,
                        "Choose a program to trash".into(),
                        SelectKind::InstalledRunnerProgram,
                        PromptContext::RototurretTrash { ice, resume_index: i + 1 },
                    );
                    return; // firing resumes when the prompt resolves
                }
            }
            SubEffect::Ability { effects, .. } => {
                st.after_effects = Some(AfterEffects::ResumeSubs { ice, index: i + 1 });
                ir::queue_effects_back(st, ice, effects);
                ir::run_effects(st);
                return; // run_effects resumes firing when the queue drains
            }
        }
    }
    leave_encounter(st);
}

// ── breaker paid abilities ─────────────────────────────────────────────────

pub fn breaker_ability(
    st: &mut GameState,
    breaker: Cid,
    bd: BreakerDef,
    index: usize,
) -> Result<(), EngineError> {
    if st.any_prompt_open() {
        return Err(EngineError::PromptOpen);
    }
    let Some(run) = &st.run else {
        return Err(EngineError::InvalidCommand("no run".into()));
    };
    if run.phase != RunPhase::EncounterIce {
        return Err(EngineError::InvalidCommand("not encountering ice".into()));
    }
    let Some(ice) = encountered_ice(st) else {
        return Err(EngineError::InvalidCommand("no encountered ice".into()));
    };
    match index {
        0 => {
            // Break a subroutine.
            let ice_def = st.card(ice).def();
            if ice_def.ice_subtype != Some(bd.breaks) {
                return Err(EngineError::InvalidCommand("wrong breaker type".into()));
            }
            if st.breaker_strength(breaker) < st.ice_strength(ice) {
                return Err(EngineError::InvalidCommand("insufficient strength".into()));
            }
            if st.spendable(Side::Runner) < bd.break_cost as i64 {
                return Err(EngineError::CantAfford);
            }
            let unbroken: Vec<(usize, String)> = ice_def
                .subroutines
                .iter()
                .enumerate()
                .filter(|(i, _)| !*st.card(ice).broken.get(*i).unwrap_or(&false))
                .map(|(i, s)| (i, s.label()))
                .collect();
            if unbroken.is_empty() {
                return Err(EngineError::InvalidCommand("nothing to break".into()));
            }
            let mut labels: Vec<String> = unbroken.iter().map(|(_, l)| l.clone()).collect();
            labels.push("Done".into());
            let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
            st.prompt_buttons(
                Side::Runner,
                "Break a subroutine".into(),
                &refs,
                PromptContext::BreakChooseSub { breaker, ice },
            );
            Ok(())
        }
        1 => {
            // Pump strength.
            let Some((cost, amount)) = bd.pump else {
                return Err(EngineError::InvalidCommand("no pump ability".into()));
            };
            if st.spendable(Side::Runner) < cost as i64 {
                return Err(EngineError::CantAfford);
            }
            st.pay_credits(Side::Runner, cost as i64);
            {
                let c = st.card_mut(breaker);
                if bd.pump_for_run {
                    c.pump_run += amount as i32;
                } else {
                    c.pump_encounter += amount as i32;
                }
            }
            let title = st.card(breaker).title().to_string();
            let s = st.breaker_strength(breaker);
            let dur = if bd.pump_for_run { " for the remainder of the run" } else { "" };
            st.side_log(
                Side::Runner,
                format!("pays {cost} [Credits] to increase the strength of {title} to {s}{dur}"),
            );
            Ok(())
        }
        _ => Err(EngineError::InvalidCommand("no such ability".into())),
    }
}

pub fn resolve_break_choice(
    st: &mut GameState,
    breaker: Cid,
    ice: Cid,
    label: &str,
) -> Result<(), EngineError> {
    if label == "Done" {
        return Ok(());
    }
    let bd = st
        .card(breaker)
        .def()
        .breaker
        .ok_or(EngineError::InvalidCard)?;
    let subs = st.card(ice).def().subroutines;
    let target = subs
        .iter()
        .enumerate()
        .find(|(i, s)| s.label() == label && !*st.card(ice).broken.get(*i).unwrap_or(&false))
        .map(|(i, _)| i)
        .ok_or(EngineError::BadChoice)?;
    if !st.pay_credits(Side::Runner, bd.break_cost as i64) {
        return Err(EngineError::CantAfford);
    }
    if let Some(b) = st.card_mut(ice).broken.get_mut(target) {
        *b = true;
    }
    let btitle = st.card(breaker).title().to_string();
    let ititle = st.card(ice).title().to_string();
    st.side_log(
        Side::Runner,
        format!("uses {btitle} to break \"{label}\" on {ititle}"),
    );
    Ok(())
}

// ── success and breach ─────────────────────────────────────────────────────

fn do_success(st: &mut GameState) {
    let Some(run) = &mut st.run else { return };
    run.successful = true;
    run.phase = RunPhase::Success;
    let server = run.server;
    let disp = server.display();
    st.side_log(Side::Runner, format!("makes a successful run on {disp}"));
    st.runner_run_this_turn = true;

    // Successful-run abilities (Gabriel's HQ credits, Datasucker's virus).
    ir::fire_event(st, Event::SuccessfulRun(server));
    if st.game_over() {
        return;
    }
    breach_start(st);
}

fn breach_start(st: &mut GameState) {
    let Some(run) = st.run.clone() else { return };
    let server = run.server;
    // Breach-window abilities contribute bonus accesses (Legwork, Maker's Eye).
    st.access_bonus_accum = 0;
    ir::fire_event(st, Event::BreachServer { server, source: run.source });
    if st.game_over() {
        return;
    }
    let n_access = 1 + std::mem::take(&mut st.access_bonus_accum) as usize;
    let queue: Vec<Cid> = match server {
        ServerId::Rd => {
            let deck = st.deck(Side::Corp).clone();
            deck.into_iter().take(n_access).collect()
        }
        ServerId::Hq => {
            let hand = st.hand(Side::Corp).clone();
            let n = n_access.min(hand.len());
            st.pick_random(hand, n)
        }
        ServerId::Archives => {
            let discard = st.discard(Side::Corp).clone();
            for &cid in &discard {
                st.card_mut(cid).faceup = true;
            }
            let agendas: Vec<Cid> = discard
                .iter()
                .copied()
                .filter(|&c| st.card(c).is_agenda())
                .collect();
            let total = discard.len();
            st.side_log(Side::Runner, format!("accesses {total} cards from Archives"));
            agendas
        }
        ServerId::Remote(_) => st
            .server(server)
            .map(|s| s.content.clone())
            .unwrap_or_default(),
    };
    let disp = server.display();
    st.side_log(Side::Runner, format!("breaches {disp}"));
    st.breach = Some(BreachState { server, queue, current: None });
    breach_continue(st);
}

/// Pop the next card to access; on-access abilities (ambushes) resolve
/// before its steal/trash prompt. Finish the run when the queue empties.
pub fn breach_continue(st: &mut GameState) {
    let Some(breach) = &mut st.breach else { return };
    if let Some(cid) = breach.current {
        // An access is already in flight (shouldn't normally re-enter here).
        present_access_prompt(st, cid);
        return;
    }
    if breach.queue.is_empty() {
        st.breach = None;
        end_run(st, true);
        return;
    }
    let cid = breach.queue.remove(0);
    breach.current = Some(cid);
    let title = st.card(cid).title().to_string();
    st.side_log(Side::Runner, format!("accesses {title}"));
    // On-access triggers (Snare!, Ghost Branch, Project Junebug, Psychic
    // Field) fire now; the steal/trash prompt follows when they finish.
    st.after_effects = Some(AfterEffects::PresentAccess { cid });
    ir::fire_event(st, Event::Accessed(cid));
}

/// The steal / trash / no-action decision for an accessed card.
pub fn present_access_prompt(st: &mut GameState, cid: Cid) {
    if st.game_over() {
        return;
    }
    let Some(breach) = &st.breach else { return };
    let in_archives = breach.server == ServerId::Archives;
    let title = st.card(cid).title().to_string();
    let msg = format!("You accessed {title}.");

    if st.card(cid).is_agenda() {
        st.prompt_buttons(
            Side::Runner,
            msg,
            &["Steal"],
            PromptContext::AccessSteal { cid },
        );
        return;
    }
    let trash_cost = st.card(cid).def().trash_cost;
    match trash_cost {
        Some(tc) if !in_archives && st.spendable(Side::Runner) >= tc as i64 => {
            let pay = format!("Pay {tc} [Credits] to trash");
            let labels = [pay.as_str(), "No action"];
            st.prompt_buttons(
                Side::Runner,
                msg,
                &labels,
                PromptContext::AccessTrashOrNo { cid, trash_cost: tc },
            );
        }
        _ => {
            st.prompt_buttons(
                Side::Runner,
                msg,
                &["No action"],
                PromptContext::AccessNoAction { cid },
            );
        }
    }
}

pub fn end_run(st: &mut GameState, successful: bool) {
    let (server, source) = match &st.run {
        Some(r) => (r.server, r.source),
        None => return,
    };
    for c in st.cards.iter_mut() {
        c.pump_encounter = 0;
        c.pump_run = 0;
        c.strength_mod_encounter = 0;
        c.broken.clear();
    }
    st.run = None;
    st.breach = None;
    if !successful {
        let disp = server.display();
        st.system_log(format!("The run on {disp} was unsuccessful."));
    } else {
        st.system_log("The run ends.".into());
    }
    // Run-ends abilities (Dirty Laundry's payout).
    ir::fire_event(st, Event::RunEnds { server, successful, source });
    st.prune_empty_remotes();
}
