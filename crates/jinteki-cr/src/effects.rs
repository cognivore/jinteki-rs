//! Expected effects of imminent instructions (§9.9): [`EffectAtom`]s with
//! continuously-recomputed modifiable values (9.9.6/9.9.7), aggregation
//! classes (9.12.2b/c), the must-be-positive resolution rule (9.9.7d),
//! prevent-all removal (9.9.7b), and ordinal "would" trackers (9.9.5a).

use crate::object::{ObjectId, Side};

/// CR 10.4.2: the three damage types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DamageKind {
    Meat,
    Net,
    Core,
}

/// Classes of effect an atom can carry. The class drives interrupt relevance
/// (9.9.3), aggregation (9.12.2c), and ordinal-would tracking (9.9.5a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectClass {
    Damage(DamageKind),
    TakeTags,
    GainCredits,
    LoseCredits,
    GainClicks,
    LoseClicks,
    Draw,
    /// Trashing a specific set of cards (value = count; targets carried).
    TrashCards,
    EndTheRun,
    Bypass,
    StealAgenda,
    /// Breaching a server (6.9.5b / 7.5) — the replacement-effect target of
    /// the Security Testing / Account Siphon class (9.9.11a).
    Breach,
    /// Accessing the chosen candidate (7.5.5) — the replacement-effect
    /// target of the Immolation Script class (7.4.3).
    AccessCard,
    /// CR 9.9.6c: a cost that would be paid while resolving an effect — the
    /// install/play cost payment steps. Its value is the credits that would
    /// be paid, and an interrupt can modify it (Patchwork class) exactly as
    /// one can modify an imminent damage value.
    PayCost,
    /// CR 6.9.5a: the run being DECLARED successful. It carries no modifiable
    /// value — 6.7.1's declaration is not a number — and it exists for the
    /// one thing 9.9.3 needs of an expected effect: something a "would"
    /// trigger condition can be met by. Without it the Success Phase's own
    /// step is indistinguishable from every other structural step, and an
    /// ability that acts one instruction BEFORE the declaration has no
    /// imminence to be relevant to.
    ///
    /// 9.9.2 removes it: a Crisium-class "runs on this server cannot be
    /// declared successful" means the run would not be declared successful,
    /// and the expected effects have to say so.
    DeclareRunSuccessful,
    /// Structure-internal effects with no modifiable value.
    Structural,
}

/// CR 9.12.2c: the closed list of aggregated effect classes. If all effects
/// tied to a "for each"-style quantity are in this list, they are performed
/// once with aggregated values; otherwise nothing aggregates (9.12.2b).
pub fn is_aggregated_class(class: EffectClass) -> bool {
    cite!("rule_aggregated_instructions");
    matches!(
        class,
        EffectClass::GainCredits
            | EffectClass::LoseCredits
            | EffectClass::GainClicks
            | EffectClass::LoseClicks
            | EffectClass::TakeTags
            | EffectClass::Draw
            | EffectClass::TrashCards
            | EffectClass::Damage(_) // "trashing a number of cards from specified locations (including by damage)"
    )
}

/// One expected effect of an imminent instruction, with its modifiable value.
#[derive(Debug, Clone)]
pub struct EffectAtom {
    pub class: EffectClass,
    /// CR 9.9.6: the modifiable value (damage amount, tag count, …). May go
    /// below 0 while imminent (9.9.7a).
    pub value: i64,
    /// CR 9.9.6a/b: tags and damage must be > 0 at resolution to occur.
    pub must_be_positive: bool,
    /// CR 9.3.3g / 9.4.5: "cannot be prevented" restriction rides the value.
    pub unpreventable: bool,
    /// The player affected (damage/tags: the Runner; credits: `side`).
    pub side: Side,
    /// Specific objects acted on (TrashCards targets etc.).
    pub targets: Vec<ObjectId>,
    /// CR 9.9.7b: "prevent all X" removes the atom entirely; we keep a
    /// tombstone so nothing remains to modify.
    pub removed: bool,
}

impl EffectAtom {
    pub fn new(class: EffectClass, value: i64, side: Side) -> Self {
        let must_be_positive = matches!(class, EffectClass::Damage(_) | EffectClass::TakeTags);
        cite!("rule_modifiable_value_tags");
        cite!("rule_modifiable_value_damage");
        EffectAtom {
            class,
            value,
            must_be_positive,
            unpreventable: false,
            side,
            targets: Vec::new(),
            removed: false,
        }
    }

    pub fn with_targets(mut self, targets: Vec<ObjectId>) -> Self {
        self.targets = targets;
        self
    }

    /// Is this atom still part of the expected effects?
    pub fn expected(&self) -> bool {
        !self.removed
    }

    /// CR 9.9.7d: at resolution, a must-be-positive value ≤ 0 drops that part
    /// of the effect; the rest of the ability resolves (Golden Rules 1.2.3/4).
    pub fn occurs_at_resolution(&self) -> bool {
        cite!("rule_negative_values_resolution");
        if self.removed {
            return false;
        }
        if self.must_be_positive {
            self.value > 0
        } else {
            true
        }
    }

    /// CR 9.9.5: prevent/avoid N — decrease the value (may go below 0 while
    /// imminent, 9.9.7a).
    pub fn prevent(&mut self, n: i64) {
        cite!("rule_negative_values_imminent");
        if !self.unpreventable {
            self.value -= n;
        }
    }

    /// CR 9.9.7b: prevent all — remove the atom entirely; there is no longer
    /// a value to be modified.
    pub fn prevent_all(&mut self) {
        cite!("rule_prevent_all");
        if !self.unpreventable {
            self.removed = true;
        }
    }
}

/// Key for ordinal "would" trackers (9.9.5a): scope × effect class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WouldScope {
    /// "…each turn" — reset when a turn begins.
    Turn,
    /// "…during a run / each run" — reset per run instance.
    Run,
}

/// Per-period counters of *imminences* (not resolutions): "the first time you
/// would X" counts the times X became imminent (9.9.5a — Tori Hanzō).
///
/// The count is per PLAYER as well as per class, because the sentences are:
/// "the first time each turn YOU would draw" (The Class Act) asks about the
/// player the effect is on, and every atom already names them. For the
/// classes that only ever affect one player — damage and tags are always the
/// Runner's — this changes nothing.
#[derive(Debug, Clone, Default)]
pub struct WouldCounters {
    counts: std::collections::HashMap<(WouldScope, EffectClass, Side), u32>,
}

impl WouldCounters {
    /// Bump when an atom of this class becomes imminent for this player;
    /// returns the ordinal (1-based) of this imminence within each scope.
    pub fn bump(&mut self, class: EffectClass, side: Side) {
        cite!("rule_ordinal_would");
        for scope in [WouldScope::Turn, WouldScope::Run] {
            *self.counts.entry((scope, class, side)).or_insert(0) += 1;
        }
    }

    pub fn count(&self, scope: WouldScope, class: EffectClass, side: Side) -> u32 {
        *self.counts.get(&(scope, class, side)).unwrap_or(&0)
    }

    pub fn reset_scope(&mut self, scope: WouldScope) {
        self.counts.retain(|(s, _, _), _| *s != scope);
    }
}
