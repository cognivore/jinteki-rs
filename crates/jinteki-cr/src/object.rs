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
/// assigns an order to the ice protecting that server. 4.6.9a/b: the ice
/// protecting a server is ORDERED, innermost first, and 4.6.9e says a change
/// to the ice between a piece and its server changes that piece's position —
/// which positions-as-elements gives for free.
///
/// CR 6.2.6 is why this is a struct and not an index: the Runner's current
/// position "is a specific element of the sequence of positions, not an index
/// into that sequence", so a position added or removed outward (6.2.6a) or
/// inward (6.2.6b) of theirs cannot move them.
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
    /// CR 4.6: the Runner's rig (installed Runner cards). 4.6.5c: they have
    /// no specific location in the play area.
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
    /// CR 1.5.4a: the pile of additional identity cards a player brought
    /// "along with their deck", kept OUTSIDE the game.
    ///
    /// Distinct from [`Zone::RemovedFromGame`], and the distinction is the
    /// whole point: 4.9.5 says a removed card "cannot move out of the
    /// removed-from-game zone or otherwise be interacted with", while 1.5.4b
    /// says a card here is exactly what an ability naming another identity
    /// refers to — and that an identity leaving the play area comes BACK
    /// here. Gone for good versus available.
    ///
    /// A card here is inactive (`card_active` has no arm for it), which is
    /// what keeps a pile identity's abilities silent until 1.8.3d/3.1.1 puts
    /// it in the play area.
    OutsideGame(Side),
}

impl Zone {
    /// CR 4.1.4/4.1.5/4.1.6: a zone is PUBLIC (cards freely visible to both
    /// players unless facedown), HIDDEN (visible to neither) or SECRET
    /// (visible only to their controller). That three-way split is what
    /// `Vm::identity_visible_to` reads off the zone, and 4.1.1a's eight zone
    /// types are the variants of this enum — 4.1.1b: each player has their own
    /// deck, hand, discard pile and score area, and the rest are shared.
    /// 4.1.1c: a card is in exactly one zone, which is why `Object::zone` is
    /// one field; 4.1.2b: a move is instantaneous, so `Vm::move_card` never
    /// leaves a card in two places.
    pub fn visibility_class(self) -> &'static str {
        cite!("rule_zone");
        cite!("rule_zone_types");
        cite!("rule_player_zones");
        cite!("rule_card_in_one_zone");
        cite!("rule_move_between_zones");
        cite!("rule_move_instantaneous");
        cite!("rule_location");
        cite!("rule_deck_location");
        cite!("rule_play_area_location");
        cite!("rule_hosted_object_location");
        cite!("rule_default_location");
        match self {
            Zone::Deck(_) => {
                cite!("rule_hidden_zone");
                "hidden"
            }
            Zone::Hand(_) => {
                cite!("rule_secret_zone");
                "secret"
            }
            _ => {
                cite!("rule_public_zone");
                "public"
            }
        }
    }

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
            // 1.5.4a: one pile per player, and moving between a pile and
            // anywhere else is a zone change (1.12.3 re-makes the object).
            Zone::OutsideGame(Side::Corp) => 12,
            Zone::OutsideGame(Side::Runner) => 13,
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
    /// CR 1.9.5f: advancement counters, used mainly on installed agendas to
    /// track the Corp's progress toward the advancement requirement (§1.18).
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

/// CR 1.12.1 / 1.15.1: ONE counter, as an object that can be targeted.
///
/// A counter's identity here is derived from where it sits — its host, its
/// kind, and its ordinal among that host's counters of that kind — rather
/// than stored, because `Object::counters` is a count per kind. That is exact
/// for addressing counters at the moment an instruction announces them
/// (1.15.2), which is what 1.15.1 needs; it is NOT full 1.12.1 identity, since
/// a counter that moves between cards gets a new `CounterRef`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CounterRef {
    pub host: ObjectId,
    pub kind: CounterKind,
    /// 0-based among the host's counters of this kind.
    pub index: u32,
}

