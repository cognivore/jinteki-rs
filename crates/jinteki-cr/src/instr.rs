//! The Instruction vocabulary (§9.3.4, §9.11) for the W1 kernel wave.
//!
//! Card-text instructions (9.11.3: one sentence = one instruction) plus the
//! timing-structure-internal instructions the §11 step tables need
//! (9.11.2: each step in a timing structure forms a single instruction).
//! Every variant resolves to real state mutation — no silent no-ops.

use crate::effects::DamageKind;
use crate::object::{CardType, ObjectId, ServerId, Side, Zone};

/// The ONE selector language for quantity positions (ARCHITECTURE §12
/// rule 5): a pure data expression evaluated against world state, returning
/// an integer. Calculated quantities (9.12.2) are these expressions; their
/// dependencies are readable from the expression itself, which is what lets
/// the characteristics pipeline (9.12.1) and calculated-quantity timing
/// (9.12.2) re-evaluate them reactively. Never a closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quantity {
    /// A printed constant.
    Const(i64),
    /// "…for each <object matching the filter>" — the count of matching
    /// objects (9.12.2a).
    Count(TargetFilter),
    /// "…for each <kind> counter hosted on this card" — counts hosted
    /// counters INCLUDING those set aside by a [trash] trigger cost (9.5.5).
    CountersOnSource(crate::object::CounterKind),
    /// Sum of two quantities ("2 plus 1 for each …").
    Plus(Box<Quantity>, Box<Quantity>),
    /// Scale ("N for each …").
    Times(i64, Box<Quantity>),
    /// CR 9.12.2e: a value defined by X, where the ability defining X lives
    /// on the source (the Surveyor class shares one X between a strength
    /// definition and a trace). While the defining ability is inactive or
    /// lost, X is treated as 0.
    XOfSource(Box<Quantity>),
}

impl Quantity {
    /// Shorthand for a printed constant.
    pub fn c(n: i64) -> Quantity {
        Quantity::Const(n)
    }
    /// "base plus per × (counters of `kind` on this card)" — the common
    /// calculated-quantity shape (9.12.2b, Urtica/Fermenter classes).
    pub fn base_plus_per_counter(base: i64, per: i64, kind: crate::object::CounterKind) -> Quantity {
        Quantity::Plus(
            Box::new(Quantity::Const(base)),
            Box::new(Quantity::Times(per, Box::new(Quantity::CountersOnSource(kind)))),
        )
    }
}

