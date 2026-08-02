//! Legality enumeration and the random-walk bot.
//!
//! `enumerate_actions` is the playable-milestone slice of DESIGN.md SYS-F-2:
//! for a state and a side, the exact set of commands the engine will accept.
//! The UI uses it for highlights; the bot samples from it; the self-play fuzz
//! test asserts the enumerator ⇄ executor coherence property (DP-3 lite).

use crate::state::*;
use crate::types::*;
use rand::Rng;

pub type Action = Command;

pub fn enumerate_actions(st: &GameState, side: Side) -> Vec<Command> {
    let mut out = Vec::new();
    if st.game_over() {
        return out;
    }

    // 1. An open prompt for this side dominates everything.
    if let Some(p) = st.current_prompt(side) {
        for c in &p.choices {
            out.push(Command::Choice { uuid: c.uuid.clone() });
        }
        if let Some(kind) = p.select {
            for cid in select_targets(st, kind) {
                out.push(Command::Select { cid });
            }
        }
        return out;
    }
    // 2. Waiting on the opponent's prompt: nothing to do.
    if st.any_prompt_open() {
        return out;
    }

    // 3. Run decision points.
    if let Some(run) = &st.run {
        // Hosted-counter paid abilities usable during a run (Data Raven,
        // Nisei MK II, Datasucker).
        push_counter_abilities(st, side, &mut out);
        if side != Side::Runner {
            return out;
        }
        match run.phase {
            RunPhase::EncounterIce => {
                if let Some(ice) = encounter_ice(st) {
                    for &b in &st.rig.programs {
                        let Some(bd) = st.card(b).def().breaker else { continue };
                        if st.card(ice).def().ice_subtype == Some(bd.breaks)
                            && st.breaker_strength(b) >= st.ice_strength(ice)
                            && st.spendable(Side::Runner) >= bd.break_cost as i64
                            && has_unbroken(st, ice)
                        {
                            out.push(Command::Ability { cid: b, index: 0 });
                        }
                        if let Some((cost, _)) = bd.pump {
                            if st.spendable(Side::Runner) >= cost as i64 {
                                out.push(Command::Ability { cid: b, index: 1 });
                            }
                        }
                    }
                }
                out.push(Command::Continue);
            }
            RunPhase::Movement => {
                out.push(Command::Continue);
                out.push(Command::JackOut);
            }
            _ => {}
        }
        return out;
    }

    // 4. Turn actions.
    match st.turn_state {
        TurnState::AwaitingStart if st.active == side => {
            out.push(Command::StartTurn);
            return out;
        }
        TurnState::Acting if st.active == side => {}
        _ => return out,
    }

    let clicks = st.clicks(side);
    let credits = st.spendable(side);

    out.push(Command::EndTurn);

    // Zero-click actions: scoring, rezzing, hosted-counter abilities (corp).
    if side == Side::Corp {
        for (_, srv) in &st.servers {
            for &cid in &srv.content {
                let c = st.card(cid);
                if c.is_agenda()
                    && c.advancement >= c.def().advancement_requirement.unwrap_or(u32::MAX)
                {
                    out.push(Command::Score { cid });
                }
                if !c.rezzed && !c.is_agenda() && credits >= c.def().cost as i64 {
                    out.push(Command::Rez { cid });
                }
            }
        }
    }
    push_counter_abilities(st, side, &mut out);

    if clicks < 1 {
        return out;
    }

    out.push(Command::Credit);
    if !st.deck(side).is_empty() {
        out.push(Command::Draw);
    }

    for &cid in st.hand(side) {
        let def = st.card(cid).def();
        match def.kind {
            CardType::Operation | CardType::Event if def.side == side => {
                let cond_ok = match def.play_condition {
                    None | Some(Condition::Always) => true,
                    Some(Condition::RunnerSuccessfulRunLastTurn) => st.runner_run_last_turn,
                    Some(_) => false,
                };
                if cond_ok && credits >= def.cost as i64 {
                    out.push(Command::Play { cid });
                }
            }
            CardType::Agenda | CardType::Asset if side == Side::Corp => {
                out.push(Command::InstallCorp { cid, server: "New remote".into() });
                for (id, srv) in &st.servers {
                    if let ServerId::Remote(_) = id {
                        if srv.content.is_empty() {
                            out.push(Command::InstallCorp { cid, server: id.key() });
                        }
                    }
                }
            }
            CardType::Ice if side == Side::Corp => {
                for (id, srv) in &st.servers {
                    if credits >= srv.ices.len() as i64 {
                        out.push(Command::InstallCorp { cid, server: id.key() });
                    }
                }
                out.push(Command::InstallCorp { cid, server: "New remote".into() });
            }
            CardType::Program | CardType::Hardware | CardType::Resource
                if side == Side::Runner =>
            {
                let mu_ok = def.kind != CardType::Program
                    || st.mu_used() + def.mu_cost as i32 <= st.mu_limit();
                if credits >= def.cost as i64 && mu_ok {
                    out.push(Command::InstallRunner { cid });
                }
            }
            _ => {}
        }
    }

    if side == Side::Corp && credits >= 1 {
        for (_, srv) in &st.servers {
            for &cid in srv.content.iter().chain(srv.ices.iter()) {
                let c = st.card(cid);
                if c.is_agenda() || c.def().advanceable {
                    out.push(Command::Advance { cid });
                }
            }
        }
    }

    // Click abilities on installed cards (Armitage, Regolith).
    let installed: Vec<Cid> = match side {
        Side::Runner => st.rig.resources.clone(),
        Side::Corp => st
            .servers
            .iter()
            .flat_map(|(_, s)| s.content.iter().copied())
            .collect(),
    };
    for cid in installed {
        let c = st.card(cid);
        if c.def().click_ability.is_some()
            && c.counters.credit > 0
            && (side == Side::Runner || c.rezzed)
        {
            out.push(Command::Ability { cid, index: 0 });
        }
    }

    if side == Side::Runner {
        for (id, _) in &st.servers {
            out.push(Command::Run { server: *id });
        }
        if st.tags > 0 && credits >= 2 {
            out.push(Command::RemoveTag);
        }
    }

    if side == Side::Corp {
        if st.tags > 0 && !st.rig.resources.is_empty() && clicks >= 1 && credits >= 2 {
            out.push(Command::TrashResource);
        }
        if clicks >= 3 {
            out.push(Command::Purge);
        }
    }

    out
}