/// The printed (immutable) face of a card — the base of the characteristics
/// pipeline (9.12.1d starts "begin with each object's printed characteristics").
#[derive(Debug, Clone)]
pub struct PrintedCard {
    pub name: &'static str,
    pub side: Side,
    pub card_type: CardType,
    /// CR 2.13.1/2.13.3: the card's faction, exactly as printed. `None` is
    /// "this card does not say" — a testkit shape that has no faction to
    /// declare — and is NOT the same as the neutral faction, which 2.13.2
    /// gives a printed identity of its own ("a white background and no
    /// logo"). Deck construction (1.4.5) reads factions before the game
    /// begins; a faction is a runtime characteristic too, because 1.5.4b's
    /// identity references are stated in terms of it.
    pub faction: Option<&'static str>,
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
    /// CR 1.16.10: "As an additional cost to score this agenda, …" (Azef
    /// Protocol class). 1.16.10c: an effect that normally has no cost gains
    /// one, and the checkpoint after paying it resolves BEFORE the effect.
    pub additional_score_cost: Option<crate::ability::Cost>,
    /// CR 1.16.2c: this card's printed play/install/rez cost is X, and the
    /// quantity is the restriction the card states on the value the payer
    /// may announce ("X must be equal to or less than …").
    pub cost_x: Option<crate::instr::Quantity>,
    /// CR 1.16.10: "As an additional cost to play this operation/event, …"
    /// (24/7 News Cycle class). 1.16.10b combines it with the printed play
    /// cost into ONE payment at step 8.6.7b.
    pub additional_play_cost: Option<crate::ability::Cost>,
    /// CR 1.6.6-adjacent: identities that change the starting hand size
    /// ("You draw a starting hand of 9 cards." — Andromeda class). None = 5.
    pub starting_hand_size: Option<u32>,
    /// CR 1.4 double-sided identities: the back face's printed
    /// characteristics ("flip this identity" — Nebula/Gemilang class).
    pub flip_face: Option<Box<PrintedCard>>,
    /// CR 1.10.3c: "credits hosted on cards may only be spent as the card's
    /// ability allows" — so what the card allows is the content, not a
    /// yes/no (Fencer Fueno allows anything, Miss Bones allows one class of
    /// payment). `None` is a card whose hosted credits cannot be spent at
    /// all. Drives bid legality (10.14.3) as well as ordinary payments.
    pub hosted_credits_spendable: Option<crate::instr::CreditUse>,
    pub abilities: Vec<AbilityDef>,
}

impl PrintedCard {
    pub fn vanilla(name: &'static str, side: Side, card_type: CardType) -> Self {
        PrintedCard {
            name,
            side,
            card_type,
            faction: None,
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
            additional_score_cost: None,
            cost_x: None,
            additional_play_cost: None,
            starting_hand_size: None,
            flip_face: None,
            hosted_credits_spendable: None,
            abilities: Vec::new(),
        }
    }

    /// CR 10.13.1: the **dividends** keyword. "Dividends N" IS a conditional
    /// ability — "When you score this agenda, place N agenda counters on it
    /// for each hosted advancement counter past its advancement requirement"
    /// — so the keyword is expanded into that ability here, through the
    /// public vocabulary, exactly as the §9.11 transcriber will.
    ///
    /// CR 10.13.2: the requirement and the counters are read as of the moment
    /// the agenda began to be scored; both selectors resolve against the
    /// 1.17.8 last-known snapshot once the agenda has moved.
    pub fn with_dividends(mut self, n: i64) -> Self {
        cite!("rule_dividends");
        cite!("rule_dividends_timing");
        use crate::instr::{Instruction, Quantity, TargetSpec};
        self.abilities.push(
            crate::ability::AbilityDef::conditional(
                crate::ability::TriggerCond::SelfScored { requires: Vec::new() },
                vec![Instruction::PlaceCounters {
                    target: TargetSpec::SelfSource,
                    kind: CounterKind::Agenda,
                    amount: Quantity::Times(
                        n,
                        Box::new(Quantity::Minus(
                            Box::new(Quantity::CountersOnSource(CounterKind::Advancement)),
                            Box::new(Quantity::RequirementOfSource),
                        )),
                    ),
                }],
                false,
            )
            .labeled("dividends: agenda counters per excess advancement"),
        );
        self
    }
}

