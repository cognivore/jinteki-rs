//! Lingering effects (§9.10): engine-owned records `{source, payload,
//! duration}`; payload kinds: value modifier, granted ability, replacement
//! effect, maintained choice, delayed conditional. Implicit durations
//! (9.6.13c/d), structure-not-in-progress expiry (9.10.4), icebreaker-pump
//! default duration (9.10.4a), maintained-choice durations (9.10.3).

use crate::ability::AbilityDef;
use crate::effects::EffectClass;
use crate::object::{ObjectId, ServerId};

/// Durations are bound to *specific structure instances* so a later run/turn
/// does not resurrect an expired effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    /// "…for the remainder of this encounter" (9.10.4a) — bound to an
    /// encounter instance id.
    Encounter(u64),
    /// "…for the remainder of this run" — bound to a run instance id.
    Run(u64),
    /// "…until the end of the turn" — bound to a turn sequence number.
    Turn(u64),
    /// 9.6.13c: delayed conditional with no stated duration — until the next
    /// time it resolves.
    UntilResolved,
    /// 9.10.3c: until the source object becomes inactive.
    WhileSourceActive,
    /// 9.10.4: created referencing a structure not in progress — expires at
    /// the next checkpoint.
    ExpiredImmediately,
    /// 9.12.5c: persistent abilities — until the reaction window after
    /// `step_run_complete` of the bound run closes.
    PersistUntilAfterRun(u64),
}