/// A single instruction: the atomic unit of ability resolution (9.3.4c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // ---- card-text vocabulary -------------------------------------------
    /// "Gain N credits." — N is a quantity position (9.12.2: "…for each" is
    /// the same instruction with a computed selector).
    GainCredits(Side, Quantity),
    /// "Lose N credits." (loses as much as possible if short)
    LoseCredits(Side, u32),
    /// "Draw N cards."
    Draw(Side, u32),
    /// "Do N <kind> damage." / "Suffer N <kind> damage."
    /// `responsible` per 10.4.1 (Corp "does", Runner "suffers"). The amount
    /// is a quantity position: "2 net plus 1 per advancement counter" is one
    /// instruction whose selector aggregates into a single instance
    /// (9.12.2b/c, Urtica class).
    Damage { kind: DamageKind, amount: Quantity, responsible: Side },
    /// "Take N tags." (the Runner)
    GainTags(u32),
    /// "Trash <targets>." — one effect acting on the whole set (9.12.2a).
    TrashCards(TargetSpec),
    /// "End the run."
    EndTheRun,
    /// An optional part its controller may decline (9.6.9c): "you may …".
    DeclineableChoice(Box<Instruction>),
    /// CR 9.11.4f / 1.16.11a: "you may pay [cost] to [effect]" — the pay/
    /// decline choice ends an instruction; the paid-for branch becomes the
    /// next instruction.
    NestedCostThen {
        cost: crate::ability::Cost,
        effect: Box<Instruction>,
        /// Who pays (None = the ability's controller). "…unless the Runner
        /// pays" names the payer explicitly.
        payer: Option<crate::object::Side>,
    },
    /// CR 1.16.11b: "[effect] unless [cost]" — paying suppresses the effect;
    /// declining (or being unable to pay) makes it the next instruction.
    NestedCostUnless {
        cost: crate::ability::Cost,
        effect: Box<Instruction>,
        payer: Option<crate::object::Side>,
    },
    /// "Move the (set-aside) hosted counters to <target>" (Reconstruction
    /// Contract class, 9.5.5).
    MoveSetAsideCounters { kind: crate::object::CounterKind, target: TargetSpec },
    /// Combined-sentence instruction: several effects in ONE instruction
    /// (e.g. Snare!'s "Do 3 net damage and give the Runner 1 tag.").
    Combined(Vec<Instruction>),
    /// Interrupt-effect: prevent N damage of a kind (9.9.5).
    PreventDamage { kind: DamageKind, amount: u32 },
    /// Interrupt-effect: prevent ALL damage of a kind (9.9.7b).
    PreventAllDamage { kind: DamageKind },
    /// Interrupt-effect: avoid N tags (9.9.5).
    AvoidTags(u32),
    /// Interrupt-effect: increase imminent damage by N (The Cleaners class).
    IncreaseImminentDamage { kind: DamageKind, amount: u32 },
    /// Interrupt-effect: prevent a specific object from being trashed
    /// (Sacrificial Construct class).
    PreventTrashOf(ObjectId),
    /// "Do N <kind> damage. This damage cannot be prevented." (Flare class;
    /// 9.3.3g/9.4.5: the restriction rides the value.)
    DamageUnpreventable { kind: DamageKind, amount: Quantity, responsible: Side },
    /// Interrupt-effect: replace the imminent damage's type (Tori Hanzō
    /// class; 9.9.10: applies immediately when the interrupt resolves).
    ReplaceImminentDamageKind { to: DamageKind },
    /// "Run any server." / "make another run" (Doppelgänger class) — pushes
    /// a nested run timing structure.
    InitiateRun(ServerId),
    /// "Trace [N] — if successful, …; if unsuccessful, …" (10.8). Expanded
    /// by the resolution loop into the 10.8.6 step sequence. The base is a
    /// quantity position: Trace[3] is a constant selector; "Trace[X], X = 2
    /// per ice protecting this server" (Surveyor class) is
    /// `XOfSource(Times(2, Count(IceProtectingSourceServer)))` — evaluated
    /// when the trace initiates (9.12.2e), 0 if the defining ability is
    /// inactive or lost.
    Trace {
        base: Quantity,
        if_successful: Vec<Instruction>,
        if_unsuccessful: Vec<Instruction>,
        /// "When the trace is determined…, if your trace strength is N or
        /// greater, …" (Gemini class, 10.8.5).
        determined_min: Option<(i64, Vec<Instruction>)>,
    },
    /// 10.8.6a: the trace initiates ("when initiated" conditions meet); the
    /// base trace strength is a modifiable value (9.9.6d).
    TraceInitiate { base: i64 },
    /// 10.8.6c: the Corp may spend credits to increase the trace strength.
    TraceCorpSpend,
    /// 10.8.6d: the Runner may spend credits to increase their link strength.
    TraceRunnerSpend,
    /// 10.8.6e: determine success; the associated conditionals pend (10.8.5).
    TraceDetermine {
        if_successful: Vec<Instruction>,
        if_unsuccessful: Vec<Instruction>,
        determined_min: Option<(i64, Vec<Instruction>)>,
    },
    /// "Play a Psi Game." — one instruction: sealed bids, reveal, immediate
    /// spend, branch (10.14.6).
    PsiGame { on_match: Vec<Instruction>, on_differ: Vec<Instruction> },
    /// "<Ice> gains N copies of <sub> …" (Brainstorm class grants to
    /// itself; a Marker-class ability grants to another piece of ice —
    /// category 9.8.3e, external, ordered oldest-first by grant time). The
    /// INSTRUCTION is the position; `to` and `duration` are the content, so
    /// self-grants and external grants are one variant (§12 rule 2).
    GrantSubroutines {
        to: TargetSpec,
        count: u32,
        sub: Box<crate::ability::AbilityDef>,
        before: bool,
        duration: crate::lingering::WantedDuration,
    },
    /// "The Corp discards N cards from HQ." (Utopia Shard class driver.)
    CorpDiscards { count: u32 },
    /// "The Runner cannot access cards other than this one for the
    /// remainder of the run." (Ash class, 7.4.2.)
    RestrictAccessToSelf,
    /// Create a delayed conditional ability (9.6.13) with the given
    /// duration request; "when this run ends" with no run → never created
    /// (9.6.13d).
    CreateDelayedConditional {
        def: Box<crate::ability::AbilityDef>,
        duration: crate::lingering::WantedDuration,
    },
    /// Create a lingering effect (§9.10) with a stated duration: "prevent
    /// all damage for the remainder of this run", "the next time you would
    /// breach, instead …", "access N additional cards". The INSTRUCTION is
    /// the position; [`LingeringSpec`] is the content, so the whole class is
    /// expressible without a bespoke instruction per card (§12 rule 2). The
    /// requested duration is bound to the structure instance in progress at
    /// resolution (9.10.4).
    CreateLingeringEffect {
        payload: LingeringSpec,
        duration: crate::lingering::WantedDuration,
    },
    /// "The Runner loses N memory units until end of turn." (Bad Times.)
    ReduceRunnerMemoryThisTurn(u32),
    /// CR 9.11.4g / 9.12.3c-d: choose one of several optioned effects; the
    /// choice ends an instruction and must select a fully-resolvable option
    /// if any exists; the chosen effect is then separately interruptible.
    ChooseOne { options: Vec<(&'static str, Vec<Instruction>)> },
    /// "Break up to N subroutines on the encountered ice." (first unbroken)
    BreakSubroutines { count: u32 },
    /// "Bypass the ice you are encountering." — ends the encounter (6.5.8).
    BypassEncounteredIce,
    /// Icebreaker pump: "+N strength" with implicit remainder-of-encounter
    /// duration (9.10.4a).
    PumpStrengthSelf { amount: i32 },
    /// "Place N advancement counters on <target>" / advance bookkeeping.
    PlaceCounters { target: TargetSpec, kind: crate::object::CounterKind, amount: u32 },
    /// "Trash this card." (self-referencing; strandable per 9.1.4)
    TrashSelf,
    /// Steal the accessed agenda (7.1.4 via access step 7.2.3).
    StealSelfAgenda,
    /// §8.5: install one card. The resolution loop expands this into the
    /// 8.5.16 step sequence (installing is a procedure, NOT a timing
    /// structure — 9.2.2e; its only explicitly-called-for checkpoint is the
    /// cost-paid one at 8.5.16d).
    InstallCard {
        card: TargetSpec,
        dest: InstallDest,
        /// 8.5.15: rez directly after the installation is complete.
        and_rez: bool,
        /// 1.16.5c: "ignoring all costs".
        ignore_costs: bool,
        /// 8.5.13c: a requirement imposed by the installing ability that
        /// must be verified by revealing a hidden card.
        reveal_check: Option<RevealCheck>,
    },
    /// 8.5.5: an effect installing more than one card — the cards are chosen
    /// and installed ONE AT A TIME, each as a separate instruction
    /// (9.11.4b). `and_rez_if_able` is the Ad Blitz "if able" stipulation
    /// (8.5.13d): unrezzable cards cannot be chosen.
    InstallCards {
        count: u32,
        from_hand_of: Side,
        filter: InstallFilter,
        dest: InstallDest,
        and_rez: bool,
        and_rez_if_able: bool,
        ignore_costs: bool,
    },
    /// 8.5.16a–c: place into the play area (not installed, not active),
    /// declare the destination, trash like cards.
    InstallStepPlace,
    /// 8.5.16d: pay the install cost. (The post-instruction checkpoint IS
    /// the 10.3.4 cost-paid checkpoint — 8.5.11c.)
    InstallStepPayCost,
    /// 8.5.16e–f: create the server if new, move the card, it becomes
    /// installed (faceup → active); "when installed" conditions are met and
    /// the install effect is complete.
    InstallStepComplete,
    /// 8.5.15 → 8.1.2d: pay the rez cost of the just-installed card. (The
    /// post-instruction checkpoint is the cost-paid checkpoint; per the
    /// 9.6.5b THG example this is the checkpoint that processes the
    /// CardInstalled change, while the card is still facedown.)
    InstallRezPayCost,
    /// Finish rezzing: the card turns faceup and becomes active.
    InstallRezFinish,
    /// §8.6: play one event/operation. Expanded by the resolution loop into
    /// the 8.6.7 step sequence.
    PlayCard { card: TargetSpec, ignore_costs: bool },
    /// 8.6.3: an effect playing more than one card — chosen and played one
    /// at a time, each as a separate instruction (9.11.4b).
    PlayCards { count: u32, from_hand_of: Side, ignore_costs: bool },
    /// 8.6.7a: place the card faceup in the play area; not installed, not
    /// yet active.
    PlayStepPlace,
    /// 8.6.7b: pay the play cost. (Post-instruction checkpoint = 10.3.4.)
    PlayStepPayCost,
    /// 8.6.7c–d: the card becomes active; conditions related to playing it
    /// are met. (The post-instruction checkpoint is 8.6.7e.)
    PlayStepActivate,
    /// 8.6.7f: resolve the play abilities of the card (a nested frame).
    PlayStepResolve,
    /// 8.6.7g–i: trash the card if applicable (8.6.6); after-resolve
    /// conditions are met; the play effect is complete.
    PlayStepFinish,
    /// "Remove this card from the game." (Ashen Epilogue class; 8.6.6a — a
    /// played card no longer in the play area is not trashed.)
    RemoveSelfFromGame,
    /// "If you have at least N link, <effect>." — the requirement lives in
    /// the INSTRUCTIONS, not the trigger condition (9.6.5d): it is checked
    /// when this instruction resolves.
    IfRunnerLinkAtLeast { n: u32, then: Box<Instruction> },
    /// The Corp rearranges (or looks at and returns) cards in R&D
    /// (Bacterial Programming class). The returned cards are NEW OBJECTS
    /// (1.12.3), so 7.4.7a's "already chosen" bookkeeping forgets them and
    /// the breach continues from the top of R&D.
    CorpRearrangesRnd,
    /// "Add a card from Archives to the top of R&D." (Seidr class.)
    MoveToTopOfRnd { card: TargetSpec },
    /// CR 8.7.1–8.7.3: "Search <zone> for <criteria>." The searching player
    /// looks at every card in the zone (8.7.1; hidden/secret zones are
    /// temporarily visible to them alone, 8.7.1a) and may FIND up to `count`
    /// cards matching the criteria (8.7.2), taking them from the zone and
    /// setting them aside facedown (4.8.4). A searched DECK is reshuffled
    /// immediately when the search completes, before any remaining effect of
    /// this ability resolves (8.7.3); resolution then resumes (8.7.4) with
    /// the found cards addressable as [`TargetSpec::FoundBySearch`].
    ///
    /// The find is NOT a target announcement (2.x `rule_searching_does_not
    /// _target`), so it is asked at RESOLUTION time, never from the
    /// announce-targets phase.
    Search {
        /// The zone searched (§4). Only decks are shuffled afterwards.
        zone: Zone,
        /// CR 8.7.2a: the criteria a found card must match — a conjunction
        /// of the shared filter vocabulary. Empty = "a card" (no criteria).
        criteria: Vec<TargetFilter>,
        /// How many cards may be found — a quantity position (§12 rule 6).
        count: Quantity,
        /// CR 8.7.2e: searching a deck for cards with specified criteria may
        /// fail to find. Where it is false, 8.7.2d forces the player to find
        /// as many matching cards as the zone holds, up to `count`.
        may_fail: bool,
    },
    /// "Add <cards> to your grip/HQ." (8.2 movement to a hand.) The cards
    /// position takes the shared [`TargetSpec`], so "the cards found by this
    /// ability's search" (8.7.4), a fixed card and an announced choice are
    /// all one instruction.
    AddCardsToHand { cards: TargetSpec },
    /// "<side> trashes N random cards from their grip/HQ." (Personality
    /// Profiles class; the random selection is the mechanism 10.4.2 damage
    /// uses.) The count is a quantity position (§12 rule 6).
    TrashRandomFromHand { side: Side, count: Quantity },

    // ---- timing-structure-internal vocabulary ---------------------------
    /// `step_corp_turn_allotted_clicks` / `step_runner_turn_allotted_clicks`.
    GainAllottedClicks(Side),
    /// `step_*_recurring_credits_refill`.
    RefillRecurring(Side),
    /// `step_*_formal_begin`: "The <side>'s turn begins."
    TurnFormallyBegins(Side),
    /// `step_corp_turn_mandatory_draw`.
    MandatoryDraw,
    /// `step_*_discard`: discard down to maximum hand size (5.5.4).
    DiscardToHandSize(Side),
    /// `step_*_lose_unspent_clicks`.
    LoseUnspentClicks(Side),
    /// `step_*_formal_end`: "The <side>'s turn ends."
    TurnFormallyEnds(Side),
    /// `step_*_complete`: turn structure complete.
    TurnComplete(Side),
    /// `step_initiation_announce`: the Runner announces the attacked server.
    AnnounceAttackedServer(ServerId),
    /// `step_initiation_bad_publicity`: fill the bad publicity fund.
    FillBadPubFund,
    /// `step_initiation_formal_begin`: the run begins.
    RunFormallyBegins,
    /// `step_runner_position`: position set to the outermost ice, if any.
    SetPositionOutermost,
    /// `step_approach_begins`: the Runner approaches ice.
    ApproachIce,
    /// `step_encounter_begins`: the Runner encounters ice.
    EncounterIce,
    /// `step_resolve_subroutine` (yes-branch): the Corp resolves the next
    /// unbroken subroutine.
    ResolveNextSubroutine,
    /// `step_pass_ice`.
    PassIce,
    /// `step_jack_out_choice`: the Runner may jack out.
    JackOutChoice,
    /// `step_move_position`: move 1 position inward, if possible.
    MovePositionInward,
    /// `step_approach_server`.
    ApproachServer,
    /// `step_run_declared_successful`.
    DeclareRunSuccessful,
    /// `step_breach` / standalone breach: breach a server.
    BreachServer(ServerId),
    /// `step_open_priority_windows_closed` (6.9.6a).
    CloseRunPriorityWindows,
    /// `step_run_ends_bad_publicity`: empty the fund.
    EmptyBadPubFund,
    /// `step_run_declared_unsuccessful` (conditional on 6.8.4).
    DeclareRunUnsuccessfulIfApplicable,
    /// `step_run_complete`.
    RunComplete,
    /// `step_breaching_begins`.
    BreachBegins,
    /// `step_flip_archives`.
    FlipArchivesFaceup,
    /// `step_determine_candidates_limit` (HQ/R&D access counts).
    DetermineAccessLimit,
    /// `step_choose_candidate` yes-branch: Runner chooses a candidate (11.5).
    ChooseCandidate,
    /// `step_access_candidate`: access the chosen card (runs the 7.2 table).
    AccessChosenCandidate,
    /// `step_breach_complete`.
    BreachComplete,
    /// `step_card_accessed` (7.2.1).
    CardBecomesAccessed,
    /// `step_mid_access_ability` (7.2.2): the mid-access ability window.
    MidAccessWindow,
    /// `step_access_agenda` (7.2.3): if it is an agenda, the Runner steals it.
    StealIfAgenda,
    /// `step_access_complete` (7.2.4).
    AccessComplete,
}

/// What a card's text asks a lingering effect to DO (§9.10 payload classes),
/// as data. Paired with a [`crate::lingering::WantedDuration`] by
/// [`Instruction::CreateLingeringEffect`]; the effect's source is the
/// resolving ability's source object (9.10.1: the effect then exists
/// independently of it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LingeringSpec {
    /// "Prevent all damage." for the duration (The Noble Path class; 6.8.5 —
    /// a run-bound shield expires at step 6.9.6d).
    PreventAllDamage,
    /// CR 9.9.8c: a replacement effect created ahead of time — "instead of
    /// <the effect class>, <transform>" (Security Testing / Account Siphon /
    /// Showing Off / Immolation Script classes).
    Replacement {
        applies_to: crate::effects::EffectClass,
        with: crate::lingering::ReplacementTransform,
    },
    /// "Access N additional cards from <server>." (The Maker's Eye class;
    /// added to the 7.3.6 access limit at breach step 7.5.3.)
    AdditionalAccess { server: ServerId, extra: u32 },
}