/// A game object: a card (or card-as-counter) with position and state.
#[derive(Debug, Clone)]
pub struct Object {
    pub id: ObjectId,
    pub printed: PrintedCard,
    /// CR double-sided identities: which face is up. Only ever true for an
    /// identity with a `flip_face`.
    pub flipped: bool,
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
    /// CR 10.9.1: the counter kinds this card has been LOADED with. A card is
    /// EMPTY when it no longer hosts any counters of a kind previously loaded
    /// onto it — so a card that was never loaded can never become empty
    /// (10.9.2), and counters of another kind placed on it without loading do
    /// not meet an "empty" condition when they go (10.9.3).
    pub loaded_kinds: std::collections::BTreeSet<CounterKind>,
    /// CR 4.8.3: where this card was before it was set aside. A move OUT of
    /// the set-aside zone is reported as though the card came from here, so
    /// an ability that does not refer to the set-aside zone (which is every
    /// ability but the one that did the setting aside) sees the move it would
    /// have seen without it.
    pub set_aside_from: Option<Zone>,
    /// CR 8.5.16a / 8.6.7a: placed into the play area as the first step of
    /// installing/playing — "It is not yet installed or active."
    pub staged: bool,
    /// CR 1.12.3: a card that changes zones becomes a NEW object. The kernel
    /// keeps one [`ObjectId`] per physical card and stamps each existence
    /// with a generation, bumped whenever the card changes zone — so
    /// `(id, generation)` is the object identity the CR talks about, and
    /// "the same card" is the id alone (1.12.6's previous-object relation).
    pub generation: u32,
    /// CR 1.17.8 / 10.13.2: an agenda that has been scored or stolen keeps no
    /// advancement counters (1.17.5), and any declaration modifying its
    /// advancement requirement has stopped applying — so an ability that met
    /// its condition FROM the scoring reads the last known values, captured
    /// here as `(advancement counters, advancement requirement)` just before
    /// the agenda moved.
    pub scored_snapshot: Option<(u32, u32)>,
    /// CR 4.6.6i: "the server associated with the previous location of the
    /// card" — stamped whenever the card leaves a server, its root, or a
    /// position protecting it. An ability on a card that has left a server
    /// still says "this server" about the one it left (see `Vm::this_server`).
    pub last_server: Option<ServerId>,
    /// CR 4.8.7 / 1.21.1b: the facedown set-aside GROUP this card belongs to.
    /// Facedown cards set aside at the same time by the same effect "must be
    /// kept in distinct groups according to the effect that sets them aside",
    /// and within a group they are not ordered — so the group is the unit of
    /// information (10.2) and the player carrying the effect out is the one
    /// who may look at them (8.3.3a).
    pub set_aside_group: Option<crate::view::SetAsideGroup>,
    /// CR 10.1.3: this card was added to a score area **as an agenda**, and
    /// this is the agenda point value the converting effect specified. "The
    /// card loses all its previous properties and gains only those properties
    /// specified in the effect converting it", so while this is set the card
    /// is an agenda worth exactly this many points and nothing else about its
    /// printed face applies. The conversion "lasts until the card moves to a
    /// zone that is not a score area", which is where `Vm::move_card` clears
    /// it.
    pub converted_agenda: Option<i32>,
}

