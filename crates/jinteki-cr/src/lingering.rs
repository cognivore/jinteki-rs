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
    /// CR 2.16.5 / 9.12.1b: subtypes added to or removed from an object for
    /// a duration (Tinkering class). Instances COUNT — a subtype is present
    /// while its adds outnumber its removals.
    SubtypeMod { target: ObjectId, add: Vec<&'static str>, remove: Vec<&'static str> },
    /// CR 9.1.9/9.10.2: an ability granted to an object for a duration.
    GrantedAbility { to: ObjectId, def: AbilityDef },
    /// CR 9.9.8c: a replacement effect created ahead of time. `applies_to`
    /// selects the expected-effect class it can replace; `replace_with` is
    /// the kernel-wave transform.
    ReplacementEffect {
        applies_to: EffectClass,
        replace_with: ReplacementTransform,
        /// CR 6.7.4c: the replacement is one its controller may OPTIONALLY
        /// carry out, so applying it is a Decision, made where the effect it
        /// replaces would happen — for a breach, step 6.9.5b, after
        /// everything the 6.9.5a reaction window held has resolved.
        optional: bool,
    },
    /// CR 9.10.3: a remembered choice (server, object) other abilities of the
    /// same source read.
    MaintainedChoice { key: &'static str, choice: ChoiceValue },
    /// CR 9.6.13: a delayed conditional ability maintained by this effect.
    /// `bound_targets` is CR 1.15.4 crossing the gap the delay makes: the
    /// targets the ability that created this one had already announced, kept
    /// here because the frame that announced them is gone by the time this
    /// ability resolves. Empty for a delayed conditional the rules themselves
    /// create (7.3.8's postponed breach, 8.6.6's removal after play).
    ///
    /// `bound_installs` crosses the same gap for the cards that ability's own
    /// install instructions INSTALLED. They are not targets — an install whose
    /// card came from a search announces nothing at all (8.7.4's find is not
    /// 1.15.2's announcement) — so they cannot ride `bound_targets`, and a
    /// sentence saying "that program" about the card its own earlier sentence
    /// installed (Kabonesa Wu) has nothing else to read.
    DelayedConditional {
        def: AbilityDef,
        bound_targets: Vec<ObjectId>,
        bound_installs: Vec<ObjectId>,
    },
    /// CR 9.12.5: a persistent ability persisting after its source was
    /// trashed during an access; applicable only to the bound run (9.12.5d).
    PersistedAbility { def: AbilityDef, run_id: u64 },
    /// CR 9.8.3a/e: a subroutine granted to a piece of ice by an external
    /// ability; ordering inside its category is by grant sequence.
    /// `ord` is the position of this subroutine WITHIN its grant: one effect
    /// granting several subroutines (Loki class) orders them among themselves
    /// by the order they had on the card they came from, while 9.8.3a/e order
    /// the GRANTS against each other by `seq`. `placement` is 9.8.2c's
    /// declared position in the ice's whole subroutine list, applied after the
    /// category sort ("regardless of categories").
    GrantedSubroutine {
        to: ObjectId,
        sub: AbilityDef,
        before: bool,
        seq: u64,
        ord: u32,
        placement: Option<usize>,
    },
    /// CR 7.4.2: "the Runner cannot access any card other than <obj> for
    /// the remainder of the run" (Ash class).
    RestrictCandidatesTo(ObjectId),
    /// CR 7.4.2b: "the Runner cannot access more than N cards during this
    /// run" (Hudson 1.0 class). Until N accesses have actually been performed
    /// during the run (7.3.6) the effect does nothing at all — in particular
    /// it never touches the random access limit, which 7.3.5 fixes at the
    /// beginning of the breach. From then on nothing is a candidate.
    AccessLimitThisRun { limit: u32 },
    /// CR 9.5.3a: "the Runner cannot use <card>'s abilities" for a duration
    /// (Wendigo class). A prohibition on USE, so the abilities are not
    /// offered in any window — and, since paid abilities are always optional
    /// (9.5.3), a 9.12.3a "must" cannot force one that is prohibited.
    CannotUseAbilitiesOf(ObjectId),
    /// CR 8.6.6c: a played card kept in the play area instead of being
    /// trashed at 8.6.7g; when one of the indicated effects occurs, the
    /// effect expires at checkpoint step 10.3.1b and the card is trashed as
    /// if completing its resolution.
    ///
    /// `until` is copied off the declaration that created the shield rather
    /// than re-read from the card, because 9.1.9a can take the ability away
    /// while the shield is standing (a second Direct-Access-class effect) and
    /// 8.6.6c's lingering effect has already been created by then — the
    /// duration is a property of the effect, not of the ability.
    PlayedTrashShield { card: ObjectId, until: Vec<crate::ability::TriggerCond> },
    /// "Prevent all damage." for a duration (The Noble Path class; 6.8.5) —
    /// removes damage from expected effects while it lives. Run-bound
    /// durations expire at step 6.9.6d (10.3.1b of the checkpoint after the
    /// run frame pops), which is exactly when Dedicated-Response-Team-class
    /// run-ends damage resolves unshielded.
    DamagePreventionAll,
    /// CR 10.11.4: the designation of a server as the mark, which the rule
    /// says IS a lingering effect and one that expires at the end of the
    /// turn. `since` is the change-log index at which the designation
    /// happened: 10.11.5 says a condition checking a game property related to
    /// the mark "only checks from the moment that server was designated", so
    /// the kernel keeps the moment.
    MarkDesignation { server: ServerId, since: usize },
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
    /// The general 9.9.2 form: "instead of <the effect class>, <do these
    /// instructions>". The replaced effect's atom is removed and the
    /// replacement's own instructions resolve in its place, as a rules
    /// ability of the lingering effect's source — so they go through the
    /// full pipeline (imminence, windows, prevention: Account Siphon's tags
    /// are avoidable exactly because of this).
    SuppressAndResolve(Vec<crate::instr::Instruction>),
    /// "Instead of accessing the chosen candidate, remove it from the game"
    /// (Archives Interface class). The access does not happen, so it is not
    /// counted by 7.3.6 — and the chosen candidate is still consumed (7.4.3).
    SuppressAccessAndRemoveChosen,
    /// "Breach, but access from the bottom of R&D" (Showing Off class,
    /// 7.4.7b): the breach is REPLACED but still expected — a subsequent
    /// replacement can still act on it (the 9.9.11a example 2). The kernel
    /// keeps the atom in place; bottom-up candidate order arrives with the
    /// card layer.
    BreachFromBottom,
    /// CR 9.9.9c: "instead, add it to your score area with N hosted counters"
    /// (Project Vacheron class). The agenda IS still added to the score area
    /// — the replacement's result still includes the effect it replaced — but
    /// the replacement cannot apply to its own result, which is the whole of
    /// the rule. The atom stays expected, so a DIFFERENT replacement could
    /// still act on it (9.9.11a).
    StealWithHostedCounters { kind: crate::object::CounterKind, amount: u32 },
    /// "Instead of accessing the chosen card, trash <target>" (Immolation
    /// Script class, 7.4.3): the access is suppressed and another card is
    /// trashed. The chosen candidate stays chosen (7.4.3: it ceases to be a
    /// candidate whether or not it was accessed).
    SuppressAccessAndTrashOther(ObjectId),
}

