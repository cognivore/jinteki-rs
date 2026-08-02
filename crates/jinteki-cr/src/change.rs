//! The change buffer: every mutation appends a [`GameChange`]; checkpoint
//! step (a) (`step_checkpoint_conditional_abilities`, 10.3.1a) scans "the
//! changes to the game state since the beginning of the last checkpoint", and
//! "had"-style conditions read the snapshot taken at the *previous*
//! checkpoint's step (a) (`rule_instruction_requirements_past_state`, 9.6.6a).

use crate::effects::DamageKind;
use crate::object::{ObjectId, ServerId, Side, Zone};

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
    ClicksLost { side: Side, amount: u32 },
    CardDrawn { side: Side, obj: ObjectId },
    /// One record per point batch: `cards` are the simultaneous random trashes.
    DamageSuffered { kind: DamageKind, amount: u32, cards: Vec<ObjectId> },
    TagsTaken { amount: u32 },
    TagRemoved,
    /// A card was trashed. `by` is the player whose effect trashed it.
    CardTrashed { obj: ObjectId, by: Side, was_zone: Zone },
    CardDiscarded { obj: ObjectId, side: Side },
    CardInstalled { obj: ObjectId, side: Side },
    /// CR 8.5.13c/d: a card was revealed to verify an installation.
    CardRevealed { obj: ObjectId },
    CardUninstalled { obj: ObjectId, was_zone: Zone },
    CardRezzed { obj: ObjectId },
    CardMoved { obj: ObjectId, from: Zone, to: Zone },
    CounterPlaced { obj: ObjectId, kind: crate::object::CounterKind, amount: u32 },
    AgendaScored { obj: ObjectId, points: i32 },
    AgendaStolen { obj: ObjectId, points: i32 },
    CardAccessed { obj: ObjectId },
    AccessEnded { obj: ObjectId },
    TurnBegan { side: Side },
    TurnEnded { side: Side },
    ActionPhaseEnded { side: Side },
    RunBegan { server: ServerId },
    RunDeclaredSuccessful { server: ServerId },
    RunDeclaredUnsuccessful { server: ServerId },
    /// The run is complete (`step_run_complete`, 6.9.6d).
    RunEnded { server: ServerId, run_id: u64 },
    EncounterBegan { ice: ObjectId, encounter_id: u64 },
    EncounterEnded { ice: ObjectId, encounter_id: u64 },
    IceApproached { ice: ObjectId },
    IcePassed { ice: ObjectId },
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
