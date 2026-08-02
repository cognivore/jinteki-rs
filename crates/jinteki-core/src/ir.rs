//! The ability-IR runtime: ONE event pipeline for every triggered ability.
//!
//! `fire_event` gathers matching `TriggeredAbility` registrations from active
//! cards — active player's cards first, mirroring the reference's
//! `gather-events` — queues their effect sequences, and drains the queue with
//! `run_effects`. Effects that need a decision open a prompt whose
//! `PromptContext` carries the continuation branches as data; resolving the
//! prompt pushes the chosen branch back onto the queue and drains again.

use crate::runs;
use crate::state::*;
use crate::types::*;

/// A concrete game occurrence, dispatched through the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    TurnBegins(Side),
    TurnEnds(Side),
    SuccessfulRun(ServerId),
    RunEnds {
        server: ServerId,
        successful: bool,
        source: Option<Cid>,
    },
    BreachServer {
        server: ServerId,
        source: Option<Cid>,
    },
    AgendaScored(Cid),
    AgendaStolen(Cid),
    Accessed(Cid),
    Exposed(Cid),
    Rezzed(Cid),
    Installed(Cid),
    Encountered(Cid),
    /// This card was just played (operations/events).
    PlayedSelf(Cid),
    /// An operation was played (identity hooks like BABW).
    PlayedOperation(Cid),
}

impl Event {
    /// The card an `...Self` trigger would refer to.
    fn self_cid(&self) -> Option<Cid> {
        match self {
            Event::AgendaScored(c)
            | Event::Accessed(c)
            | Event::Exposed(c)
            | Event::Rezzed(c)
            | Event::Installed(c)
            | Event::Encountered(c)
            | Event::PlayedSelf(c) => Some(*c),
            _ => None,
        }
    }
}

/// Does `trigger` on `holder` match `event`?
fn trigger_matches(st: &GameState, holder: Cid, trigger: Trigger, ev: &Event) -> bool {
    match (trigger, ev) {
        // ── self-directed ──────────────────────────────────────────────────
        (Trigger::OnScoreSelf, Event::AgendaScored(c)) => *c == holder,
        (Trigger::OnAccessSelf { installed_only }, Event::Accessed(c)) => {
            if *c != holder {
                return false;
            }
            let zone = st.card(holder).zone;
            if installed_only {
                matches!(zone, Zone::InServer { ice: false, .. })
            } else {
                // "anywhere except in Archives" (Snare!): an accessed card in
                // the discard pile is the Archives case.
                !matches!(zone, Zone::Discard)
            }
        }
        (Trigger::OnExposeSelf, Event::Exposed(c)) => *c == holder,
        (Trigger::OnRezSelf, Event::Rezzed(c)) => *c == holder,
        (Trigger::OnInstallSelf, Event::Installed(c)) => *c == holder,
        (Trigger::OnEncounterSelf, Event::Encountered(c)) => *c == holder,
        (Trigger::OnPlaySelf, Event::PlayedSelf(c)) => *c == holder,
        // ── global ─────────────────────────────────────────────────────────
        (Trigger::TurnBegins(s), Event::TurnBegins(t)) => s == *t,
        (Trigger::TurnEnds(s), Event::TurnEnds(t)) => s == *t,
        (Trigger::SuccessfulRun(f), Event::SuccessfulRun(server)) => f.matches(*server),
        (Trigger::RunEnds, Event::RunEnds { .. }) => true,
        (Trigger::BreachServer(f), Event::BreachServer { server, .. }) => f.matches(*server),
        (Trigger::AgendaScored, Event::AgendaScored(c)) => *c != holder,
        (Trigger::AgendaStolen, Event::AgendaStolen(_)) => true,
        (Trigger::PlayOperationWithSubtype(sub), Event::PlayedOperation(c)) => {
            st.card(*c).def().subtypes.contains(&sub)
        }
        _ => false,
    }
}

fn condition_holds(st: &GameState, holder: Cid, cond: Condition, ev: &Event) -> bool {
    match cond {
        Condition::Always => true,
        Condition::AdvancementPositive => st.card(holder).advancement > 0,
        Condition::RunnerSuccessfulRunLastTurn => st.runner_run_last_turn,
        Condition::RunSuccessful => matches!(ev, Event::RunEnds { successful: true, .. }),
    }
}