/// Hosted-counter paid abilities whose timing window is open right now.
/// Mirrors `engine::use_counter_ability`'s gating exactly (DP-3 soundness).
fn push_counter_abilities(st: &GameState, side: Side, out: &mut Vec<Command>) {
    let in_run = st.run.is_some();
    let in_encounter = in_run
        && st.run.as_ref().map(|r| r.phase) == Some(RunPhase::EncounterIce)
        && encounter_ice(st).map(|i| st.card(i).rezzed) == Some(true);
    let action_window =
        st.turn_state == TurnState::Acting && st.active == side && !in_run && st.breach.is_none();
    let mut candidates: Vec<Cid> = Vec::new();
    match side {
        Side::Corp => {
            candidates.extend(
                st.all_installed_corp()
                    .into_iter()
                    .filter(|&c| st.card(c).rezzed),
            );
            candidates.extend(st.scored(Side::Corp).iter().copied());
        }
        Side::Runner => candidates.extend(st.all_installed_runner()),
    }
    for cid in candidates {
        let def = st.card(cid).def();
        if def.side != side {
            continue;
        }
        let base = if def.click_ability.is_some() { 1 } else { 0 };
        for (i, ab) in def.counter_abilities.iter().enumerate() {
            let timing_ok = match ab.timing {
                AbilityTiming::Anytime => action_window || in_run,
                AbilityTiming::DuringRun => in_run,
                AbilityTiming::DuringEncounter => in_encounter,
            };
            let (kind, n) = ab.cost;
            if timing_ok && st.card(cid).counters.get(kind) >= n {
                out.push(Command::Ability { cid, index: base + i });
            }
        }
    }
}

fn encounter_ice(st: &GameState) -> Option<Cid> {
    let run = st.run.as_ref()?;
    let srv = st.server(run.server)?;
    if run.position == 0 || run.position > srv.ices.len() {
        return None;
    }
    Some(srv.ices[run.position - 1])
}

fn has_unbroken(st: &GameState, ice: Cid) -> bool {
    let n = st.card(ice).def().subroutines.len();
    (0..n).any(|i| !*st.card(ice).broken.get(i).unwrap_or(&false))
}

fn select_targets(st: &GameState, kind: SelectKind) -> Vec<Cid> {
    match kind {
        SelectKind::UnrezzedInstalledIce => st
            .all_installed_ice()
            .into_iter()
            .filter(|&c| !st.card(c).rezzed)
            .collect(),
        SelectKind::UnrezzedInstalledCorpCard => st
            .all_installed_corp()
            .into_iter()
            .filter(|&c| !st.card(c).rezzed)
            .collect(),
        SelectKind::InstalledRunnerProgram => st.rig.programs.clone(),
        SelectKind::InstalledRunnerResource => st.rig.resources.clone(),
        SelectKind::OwnHandCard(side) => st.hand(side).clone(),
    }
}

/// Pick the bot's next command, or None if it has no decision to make.
/// Random walk with light anti-stall shaping: spend clicks before ending the
/// turn, prefer continuing runs to jacking out, prefer breaking to eating subs.
pub fn random_walk_step<R: Rng>(st: &GameState, side: Side, rng: &mut R) -> Option<Command> {
    let actions = enumerate_actions(st, side);
    if actions.is_empty() {
        return None;
    }
    let clicks = st.clicks(side);
    let weighted: Vec<(u32, &Command)> = actions
        .iter()
        .map(|a| {
            let w: u32 = match a {
                Command::StartTurn => 1000,
                Command::EndTurn => {
                    if clicks > 0 {
                        1
                    } else {
                        200
                    }
                }
                Command::JackOut => 3,
                Command::Continue => 12,
                Command::Ability { .. } => 14,
                Command::Score { .. } => 60,
                Command::Purge => 1,
                Command::Choice { .. } | Command::Select { .. } => 10,
                _ => 10,
            };
            (w, a)
        })
        .collect();
    let total: u32 = weighted.iter().map(|(w, _)| w).sum();
    let mut roll = rng.random_range(0..total);
    for (w, a) in &weighted {
        if roll < *w {
            return Some((*a).clone());
        }
        roll -= w;
    }
    weighted.last().map(|(_, a)| (*a).clone())
}
