//! Abilities: the five types (9.3.7), the six flags (9.3.6), activity
//! (9.1.7 with ALL of 9.1.8's exceptions), pending instances (9.6.2-9.6.4),
//! independence points (9.5.4/9.6.12/9.8.8 enforced per 9.1.4),
//! optional-vs-mandatory (9.6.9), and static-condition repetition with the
//! no-effect throttle (9.6.7).

use crate::change::GameChange;
use crate::effects::DamageKind;
use crate::instr::Instruction;
use crate::object::{CardType, Object, ObjectId, ServerId, Side, Zone};

/// CR 9.1.1e / 9.3.7: every ability is exactly one of these five types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityKind {
    Static,
    Paid,
    Conditional,
    Play,
    Subroutine,
}

/// CR 9.3.6a: the six ability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityFlag {
    /// 9.3.6b: mid-access-window-only paid abilities.
    Access,
    /// 9.3.6c: icebreaker strength-gated abilities.
    Interface,
    /// 9.3.6d: interrupt-window-only abilities.
    Interrupt,
    /// 9.3.6e: can persist after trash-during-access.
    Persistent,
    /// 9.3.6f: active only at threat ≥ N.
    Threat(u8),
    /// 9.3.6g: usable once per turn.
    OncePerTurn,
}

/// Trigger conditions the W1 kernel can detect in checkpoint step (a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerCond {
    /// "When your turn begins." (side = controller's side)
    TurnBegins(Side),
    /// "When this run ends." Optionally only if it was successful.
    RunEnds { successful_only: bool },
    /// "Whenever a run on this server ends." (AMAZE class; source in root)
    RunOnThisServerEnds,
    /// "Whenever the Runner trashes a Corp card." — one instance per card
    /// (per-occurrence, 9.6.4b / 9.12.2a Hostile Infrastructure).
    RunnerTrashesCorpCard,
    /// "Whenever the Runner trashes at least 1 Corp card." — one instance per
    /// event (Warroid Tracker class, 9.12.2a).
    RunnerTrashesAtLeastOneCorpCard,
    /// "When you access this card." (active while inactive, 9.1.8a)
    SelfAccessed,
    /// "When the Runner encounters this ice."
    SelfEncountered,
    /// "Whenever the Runner encounters a piece of ice." (Runner-side class)
    EncounterBegins,
    /// "Whenever the Runner takes a tag." (Mr. Stone class)
    RunnerTakesTag,
    /// "Whenever you use a [trash] ability." (Geist-adjacent test class)
    UsesTrashAbility(Side),
    /// "Whenever you advance a card." `had_no_advancement` adds the
    /// 9.6.6a "had"-condition read against the previous checkpoint snapshot.
    AdvancesCard { had_no_advancement: bool },
    /// Interrupt trigger: "…would do damage" (ordinal: Some(1) = "the first
    /// time each run you would…", Tori Hanzō class).
    WouldDamage { kind: Option<DamageKind>, first_each_run: bool },
    /// Interrupt trigger: "…would take tags during a run" (Jesminder class:
    /// `during_run` requires a run to be in progress).
    WouldTakeTags { during_run: bool },
}

/// Static conditions (9.6.7) for repeat-while-true conditionals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticCond {
    /// "While this card's host has 0 or less strength…" (Parasite class).
    HostStrengthAtMost(i32),
}

/// CR 9.6.1a: the primary condition is a trigger or static condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Trigger(TriggerCond),
    Static(StaticCond),
}

/// Trigger cost of a paid ability (1.16.8: trigger costs; paid all at once).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cost {
    pub credits: u32,
    pub clicks: u32,
    /// [trash]: trash this card as part of the cost.
    pub trash_self: bool,
}

impl Cost {
    pub fn credits(n: u32) -> Self {
        Cost { credits: n, ..Default::default() }
    }
    pub fn trash_self() -> Self {
        Cost { trash_self: true, ..Default::default() }
    }
    pub fn free() -> Self {
        Cost::default()
    }
}

/// Declarations of a static ability (kernel-wave subset). Statics never
/// resolve (9.4.1) — the VM queries them continuously.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticDecl {
    /// Characteristic modification of the source's host (Hush class) or self.
    StrengthMod { target_self: bool, delta: i32 },
    /// Remove all abilities of the host (Hush) — 9.12.1d/e material.
    RemoveHostAbilities,
    /// "This ice cannot be trashed by <side>'s card abilities."
    /// (Architect class; a restriction active per 9.1.8.)
    CannotBeTrashed,
    /// "Runs on this server cannot be declared successful." (Crisium class.)
    RunsNotDeclaredSuccessful,
    /// Memory limit modifier (Runner).
    MemoryLimitMod(i32),
    /// "+N to the amount of <kind> damage done by <responsible>."
    /// (The Cleaners class — modifies imminent damage values via statics.)
    DamageBonus { kind: DamageKind, responsible: Side, amount: i64 },
}