/// Targets, either fixed at card-compile time or chosen at announce time
/// (9.3.4b: targets are announced before the instruction becomes imminent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetSpec {
    /// Fixed objects (test-card layer wires these directly).
    Objects(Vec<ObjectId>),
    /// The ability's own source.
    SelfSource,
    /// The source's host (Parasite-style).
    HostOfSource,
    /// The card currently being accessed.
    AccessedCard,
    /// The ice currently being encountered (Forked class).
    EncounteredIce,
    /// Chosen by the controller at announce time from a filter.
    Choose { count: u32, filter: TargetFilter },
    /// The top N cards of a deck (Breached Dome-style).
    TopOfDeck(Side, u32),
    /// CR 8.7.4: the cards found by this ability's search, still set aside
    /// facedown (4.8.4). This is how an install/play/add-to-hand instruction
    /// refers to them without a per-card hook.
    FoundBySearch,
}

/// The shared object-filter language: announce-time target filters, the
/// counting filters `Quantity::Count` evaluates, and the 8.7.2a criteria of
/// a search (§12 rule 5 — one filter vocabulary for choosing, counting and
/// finding). Each variant is one predicate atom; several atoms combine as a
/// conjunction wherever the CR speaks of "the criteria" in the plural.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFilter {
    InstalledCorpCard,
    InstalledRunnerCard,
    InstalledResource,
    /// Ice protecting the server the source is protecting (Surveyor-class
    /// counting; empty when the source is not protecting a server).
    IceProtectingSourceServer,
    /// Cards in a player's hand (Ashigaru-class counting).
    CardsInHandOf(Side),
    // ---- card-characteristic atoms (§2), location-agnostic --------------
    /// CR 2.15: "a program", "an asset", "a piece of ice" — the card's type.
    CardTypeIs(CardType),
    /// CR 2.16: "a virus program", "a region" — an effective subtype
    /// (9.12.1b counting applies through the characteristics pipeline).
    HasSubtype(&'static str),
    /// CR 2.3: "…with printed install/rez/play cost N or lower".
    PrintedCostAtMost(u32),
}

