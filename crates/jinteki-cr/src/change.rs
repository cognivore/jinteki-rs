//! The change buffer: every mutation appends a [`GameChange`]; checkpoint
//! step (a) (`step_checkpoint_conditional_abilities`, 10.3.1a) scans "the
//! changes to the game state since the beginning of the last checkpoint", and
//! "had"-style conditions read the snapshot taken at the *previous*
//! checkpoint's step (a) (`rule_instruction_requirements_past_state`, 9.6.6a).

use crate::effects::DamageKind;
use crate::object::{ObjectId, ServerId, Side, Zone};

/// CR 5.2.6/5.2.7: the basic actions, as identities (5.2.5a: "actions are
/// the same if they are all the same basic action").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicAction {
    Credit,
    Draw,
    Run,
    RemoveTag,
    /// 5.2.6e: "[click]: Play 1 operation from HQ."
    PlayOperation,
    /// 5.2.6d / 5.2.7d: "[click]: Install 1 … card."
    Install,
    /// 5.2.6f: "[click], 1[credit]: Advance 1 installed card."
    Advance,
    /// 5.2.6g: "[click], 2[credit]: Trash 1 resource."
    TrashResource,
    /// 5.2.6h: "[click][click][click]: Purge virus counters."
    Purge,
}

/// CR 5.2.5a/b: what makes two actions the same or different — the basic
/// action they are, or the CARD ABILITY that initiated them ("instances of
/// equivalent abilities on different cards are still different actions", so
/// the identity is the ability reference, not its text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionIdentity {
    Basic(BasicAction),
    CardAbility(crate::ability::AbilityRef),
}

/// The record vocabulary. One record per occurrence; simultaneous set-effects
/// produce one record per member *plus* shared `group` so per-event triggers
/// (Warroid Tracker class, 9.12.2a) can collapse them while per-occurrence
/// triggers (Hostile Infrastructure class, 9.6.4b) see each one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameChange {
    CreditsGained { side: Side, amount: u32 },
    CreditsLost { side: Side, amount: u32 },
    ClicksGained { side: Side, amount: u32 },
    ClickSpent { side: Side },
    /// CR 5.2.5: a player took an action. The identity is what 5.2.5a/b say
    /// makes two actions "the same" or "different".
    ActionTaken { side: Side, action: ActionIdentity },
    ClicksLost { side: Side, amount: u32 },
    CardDrawn { side: Side, obj: ObjectId },
    /// One record per point batch: `cards` are the simultaneous random trashes.
    DamageSuffered { kind: DamageKind, amount: u32, cards: Vec<ObjectId> },
    TagsTaken { amount: u32 },
    TagRemoved,
    /// CR 9.9.5-adjacent: the Runner AVOIDED receiving tags (Thunder Art
    /// Gallery class conditions).
    TagsAvoided { amount: u32 },
    /// CR 9.9.7f: an interrupt from `by` decreased — or removed — the amount
    /// of an imminent damage effect whose value was GREATER THAN 0. That last
    /// clause is the whole rule: preventing an effect already reduced to 0
    /// prevents no damage (the Guru Davinder example).
    DamagePrevented { by: ObjectId, kind: DamageKind, amount: u32 },
    /// A card was trashed. `by` is the player whose effect trashed it.
    CardTrashed { obj: ObjectId, by: Side, was_zone: Zone },
    CardDiscarded { obj: ObjectId, side: Side },
    /// CR 8.5.16f. `from` is the zone the card is treated as having been
    /// installed FROM — 4.8.3 substitutes the pre-set-aside location for a
    /// card installed out of the set-aside zone, which is the only way an
    /// "install a program from your heap" condition can see a searched
    /// install at all.
    CardInstalled { obj: ObjectId, side: Side, from: Zone },
    /// CR 8.5.13c/d: a card was revealed to verify an installation.
    CardRevealed { obj: ObjectId },
    /// CR 1.21.2: a player LOOKED at a card — they may see its front face
    /// without showing it to the other player. 1.21.5 keeps it distinct from
    /// revealing, exposing and accessing.
    CardLookedAt { obj: ObjectId, by: Side },
    /// CR 1.21.4: a card was EXPOSED — revealed, except that only installed
    /// unrezzed cards can be exposed. 1.21.5 keeps it distinct from a reveal.
    CardExposed { obj: ObjectId },
    /// CR 8.6.7d: conditions related to playing an event/operation are met.
    CardPlayed { obj: ObjectId, side: Side },
    /// CR 8.6.7h: conditions related to finishing resolving it are met.
    CardPlayResolved { obj: ObjectId },
    CardUninstalled { obj: ObjectId, was_zone: Zone },
    /// CR 8.1.2: a card was rezzed. The card TYPE travels with the record
    /// because "whenever you rez a piece of ice" is answered at the
    /// checkpoint scan, when the object may already have moved on.
    CardRezzed { obj: ObjectId, card_type: crate::object::CardType },
    /// CR 8.1.2 / 1.12.5: a card turned facedown again. It does NOT become a
    /// new object — it never left the play area.
    CardDerezzed { obj: ObjectId },
    CardMoved { obj: ObjectId, from: Zone, to: Zone },
    CounterPlaced { obj: ObjectId, kind: crate::object::CounterKind, amount: u32 },
    /// CR 1.18.1: a card was ADVANCED — an advancement counter was placed on
    /// it by an advance. 1.18.2: placing an advancement counter directly, or
    /// moving one from another card, is NOT advancing, and records only
    /// `CounterPlaced`, so a "whenever you advance" condition is not met.
    CardAdvanced { obj: ObjectId },
    /// CR 10.1.2: the Corp purged virus counters. Recorded once per purge,
    /// whether or not any counters were on the board — the condition Clot's
    /// class meets is "when the Corp purges virus counters", not "when a
    /// virus counter is removed" (the removals are their own records).
    VirusCountersPurged,
    /// Counters left a card or a player and returned to the bank (1.9.2) —
    /// spent, removed, or trashed with their host (1.13.13).
    CounterRemoved { obj: Option<ObjectId>, kind: crate::object::CounterKind, amount: u32 },
    /// CR 1.13.1: a host relationship was created between two objects.
    CardHosted { obj: ObjectId, host: ObjectId },
    AgendaScored { obj: ObjectId, points: i32 },
    /// CR 8.2.5: an agenda was forfeited — moved from a score area to the
    /// removed-from-game zone, so its agenda points no longer contribute.
    AgendaForfeited { obj: ObjectId, by: Side },
    AgendaStolen { obj: ObjectId, points: i32 },
    /// CR 10.6.1: a player took bad publicity counters.
    BadPublicityTaken { side: Side, amount: u32 },
    CardAccessed { obj: ObjectId },
    AccessEnded { obj: ObjectId },
    TurnBegan { side: Side },
    TurnEnded { side: Side },
    ActionPhaseEnded { side: Side },
    /// CR rule_identity_double_sided: a double-sided identity turned over.
    IdentityFlipped { side: Side },
    RunBegan { server: ServerId },
    RunDeclaredSuccessful { server: ServerId },
    RunDeclaredUnsuccessful { server: ServerId },
    /// The run is complete (`step_run_complete`, 6.9.6d).
    RunEnded { server: ServerId, run_id: u64 },
    EncounterBegan { ice: ObjectId, encounter_id: u64 },
    EncounterEnded { ice: ObjectId, encounter_id: u64 },
    IceApproached { ice: ObjectId },
    /// CR 6.9.4a: the Runner passed a piece of ice. `after_encounter` is
    /// 6.1.3e's "direct sequence" test — whether this pass directly follows
    /// an Encounter Ice Phase WITH THIS ICE — and the other two flags carry
    /// what happened during that encounter, since 6.1.3f scopes "after fully
    /// breaking it" to that encounter and 9.8.9's Persephone class asks
    /// whether any subroutine resolved from this ice.
    IcePassed { ice: ObjectId, after_encounter: bool, fully_broken: bool, subs_resolved: bool },
    ServerApproached { server: ServerId },
    /// CR 1.16.3: a cost was paid (zero costs are real, 1.16.1d).
    CostPaid { side: Side, credits: u32, clicks: u32, trashed: Vec<ObjectId> },
    /// CR 9.1.6: an ability/source was used.
    AbilityUsed { source: ObjectId },
    /// A [trash]-cost ability's trigger cost trashed its own source
    /// (9.1.6a-adjacent; drives Geist-style "use a trash ability" triggers).
    TrashAbilityUsed { source: ObjectId, side: Side },
    BreachBegan { server: ServerId },
    BreachEnded { server: ServerId },
    CardEnteredRoot { obj: ObjectId, server: ServerId },
    SubroutineResolved { ice: ObjectId, index: usize },
    /// CR 9.12.2d: "all subroutines broken" became satisfied for this
    /// encounter — including vacuously, for ice with zero subroutines, as
    /// soon as step 6.9.3b begins.
    AllSubsBroken { ice: ObjectId },
    /// CR 8.7.3: a deck was shuffled. Recorded when the shuffle happens, so
    /// the log order witnesses "immediately, before continuing to resolve
    /// any remaining effects".
    DeckShuffled { side: Side },
    /// CR 8.7.5: a search of `zone` by `by` is COMPLETE and any necessary
    /// shuffling has been performed — recorded after the shuffle so a
    /// condition involving a search cannot become met before it.
    ZoneSearched { by: Side, zone: Zone },
    /// 10.8.6a: a trace initiated.
    TraceInitiated { base: i64 },
    /// 10.8.6e: the trace was determined.
    TraceDetermined { success: bool, trace_strength: i64, link_strength: i64 },
    GameBegan,
}