/// Is this card's triggered-ability registration live?
/// Identities and scored agendas are always active; corp installed cards must
/// be rezzed — except self-directed access/expose triggers, which fire on
/// facedown cards by design (ambushes) — and runner installed cards are
/// always active. `is_event_source` marks the card whose play made the
/// current run: its run-lifecycle registrations stay live from the discard
/// (Dirty Laundry's run-ends, Legwork's breach bonus).
fn registration_active(st: &GameState, cid: Cid, trigger: Trigger, is_event_source: bool) -> bool {
    let c = st.card(cid);
    match c.zone {
        Zone::Identity | Zone::Scored(_) | Zone::Rig => true,
        Zone::InServer { .. } => {
            if matches!(
                trigger,
                Trigger::OnAccessSelf { .. }
                    | Trigger::OnExposeSelf
                    | Trigger::OnRezSelf
                    | Trigger::OnEncounterSelf
            ) {
                true
            } else {
                c.rezzed
            }
        }
        // Cards elsewhere are inactive, except self triggers that explicitly
        // reach into hidden zones (Snare! in HQ/R&D) and run-source events in
        // the discard (Dirty Laundry, Legwork).
        Zone::Hand | Zone::Deck | Zone::Discard => match trigger {
            Trigger::OnAccessSelf { .. } | Trigger::OnPlaySelf => true,
            Trigger::RunEnds
            | Trigger::BreachServer(_)
            | Trigger::SuccessfulRun(_) => is_event_source,
            _ => false,
        },
        _ => false,
    }
}

