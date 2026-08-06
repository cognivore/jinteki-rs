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
    /// CR 1.10.3a: credits entered a credit pool from the bank. `source` is
    /// the ability's SOURCE where an ability did it (9.1.4), and `None` where
    /// nothing on a card did — 5.2.6b's basic credit action, which no card
    /// can be said to have caused. A sentence naming what the credits came
    /// "through" (The Zwicky Group) is asked of this field.
    CreditsGained { side: Side, amount: u32, source: Option<ObjectId> },
    /// CR 1.10.3b: credits were forced out of a credit pool and back to the
    /// bank. `source` is the same field [`GameChange::CreditsGained`] carries
    /// and reads the same way (9.1.4): the ability's SOURCE where an ability
    /// caused the loss, `None` where nothing on a card did. A sentence asking
    /// what CAUSED the Runner to lose credits (GameNET) is asked of it.
    CreditsLost { side: Side, amount: u32, source: Option<ObjectId> },
    ClicksGained { side: Side, amount: u32 },
    ClickSpent { side: Side },
    /// CR 5.2.5: a player took an action. The identity is what 5.2.5a/b say
    /// makes two actions "the same" or "different".
    ActionTaken { side: Side, action: ActionIdentity },
    /// CR 5.2.2a/d: the action is COMPLETE — the game may advance past the
    /// action step that ran it, which 5.2.2a makes the definition and 5.2.2d
    /// makes the moment an "action finishing" condition is met. Distinct
    /// from `ActionTaken`, which is 5.2.2's INITIATION: everything an action
    /// does happens between the two.
    ///
    /// `action` is the same 5.2.5a/b identity `ActionTaken` records, carried
    /// here as well because a sentence naming the action that finished ("an
    /// action on an **expendable** card", Nuvem SA) is answered at the
    /// checkpoint scan, by which time 5.2.2a's action step is over and the
    /// state no longer says which action ran in it. It is the same reason
    /// [`GameChange::CardRezzed`] carries the card type.
    ActionCompleted { side: Side, action: ActionIdentity },
    ClicksLost { side: Side, amount: u32 },
    /// CR 8.4: a card was drawn. `source` is the ability that said to (9.1.3),
    /// and `None` is a draw the RULES required — 5.6.2b's mandatory draw and
    /// the basic draw action, which no card can be said to have caused. It is
    /// the same field [`GameChange::CreditsGained`] carries, and the log tells
    /// "Corp: Predictive Planogram — draws X." from "Corp: draws X." with it.
    CardDrawn { side: Side, obj: ObjectId, source: Option<ObjectId> },
    /// One record per point batch: `cards` are the simultaneous random trashes.
    ///
    /// `responsible` is 10.4.1's responsible player, a fact about the
    /// OCCURRENCE: a Corp card that "does" damage makes the Corp responsible,
    /// and one that directs or allows the Runner to "suffer" damage makes it
    /// the Runner. It is recorded here rather than asked of anything later
    /// because nothing later could answer it — the source's printed wording
    /// decided it at the moment the damage resolved.
    DamageSuffered { kind: DamageKind, amount: u32, cards: Vec<ObjectId>, responsible: Side },
    /// `had` is the number of tags the Runner had BEFORE this taking — a fact
    /// about the occurrence, recorded because a condition's "if you had no
    /// tags" (Sebastião Souza Pessoa) is read at the checkpoint scan, by
    /// which time the pool already counts the tags this very record added.
    /// Reading current state there would make the requirement false at every
    /// occurrence that met it.
    TagsTaken { amount: u32, had: u32 },
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
    ///
    /// `while_accessed` is CR 7.1.2's "a card you are accessing": the card
    /// trashed WAS the card being accessed at the moment it was trashed. It
    /// is recorded here rather than asked of the state later because 9.6.5c's
    /// ordinal is answered of the PAST — "the first time each turn you trash a
    /// card you are accessing" counts earlier trashes as they stood then, and
    /// the access they happened inside is long over by the time the count is
    /// taken.
    ///
    /// `was_rezzed` is CR 8.1.2's "a rezzed card" — the card was a faceup
    /// installed Corp card at the moment of the trash ("when you trash a
    /// **rezzed** card", Ob Superheavy Logistics). A fact of the same kind as
    /// `was_zone`, recorded for the same reason: the card has moved (and a
    /// Corp trash has turned it facedown, 10.3.1a) by the time any condition
    /// is scanned, so the state can no longer answer it.
    ///
    /// `during_install` is CR 8.5.11a's like-card trash — the trash the
    /// install procedure itself performs, which "except during installation"
    /// (Ob again) excludes. Recorded as "an installation was in progress at
    /// the moment of the trash", which in this kernel only the 8.5.16a
    /// like-card trash can be: installing is a procedure with no timing
    /// structure (9.2.2e), so no ability-driven trash can resolve mid-install.
    /// If an interrupt ever could, this approximation would read it as
    /// during-install too — noted honestly here rather than hidden.
    CardTrashed {
        obj: ObjectId,
        by: Side,
        was_zone: Zone,
        while_accessed: bool,
        was_rezzed: bool,
        during_install: bool,
    },
    /// A card was DISCARDED — moved to its owner's discard pile by a player
    /// rather than by a trash.
    ///
    /// `to_hand_size` is CR 5.5.4's stipulation: this discard was the discard
    /// step's, the one a player makes "to reach their maximum hand size". It
    /// is recorded here rather than asked of the state afterwards for the same
    /// reason [`GameChange::CardTrashed::while_accessed`] is — it is a fact
    /// about the MOMENT of the discard, and the step is over by the time any
    /// condition is scanned.
    CardDiscarded { obj: ObjectId, side: Side, to_hand_size: bool },
    /// CR 8.5.16f. `from` is the zone the card is treated as having been
    /// installed FROM — 4.8.3 substitutes the pre-set-aside location for a
    /// card installed out of the set-aside zone, which is the only way an
    /// "install a program from your heap" condition can see a searched
    /// install at all.
    ///
    /// `to` is the zone the card was installed INTO, settled by step 8.5.16b's
    /// destination and read here for the same reason
    /// [`GameChange::CardTrashed::was_zone`] is: it is a fact about the MOMENT
    /// of the install, and the card can have moved on by the time any
    /// condition is scanned — which matters most for 9.6.5c's ordinal, whose
    /// scan asks the whole condition of EARLIER changes to decide whether they
    /// were among "the times". A card installed in a remote and trashed later
    /// the same turn still spent that ordinal.
    ///
    /// 1.13.12 puts a hosted card in its host's zone, so a card hosted at
    /// install time records the server 4.6.6b puts the host in.
    ///
    /// `by_ability_of` is the card whose PRINTED ability's resolution
    /// performed this install — one more fact about the MOMENT, recorded for
    /// the same reason as the others: "When you install that card, …"
    /// (Topan class) is a condition met by the install its own card's ability
    /// made, and the frame that made it is popped long before any condition
    /// is scanned. `None` when a rules frame installed the card (5.2.6d /
    /// 5.2.7d's basic action is the player's, not a card's — the frame's
    /// `AbilityRef` convention marks it) and for 8.8.4b's swap, where the
    /// card became installed without any installation being performed.
    CardInstalled { obj: ObjectId, side: Side, from: Zone, to: Zone, by_ability_of: Option<ObjectId> },
    /// CR 1.21.3: a card was REVEALED — shown to all players, then returned
    /// to its previous state. `by` is the player who revealed it, which is
    /// the ability's controller and NOT the card's owner: a sentence reading
    /// "the first time each turn YOU reveal a card" (Hyoubu Institute) is met
    /// by the Corp revealing a card out of the Runner's grip.
    CardRevealed { obj: ObjectId, by: Side },
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
    ///
    /// `by` is the player who played the card — 8.6.2's "the player playing
    /// the card", which is the same player [`GameChange::CardPlayed`] names at
    /// step (d). It is recorded because a sentence saying "whenever **you**
    /// finish resolving an operation" names one, and the card itself cannot
    /// answer it: 8.6.7g has already trashed the operation by the time this
    /// step is reached, and a card's OWNER (1.14.1) is not who played it.
    CardPlayResolved { obj: ObjectId, by: Side },
    CardUninstalled { obj: ObjectId, was_zone: Zone },
    /// CR 8.1.2: a card was rezzed. The card TYPE travels with the record
    /// because "whenever you rez a piece of ice" is answered at the
    /// checkpoint scan, when the object may already have moved on.
    CardRezzed { obj: ObjectId, card_type: crate::object::CardType },
    /// CR 8.1.2 / 1.12.5: a card turned facedown again. It does NOT become a
    /// new object — it never left the play area.
    CardDerezzed { obj: ObjectId },
    CardMoved { obj: ObjectId, from: Zone, to: Zone },
    /// §4.9: an ability's resolution REMOVED cards from the game — the
    /// `RemoveCardsFromGame` instruction's own record, beside the `CardMoved`s
    /// the movements themselves record. `by_ability_of` is the card whose
    /// ability performed the removal, recorded for the reason
    /// [`GameChange::CardInstalled`]'s `by_ability_of` is: "Ignore this
    /// ability if you have already removed a card from the game **with it**
    /// this turn" (Skorpios Defense Systems) is a stipulation about WHOSE
    /// ability removed, and the frame that did it is popped long before the
    /// sentence asks. An announcement that chose nothing records an empty
    /// `objs` — it removed no card, and answers for none.
    CardsRemovedFromGame { objs: Vec<ObjectId>, by_ability_of: Option<ObjectId> },
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
    /// CR 10.2.2a: this side SEALED a choice of identity back face ("secretly
    /// set your identity to any copy of Méliès U"). The record carries no
    /// face on purpose — the change log is open information (10.2.1), and
    /// WHICH copy was chosen is hidden until the flip reveals it, exactly as
    /// a psi bid is sealed until 10.14.6c. What both players are entitled to
    /// is what this record says: the set happened.
    IdentityFaceSecretlySet { side: Side },
    RunBegan { server: ServerId },
    /// CR 6.8.4: the run was declared successful. `subroutine_resolved` is
    /// whether any subroutine had resolved during this run by that moment
    /// (9.8.7) — a fact about the MOMENT of the declaration, recorded here
    /// because 9.6.5c's ordinal re-asks the whole condition of every earlier
    /// change this turn, and by then the run's own history window is closed
    /// ("…a run becomes successful **after a subroutine resolved during that
    /// run**", Ryō "Phoenix" Ōno).
    RunDeclaredSuccessful { server: ServerId, subroutine_resolved: bool },
    RunDeclaredUnsuccessful { server: ServerId },
    /// The run is complete (`step_run_complete`, 6.9.6d).
    RunEnded { server: ServerId, run_id: u64 },
    EncounterBegan { ice: ObjectId, encounter_id: u64 },
    /// CR 6.5.10: the encounter ended. `ice_advanced` is 1.18.2 asked of the
    /// ice at that moment — at least one advancement counter — recorded here
    /// rather than read off the board later, because the counters can move or
    /// leave with the ice before 9.6.5c's ordinal re-asks the condition of
    /// this occurrence ("an encounter with an **advanced** piece of ice
    /// ends", Weyland Consortium: Builder of Nations).
    EncounterEnded { ice: ObjectId, encounter_id: u64, ice_advanced: bool },
    IceApproached { ice: ObjectId },
    /// CR 6.9.4a: the Runner passed a piece of ice. `after_encounter` is
    /// 6.1.3e's "direct sequence" test — whether this pass directly follows
    /// an Encounter Ice Phase WITH THIS ICE — and the other two flags carry
    /// what happened during that encounter, since 6.1.3f scopes "after fully
    /// breaking it" to that encounter and 9.8.9's Persephone class asks
    /// whether any subroutine resolved from this ice. `rezzed` is 8.1's
    /// status of the ice at the moment it was passed — recorded here rather
    /// than read off the board later, because a rez or derez after the pass
    /// would change what 9.6.5c's ordinal sees when it re-asks the condition
    /// of this occurrence ("the Runner passes a **rezzed** piece of bioroid
    /// ice", Haas-Bioroid: Architects of Tomorrow).
    IcePassed {
        ice: ObjectId,
        after_encounter: bool,
        fully_broken: bool,
        subs_resolved: bool,
        rezzed: bool,
    },
    ServerApproached { server: ServerId },
    /// CR 1.16.3: a cost was paid (zero costs are real, 1.16.1d).
    ///
    /// `source` is what the payment was FOR — the source of the ability whose
    /// cost this was (9.1.3), and `None` for a basic action's own cost, which
    /// no card can be said to have caused (`Vm::cause_of` is where that is
    /// decided, since 9.1.1 counts the basic actions and the basic trash
    /// ability among the abilities). It is the same field
    /// [`GameChange::CreditsGained`] and [`GameChange::CreditsLost`] carry,
    /// and a sentence asking what made a player SPEND credits (GameNET) is
    /// asked of it.
    CostPaid {
        side: Side,
        credits: u32,
        clicks: u32,
        trashed: Vec<ObjectId>,
        source: Option<ObjectId>,
    },
    /// CR 9.1.6: an ability/source was used.
    AbilityUsed { source: ObjectId },
    /// CR 9.11.4g: a player chose one of an instruction's options, and this
    /// is WHICH — the option's own printed label, as it was offered.
    ///
    /// The choice is the only part of a modal card that the resulting state
    /// does not say. "Corp: gains 3[c]." is true of Predictive Planogram, of
    /// Hedge Fund and of an identity's trigger alike; nothing downstream can
    /// reconstruct that the Corp was ASKED and answered "Gain 3[credit]."
    /// rather than "Draw 3 cards." So the answer is recorded where it is
    /// made, and the log reads it back — a modal card's decision is a thing
    /// players audit, and 10.2.1 makes it open information.
    ///
    /// `label` is the offered string verbatim rather than an index, because
    /// an index means nothing to a reader and the resolvable-options filter
    /// (9.12.3c) makes indices differ between the offer and the printed list.
    OptionChosen { source: ObjectId, side: Side, label: &'static str },
    /// A trash ability was used. `basic` says WHICH: `true` is 7.1.5's basic
    /// trash ability, where the Runner pays an accessed card's trash cost;
    /// `false` is a card's own printed [trash] ability, whose 1.19.4 trigger
    /// cost trashed its source. They are different abilities and a card that
    /// names one does not name the other (Geist prints the [trash] symbol),
    /// so the record has to say which happened.
    TrashAbilityUsed { source: ObjectId, side: Side, basic: bool },
    BreachBegan { server: ServerId },
    BreachEnded { server: ServerId },
    CardEnteredRoot { obj: ObjectId, server: ServerId },
    /// CR 4.6.8d / 8.5.2a: a REMOTE server came into existence — step
    /// 8.5.16e of an installation put the first card in its root or
    /// protecting it. Only remotes are ever created: 4.6.5's central servers
    /// exist for the whole game, so there is no record for them to make.
    ///
    /// `by` is the player whose installation created it, which is the Corp in
    /// every case the rules allow — recorded anyway, because a sentence
    /// saying "**you** create a remote server" names a player and the record
    /// has to be able to answer it.
    RemoteServerCreated { server: ServerId, by: Side },
    SubroutineResolved { ice: ObjectId, index: usize },
    /// CR 9.12.2d: "all subroutines broken" became satisfied for this
    /// encounter — including vacuously, for ice with zero subroutines, as
    /// soon as step 6.9.3b begins. 6.5.7a: this is the moment the Runner
    /// fully breaks `ice`, and 6.5.7d never retracts it.
    ///
    /// `by` is CR 6.5.7b's second fully-breaker: "if all its subroutines were
    /// broken using abilities on a **single object**, that object also fully
    /// breaks the ice". It is an `Option` because a full break does NOT
    /// always have one, and the rule says so twice:
    ///
    /// * 6.5.7b's condition can simply fail — two breakers sharing the work
    ///   on one piece of ice means all its subroutines were not broken using
    ///   abilities on a single object, and then NO object fully breaks it,
    ///   only the Runner does.
    /// * 6.5.7c settles the vacuous case in as many words: ice with no
    ///   subroutines is fully broken by the Runner when step 6.9.3b begins,
    ///   and "no objects fully break the ice in this case".
    ///
    /// So the Runner always fully breaks (`ice` is always named) and an
    /// object sometimes does. `None` is the honest record of "nobody", not a
    /// missing lookup.
    AllSubsBroken { ice: ObjectId, by: Option<ObjectId> },
    /// CR 9.8.6: ONE subroutine on `ice` was broken. Recorded per
    /// subroutine, so an ability that watches for breaking is met once for
    /// each — Gold Farmer taxes twice when both its subroutines are broken.
    /// `printed` distinguishes 9.8.3's printed-origin subroutines from ones
    /// a grant added, which is the distinction "a PRINTED subroutine on this
    /// ice" draws.
    SubroutineBroken { ice: ObjectId, printed: bool },
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
    /// CR 10.14.6c: both players' secretly spent credits were REVEALED. The
    /// reveal is a step inside the one instruction 10.14.6 constructs, so it
    /// records nothing about who spent what beyond the two amounts — the
    /// spends themselves are already `CreditsLost` records of their own, and
    /// a condition asking about the reveal (Nisei Division) is asking about
    /// this moment and not about them.
    SecretlySpentCreditsRevealed { corp: u32, runner: u32 },
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

/// CR 1.15.4: the cards an occurrence NAMES — what a printed "it", "that
/// card" or "those cards" points at once the condition has been met. Listed
/// rather than wildcarded on purpose: a change that names a card only
/// incidentally names none here, so a card that says "it" about such an
/// occurrence acts on nothing rather than on the wrong card.
///
/// Most occurrences name exactly one card, and the singular readings
/// (`AbilityInstance::triggering_card`) take the first of these. An
/// occurrence that names SEVERAL is not a defect of that reading but 9.12.2c
/// at work: damage trashes its cards simultaneously (10.4.3), so one
/// occurrence names all of them, and "all copies of **that card**" (Chronos
/// Protocol: Haas-Bioroid) reads over every card the occurrence named.
pub fn cards_named_by(c: &GameChange) -> Vec<ObjectId> {
    match c {
        GameChange::CardInstalled { obj, .. }
        | GameChange::CardRezzed { obj, .. }
        | GameChange::CardDerezzed { obj }
        | GameChange::CardTrashed { obj, .. }
        // 5.5.4: "…install 1 program or piece of hardware from among THOSE
        // CARDS" — a discard names the card it moved, which is what lets the
        // sentence that follows the condition act on it.
        | GameChange::CardDiscarded { obj, .. }
        | GameChange::CardAdvanced { obj }
        | GameChange::CardExposed { obj }
        | GameChange::CardRevealed { obj, .. }
        | GameChange::CardAccessed { obj }
        | GameChange::CardPlayed { obj, .. }
        | GameChange::AgendaScored { obj, .. }
        | GameChange::AgendaStolen { obj, .. }
        // 8.2.5: "the forfeited agenda" — the card is in the removed-from-game
        // zone by now, and 1.15.4 still names it, which is what lets a
        // sentence read its printed agenda point value afterwards.
        | GameChange::AgendaForfeited { obj, .. } => vec![*obj],
        // 10.4.2a/b + 10.4.3: the damage procedure's trashes, which this one
        // change carries because they happen simultaneously. They are what
        // "whenever the Runner trashes a card for brain damage" is met by, so
        // they are what its sentence's "that card" means.
        GameChange::DamageSuffered { cards, .. } => cards.clone(),
        _ => Vec::new(),
    }
}
