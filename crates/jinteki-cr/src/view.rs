//! CR §10.2: information — what each player is *entitled to know*.
//!
//! The kernel has always held one state. §10.2 says that state is not one
//! thing to the two players: some of it is **open information** (10.2.3a —
//! available to both) and some is **hidden information** (10.2.2a — available
//! to one or neither), and a player "cannot learn hidden information without
//! the aid of a game effect, rule, or another player verbally communicating
//! the information" (10.2.2b).
//!
//! Two pieces express that here:
//!
//! * [`CardView`] — one card as one player sees it. `Seen` names the object
//!   whose front face that player is entitled to; `Unseen` is a card whose
//!   PRESENCE is open (10.2.3a: "the number of cards in HQ, R&D, the stack,
//!   and the grip") while its identity is not. Two `Unseen` entries compare
//!   EQUAL — which is exactly what "the Runner cannot tell which" means.
//! * [`Sightings`] — 10.2.2b's record of the hidden information each player
//!   HAS been shown by a game effect (1.21.2 looking, 1.21.3 revealing,
//!   1.21.4 exposing). Everything else is derived from the state, so nothing
//!   here can drift from it.
//!
//! Verbal communication (10.2.2b's third channel) is deliberately outside the
//! kernel: there is no instruction, decision or record by which a player
//! asserts something to their opponent, so a claim changes neither the game
//! state nor either view — which is the mechanical content of "that player is
//! not required to tell the truth".

use crate::object::{ObjectId, ServerId, Side, Zone};

/// CR 10.2.1: how one piece of information is classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Info {
    /// CR 10.2.3a: available to both players.
    Open,
    /// CR 10.2.2a: unavailable to one or more players.
    Hidden,
}

/// CR 10.2.2a / 10.2.3a: one card as one player sees it.
///
/// The distinction this type draws is between a card's IDENTITY (its front
/// face) and its PRESENCE. An unrezzed installed card is an object both
/// players can point at and target, and its presence is open information;
/// only its controller knows what it is. So the presence is the entry, and
/// the identity is whether the entry names an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CardView {
    /// This player is entitled to this card's front face.
    Seen(ObjectId),
    /// A card is here; which card it is, is hidden information (10.2.2a).
    Unseen,
}

impl CardView {
    pub fn seen(self) -> Option<ObjectId> {
        match self {
            CardView::Seen(o) => Some(o),
            CardView::Unseen => None,
        }
    }
    pub fn is_seen(self) -> bool {
        matches!(self, CardView::Seen(_))
    }
    /// CR 10.2.1: this entry's classification for this player.
    pub fn info(self) -> Info {
        match self {
            CardView::Seen(_) => Info::Open,
            CardView::Unseen => Info::Hidden,
        }
    }
}

/// CR 10.2.2b: the hidden information each player has been SHOWN — the only
/// way a player learns it, short of a rule that makes it open.
///
/// A sighting lapses when the card moves (1.21.6: "each such card remains
/// visible to the relevant player(s) until the entire ability is finished
/// resolving **or the card moves to a different location**"). Everything a
/// player is *continuously* entitled to — their own hand, their own facedown
/// cards, faceup cards anywhere — is derived from the state instead, so it
/// needs no record and cannot go stale.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sightings {
    corp: std::collections::BTreeSet<ObjectId>,
    runner: std::collections::BTreeSet<ObjectId>,
}

impl Sightings {
    fn set(&mut self, side: Side) -> &mut std::collections::BTreeSet<ObjectId> {
        match side {
            Side::Corp => &mut self.corp,
            Side::Runner => &mut self.runner,
        }
    }
    /// CR 1.21.2: this player has been allowed to see this card's front face.
    pub fn show(&mut self, side: Side, obj: ObjectId) {
        cite!("rule_look");
        self.set(side).insert(obj);
    }
    /// CR 1.21.3: revealing shows the front face to ALL players.
    pub fn show_all(&mut self, obj: ObjectId) {
        cite!("rule_reveal");
        self.corp.insert(obj);
        self.runner.insert(obj);
    }
    /// CR 1.21.6: the card moved, so the sighting lapses for everyone.
    pub fn forget(&mut self, obj: ObjectId) {
        cite!("rule_remain_visible");
        self.corp.remove(&obj);
        self.runner.remove(&obj);
    }
    pub fn shown(&self, side: Side, obj: ObjectId) -> bool {
        match side {
            Side::Corp => self.corp.contains(&obj),
            Side::Runner => self.runner.contains(&obj),
        }
    }
}