/// CR 8.5.16b: the install destination, declared as part of installing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDest {
    /// Corp: the root of an existing server (8.5.2b/c).
    Root(ServerId),
    /// Corp: create a new remote server (8.5.2a).
    NewRemoteRoot,
    /// Corp: protecting a server, outermost position (8.5.2d).
    Protecting(ServerId),
    /// "directly inward" from the ability's source ice (Brân class). If the
    /// source is not protecting a server, the destination cannot be
    /// identified and no installation takes place (8.5.14).
    InwardFromSource,
    /// Runner: the rig (8.5.4).
    Rig,
    /// Hosted on a specific card (8.5.1a).
    HostedOn(ObjectId),
    /// The root of the server currently being breached (Ganked/Drafter
    /// class; resolved when the destination is declared).
    BreachedServerRoot,
    /// Runner installs: choose an eligible host (8.5.1a — a card whose
    /// ability describes what it can host is an eligible destination) or
    /// default to the rig. The choice is announced with the targets.
    RunnerChoiceHostOrRig,
}

/// Card-class filter for multi-install effects (8.5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallFilter {
    Program,
    Ice,
    Any,
}

/// CR 8.5.13c: a stipulation of the installing ability that must be
/// verified by revealing the card when it is not otherwise visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealCheck {
    /// "…with printed rez cost N or lower" (Ob Superheavy class).
    PrintedRezCostAtMost(u32),
}
