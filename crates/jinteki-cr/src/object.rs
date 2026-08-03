//! Objects, zones (§4), ownership/control (§1.14), and the characteristics
//! pipeline (9.12.1a/b/d/e).
//!
//! An object is a card or counter addressed by [`ObjectId`] (CR 1.12). Zones
//! are per §4. Effective characteristics (strength, subtypes, abilities
//! present) are computed from printed values by dependency-ordered application
//! of active static/lingering effects (9.12.1d/e) with the value-stacking rule
//! set → increase → decrease (9.12.1a) and count-based subtypes (9.12.1b).

use crate::ability::AbilityDef;
use std::collections::BTreeSet;

/// CR 1.1.1: the two players / game roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Corp,
    Runner,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Corp => Side::Runner,
            Side::Runner => Side::Corp,
        }
    }
}

/// Object identity (CR 1.12.1: cards and counters are objects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub u32);

/// Servers (CR 4.6.5-4.6.7): three centrals plus numbered remotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ServerId {
    Hq,
    Rnd,
    Archives,
    Remote(u32),
}

impl ServerId {
    /// CR 4.6.5: HQ, R&D, and Archives are the central servers.
    pub fn is_central(self) -> bool {
        !matches!(self, ServerId::Remote(_))
    }
}

/// CR 6.2.1: ONE position in a server's sequence of positions, which is what
/// assigns an order to the ice protecting that server.
///
/// A position is an OBJECT OF ITS OWN, not an index: 6.2.6 says the Runner's
/// current position "is a specific element of the sequence of positions, not
/// an index into that sequence", so adding or removing other positions cannot
/// move them. Positions are created by 6.2.2 (an ice installed protecting a
/// server, or an installed ice moved), destroyed by 6.2.4 (vacated, at
/// checkpoint step 10.3.1i unless the Runner is standing in them), and
/// neither created nor destroyed by 6.2.2f (a swap re-occupies the existing
/// positions). A position can be momentarily vacant — that is exactly the
/// state 6.2.4 cleans up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcePosition {
    /// Stable identity: what `RunCtx.position` names (6.2.6).
    pub id: u64,
    /// The ice occupying this position (6.2.1: exactly 1 at a time), or
    /// `None` while the position is vacant.
    pub ice: Option<ObjectId>,
}

/// Game zones per §4: deck (4.2), hand (4.3), discard pile (4.4), score area
/// (4.5), play area (4.6), bank (4.7), set-aside (4.8), removed (4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    /// CR 4.2 (`sec_deck`): R&D / the stack.
    Deck(Side),
    /// CR 4.3 (`sec_hand`): HQ / the grip.
    Hand(Side),
    /// CR 4.4 (`sec_discard_pile`): Archives / the heap.
    Discard(Side),
    /// CR 4.5 (`sec_score_area`).
    ScoreArea(Side),
    /// CR 4.6: installed in the root of a server (Corp assets/upgrades/agendas).
    Root(ServerId),
    /// CR 4.6: installed ice protecting a server. Order lives on the server.
    Ice(ServerId),
    /// CR 4.6: the Runner's rig (installed Runner cards).
    Rig,
    /// CR 4.6: identity / play-area home of identities and currently-resolving
    /// events/operations.
    PlayArea(Side),
    /// CR 4.7 (`sec_bank`).
    Bank,
    /// CR 4.8 (`sec_set_aside`).
    SetAside,
    /// CR 4.9 (`sec_removed_from_game`).
    RemovedFromGame,
}

impl Zone {
    /// Hidden/inactive zones for card activity purposes (CR 1.8.3-adjacent).
    pub fn is_installed(self) -> bool {
        matches!(self, Zone::Root(_) | Zone::Ice(_) | Zone::Rig)
    }

    /// CR 4.6: the play area is ONE zone. `Root`/`Ice`/`Rig`/`PlayArea` are
    /// locations within it, so moving between them is a move within a zone
    /// (1.12.4) and not a move to another zone (1.12.3).
    pub fn zone_class(self) -> u8 {
        cite!("rule_play_area");
        match self {
            Zone::Root(_) | Zone::Ice(_) | Zone::Rig | Zone::PlayArea(_) => 0,
            Zone::Deck(Side::Corp) => 1,
            Zone::Deck(Side::Runner) => 2,
            Zone::Hand(Side::Corp) => 3,
            Zone::Hand(Side::Runner) => 4,
            Zone::Discard(Side::Corp) => 5,
            Zone::Discard(Side::Runner) => 6,
            Zone::ScoreArea(Side::Corp) => 7,
            Zone::ScoreArea(Side::Runner) => 8,
            Zone::Bank => 9,
            Zone::SetAside => 10,
            Zone::RemovedFromGame => 11,
        }
    }
}