/// Buffer + previous-checkpoint snapshot (9.6.6a).
#[derive(Debug, Clone, Default)]
pub struct ChangeBuffer {
    pub log: Vec<GameChange>,
    /// Group stamps parallel to `log`: records emitted by the same atomic
    /// effect share a group (9.12.2a: one effect acting on a set of cards
    /// acts on all of them simultaneously).
    pub groups: Vec<u64>,
    /// Index into `log` where the *current* "since the beginning of the last
    /// checkpoint" window starts (10.3.1a).
    pub last_checkpoint_start: usize,
    /// Monotone group counter.
    pub next_group: u64,
    /// Checkpoints seen (for instance bookkeeping).
    pub checkpoint_seq: u64,
}

impl ChangeBuffer {
    pub fn record(&mut self, c: GameChange) {
        self.log.push(c);
        self.groups.push(self.next_group);
    }

    /// Start a new atomicity group (called per instruction resolution /
    /// per atomic game action).
    pub fn bump_group(&mut self) {
        self.next_group += 1;
    }

    /// CR 10.3.1a: the changes since the beginning of the last checkpoint.
    pub fn since_last_checkpoint(&self) -> impl Iterator<Item = (&GameChange, u64)> {
        cite!("step_checkpoint_conditional_abilities");
        self.log[self.last_checkpoint_start..]
            .iter()
            .zip(self.groups[self.last_checkpoint_start..].iter().copied())
    }

    /// Mark the beginning of a checkpoint's step (a): everything before this
    /// index has been scanned; the *next* checkpoint scans from here.
    pub fn begin_checkpoint_scan(&mut self) -> usize {
        let old = self.last_checkpoint_start;
        self.last_checkpoint_start = self.log.len();
        self.checkpoint_seq += 1;
        old
    }
}