/// All cards whose registrations are consulted for a global event, active
/// player's cards first (identity, scored area, installed cards), then the
/// run-source card if any.
fn gather_order(st: &GameState, ev: &Event) -> Vec<Cid> {
    let mut out = Vec::new();
    let push_side = |st: &GameState, out: &mut Vec<Cid>, side: Side| {
        out.push(st.identity(side));
        out.extend(st.scored(side).iter().copied());
        match side {
            Side::Corp => out.extend(st.all_installed_corp()),
            Side::Runner => out.extend(st.all_installed_runner()),
        }
    };
    push_side(st, &mut out, st.active);
    push_side(st, &mut out, st.active.opponent());
    // The card that made the current run participates even from the discard
    // (run events: Dirty Laundry's run-ends, Legwork's breach bonus).
    let source = match ev {
        Event::RunEnds { source, .. } | Event::BreachServer { source, .. } => *source,
        _ => st.run.as_ref().and_then(|r| r.source),
    };
    if let Some(s) = source {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Dispatch one event: queue every matching registration's effects, then
/// drain the queue.
pub fn fire_event(st: &mut GameState, ev: Event) {
    if st.game_over() {
        return;
    }
    // The card whose play made the current run keeps its run-lifecycle
    // registrations live from the discard.
    let event_source = match &ev {
        Event::RunEnds { source, .. } | Event::BreachServer { source, .. } => *source,
        _ => st.run.as_ref().and_then(|r| r.source),
    };
    // Self-directed triggers fire even when the card is not "active" in the
    // usual sense (facedown ambushes, cards in HQ/R&D, played events).
    let mut to_queue: Vec<(Cid, usize, &'static TriggeredAbility)> = Vec::new();
    if let Some(cid) = ev.self_cid() {
        for (i, ab) in st.card(cid).def().triggered.iter().enumerate() {
            if trigger_matches(st, cid, ab.trigger, &ev)
                && registration_active(st, cid, ab.trigger, event_source == Some(cid))
                && condition_holds(st, cid, ab.condition, &ev)
            {
                to_queue.push((cid, i, ab));
            }
        }
    }
    for cid in gather_order(st, &ev) {
        if Some(cid) == ev.self_cid() {
            continue; // already handled above
        }
        for (i, ab) in st.card(cid).def().triggered.iter().enumerate() {
            // Self triggers never match foreign cards; globals checked here.
            if trigger_matches(st, cid, ab.trigger, &ev)
                && registration_active(st, cid, ab.trigger, event_source == Some(cid))
                && condition_holds(st, cid, ab.condition, &ev)
            {
                to_queue.push((cid, i, ab));
            }
        }
    }
    for (cid, i, ab) in to_queue {
        if ab.once_per_turn {
            if st.fired_this_turn.contains(&(cid, i)) {
                continue;
            }
            st.fired_this_turn.push((cid, i));
        }
        queue_effects_back(st, cid, ab.effects);
    }
    run_effects(st);
}

/// Append an effect sequence to the back of the queue.
pub fn queue_effects_back(st: &mut GameState, source: Cid, effects: &'static [Effect]) {
    for &effect in effects {
        st.pending.push_back(PendingEffect { source, effect });
    }
}

/// Push an effect sequence to the FRONT of the queue (chosen prompt branch
/// runs before whatever was already queued).
pub fn queue_effects_front(st: &mut GameState, source: Cid, effects: &'static [Effect]) {
    for &effect in effects.iter().rev() {
        st.pending.push_front(PendingEffect { source, effect });
    }
}

enum Flow {
    Continue,
    Suspended,
}

/// Drain the pending-effect queue until it suspends on a prompt or empties.
/// On empty, runs the stored continuation (resume subroutines / present the
/// access prompt).
pub fn run_effects(st: &mut GameState) {
    loop {
        if st.game_over() {
            return;
        }
        let Some(PendingEffect { source, effect }) = st.pending.pop_front() else {
            break;
        };
        match apply_effect(st, source, effect) {
            Flow::Continue => continue,
            Flow::Suspended => return,
        }
    }
    if st.game_over() {
        return;
    }
    match st.after_effects.take() {
        Some(AfterEffects::ResumeSubs { ice, index }) => {
            runs::fire_subs_from(st, ice, index);
        }
        Some(AfterEffects::PresentAccess { cid }) => {
            runs::present_access_prompt(st, cid);
        }
        None => {}
    }
}

fn amount(st: &GameState, source: Cid, a: Amount) -> u32 {
    match a {
        Amount::Fixed(n) => n,
        Amount::PerAdvancement(n) => n * st.card(source).advancement,
        Amount::RunnerHandSize => st.hand(Side::Runner).len() as u32,
    }
}

fn source_title(st: &GameState, source: Cid) -> String {
    st.card(source).title().to_string()
}

fn apply_effect(st: &mut GameState, source: Cid, effect: Effect) -> Flow {
    match effect {
        Effect::GainCredits(side, n) => {
            st.gain_credits(side, n as i64);
            let title = source_title(st, source);
            st.side_log(side, format!("uses {title} to gain {n} [Credits]"));
            Flow::Continue
        }
        Effect::LoseCredits(side, n) => {
            let loss = (n as i64).min(st.credits(side));
            st.gain_credits(side, -loss);
            let title = source_title(st, source);
            st.side_log(side, format!("loses {loss} [Credits] ({title})"));
            Flow::Continue
        }
        Effect::Draw(side, n) => {
            let drawn = st.draw_n(side, n as usize);
            let title = source_title(st, source);
            st.side_log(side, format!("uses {title} to draw {drawn} cards"));
            Flow::Continue
        }
        Effect::Damage(kind, a) => {
            let n = amount(st, source, a);
            damage(st, kind, n);
            Flow::Continue
        }
        Effect::GainTags(a) => {
            let n = amount(st, source, a);
            if n > 0 {
                st.tags += n;
                let title = source_title(st, source);
                st.side_log(
                    Side::Runner,
                    format!("takes {n} tag{} ({title})", if n == 1 { "" } else { "s" }),
                );
            }
            Flow::Continue
        }
        Effect::LoseTags(n) => {
            let removed = n.min(st.tags);
            st.tags -= removed;
            if removed > 0 {
                st.side_log(Side::Runner, format!("loses {removed} tags"));
            }
            Flow::Continue
        }
        Effect::GainBadPub(n) => {
            st.bad_pub += n;
            st.side_log(Side::Corp, format!("takes {n} bad publicity"));
            Flow::Continue
        }
        Effect::PlaceCounters(kind, n) => {
            *st.card_mut(source).counters.get_mut(kind) += n;
            Flow::Continue
        }
        Effect::TakeCreditsFromSelf(n) => {
            let side = st.card(source).def().side;
            let take = n.min(st.card(source).counters.credit);
            st.card_mut(source).counters.credit -= take;
            st.gain_credits(side, take as i64);
            let title = source_title(st, source);
            st.side_log(side, format!("uses {title} to gain {take} [Credits]"));
            if st.card(source).counters.credit == 0 {
                st.trash(source, true);
                st.side_log(side, format!("trashes {title}"));
            }
            Flow::Continue
        }
        Effect::TrashSelf => {
            let title = source_title(st, source);
            st.trash(source, true);
            st.system_log(format!("{title} is trashed."));
            Flow::Continue
        }
        Effect::EndTheRun => {
            if st.run.is_some() {
                // An ended run aborts everything queued behind it.
                st.pending.clear();
                st.after_effects = None;
                st.breach = None;
                runs::end_run(st, false);
            }
            Flow::Continue
        }
        Effect::ExposeSelect => {
            let targets = expose_targets(st);
            if targets.is_empty() {
                st.system_log("No installed cards can be exposed.".into());
                return Flow::Continue;
            }
            st.prompt_select(
                Side::Runner,
                "Choose a card to expose".into(),
                SelectKind::UnrezzedInstalledCorpCard,
                PromptContext::ExposePick { source },
            );
            Flow::Suspended
        }
        Effect::RezIceIgnoringCosts => {
            let any_target = st.all_installed_ice().iter().any(|&i| !st.card(i).rezzed);
            if !any_target {
                return Flow::Continue;
            }
            st.prompt_select(
                Side::Corp,
                "Choose a piece of ice to rez, ignoring all costs".into(),
                SelectKind::UnrezzedInstalledIce,
                PromptContext::RezIceFree { source },
            );
            Flow::Suspended
        }
        Effect::AccessBonus(n) => {
            st.access_bonus_accum += n;
            Flow::Continue
        }
        Effect::ModIceStrengthThisEncounter(d) => {
            if let Some(ice) = runs::encountered_ice(st) {
                st.card_mut(ice).strength_mod_encounter += d;
                let s = st.ice_strength(ice);
                let title = st.card(ice).title().to_string();
                st.system_log(format!("{title} has strength {s} for the encounter."));
            }
            Flow::Continue
        }
        Effect::Optional { prompt, cost, yes, no } => {
            let who = st.card(source).def().side;
            // "You may pay N": no prompt at all if the cost is unaffordable
            // (the reference's :req can-pay? gate).
            if cost > 0 && st.spendable(who) < cost as i64 {
                queue_effects_front(st, source, no);
                return Flow::Continue;
            }
            st.prompt_buttons(
                who,
                prompt.to_string(),
                &["Yes", "No"],
                PromptContext::EffectOptional { source, cost, yes, no },
            );
            Flow::Suspended
        }
        Effect::Choose { who, options } => {
            let labels: Vec<&str> = options.iter().map(|o| o.label).collect();
            let title = source_title(st, source);
            st.prompt_buttons(
                who,
                format!("{title}: choose one"),
                &labels,
                PromptContext::EffectChoose { source, options },
            );
            Flow::Suspended
        }
        Effect::Trace { base, on_success, on_fail } => {
            let title = source_title(st, source);
            st.side_log(
                Side::Corp,
                format!("uses {title} to initiate a trace with strength {base}"),
            );
            let max = st.spendable(Side::Corp).max(0) as u32;
            let labels: Vec<String> = (0..=max).map(|i| i.to_string()).collect();
            let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
            st.prompt_buttons(
                Side::Corp,
                format!("Boost trace strength? (base {base})"),
                &refs,
                PromptContext::TraceBoostCorp { source, base, on_success, on_fail },
            );
            Flow::Suspended
        }
        Effect::Psi { on_equal, on_differ } => {
            st.psi_bids = [None, None];
            let title = source_title(st, source);
            for side in [Side::Corp, Side::Runner] {
                let max = st.spendable(side).clamp(0, 2) as u32;
                let labels: Vec<String> =
                    (0..=max).map(|i| format!("{i} [Credits]")).collect();
                let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                // The prompt shows only this side's options; the opponent's
                // bid is never revealed in the message.
                st.prompt_buttons(
                    side,
                    format!("Choose an amount to spend for {title}"),
                    &refs,
                    PromptContext::PsiBid { source, on_equal, on_differ },
                );
            }
            Flow::Suspended
        }
    }
}

// ── trace / psi resolution (called from engine prompt handlers) ────────────

pub fn resolve_trace_corp_boost(
    st: &mut GameState,
    source: Cid,
    base: u32,
    boost: u32,
    on_success: &'static [Effect],
    on_fail: &'static [Effect],
) {
    st.pay_credits(Side::Corp, boost as i64);
    let corp_strength = base + boost;
    st.side_log(
        Side::Corp,
        format!("spends {boost} [Credits] to increase trace strength to {corp_strength}"),
    );
    let link = st.link();
    let max = st.spendable(Side::Runner).max(0) as u32;
    let labels: Vec<String> = (0..=max).map(|i| i.to_string()).collect();
    let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    st.prompt_buttons(
        Side::Runner,
        format!("Boost link strength? (trace strength {corp_strength}, link {link})"),
        &refs,
        PromptContext::TraceBoostRunner { source, corp_strength, on_success, on_fail },
    );
}

pub fn resolve_trace_runner_boost(
    st: &mut GameState,
    source: Cid,
    corp_strength: u32,
    boost: u32,
    on_success: &'static [Effect],
    on_fail: &'static [Effect],
) {
    st.pay_credits(Side::Runner, boost as i64);
    let runner_strength = st.link() + boost;
    st.side_log(
        Side::Runner,
        format!("spends {boost} [Credits] to increase link strength to {runner_strength}"),
    );
    let successful = corp_strength > runner_strength;
    st.system_log(format!(
        "The trace was {}successful.",
        if successful { "" } else { "un" }
    ));
    if successful {
        queue_effects_front(st, source, on_success);
    } else {
        queue_effects_front(st, source, on_fail);
    }
    run_effects(st);
}

/// Record one side's psi bid; when both are in, pay and resolve.
pub fn resolve_psi_bid(
    st: &mut GameState,
    side: Side,
    bid: u32,
    source: Cid,
    on_equal: &'static [Effect],
    on_differ: &'static [Effect],
) {
    let i = if side == Side::Corp { 0 } else { 1 };
    st.psi_bids[i] = Some(bid);
    let [Some(corp_bid), Some(runner_bid)] = st.psi_bids else {
        return; // waiting for the other side
    };
    st.psi_bids = [None, None];
    st.pay_credits(Side::Corp, corp_bid as i64);
    st.pay_credits(Side::Runner, runner_bid as i64);
    st.side_log(Side::Corp, format!("reveals a bid of {corp_bid} [Credits]"));
    st.side_log(Side::Runner, format!("reveals a bid of {runner_bid} [Credits]"));
    if corp_bid == runner_bid {
        queue_effects_front(st, source, on_equal);
    } else {
        queue_effects_front(st, source, on_differ);
    }
    run_effects(st);
}

// ── expose ─────────────────────────────────────────────────────────────────

pub fn expose_targets(st: &GameState) -> Vec<Cid> {
    st.all_installed_corp()
        .into_iter()
        .filter(|&c| !st.card(c).rezzed)
        .collect()
}

/// Expose one facedown installed corp card: log it and fire on-expose.
pub fn expose_card(st: &mut GameState, source: Option<Cid>, target: Cid) {
    let target_title = st.card(target).title().to_string();
    let via = source
        .map(|s| format!("uses {} to expose", st.card(s).title()))
        .unwrap_or_else(|| "exposes".into());
    st.side_log(Side::Runner, format!("{via} {target_title}"));
    fire_event(st, Event::Exposed(target));
}

// ── damage ─────────────────────────────────────────────────────────────────

/// Deal damage to the runner: trash n random cards from the grip; flatline
/// if the grip is smaller. Core (brain) damage also permanently lowers the
/// runner's maximum hand size.
pub fn damage(st: &mut GameState, kind: DamageKind, n: u32) {
    if n == 0 {
        return;
    }
    let word = kind.as_str();
    if kind == DamageKind::Brain {
        st.brain_damage += n;
    }
    let hand = st.hand(Side::Runner).clone();
    if (hand.len() as u32) < n {
        st.side_log(Side::Runner, format!("suffers {n} {word} damage"));
        st.declare_winner(Side::Corp, "Flatline");
        return;
    }
    let victims = st.pick_random(hand, n as usize);
    let mut names = Vec::new();
    for cid in victims {
        names.push(st.card(cid).title().to_string());
        st.trash(cid, true);
    }
    let list = names.join(", ");
    st.side_log(
        Side::Runner,
        format!("suffers {n} {word} damage, trashing {list}"),
    );
}