/// What the lingering effect does while it lives.
#[derive(Debug, Clone)]
pub enum Payload {
    /// Value modifier: strength delta on a specific object (icebreaker pumps,
    /// Bad-Times-style memory mods are StaticDecl-shaped ops).
    StrengthMod { target: ObjectId, delta: i32 },
    /// Memory limit modifier (Bad Times class).
    MemoryLimitMod { delta: i32 },
    /// CR 9.1.9/9.10.2: an ability granted to an object for a duration.
    GrantedAbility { to: ObjectId, def: AbilityDef },
    /// CR 9.9.8c: a replacement effect created ahead of time. `applies_to`
    /// selects the expected-effect class it can replace; `replace_with` is
    /// the kernel-wave transform.
    ReplacementEffect {
        applies_to: EffectClass,
        replace_with: ReplacementTransform,
    },
    /// CR 9.10.3: a remembered choice (server, object) other abilities of the
    /// same source read.
    MaintainedChoice { key: &'static str, choice: ChoiceValue },
    /// CR 9.6.13: a delayed conditional ability maintained by this effect.
    DelayedConditional { def: AbilityDef },
    /// CR 9.12.5: a persistent ability persisting after its source was
    /// trashed during an access; applicable only to the bound run (9.12.5d).
    PersistedAbility { def: AbilityDef, run_id: u64 },
    /// CR 9.8.3a/e: a subroutine granted to a piece of ice by an external
    /// ability; ordering inside its category is by grant sequence.
    GrantedSubroutine { to: ObjectId, sub: AbilityDef, before: bool, seq: u64 },
    /// CR 7.4.2: "the Runner cannot access any card other than <obj> for
    /// the remainder of the run" (Ash class).
    RestrictCandidatesTo(ObjectId),
    /// CR 8.6.6c: a played card kept in the play area instead of being
    /// trashed at 8.6.7g; when the indicated effect occurs (kernel wave: the
    /// Runner steals an agenda), the effect expires at checkpoint step
    /// 10.3.1b and the card is trashed as if completing its resolution.
    PlayedTrashShield { card: ObjectId },
    /// "Prevent all damage." for a duration (The Noble Path class; 6.8.5) —
    /// removes damage from expected effects while it lives. Run-bound
    /// durations expire at step 6.9.6d (10.3.1b of the checkpoint after the
    /// run frame pops), which is exactly when Dedicated-Response-Team-class
    /// run-ends damage resolves unshielded.
    DamagePreventionAll,
    /// "Access N additional cards" (The Maker's Eye / Seidr class; adds to
    /// the 7.3.6 random access limit at step 7.5.3).
    AdditionalAccess { server: ServerId, extra: u32 },
}

/// Kernel-wave replacement transforms (the mechanism is real; the vocabulary
/// grows with the card layer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplacementTransform {
    /// "Instead, do nothing" — removes the class's atoms.
    Suppress,
    /// Replace damage kind (Tori Hanzō class).
    ChangeDamageKind(crate::effects::DamageKind),
    /// "Instead of breaching, gain N[c]" (Security Testing / Account Siphon
    /// class): removes the atom and pays out to the effect's controller.
    SuppressAndGainCredits(u32),
    /// "Breach, but access from the bottom of R&D" (Showing Off class,
    /// 7.4.7b): the breach is REPLACED but still expected — a subsequent
    /// replacement can still act on it (the 9.9.11a example 2). The kernel
    /// keeps the atom in place; bottom-up candidate order arrives with the
    /// card layer.
    BreachFromBottom,
    /// "Instead of accessing the chosen card, trash <target>" (Immolation
    /// Script class, 7.4.3): the access is suppressed and another card is
    /// trashed. The chosen candidate stays chosen (7.4.3: it ceases to be a
    /// candidate whether or not it was accessed).
    SuppressAccessAndTrashOther(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceValue {
    Server(ServerId),
    Object(ObjectId),
}

/// One lingering effect record (9.10.1): exists independently of its source.
#[derive(Debug, Clone)]
pub struct LingeringEffect {
    pub id: u64,
    pub source: ObjectId,
    pub payload: Payload,
    pub duration: Duration,
    /// CR 9.9.9c: effects this replacement has already applied to (at most
    /// once per effect). Keyed by imminence sequence numbers.
    pub applied_to: Vec<u64>,
}

impl LingeringEffect {
    /// CR 9.10.1 + step 10.3.1b: has this duration passed, given the current
    /// structure instances?
    pub fn expired(
        &self,
        current_encounter: Option<u64>,
        current_run: Option<u64>,
        current_turn: u64,
        source_active: bool,
    ) -> bool {
        cite!("step_checkpoint_duration_abilities");
        match self.duration {
            Duration::Encounter(e) => current_encounter != Some(e),
            Duration::Run(r) => current_run != Some(r),
            Duration::Turn(t) => current_turn != t,
            Duration::UntilResolved => false, // removed on resolution instead
            Duration::WhileSourceActive => !source_active,
            Duration::ExpiredImmediately => true,
            // Persist handling is window-driven; the VM flips these to
            // ExpiredImmediately when the after-run reaction window closes.
            Duration::PersistUntilAfterRun(_) => false,
        }
    }
}

/// CR 9.10.4: a duration referencing a timing structure that is not in
/// progress expires immediately (kept only until the next checkpoint).
pub fn bind_duration(
    wanted: WantedDuration,
    current_encounter: Option<u64>,
    current_run: Option<u64>,
    current_turn: u64,
) -> Duration {
    cite!("rule_lingering_effect_inapplicable_timing_structure");
    match wanted {
        WantedDuration::ThisEncounter => match current_encounter {
            Some(e) => Duration::Encounter(e),
            None => Duration::ExpiredImmediately,
        },
        WantedDuration::ThisRun => match current_run {
            Some(r) => Duration::Run(r),
            None => Duration::ExpiredImmediately,
        },
        WantedDuration::ThisTurn => Duration::Turn(current_turn),
        WantedDuration::UntilResolved => Duration::UntilResolved,
        WantedDuration::WhileSourceActive => Duration::WhileSourceActive,
    }
}

/// Author-facing duration requests, bound to instances at creation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WantedDuration {
    ThisEncounter,
    ThisRun,
    ThisTurn,
    UntilResolved,
    WhileSourceActive,
}