/// One ability as printed/granted: the unit of rules text (9.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityDef {
    pub kind: AbilityKind,
    pub flags: Vec<AbilityFlag>,
    /// Conditional abilities: the primary condition (9.6.1).
    pub condition: Option<Condition>,
    /// Paid abilities: the trigger cost (9.5.1).
    pub cost: Option<Cost>,
    /// Non-static abilities: instructions in order (9.1.1g, 9.1.2).
    pub instructions: Vec<Instruction>,
    /// Static abilities: declarations (9.3.7a).
    pub statics: Vec<StaticDecl>,
    /// CR 9.6.9: optional iff the ability could have no effects at all
    /// ("may"/"allows"/once-per-turn). Mandatory otherwise.
    pub optional: bool,
    /// Human-readable tag for tests/logs.
    pub label: &'static str,
}

impl AbilityDef {
    pub fn conditional(cond: TriggerCond, instrs: Vec<Instruction>, optional: bool) -> Self {
        AbilityDef {
            kind: AbilityKind::Conditional,
            flags: Vec::new(),
            condition: Some(Condition::Trigger(cond)),
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional,
            label: "",
        }
    }

    pub fn paid(cost: Cost, instrs: Vec<Instruction>) -> Self {
        // CR 9.5.3: paid abilities are always optional.
        AbilityDef {
            kind: AbilityKind::Paid,
            flags: Vec::new(),
            condition: None,
            cost: Some(cost),
            instructions: instrs,
            statics: Vec::new(),
            optional: true,
            label: "",
        }
    }

    pub fn subroutine(instrs: Vec<Instruction>) -> Self {
        AbilityDef {
            kind: AbilityKind::Subroutine,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional: false,
            label: "",
        }
    }

    pub fn static_ability(statics: Vec<StaticDecl>) -> Self {
        AbilityDef {
            kind: AbilityKind::Static,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: Vec::new(),
            statics,
            optional: false,
            label: "",
        }
    }

    pub fn with_flag(mut self, f: AbilityFlag) -> Self {
        self.flags.push(f);
        self
    }

    pub fn labeled(mut self, l: &'static str) -> Self {
        self.label = l;
        self
    }

    pub fn has_flag(&self, f: AbilityFlag) -> bool {
        self.flags.contains(&f)
    }

    /// CR 9.9.1: an interrupt is flagged [interrupt] or uses
    /// prevent/avoid/would. In the kernel the card layer sets the flag or the
    /// instruction vocabulary implies it.
    pub fn is_interrupt(&self) -> bool {
        cite!("rule_interrupt_keywords");
        if self.has_flag(AbilityFlag::Interrupt) {
            return true;
        }
        if let Some(Condition::Trigger(
            TriggerCond::WouldDamage { .. } | TriggerCond::WouldTakeTags { .. },
        )) = self.condition
        {
            return true;
        }
        self.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::PreventDamage { .. }
                    | Instruction::PreventAllDamage { .. }
                    | Instruction::AvoidTags(_)
                    | Instruction::IncreaseImminentDamage { .. }
                    | Instruction::PreventTrashOf(_)
            )
        })
    }

    /// CR 5.2.1: an action is a paid ability whose cost begins with [click].
    pub fn is_action(&self) -> bool {
        cite!("rule_action");
        self.kind == AbilityKind::Paid && self.cost.as_ref().map(|c| c.clicks > 0).unwrap_or(false)
    }
}

/// Reference to one ability on one object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilityRef {
    pub obj: ObjectId,
    pub index: usize,
}

/// A pending/resolving instance of a conditional ability (9.6.2).
#[derive(Debug, Clone)]
pub struct AbilityInstance {
    pub id: u64,
    pub ability: AbilityRef,
    /// Snapshot of the definition at creation (survives source movement).
    pub def: AbilityDef,
    pub controller: Side,
    /// CR 9.6.9: mandatory instances gate passing (9.2.8e).
    pub mandatory: bool,
    /// The window this instance is pending in (fixed set, 9.2.8a).
    pub window: Option<u64>,
    /// CR 9.1.8g: condition met by the source's own move to an inactive
    /// zone — the ability remains active until this instance resolves.
    pub hangover: bool,
    /// CR 9.6.12/9.5.4/9.8.8 → 9.1.4: once independent, a source zone change
    /// strands self-referencing effects. `source_zone_stamp` is the source's
    /// move counter at independence.
    pub independent: bool,
    pub source_move_stamp: u64,
    /// Group of the change occurrence that created this instance.
    pub occurrence_group: u64,
    /// For delayed conditionals: the lingering effect maintaining it.
    pub from_lingering: Option<u64>,
    /// Structure instance this pending is tied to (persistent/9.12.5d and
    /// run-scoped conditions).
    pub run_id: Option<u64>,
}