impl IcePosition {
    /// CR 6.2.1 / 4.6.9a-e: the position's identity, which is what the
    /// Runner's position names (6.2.6) and what a swap re-occupies (6.2.2f).
    pub fn id(&self) -> u64 {
        cite!("rule_position");
        cite!("rule_ice_ordered");
        cite!("rule_innermost_ice");
        cite!("rule_ice_move");
        cite!("rule_ice_change_can_affect_position");
        cite!("rule_ice_change_outward");
        cite!("rule_ice_change_inward");
        self.id
    }
}

impl CounterKind {
    /// A **citation anchor**: CR 1.9's counter types, which this enum and
    /// `PlayerState` between them carry.
    ///
    /// 1.9.1: counters are game pieces tracking resources, effects and
    /// statuses; 1.9.1a: "counter" and "token" are interchangeable. 1.9.2a:
    /// counters placed on a card without a designated source come from the
    /// bank. 1.9.5a-i: credit (the pools), click (`PlayerState::clicks`), tag
    /// (`tags`), bad publicity (`bad_publicity`), core damage
    /// (`core_damage`), advancement, virus, power and agenda counters.
    /// 1.9.5j's CONDITION counters — counters with rules text — are the one
    /// type the kernel has no representation for.
    pub fn types() {
        cite!("rule_counters_cards");
        cite!("rule_counter_token");
        cite!("rule_bank_default");
        cite!("rule_type_credit_counter");
        cite!("rule_type_click_counter");
        cite!("rule_type_tag_counter");
        cite!("rule_type_bad_pub_counter");
        cite!("rule_type_core_damage_counter");
        cite!("rule_type_advancement_counter");
        cite!("rule_type_virus_counter");
        cite!("rule_type_power_counter");
        cite!("rule_type_agenda_counter");
    }
}

