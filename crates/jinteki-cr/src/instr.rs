//! The Instruction vocabulary (§9.3.4, §9.11) for the W1 kernel wave.
//!
//! Card-text instructions (9.11.3: one sentence = one instruction) plus the
//! timing-structure-internal instructions the §11 step tables need
//! (9.11.2: each step in a timing structure forms a single instruction).
//! Every variant resolves to real state mutation — no silent no-ops.

use crate::effects::DamageKind;
use crate::object::{ObjectId, ServerId, Side};

/// A single instruction: the atomic unit of ability resolution (9.3.4c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // ---- card-text vocabulary -------------------------------------------
    /// "Gain N credits."
    GainCredits(Side, u32),
    /// "Lose N credits." (loses as much as possible if short)
    LoseCredits(Side, u32),
    /// "Draw N cards."
    Draw(Side, u32),
    /// "Do N <kind> damage." / "Suffer N <kind> damage."
    /// `responsible` per 10.4.1 (Corp "does", Runner "suffers").
    Damage { kind: DamageKind, amount: u32, responsible: Side },
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
    /// "Gain N[c] for each <counter> hosted on this card" — counts hosted
    /// counters INCLUDING those set aside by a [trash] trigger cost (9.5.5).
    GainCreditsPerCounter { kind: crate::object::CounterKind, per: u32 },
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
    DamageUnpreventable { kind: DamageKind, amount: u32, responsible: Side },
    /// Interrupt-effect: replace the imminent damage's type (Tori Hanzō
    /// class; 9.9.10: applies immediately when the interrupt resolves).
    ReplaceImminentDamageKind { to: DamageKind },
    /// "Run any server." / "make another run" (Doppelgänger class) — pushes
    /// a nested run timing structure.
    InitiateRun(ServerId),
    /// "Trace [N] — if successful, …; if unsuccessful, …" (10.8). Expanded
    /// by the resolution loop into the 10.8.6 step sequence.
    Trace {
        base: i64,
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
    /// "This ice gains N copies of <sub> …" (Brainstorm class; category
    /// 9.8.3e — external, after, ordered oldest-first by grant time).
    GrantSubroutinesToSelf { count: u32, sub: Box<crate::ability::AbilityDef>, before: bool },
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
    /// "The Runner loses N memory units until end of turn." (Bad Times.)
    ReduceRunnerMemoryThisTurn(u32),
    /// "Do <base> + <per> × (counters on this card) <kind> damage." —
    /// a calculated quantity aggregating into ONE instance (9.12.2b/c,
    /// Urtica Cipher class).
    DamagePerCounter {
        kind: DamageKind,
        base: u32,
        per: u32,
        counter: crate::object::CounterKind,
        responsible: Side,
    },
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
    /// Chosen by the controller at announce time from a filter.
    Choose { count: u32, filter: TargetFilter },
    /// The top N cards of a deck (Breached Dome-style).
    TopOfDeck(Side, u32),
}

/// Announce-time target filters (kernel-wave subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFilter {
    InstalledCorpCard,
    InstalledRunnerCard,
    InstalledResource,
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