/// CR 2.15: card types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardType {
    // Corp
    Identity,
    Agenda,
    Asset,
    Ice,
    Operation,
    Upgrade,
    // Runner
    Event,
    Hardware,
    Program,
    Resource,
}

/// Counter kinds tracked on objects (CR 1.9.5). A counter of any of these
/// kinds can sit on a *player* (1.9.5c/d: tags and bad publicity) or be
/// **hosted** on a card (1.13.1); 1.13.3 is the rule that keeps the two
/// populations apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CounterKind {
    Advancement,
    Credit,
    Power,
    Virus,
    Agenda,
    /// CR 1.9.5d / 10.6.1: bad publicity counters. Placed on the Corp, or
    /// hosted on a card — in which case 1.13.3 makes them invisible to the
    /// fund (10.6.3a) and to abilities removing bad publicity from the Corp.
    BadPublicity,
}

/// The printed (immutable) face of a card — the base of the characteristics
/// pipeline (9.12.1d starts "begin with each object's printed characteristics").
#[derive(Debug, Clone)]
pub struct PrintedCard {
    pub name: &'static str,
    pub side: Side,
    pub card_type: CardType,
    pub subtypes: Vec<&'static str>,
    /// CR 2.7: strength (ice and icebreakers).
    pub strength: Option<i32>,
    /// CR 2.3: play/install/rez cost as printed.
    pub cost: Option<u32>,
    /// CR 2.4: advancement requirement (agendas).
    pub advancement_requirement: Option<u32>,
    /// CR 2.5: agenda points.
    pub agenda_points: Option<i32>,
    /// CR 2.6: trash cost.
    pub trash_cost: Option<u32>,
    /// CR 2.8: memory cost (programs).
    pub memory_cost: Option<u32>,
    /// CR 2.2: unique (◆).
    pub unique: bool,
    /// Console subtype shortcut for checkpoint step 10.3.1d.
    pub console: bool,
    /// Recurring credits refilled at the refill steps.
    pub recurring_credits: Option<u32>,
    /// CR 1.16.10 printed additional cost to steal (Obokata class).
    pub additional_steal_cost: Option<crate::ability::Cost>,
    /// CR 1.16.4c: an additional cost to rez (Archer class); declinable
    /// during "install and rez" effects (8.5.13d).
    pub additional_rez_cost: Option<crate::ability::Cost>,
    /// CR 1.10.3c: hosted credits on this card are spendable by its
    /// controller (Fencer Fueno class — drives bid legality, 10.14.3).
    pub hosted_credits_spendable: bool,
    pub abilities: Vec<AbilityDef>,
}

impl PrintedCard {
    pub fn vanilla(name: &'static str, side: Side, card_type: CardType) -> Self {
        PrintedCard {
            name,
            side,
            card_type,
            subtypes: Vec::new(),
            strength: None,
            cost: Some(0),
            advancement_requirement: None,
            agenda_points: None,
            trash_cost: None,
            memory_cost: None,
            unique: false,
            console: false,
            recurring_credits: None,
            additional_steal_cost: None,
            additional_rez_cost: None,
            hosted_credits_spendable: false,
            abilities: Vec::new(),
        }
    }
}