impl Object {
    /// CR rule_identity_double_sided: the face currently showing. Everything
    /// that reads printed characteristics of an ACTIVE card goes through
    /// here, so flipping an identity swaps its abilities, name and subtypes.
    pub fn face(&self) -> &PrintedCard {
        if self.flipped {
            if let Some(back) = &self.printed.flip_face {
                return back;
            }
        }
        &self.printed
    }

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
    // 4.6.4: whether a card in the play area is active is decided by its
    // status — 4.6.4a identities always are, 4.6.4b Corp cards are installed
    // unrezzed and so inactive until rezzed, 4.6.4c Runner cards are installed
    // faceup and active, 4.6.4d some are installed facedown instead, and
    // 4.6.4e a played operation or event is active for its resolution.
    cite!("rule_play_area_active_inactive");
    cite!("rule_play_area_identity");
    cite!("rule_play_area_corp_cards");
    cite!("rule_play_area_runner_cards");
    cite!("rule_play_area_faceup_facedown");
    cite!("rule_play_area_operations_events");
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
        // CR 4.5.4: "Agendas in the Corp's score area are active. Agendas in
        // the Runner's score area are inactive unless stated otherwise." The
        // "unless stated otherwise" half is 9.1.8b and lives in
        // `ability_active`, where every other 9.1.8 exception does.
        Zone::ScoreArea(s) => {
            cite!("rule_score_area_active_inactive");
            s == Side::Corp
        }
        // 8.1.4a: "installed Runner cards that are facedown do not have any
        // characteristics … and their abilities are not active."
        Zone::Rig => {
            cite!("rule_facedown_runner_cards_are_blank");
            obj.faceup
        }
        Zone::Root(_) | Zone::Ice(_) => match obj.printed.side {
            Side::Corp => obj.faceup,
            Side::Runner => true,
        },
        // CR 1.5.4a / 1.8.3d: an identity in the pile is outside the game.
        // Identities "become active when the game begins" and 3.1.1 puts the
        // single active one in the play area, so a card waiting in the pile
        // is inactive and its abilities are silent — which is what lets a
        // pile hold a Chaos Theory without granting anybody +1[mu].
        Zone::OutsideGame(_) => {
            cite!("rule_additional_identities_pile");
            cite!("rule_identity_become_active");
            false
        }
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
    /// CR 2.5 through the same 9.12.1a stages: an agenda's POINT VALUE is a
    /// characteristic like any other, so "this agenda is worth 1 more agenda
    /// point for each hosted agenda counter" (Project Beale) and "worth 1
    /// additional / 1 fewer agenda point while in the Runner's score area"
    /// (Merger, Global Food Initiative) are increases and decreases applied
    /// after any effect setting the value.
    IncreaseAgendaPoints(i32),
    DecreaseAgendaPoints(i32),
    /// 9.12.1b add/remove by counting.
    AddSubtype(&'static str),
    RemoveSubtype(&'static str),
    /// "…gains the subtypes of <that card>" (Mother Goddess class): add one
    /// instance of every subtype the named object has *after* its own
    /// characteristics are computed. This is the 9.12.1d dependency in
    /// person — the copied-from object's effective subtypes are what is
    /// copied, so an effect changing them is applied first by construction.
    CopySubtypesFrom(ObjectId),
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
    /// CR 2.5 / 1.17.1: the agenda point value, after modification. `None`
    /// for a card that prints none — 2.5.1: "the agenda point value appears
    /// only on agendas".
    pub agenda_points: Option<i32>,
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
    let mut visiting = BTreeSet::new();
    compute_effective_inner(objects, effects, target, &mut visiting)
}

/// `visiting` carries the chain of objects whose characteristics are already
/// being computed further up the call stack, so a `CopySubtypesFrom` cycle
/// (two cards each gaining the other's subtypes) terminates: the re-entered
/// object contributes its printed subtypes only.
fn compute_effective_inner(
    objects: &std::collections::BTreeMap<ObjectId, Object>,
    effects: &[CharEffect],
    target: ObjectId,
    visiting: &mut BTreeSet<ObjectId>,
) -> Effective {
    cite!("rule_dependent_effects");
    cite!("rule_independent_effects");
    let obj = &objects[&target];
    let mut eff = Effective {
        strength: obj.printed.strength,
        // 10.1.3: a card converted into an agenda in a score area "loses all
        // its previous properties and gains only those properties specified
        // in the effect converting it", so the specified point value IS the
        // printed value for the duration of the conversion.
        agenda_points: obj.converted_agenda.or(obj.printed.agenda_points),
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
    // CR 2.5 / 2.5.2: the agenda point value runs through the same 9.12.1a
    // stages. Only a card that has one at all can have it modified.
    cite!("rule_agenda_points_location");
    cite!("rule_agenda_points_citation");
    for e in &on_target {
        match e.op {
            CharOp::IncreaseAgendaPoints(v) => {
                eff.agenda_points = Some(eff.agenda_points.unwrap_or(0) + v);
            }
            CharOp::DecreaseAgendaPoints(v) => {
                eff.agenda_points = Some(eff.agenda_points.unwrap_or(0) - v);
            }
            _ => {}
        }
    }

    cite!("rule_modify_subtypes");
    let mut adds: std::collections::BTreeMap<&'static str, i32> = std::collections::BTreeMap::new();
    for s in &obj.printed.subtypes {
        *adds.entry(*s).or_insert(0) += 1;
    }
    visiting.insert(target);
    for e in &on_target {
        match e.op {
            CharOp::AddSubtype(s) => *adds.entry(s).or_insert(0) += 1,
            CharOp::RemoveSubtype(s) => *adds.entry(s).or_insert(0) -= 1,
            CharOp::CopySubtypesFrom(from) => {
                if !objects.contains_key(&from) {
                    continue;
                }
                let copied: BTreeSet<&'static str> = if visiting.contains(&from) {
                    objects[&from].printed.subtypes.iter().copied().collect()
                } else {
                    compute_effective_inner(objects, effects, from, visiting).subtypes
                };
                for s in copied {
                    *adds.entry(s).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }
    visiting.remove(&target);
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