/// CR 9.10.3: the value a maintained choice remembers. One variant per entry
/// of CR 1.15.1b's list of things an instruction can direct a player to
/// choose or name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceValue {
    Server(ServerId),
    Object(ObjectId),
    /// CR 2.16: a chosen subtype (Pelangi class).
    Subtype(&'static str),
    /// CR 2.15.2: a chosen card type (Azmari EdTech class).
    CardType(crate::object::CardType),
    /// CR 1.15.1b: a value NAMED from an open namespace — a card name (2.1.1)
    /// or a number (1.1.3).
    Named(crate::instr::NamedValue),
}

/// One lingering effect record (9.10.1): exists independently of its source.
#[derive(Debug, Clone)]
pub struct LingeringEffect {
    pub id: u64,
    pub source: ObjectId,
    pub payload: Payload,
    pub duration: Duration,
    /// CR 3.9.5c: a SECOND duration the effect also lives under — it expires
    /// only once BOTH have expired. An icebreaker's paid ability that states
    /// a duration carries the stated one here alongside the implicit
    /// "remainder of the current encounter" (3.9.5b), which is exactly what
    /// makes "+1 for the remainder of this run" resolved during an encounter
    /// outside a run last for the remainder of that encounter (and 3.4.4a
    /// say the same thing about a piece of ice's strength).
    pub also: Option<Duration>,
    /// CR 9.10.5 / 9.9.9a: a duration-modifying replacement effect has
    /// already rewritten this effect's duration once. The rewrite happens at
    /// the checkpoint where the original duration expires and only while the
    /// replacement is active, so the flag keeps it to once per effect
    /// (9.9.9c).
    pub duration_extended: bool,
    /// CR 9.9.9c: effects this replacement has already applied to (at most
    /// once per effect). Keyed by imminence sequence numbers.
    pub applied_to: Vec<u64>,
}

impl LingeringEffect {
    /// The common shape: one duration, nothing applied yet.
    pub fn new(id: u64, source: ObjectId, payload: Payload, duration: Duration) -> LingeringEffect {
        LingeringEffect {
            id,
            source,
            payload,
            duration,
            also: None,
            duration_extended: false,
            applied_to: Vec::new(),
        }
    }

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
        if let Some(also) = self.also {
            // 3.9.5c: both the stated and the implicit duration must expire.
            cite!("rule_icebreaker_strength_increase_specified");
            cite!("rule_ice_strength_modification_duration");
            if !expired_one(also, current_encounter, current_run, current_turn, source_active) {
                return false;
            }
        }
        expired_one(self.duration, current_encounter, current_run, current_turn, source_active)
    }
}

fn expired_one(
    duration: Duration,
    current_encounter: Option<u64>,
    current_run: Option<u64>,
    current_turn: u64,
    source_active: bool,
) -> bool {
    {
        match duration {
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