/// A game object: a card (or card-as-counter) with position and state.
#[derive(Debug, Clone)]
pub struct Object {
    pub id: ObjectId,
    pub printed: PrintedCard,
    pub zone: Zone,
    /// CR 8.1: faceup/facedown; a faceup installed Corp card is rezzed.
    pub faceup: bool,
    /// CR 1.14.1: the owner is the player whose deck the card started in.
    pub owner: Side,
    /// CR 1.14.3: default controller is the owner; control can change.
    pub controller: Side,
    /// CR 1.13: host relationships.
    pub host: Option<ObjectId>,
    pub hosted: Vec<ObjectId>,
    /// CR 1.13.2a: hosted by an ability that did not refer to installing it,
    /// so the card is in the play area (4.6.5h) but NOT installed and
    /// therefore not active. Also set on an installed Corp card that became
    /// hosted on a Runner card (1.13.2b).
    pub hosted_not_installed: bool,
    /// Counters hosted on this object (CR 1.9).
    pub counters: std::collections::BTreeMap<CounterKind, u32>,
    /// Monotonic stamp of the moment this card last became active — used by
    /// checkpoint step 10.3.1d ("became active most recently").
    pub active_since: u64,
    /// CR 9.5.5: temporarily set aside during a self-uninstalling trigger
    /// cost; invisible to other abilities, still "hosted" for that ability.
    pub set_aside_for_ability: bool,
    /// CR 8.5.16a / 8.6.7a: placed into the play area as the first step of
    /// installing/playing — "It is not yet installed or active."
    pub staged: bool,
    /// CR 1.12.3: a card that changes zones becomes a NEW object. The kernel
    /// keeps one [`ObjectId`] per physical card and stamps each existence
    /// with a generation, bumped whenever the card changes zone — so
    /// `(id, generation)` is the object identity the CR talks about, and
    /// "the same card" is the id alone (1.12.6's previous-object relation).
    pub generation: u32,
}

impl Object {
    pub fn counter(&self, kind: CounterKind) -> u32 {
        *self.counters.get(&kind).unwrap_or(&0)
    }
}

/// CR 1.14.4-1.14.5: "your" cards are the ones you control; abilities that
/// move/spend counters or credits act through the controller.
pub fn controls(obj: &Object, side: Side) -> bool {
    cite!("rule_controller_object");
    obj.controller == side
}