/// CR 4.8.7 / 1.21.1b: a group of facedown cards set aside at the same time by
/// the same effect. "Cards within such a group are not ordered and can be
/// freely arranged by their controller" — so the group, not the card, is the
/// unit of information, and the ORDER inside it is not information at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAsideGroup {
    /// Distinct per effect that set cards aside (4.8.7's "distinct groups
    /// according to the effect that sets them aside").
    pub id: u64,
    /// The player carrying the effect out — 8.3.3a's "that opponent cannot
    /// look at the set-aside cards during this process".
    pub by: Side,
    /// CR 8.4.2a: this group is a DRAWN set. Abilities whose trigger
    /// condition refers to cards being drawn can see it in the set-aside
    /// zone, which is an explicit exception to 4.8.3.
    pub drawn: bool,
}

/// One player's view of the game state: everything they are entitled to know.
///
/// Built by `Vm::view_of`; every field is derived, so a view is a snapshot and
/// never a second source of truth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub side: Side,
    /// Per zone, one entry per card in it, in the order that order is itself
    /// information (4.2.3 decks are ordered; 4.3.3 hands and 4.4.2 discard
    /// piles are not, and every entry a player may not see is `Unseen`, so an
    /// unordered zone carries nothing either way).
    pub zones: Vec<(Zone, Vec<CardView>)>,
    /// CR 4.8.7: the facedown set-aside groups, each as this player sees it.
    pub groups: Vec<(u64, Vec<CardView>)>,
    /// CR 9.10.3 / 10.2.3b: a maintained choice was announced (1.15.2) and is
    /// continuously available to both players — open information that "cannot
    /// be hidden from an opponent", whatever the chosen card's own identity.
    pub choices: Vec<(&'static str, ObjectId)>,
    /// CR 10.2.3a: "the number of credits in a credit pool" is open.
    pub credits: Vec<(Side, u32)>,
}

impl View {
    /// The cards in this zone, as this player sees them.
    pub fn in_zone(&self, z: Zone) -> &[CardView] {
        self.zones
            .iter()
            .find(|(zz, _)| *zz == z)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }
    /// CR 10.2.3a: the number of cards in a zone is open information, so this
    /// answers for either player and for any zone.
    pub fn count_in(&self, z: Zone) -> usize {
        self.in_zone(z).len()
    }
    /// Is this card's identity available to this player?
    pub fn sees(&self, obj: ObjectId) -> bool {
        self.zones
            .iter()
            .flat_map(|(_, v)| v.iter())
            .chain(self.groups.iter().flat_map(|(_, v)| v.iter()))
            .any(|c| *c == CardView::Seen(obj))
    }
    /// CR 4.8.7: the nth facedown set-aside group, as this player sees it.
    pub fn group(&self, n: usize) -> &[CardView] {
        self.groups.get(n).map(|(_, v)| v.as_slice()).unwrap_or(&[])
    }
    /// CR 10.2.3b: what this player has been told a maintained choice is.
    pub fn choice(&self, key: &str) -> Option<ObjectId> {
        self.choices.iter().find(|(k, _)| *k == key).map(|(_, o)| *o)
    }
    pub fn credits_of(&self, side: Side) -> u32 {
        self.credits.iter().find(|(s, _)| *s == side).map(|(_, c)| *c).unwrap_or(0)
    }
}

/// The zones a view enumerates, in a stable order.
pub fn viewable_zones(remotes: &[ServerId]) -> Vec<Zone> {
    let mut z = vec![
        Zone::Deck(Side::Corp),
        Zone::Deck(Side::Runner),
        Zone::Hand(Side::Corp),
        Zone::Hand(Side::Runner),
        Zone::Discard(Side::Corp),
        Zone::Discard(Side::Runner),
        Zone::ScoreArea(Side::Corp),
        Zone::ScoreArea(Side::Runner),
        Zone::Rig,
        Zone::PlayArea(Side::Corp),
        Zone::PlayArea(Side::Runner),
        Zone::SetAside,
        Zone::RemovedFromGame,
    ];
    for s in [ServerId::Hq, ServerId::Rnd, ServerId::Archives] {
        z.push(Zone::Root(s));
        z.push(Zone::Ice(s));
    }
    for s in remotes {
        z.push(Zone::Root(*s));
        z.push(Zone::Ice(*s));
    }
    z
}