/// CR 9.1.7 + 9.1.8: whether an ability is active. `encounter_ice` is the
/// currently-encountered ice (for 9.1.8h), `accessed` the currently-accessed
/// card (for 9.1.8a mid-access relevance).
pub fn ability_active(
    obj: &Object,
    def: &AbilityDef,
    encountered_ice: Option<ObjectId>,
    accessed_card: Option<ObjectId>,
) -> bool {
    cite!("rule_ability_active");
    if crate::object::card_active(obj) {
        return true;
    }
    // 9.1.8a: access-condition abilities are active while the card is
    // inactive (so "when accessed" fires on cards in R&D/HQ/Archives).
    if matches!(
        def.condition,
        Some(Condition::Trigger(TriggerCond::SelfAccessed))
    ) {
        cite!("rule_active_exception_access");
        return true;
    }
    if def.has_flag(AbilityFlag::Access) && accessed_card == Some(obj.id) {
        cite!("rule_active_exception_access");
        return true;
    }
    // 9.1.8h: subroutines of uninstalled encountered ice are active during
    // that encounter.
    if def.kind == AbilityKind::Subroutine && encountered_ice == Some(obj.id) {
        cite!("rule_active_exception_encounter_not_installed");
        return true;
    }
    // 9.1.8c/d/e/f: play/install/rez permissions and cost modifiers,
    // advancement-requirement modifiers, can-advance grants. The kernel-wave
    // StaticDecl set has no such declarations yet; when the card layer adds
    // them they gain activity here.
    cite!("rule_active_exception_modify_play_install_rez");
    cite!("rule_active_exception_modify_cost");
    cite!("rule_active_exception_advancement_requirement");
    cite!("rule_active_exception_can_be_advanced");
    // 9.1.8b: zone-scoped abilities (none in the W1 vocabulary).
    cite!("rule_active_exception_catchall");
    // 9.1.8g is instance-driven (hangover) and handled by the checkpoint scan.
    // 9.1.8i persistent: handled via lingering effects.
    false
}

/// Does a change record match a trigger condition? Returns per-occurrence
/// match; the checkpoint scan handles multiplicity/grouping (9.6.4b,
/// 9.12.2a) and "had"-snapshot requirements (9.6.6a).
pub fn trigger_matches(
    cond: &TriggerCond,
    change: &GameChange,
    source: &Object,
    server_of_source: Option<ServerId>,
    trashed_is_corp: impl Fn(ObjectId) -> bool,
) -> bool {
    cite!("rule_trigger_condition_checked");
    match (cond, change) {
        (TriggerCond::TurnBegins(side), GameChange::TurnBegan { side: s }) => side == s,
        (TriggerCond::RunEnds { .. }, GameChange::RunEnded { .. }) => true,
        (TriggerCond::RunOnThisServerEnds, GameChange::RunEnded { server, .. }) => {
            server_of_source == Some(*server)
        }
        (TriggerCond::RunnerTrashesCorpCard, GameChange::CardTrashed { by, obj, .. }) => {
            *by == Side::Runner && trashed_is_corp(*obj)
        }
        (
            TriggerCond::RunnerTrashesAtLeastOneCorpCard,
            GameChange::CardTrashed { by, obj, .. },
        ) => *by == Side::Runner && trashed_is_corp(*obj),
        (TriggerCond::SelfAccessed, GameChange::CardAccessed { obj }) => *obj == source.id,
        (TriggerCond::SelfEncountered, GameChange::EncounterBegan { ice, .. }) => {
            *ice == source.id
        }
        (TriggerCond::EncounterBegins, GameChange::EncounterBegan { .. }) => true,
        (TriggerCond::RunnerTakesTag, GameChange::TagsTaken { .. }) => true,
        (TriggerCond::UsesTrashAbility(side), GameChange::TrashAbilityUsed { side: s, .. }) => {
            side == s
        }
        (TriggerCond::AdvancesCard { .. }, GameChange::CounterPlaced { kind, .. }) => {
            *kind == crate::object::CounterKind::Advancement
        }
        _ => false,
    }
}

/// CR 9.6.4b vs 9.12.2a: is this trigger per-occurrence (each matching
/// change record pends an instance) or per-event (one instance per change
/// group)?
pub fn trigger_per_event(cond: &TriggerCond) -> bool {
    cite!("rule_act_on_multiple_cards");
    matches!(cond, TriggerCond::RunnerTrashesAtLeastOneCorpCard)
}

/// Is a card a Corp card by printed side (for trash-trigger filters)?
pub fn is_corp_card(t: CardType) -> bool {
    matches!(
        t,
        CardType::Identity
            | CardType::Agenda
            | CardType::Asset
            | CardType::Ice
            | CardType::Operation
            | CardType::Upgrade
    )
}

/// Zone shorthand used by trigger filters.
pub fn in_archives(z: Zone) -> bool {
    matches!(z, Zone::Discard(Side::Corp))
}