/// CR 1.8.3 (via 9.1.7): whether a *card* is active.
/// Corp cards: active while rezzed, in the score area, an identity, or a
/// currently-resolving operation. Runner cards: active while installed, an
/// identity, or a currently-resolving event.
pub fn card_active(obj: &Object) -> bool {
    cite!("rule_active_cards");
    if obj.set_aside_for_ability {
        // CR 4.8.3: other abilities cannot interact with set-aside objects.
        return false;
    }
    if obj.staged {
        // CR 8.5.16a / 8.6.7a: not yet installed or active.
        cite!("rule_steps_installing_place");
        cite!("rule_steps_playing_place");
        return false;
    }
    if obj.hosted_not_installed {
        // CR 1.13.2a / 4.6.5h: hosted without being installed — in the play
        // area, but "not installed and thus not active".
        cite!("rule_host_without_install");
        cite!("rule_play_area_not_installed_hosted");
        return false;
    }
    match obj.zone {
        Zone::PlayArea(_) => true, // identity or resolving event/operation
        Zone::ScoreArea(_) => true,
        Zone::Rig => true,
        Zone::Root(_) | Zone::Ice(_) => match obj.printed.side {
            Side::Corp => obj.faceup,
            Side::Runner => true,
        },
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Characteristics pipeline (9.12.1)
// ---------------------------------------------------------------------------

/// One modification an active static ability or lingering effect makes to an
/// object's characteristics. The selector is resolved before application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharOp {
    /// 9.12.1a first stage: set a value.
    SetStrength(i32),
    /// 9.12.1a second stage.
    IncreaseStrength(i32),
    /// 9.12.1a third stage.
    DecreaseStrength(i32),
    /// 9.12.1b add/remove by counting.
    AddSubtype(&'static str),
    RemoveSubtype(&'static str),
    /// 9.1.9: gain/lose abilities. Losing all abilities is the Hush pattern.
    RemoveAllAbilities,
}

/// A characteristic-affecting effect gathered from the board.
#[derive(Debug, Clone)]
pub struct CharEffect {
    /// Object whose ability/lingering effect produces this modification.
    pub source: ObjectId,
    /// Object being modified.
    pub target: ObjectId,
    pub op: CharOp,
}

/// The computed effective characteristics of one object.
#[derive(Debug, Clone, Default)]
pub struct Effective {
    pub strength: Option<i32>,
    pub subtypes: BTreeSet<&'static str>,
    /// Indexes into `printed.abilities` that are present (9.1.9a: a lost
    /// ability is completely ignored).
    pub ability_present: Vec<bool>,
}

/// CR 9.12.1d/e: dependency-ordered application of characteristic effects.
///
/// Model: effect B **depends on** effect A when A could change whether B is
/// active or what it does — in this kernel wave, concretely: A removes the
/// abilities of B's source (`RemoveAllAbilities`). Apply independent effects
/// first; skip effects whose source ability was removed by an earlier
/// application; on a loop, effects from hosted objects ignore their
/// dependence on their host's effects (Hush/Magnet, 9.12.1e).
pub fn compute_effective(
    objects: &std::collections::BTreeMap<ObjectId, Object>,
    effects: &[CharEffect],
    target: ObjectId,
) -> Effective {
    cite!("rule_dependent_effects");
    cite!("rule_independent_effects");
    let obj = &objects[&target];
    let mut eff = Effective {
        strength: obj.printed.strength,
        subtypes: obj.printed.subtypes.iter().copied().collect(),
        ability_present: vec![true; obj.printed.abilities.len()],
    };

    // Resolve application order over ALL effects (they interact globally).
    let mut remaining: Vec<usize> = (0..effects.len()).collect();
    let mut applied: Vec<usize> = Vec::new();
    // Sources whose abilities have been removed by an applied effect.
    let mut abilities_removed: BTreeSet<ObjectId> = BTreeSet::new();

    while !remaining.is_empty() {
        // An effect depends on another remaining effect that would remove its
        // source's abilities.
        let dep_of = |i: usize, pool: &[usize], ignore_host_dep: bool| -> bool {
            let e = &effects[i];
            pool.iter().any(|&j| {
                if j == i {
                    return false;
                }
                let other = &effects[j];
                if other.op == CharOp::RemoveAllAbilities && other.target == e.source {
                    if ignore_host_dep {
                        // 9.12.1e loop-breaker: treat effects from hosted
                        // objects as not depending on their host's effects.
                        let src = &objects[&e.source];
                        if src.host == Some(other.source) {
                            return false;
                        }
                    }
                    return true;
                }
                false
            })
        };

        let next = remaining
            .iter()
            .position(|&i| !dep_of(i, &remaining, false))
            .or_else(|| {
                // Dependency loop: apply the hosted-beats-host rule.
                remaining.iter().position(|&i| !dep_of(i, &remaining, true))
            });

        let Some(pos) = next else { break };
        let idx = remaining.remove(pos);
        let e = &effects[idx];
        // 9.12.1d: do not apply an effect whose source ability is gone.
        if abilities_removed.contains(&e.source) {
            continue;
        }
        if e.op == CharOp::RemoveAllAbilities {
            abilities_removed.insert(e.target);
        }
        applied.push(idx);
    }

    // 9.12.1a: value stacking set → increase → decrease, over applied effects
    // that touch our target. 9.12.1b: count-based subtypes.
    let on_target: Vec<&CharEffect> = applied
        .iter()
        .map(|&i| &effects[i])
        .filter(|e| e.target == target)
        .collect();

    cite!("rule_modify_value");
    for e in &on_target {
        if let CharOp::SetStrength(v) = e.op {
            eff.strength = Some(v);
        }
    }
    for e in &on_target {
        if let CharOp::IncreaseStrength(v) = e.op {
            eff.strength = Some(eff.strength.unwrap_or(0) + v);
        }
    }
    for e in &on_target {
        if let CharOp::DecreaseStrength(v) = e.op {
            eff.strength = Some(eff.strength.unwrap_or(0) - v);
        }
    }

    cite!("rule_modify_subtypes");
    let mut adds: std::collections::BTreeMap<&'static str, i32> = std::collections::BTreeMap::new();
    for s in &obj.printed.subtypes {
        *adds.entry(*s).or_insert(0) += 1;
    }
    for e in &on_target {
        match e.op {
            CharOp::AddSubtype(s) => *adds.entry(s).or_insert(0) += 1,
            CharOp::RemoveSubtype(s) => *adds.entry(s).or_insert(0) -= 1,
            _ => {}
        }
    }
    eff.subtypes = adds
        .iter()
        .filter(|(_, &n)| n > 0)
        .map(|(&s, _)| s)
        .collect();

    cite!("rule_lose_ability");
    if abilities_removed.contains(&target) {
        eff.ability_present = vec![false; obj.printed.abilities.len()];
    }

    eff
}
