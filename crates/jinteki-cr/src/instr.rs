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
    /// Difference of two quantities ("…past its advancement requirement").
    Minus(Box<Quantity>, Box<Quantity>),
    /// CR 2.4: the advancement requirement of the source agenda, as modified
    /// (a SanSan-class declaration lowers it). CR 1.17.8 / 10.13.2: for an
    /// agenda that has been scored or stolen, this — like
    /// [`Quantity::CountersOnSource`] — reads the last known value from
    /// before the move, since the real one no longer exists.
    RequirementOfSource,
    /// Scale ("N for each …").
    Times(i64, Box<Quantity>),
    /// CR 9.12.2a: "…1 for every N <things>" (Project Beale's "1 agenda
    /// counter for every 2 hosted advancement counters past 3"). The
    /// complement of [`Quantity::Times`] — integer division, so a remainder
    /// buys nothing — and a negative inner quantity yields 0, since there is
    /// no such thing as a negative number of things to count.
    PerEvery(Box<Quantity>, i64),
    /// "…for each credit lost" (Account Siphon class): credits the named
    /// player ACTUALLY lost during the resolution of the ability now
    /// resolving — the observed 1.10.3b loss, not the requested amount.
    CreditsLostThisAbility(Side),
    /// CR 7.3.6: "for each time they accessed a card during the run" — the
    /// number of accesses ACTUALLY PERFORMED during the run in progress (or
    /// the run that just ended, for a "when this run ends" ability). An
    /// access that was replaced by another effect never happened and is not
    /// counted.
    AccessesThisRun,
    /// CR 1.12.6: "for each piece of ice you passed during this run" — the
    /// number of DISTINCT ice objects the Runner passed, counted by reviewing
    /// the game history. An object that no longer exists in the present game
    /// state still counts, which is the whole of the rule.
    DistinctIcePassedThisRun,
    /// CR 9.12.2e: a value defined by X, where the ability defining X lives
    /// on the source (the Surveyor class shares one X between a strength
    /// definition and a trace). While the defining ability is inactive or
    /// lost, X is treated as 0.
    XOfSource(Box<Quantity>),
    /// CR 1.16.2c: the value the payer ANNOUNCED for X, for the cost being
    /// paid right now. 1.16.2d: "if an ability needs to know the value of a
    /// cost in a context where that cost is not being paid, treat any X that
    /// appears in that cost as 0" — which is exactly what this reads outside
    /// a payment.
    AnnouncedX,
    /// CR 10.7: the number of tags the Runner has. A quantity position
    /// (§12 rule 6), used both in effects and as a 1.16.2c restriction on X.
    RunnerTags,
    /// CR 1.10.1: "all credits in their credit pool" (Closed Accounts) — the
    /// named player's credit POOL, which 1.13.3 keeps distinct from any
    /// credits hosted on cards.
    CreditsInPoolOf(Side),
}

impl Default for Quantity {
    /// A quantity position with nothing in it is 0 (`Cost::default`).
    fn default() -> Self {
        Quantity::Const(0)
    }
}

impl Quantity {
    /// Shorthand for a printed constant.
    pub fn c(n: i64) -> Quantity {
        Quantity::Const(n)
    }
    /// CR 1.16.2c: does this quantity CONTAIN the variable X? "Some costs
    /// contain the variable X. Before a player pays such a cost, they choose
    /// and announce a positive integer or 0 to be the value for X" — so the
    /// announcement is owed by the cost's SHAPE, not by whether the ability
    /// also states a restriction on the value.
    pub fn mentions_announced_x(&self) -> bool {
        match self {
            Quantity::AnnouncedX => true,
            Quantity::Plus(a, b) | Quantity::Minus(a, b) => {
                a.mentions_announced_x() || b.mentions_announced_x()
            }
            Quantity::Times(_, q) | Quantity::PerEvery(q, _) | Quantity::XOfSource(q) => {
                q.mentions_announced_x()
            }
            _ => false,
        }
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

/// CR 6.7.4a: the set of servers an effect that initiates a run allowed the
/// Runner to choose from. A selector, not a literal list, because "a remote
/// server" names a set the game state computes (§12 rule 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunServerSet {
    /// The effect named no restriction ("make a run").
    Any,
    /// "Run a remote server."
    AnyRemote,
    /// "Run HQ." — exactly these servers.
    These(Vec<ServerId>),
}

impl RunServerSet {
    /// CR 6.7.4a: "…no longer is a server that could have been chosen for
    /// this run".
    pub fn allows(&self, s: ServerId) -> bool {
        match self {
            RunServerSet::Any => true,
            RunServerSet::AnyRemote => matches!(s, ServerId::Remote(_)),
            RunServerSet::These(v) => v.contains(&s),
        }
    }
}

/// The movement vocabulary, as a list — a **citation anchor** for the rules
/// each movement's own instruction or procedure implements.
///
/// CR 8.2.1: the MOVEMENTS — the ways a card changes zone or location under
/// special rules. Each is an instruction or a procedure below; 8.2.1a's
/// non-movements ("add", "move", "discard", "set aside", "remove from the
/// game") simply put the cards where they are told, which is why they are
/// ordinary moves in the kernel and meet no movement-related condition.
pub const fn movements() -> &'static [&'static str] {
    cite!("sec_movement");
    cite!("rule_non_movements");
    cite!("movement_arrange");
    cite!("movement_draw");
    cite!("movement_install");
    cite!("movement_play");
    cite!("movement_search");
    cite!("movement_score");
    cite!("movement_steal");
    cite!("movement_swap");
    cite!("movement_trash");
    &["arrange", "draw", "forfeit", "install", "play", "search", "score", "steal", "swap", "trash"]
}

/// A single instruction: the atomic unit of ability resolution (9.3.4c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // ---- card-text vocabulary -------------------------------------------
    /// "Gain N credits." — N is a quantity position (9.12.2: "…for each" is
    /// the same instruction with a computed selector).
    GainCredits(Side, Quantity),
    /// "Lose N credits." (1.10.3b: loses as much as possible if short.) N is
    /// a quantity position (§12 rule 6), so "the Runner loses all credits in
    /// their credit pool" (Closed Accounts) is this instruction with a
    /// selector rather than a variant of its own.
    LoseCredits(Side, Quantity),
    /// CR 1.11.3a: "A player gains clicks whenever the number of clicks they
    /// have is increased." The count is a quantity position (§12 rule 6).
    GainClicks(Side, Quantity),
    /// CR 1.11.3b: "If a player loses or spends clicks, the number of clicks
    /// they have is reduced by that amount." This is the LOSE half — Enigma's
    /// "the Runner loses [click]" — and 1.11.3b is explicit that losing and
    /// spending are not synonymous, so a card that counts spent clicks does
    /// not see this. A player with fewer clicks than the amount simply reaches
    /// zero: the number they have cannot go below it.
    LoseClicks(Side, Quantity),
    /// "Draw N cards." CR 8.4.1: drawing moves cards from a deck to a hand,
    /// and 8.4.2 makes that a PROCEDURE — the cards are set aside facedown
    /// first, are considered drawn there, and only reach the hand once every
    /// ability that acts on the draw has resolved. So this instruction
    /// expands into the 8.4.5 step sequence, exactly as an install expands
    /// into 8.5.16.
    Draw(Side, u32),
    /// CR 8.4.5a: "Set aside N cards from the top of the drawing player's
    /// deck. The cards are now considered drawn and can be looked at by their
    /// controller." They are one facedown 4.8.7 group, so 4.8.2a's exception
    /// (abilities referring to drawn cards CAN see them there) has something
    /// to name.
    DrawStepSetAside { side: Side, n: u32, group: u64 },
    /// CR 8.4.5c: "Add the set-aside cards to the player's hand." Whatever is
    /// still in the group goes — 8.4.3a's card that left is not added, and
    /// 8.4.3b's card swapped in is.
    DrawStepAddToHand { side: Side, group: u64 },
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
    /// CR 9.10.3: "choose a server", "choose an installed piece of ice",
    /// "choose an ice subtype" — the choice is REMEMBERED by a lingering
    /// effect (`Payload::MaintainedChoice`) that later abilities of the same
    /// source read by `key`. The duration is 9.10.3's, which the card layer
    /// states as one of its three cases: (a) the same duration as the effect
    /// that reads it, (b) `ThisTurn` for a bare "when your turn begins"
    /// choice, (c) `WhileSourceActive` otherwise.
    MaintainChoice { key: &'static str, of: ChoiceSpec, duration: crate::lingering::WantedDuration },
    /// CR 9.12.3a/b: "the Runner MUST trash this card, if able." The
    /// instruction states a requirement, not an effect: it forbids the Runner
    /// from passing the mid-access window (9.2.10) while a permitted means of
    /// trashing the accessed card is available to them. `means` is 9.12.3's
    /// distinction — with no means stipulated the Runner must make any
    /// decision that satisfies the requirement, including using another
    /// card's ability (9.12.3a); with a means stipulated only that means
    /// counts and nothing else can be forced (9.12.3b).
    MustTrashAccessedCard { means: TrashMeans },
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
    /// "…and access it" (§7.2): the announced cards are accessed, one at a
    /// time, each in its own access timing structure. The cards are a target
    /// POSITION, so "access the card you chose from the top of R&D" (Top Hat
    /// class) and "access this card" are the same instruction.
    AccessCards {
        cards: TargetSpec,
        /// "…you cannot steal or trash it during this access." (Pinhole
        /// Threading class; a printed restriction on the access, 1.2.2.)
        restricted: bool,
    },
    /// "…access 2 additional cards." (Cupellation class; raises 7.3.5's
    /// random-access limit for the breach in progress.)
    AdditionalAccesses(Quantity),
    /// CR 9.6.14d: "Resolve the <class> ability of <a card>." — an effect
    /// that attempts to resolve an ability of a card by naming its class
    /// rather than by the stipulation occurring. For the three conditional
    /// classes of 9.6.14 the ability is marked PENDING as though the
    /// stipulation had occurred, so it resolves through the ordinary
    /// reaction window; any additional requirements of its trigger condition
    /// must still be met by the game state, and an unmet requirement means
    /// the ability cannot even become pending. For
    /// [`crate::ability::AbilityClass::Subroutine`] the named subroutine
    /// resolves directly (9.8.10), since a subroutine is not a conditional
    /// ability and never pends.
    ///
    /// The card is a target POSITION and the class is the content (§12 rule
    /// 2), so a 24/7-News-Cycle-class "resolve the 'when scored' ability of
    /// an agenda in your score area" and a Nanisivik-Grid-class "resolve its
    /// first subroutine" are one instruction.
    ResolveAbilityOf { source: TargetSpec, which: crate::ability::AbilityClass },
    /// CR 8.1.2: "Rez <a card>." — an unrezzed card is turned faceup, which
    /// makes it active (9.1.7). 8.1.2b: card abilities can direct or allow
    /// the Corp to rez cards outside the paid ability windows of 8.1.2a;
    /// 8.1.2d: the rez cost is paid first unless the ability states that it
    /// is ignored (1.16.5c), which is what `ignore_costs` says.
    RezCard { target: TargetSpec, ignore_costs: bool },
    /// CR 5.6.2b: "Your action phase ends." (Oppo Research class.) The action
    /// phase loop takes an action only while the player has unspent [click],
    /// so ending the phase early is losing the ones they have left.
    EndActionPhase(Side),
    /// CR 1.21.2: "Look at <cards>." — the looking player sees their front
    /// faces without showing them to the other player. CR 9.11.4e: where an
    /// older card puts the look in the SAME sentence as what is then done to
    /// those cards, making the cards visible is the end of an instruction, so
    /// the look is its own instruction and a checkpoint occurs before the
    /// next one announces its targets among them.
    LookAtCards { cards: TargetSpec, by: Side },
    /// CR 1.21.4: "Expose <cards>." — the named cards are revealed, except
    /// that only INSTALLED, UNREZZED cards can be exposed. 1.21.5 keeps
    /// exposing distinct from looking, revealing and accessing, and 9.12.2
    /// does not list it among the aggregated effect classes, so exposing two
    /// cards in one instruction is two occurrences (9.6.4b).
    ExposeCards { cards: TargetSpec },
    /// CR 1.21.3: "Reveal <cards>." — show the cards' front faces to all
    /// players, then return them to their previous state. 1.21.3a: this is NOT
    /// turning them faceup, so a facedown card stays facedown; what changes is
    /// what each player has SEEN (10.2.2b). Unlike exposing (1.21.4) it is not
    /// restricted to installed unrezzed cards — a card in hand, in a deck or
    /// set aside can be revealed.
    RevealCards { cards: TargetSpec },
    /// CR 1.10.3a: "Take N[credit] from this card." — hosted credits move from
    /// a card to a credit pool, which is a GAIN (they enter the pool from
    /// another location). With fewer hosted than asked for, the card gives
    /// what it has.
    TakeHostedCredits { from: TargetSpec, amount: Quantity, to: Side },
    /// CR 1.9.2: "Remove N hosted <kind> counters." — counters leave the card
    /// and return to the bank. This is the mandatory-effect counterpart of
    /// `Cost::spend_counters`, which is the paid-ability half: costs are
    /// SPENT (1.16.1), these are simply removed.
    RemoveCounters {
        target: TargetSpec,
        kind: crate::object::CounterKind,
        amount: Quantity,
        /// "remove up to N" — take what is there when there are fewer.
        up_to: bool,
    },
    /// "Derez <targets>." (§8.1.2) — a rezzed card is turned facedown.
    /// CR 1.12.5: turning a card faceup or facedown does not make it a new
    /// object, since it does not change zones.
    Derez { target: TargetSpec },
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
    /// CR 10.5.5: "Remove N tags." — the tags return to the bank (1.9.2). A
    /// quantity position (§12 rule 6): "remove X tags" is this instruction
    /// with the announced X (Misdirection class).
    RemoveTags(Quantity),
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
    InitiateRun {
        /// CR 6.9.1a: the attacked server the Runner ANNOUNCES. `None` is an
        /// effect that named no server ("Run any server", "Run a remote
        /// server"): the Runner announces one from `allowed` when the run is
        /// initiated, and the announcement rewrites this position exactly as
        /// an install's declared destination rewrites 8.5.16b's.
        server: Option<ServerId>,
        /// CR 6.7.4a: the servers this initiating effect ALLOWED. An "If
        /// successful" ability is tied to them, so a run moved (6.1.2d) to a
        /// server the effect could not have chosen no longer meets the
        /// condition — while a move WITHIN the set keeps it.
        allowed: RunServerSet,
        /// CR 6.7.4: "If successful, …" — a conditional ability whose trigger
        /// condition is "after the run created this way becomes successful".
        /// It cannot be an ordinary delayed conditional, because 9.6.13d
        /// wants the run already in progress; the instruction that creates
        /// the run is what carries it.
        if_successful: Vec<Instruction>,
    },
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
        /// WHICH subroutines are granted (§12 rule 2: the instruction is the
        /// position, this is the content).
        grant: SubroutineGrant,
        before: bool,
        /// CR 9.8.2c: "in the order of your choice" — the granting player
        /// declares where the granted subroutines sit relative to every
        /// subroutine the ice has at that time, regardless of categories.
        any_order: bool,
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
    /// "Choose <targets>. <They> gain/lose <subtypes> [for a duration]."
    /// (Tinkering class.) CR 9.11.4c: the choosing sentence and the
    /// modifying sentence form ONE instruction — the target is announced as
    /// the instruction becomes imminent and the subtypes change when it
    /// resolves. Both the target and the duration are positions; 2.16.5
    /// counts instances, so an add and a printed subtype coexist.
    ModifySubtypes {
        target: TargetSpec,
        add: Vec<&'static str>,
        remove: Vec<&'static str>,
        duration: crate::lingering::WantedDuration,
    },
    /// "The Runner loses N memory units until end of turn." (Bad Times.)
    ReduceRunnerMemoryThisTurn(u32),
    /// CR 9.11.4g / 9.12.3c-d: choose one of several optioned effects; the
    /// choice ends an instruction and must select a fully-resolvable option
    /// if any exists; the chosen effect is then separately interruptible.
    ChooseOne { options: Vec<(&'static str, Vec<Instruction>)> },
    /// "Break N subroutines on the encountered ice." — the subroutines it
    /// acts on are a target POSITION (1.15.1: subroutines are targets like
    /// objects are), so "break up to 2 subroutines" (Cleaver class),
    /// "break all subroutines" (9.8.6a, no targets) and "break all but 1
    /// subroutine" (9.8.6b, Grappling Hook) are one instruction.
    BreakSubroutines { subs: SubroutineSpec },
    /// "Bypass the ice you are encountering." — ends the encounter (6.5.8).
    BypassEncounteredIce,
    /// CR 6.5.9a: "The Runner encounters <a piece of ice>." — a FORCED
    /// encounter: an Encounter Ice Phase resolved outside the run's normal
    /// progression, without changing the Runner's position, after which
    /// resolution returns to this instruction's ability and proceeds from
    /// there (6.5.9c: this instruction is not finished until the encounter
    /// is complete). The ice is a target POSITION, so a Chrysalis-class
    /// "they encounter it" (itself, uninstalled — its subroutines are active
    /// for exactly that encounter, 9.1.8h) and a Ganked!-class "force the
    /// Runner to encounter a rezzed piece of ice you control" are the same
    /// instruction.
    ForceEncounter { ice: TargetSpec },
    /// "+N strength" / "-N strength" on a card, for a duration. The TARGET
    /// is a position (an icebreaker pumping itself is `SelfSource`; a Devil-
    /// Charm-class ability lowering a piece of ice's strength names the ice)
    /// and so is the DURATION: `None` is "no duration stated", which on an
    /// icebreaker modifying its own strength means "for the remainder of the
    /// current encounter" (3.9.5b / 9.10.4a) and outside an encounter means
    /// "until the next checkpoint" (3.9.5d). A stated duration runs
    /// ALONGSIDE that implicit one, not instead of it (3.9.5c / 3.4.4a).
    /// The AMOUNT is a quantity position too (§12 rule 6): "+X strength"
    /// (Paperclip, Corporate Troubleshooter) and "+X strength, X = the number
    /// of installed icebreakers" (Unity) are this instruction with a
    /// selector. A negative quantity lowers the strength.
    ModifyStrength {
        target: TargetSpec,
        amount: Quantity,
        duration: Option<crate::lingering::WantedDuration>,
    },
    /// "Place N <kind> counters on <target>." The count is a quantity
    /// position (§12 rule 6): the dividends keyword's "N agenda counters for
    /// each hosted advancement counter past its advancement requirement"
    /// (10.13.1) is this instruction with a selector.
    ///
    /// CR 1.18.2: placing an advancement counter directly is NOT advancing —
    /// that is [`Instruction::AdvanceCard`].
    PlaceCounters { target: TargetSpec, kind: crate::object::CounterKind, amount: Quantity },
    /// CR 6.1.2d: "The attacked server becomes <server>." (Sneakdoor Beta
    /// class.) A few abilities change the attacked server DIRECTLY, without
    /// referring to the Runner's position — so the run's current timing step
    /// does not change, and the Runner does not approach or encounter the ice
    /// protecting the new server. (Contrast `MoveRunnerToIce`, 6.2.8a, which
    /// changes the attacked server BY moving the Runner and does change the
    /// timing step.)
    ChangeAttackedServer { server: ServerId },
    /// CR 1.15.1 / 1.18.2: "Move up to N <kind> counters from 1 other card to
    /// the chosen card" (Trick of Light class). The COUNTERS are targets and
    /// so is the destination card (1.12.1 makes a counter an object); the card
    /// the counters come from is NOT a target — it is implied by which
    /// counters were chosen, and "1 other card" is the requirement that they
    /// all share a host. 1.18.2: moving an advancement counter is not
    /// advancing, so no "whenever you advance" condition is met.
    MoveCounters {
        kind: crate::object::CounterKind,
        count: Quantity,
        up_to: bool,
        /// The destination, announced FIRST (the counters are then chosen
        /// from another card — 1.15.4's cross-instruction reference in one
        /// instruction).
        to: TargetSpec,
        /// Which cards the counters may come from.
        from_criteria: Vec<TargetFilter>,
    },
    /// CR 1.18.1: "Advance <a card>." — place an advancement counter from the
    /// bank on it, as an ADVANCE, so that "whenever you advance" conditions
    /// are met (1.18.2 distinguishes this from placing the counter directly).
    AdvanceCard { target: TargetSpec },
    /// CR 10.1.2: "Purge virus counters." — remove ALL virus counters hosted
    /// on cards and return them to the bank. One instruction, one occurrence:
    /// the rule names the whole board at once, so a condition looking for the
    /// purge is met once however many counters came off (and is met even if
    /// none did — the Corp purged).
    PurgeVirusCounters,
    /// "…flip this identity." (rule_identity_double_sided; Nebula class.)
    FlipIdentity(Side),
    /// "Shuffle up to 3 cards from Archives into R&D." (Jackson class;
    /// 8.1.4/1.12.3 — entering the deck makes new objects, and the shuffle
    /// follows.) The targets are announced (1.15.2).
    ShuffleCardsIntoDeck { targets: TargetSpec, to: Side },
    /// "Remove 1 card in the heap from the game." (Bloo Moose class; §4.9.)
    /// Distinct from RemoveSelfFromGame: the card is a TARGET.
    RemoveCardsFromGame { targets: TargetSpec },
    /// "Trash this card." (self-referencing; strandable per 9.1.4)
    TrashSelf,
    /// Steal the accessed agenda (7.1.4 via access step 7.2.3).
    StealSelfAgenda,
    /// CR 1.17.3 / 1.16.10c: "score this agenda" — the (S) option's effect,
    /// made an instruction so that an additional cost to score is paid in an
    /// ability frame's 9.5.7b PayCost phase and the checkpoint that follows
    /// resolves BEFORE the agenda moves to the score area.
    ScoreSelfAgenda,
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
        /// CR 1.16.2f: "…paying a total of N[credit] less" on an install-AND-
        /// rez effect. "Total" means the Corp divides the modifier between
        /// the install cost and the rez cost, declaring the split at the
        /// beginning of step 8.5.16d. A quantity position (§12 rule 6); 0 is
        /// "no total modifier", and the modifier is inert without `and_rez`,
        /// since there is no second cost to divide it with.
        reduce_total: Quantity,
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
    /// "If <state>, <effect>[. If you do not, <other effect>]." — a
    /// requirement that lives in the INSTRUCTIONS rather than in the trigger
    /// condition (9.6.5d), so it is checked when this instruction resolves
    /// and not when the condition was met.
    ///
    /// The predicate is the SAME `TriggerRequirement` vocabulary 9.6.5c uses
    /// on conditions — one state language for both places (§12 rule 2) —
    /// which is why "if you have at least 2 link" (Underworld Contact), "if
    /// the Runner is tagged" (IP Block) and "if you made a successful run
    /// this turn" (Mutual Favor) are one instruction rather than three.
    /// `otherwise` is the printed "if you do not" branch; empty where the
    /// sentence has none.
    IfMet {
        requires: Vec<crate::ability::TriggerRequirement>,
        then: Vec<Instruction>,
        otherwise: Vec<Instruction>,
    },
    /// CR 8.3.3 / 4.8.2: "set aside the top N cards of <a deck> facedown."
    /// The first half of the 8.3.3 arranging procedure, and the point at which
    /// 8.3.3b's "other effects on cards in a deck" become possible: while the
    /// cards are set aside, `TargetFilter::SetAsideByThisAbility` names them.
    SetAsideTopOfDeck { deck_of: Side, count: Quantity },
    /// CR 8.3.3: "…secretly puts them in the order of their choice, and
    /// returns them to the top of that deck." The order is a declaration, not
    /// a target announcement (nothing is chosen to be acted ON), and 8.3.3
    /// makes every returned card a NEW object (1.12.3). 8.3.1a: arranging 1
    /// or fewer cards does nothing.
    ArrangeSetAside { to_top_of: Side },
    /// The Corp rearranges (or looks at and returns) cards in R&D
    /// (Bacterial Programming class). The returned cards are NEW OBJECTS
    /// (1.12.3), so 7.4.7a's "already chosen" bookkeeping forgets them and
    /// the breach continues from the top of R&D.
    CorpRearrangesRnd,
    /// CR 8.2: "Add <cards> to the top / bottom of <a deck>." The cards are a
    /// target POSITION and the end of the deck is the content (§12 rule 2),
    /// so a Seidr-class "add a card from Archives to the top of R&D" and a
    /// Compile-class "add that program to the bottom of your stack" are one
    /// instruction. The deck is the card's OWNER's (4.2.1: a card can only
    /// ever be in its owner's deck).
    MoveToDeck { card: TargetSpec, top: bool },
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
    /// CR 1.17.3e / 1.17.3f / 10.1.3: "Add <cards> to <side>'s score area
    /// [as an agenda worth N agenda points]." An effect that DIRECTLY adds a
    /// card to a score area — the agenda (or the converted card) "is not
    /// considered scored or stolen", so nothing is recorded that a
    /// scored/stolen trigger condition could meet, and the (S) option's
    /// procedure is not involved at all.
    ///
    /// `as_agenda` carries 10.1.3's conversion: `Some(n)` converts a
    /// non-agenda into an agenda worth n agenda points (Fan Site's 0), which
    /// lasts until the card leaves the score area; `None` adds a card that is
    /// already an agenda, keeping its own value (Film Critic class).
    AddToScoreArea { cards: TargetSpec, to: Side, as_agenda: Option<i32> },
    /// "<side> trashes N random cards from their grip/HQ." (Personality
    /// Profiles class; the random selection is the mechanism 10.4.2 damage
    /// uses.) The count is a quantity position (§12 rule 6).
    TrashRandomFromHand { side: Side, count: Quantity },
    /// CR 1.13.1: "Host <cards> on <card>." Creating the host relationship
    /// moves each hosted object to the host's zone (1.13.12) without
    /// installing it (1.13.2a), and uninstalls an installed Corp card that
    /// becomes hosted on a Runner card (1.13.2b). Both positions take the
    /// shared [`TargetSpec`], so "host a card from HQ on this card" (Glenn
    /// Station class), "host 2 cards from your grip on this card" (Madani
    /// class) and "host that card on <another card>" are one instruction.
    HostCards { cards: TargetSpec, host: TargetSpec },
    /// CR 8.8.1: "Swap <a> with <b>." — the two cards exchange locations
    /// simultaneously (8.8.3/8.8.4), keeping whatever is hosted on either of
    /// them hosted on it (8.8.3a/8.8.4c).
    SwapCards { a: TargetSpec, b: TargetSpec },
    /// CR 6.2.2: "Move <ice> to <a position>." An installed piece of ice
    /// leaves its position for a new one, created when the movement happens
    /// and occupied immediately. The destination takes the shared
    /// [`InstallDest`] language — that is where the CR's position vocabulary
    /// (6.2.2a outermost, 6.2.2c directly inward) already lives — and a
    /// destination that names no position protecting a server moves nothing.
    MoveIce { ice: TargetSpec, dest: InstallDest },
    /// CR 6.2.8a: "Move to <a piece of ice>, then approach (or encounter)
    /// it." The Runner's position becomes that ice's position, the server it
    /// protects becomes the attacked server (6.1.2), and the run's current
    /// timing step becomes the Approach Ice Phase or the Encounter Ice Phase.
    /// 6.2.8c: with no position to move to — the Success Phase, the Run Ends
    /// Phase, or no run at all — the Runner does nothing.
    MoveRunnerToIce { ice: TargetSpec, encounter: bool },
    /// CR 1.14.5: "<player> does X." — an instruction naming the player who
    /// carries out the effect. By DEFAULT the ability's controller carries
    /// out every effect and makes every choice it requires (1.14.5); where
    /// the text specifies a player ("The Runner trashes 1 program"), that
    /// player carries out that part and makes its choices instead. The
    /// INSTRUCTION is the position and the named side is the content, so the
    /// whole class — Rototurret's "Trash 1 installed program" (unwrapped,
    /// Corp-carried) versus Bulwark's "The Runner trashes 1 installed
    /// program" (wrapped) — is one variant. 1.14.5a then reads the same
    /// attribution: a trigger condition about effects "performed by" a player
    /// is met only when that player carried the effect out.
    PerformedBy { side: Side, instr: Box<Instruction> },
    /// CR 10.12.1: "Sabotage N." — the Corp trashes N cards collectively from
    /// HQ and the top of R&D. The Corp chooses how many come out of HQ
    /// (10.12.2), with the 10.12.3a floor when R&D is short and the 10.12.3b
    /// everything-goes case when both are; all of them are trashed
    /// simultaneously and enter Archives facedown (10.12.2a). The count is a
    /// quantity position (§12 rule 6).
    Sabotage { count: Quantity },
    /// CR 1.9.5: "Remove N <kind> counters from <player>." (Scapegoat class
    /// for bad publicity.) Counters HOSTED on cards are not on a player, so
    /// this instruction can never reach them (1.13.3). The count is a
    /// quantity position (§12 rule 6). Only bad publicity (10.6.1) is wired:
    /// tags have their own removal path (5.2.7g / `TagRemoved`), and the
    /// other counter kinds only ever exist hosted on cards.
    RemoveCountersFromPlayer { side: Side, kind: crate::object::CounterKind, amount: Quantity },

    /// CR 9.9.6c: "the next card you install or play this turn costs N less"
    /// as an INTERRUPT (Patchwork class): a cost that would be paid while
    /// resolving an effect is a value, so the interrupt decreases it exactly
    /// as a damage prevention decreases an imminent damage value. 1.16.2a
    /// applies to the final value at the time the cost is paid, so the value
    /// is floored at 0 there.
    ReduceImminentCost { amount: Quantity },

    /// CR 9.12.2b: "<effects>, for each <quantity>" — the effects TIED to a
    /// calculated quantity. If every one of them is an aggregated class
    /// (9.12.2c) the group is performed ONCE with its values multiplied by
    /// the quantity; if any of them is not, the group is not aggregated at
    /// all and is performed once per unit, as separate occurrences that a
    /// per-occurrence trigger condition (9.6.4b) meets separately.
    ForEach { count: Quantity, effects: Vec<Instruction> },

    /// CR 10.11.2: "identify your mark." If no server is designated, a random
    /// CENTRAL server becomes the mark for the remainder of the turn
    /// (10.11.2a); if one already is, the instruction does nothing (10.11.3).
    IdentifyMark,

    /// CR 10.9.1: "load this card with N credits" — a placement of counters
    /// that also marks the kind as LOADED, which is what an "empty" ability
    /// on the same card is linked to (10.9.2).
    LoadCounters { target: TargetSpec, kind: crate::object::CounterKind, amount: Quantity },

    /// CR 10.6.1: "the Corp takes N bad publicity" — a bad publicity counter
    /// is placed on the player. The count is a quantity position (§12 rule 6).
    /// 10.6.3c: taking it during a run does not change that run's bad
    /// publicity fund, which was filled at step 6.9.1b.
    TakeBadPublicity { side: Side, amount: Quantity },

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

impl Instruction {
    /// "Make a run on <server>." — no server restriction (6.7.4a) and no
    /// "If successful" clause (6.7.4).
    pub fn run(server: ServerId) -> Instruction {
        Instruction::InitiateRun {
            server: Some(server),
            allowed: RunServerSet::Any,
            if_successful: Vec::new(),
        }
    }

    /// "Run any server." — CR 6.7.4a's unrestricted set, with the attacked
    /// server announced by the Runner at step 6.9.1a.
    pub fn run_any_server(if_successful: Vec<Instruction>) -> Instruction {
        Instruction::InitiateRun {
            server: None,
            allowed: RunServerSet::Any,
            if_successful,
        }
    }
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
        /// CR 6.7.4c: the replacement is one its controller may OPTIONALLY
        /// carry out, so applying it is a Decision — made where the replaced
        /// effect would happen (for a breach, step 6.9.5b).
        optional: bool,
    },
    /// "Access N additional cards from <server>." (The Maker's Eye class;
    /// added to the 7.3.6 access limit at breach step 7.5.3.)
    AdditionalAccess { server: ServerId, extra: u32 },
    /// CR 9.5.3a: "the Runner cannot use <targets>' abilities" for the
    /// duration (Wendigo class).
    CannotUseAbilitiesOf(TargetSpec),
    /// CR 7.4.2b: "the Runner cannot access more than N cards during this
    /// run" (Hudson 1.0 class). The bound is a quantity position (§12 rule 6)
    /// evaluated when the effect is created.
    AccessLimit { limit: Quantity },
}

/// CR 9.8.2/9.8.3: what an ability grants when it grants subroutines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubroutineGrant {
    /// "…gains \"[subroutine] …\"", N copies of one stated subroutine.
    Stated { count: u32, sub: Box<crate::ability::AbilityDef> },
    /// "…gains the subroutines of that ice" (Loki class): ONE effect grants
    /// several subroutines at once, and 9.8.3a orders them among themselves
    /// in the order they have on the card they were copied from.
    CopiedFrom(TargetSpec),
}

/// CR 9.10.3: what a maintained choice is a choice OF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceSpec {
    /// "…this server." (Security Testing class.) A choice BETWEEN servers is
    /// 9.11.4g's option choice — an `Instruction::ChooseOne` whose branches
    /// each maintain a different one — so this variant names one server.
    Server(ServerId),
    /// "…choose an installed piece of ice." (Femme Fatale class.) The pick is
    /// a 1.15.2 target announcement over the shared criteria vocabulary.
    Object(TargetSpec),
    /// "…this ice subtype." (Pelangi class.) As with `Server`, the choice
    /// between subtypes is an `Instruction::ChooseOne`.
    Subtype(&'static str),
}

/// CR 9.12.3a/b: how a "must trash" requirement may be satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashMeans {
    /// 9.12.3a: no means stipulated — any mid-access ability whose resolution
    /// trashes the accessed card satisfies the requirement, and the Runner is
    /// forced to use one if they can (Mumbad Virtual Tour class).
    AnyAbility,
    /// 9.12.3b: "…if you can pay its trash cost" — only the basic trash
    /// ability (7.1.5) satisfies the requirement, so an ability that would
    /// trash the card by some other means cannot be forced (Neutralize All
    /// Threats class).
    PayingTheTrashCost,
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
    /// CR 9.10.3: the object remembered by this source's maintained choice
    /// under `key` (Femme Fatale's "that ice"). Resolves to nothing when no
    /// such choice is being maintained.
    MaintainedChoice(&'static str),
    /// The ice currently being encountered (Forked class).
    EncounteredIce,
    /// Chosen by the controller at announce time from the shared filter
    /// vocabulary. Several atoms combine as a conjunction, exactly as a
    /// search's 8.7.2a criteria do — "an installed program" is
    /// `[InstalledRunnerCard, CardTypeIs(Program)]`, and a plain
    /// "1 program" is `[CardTypeIs(Program)]` with 1.15.2c supplying the
    /// play-area restriction.
    ///
    /// `count` is a quantity POSITION (§12 rule 6): "trash 1 program" is a
    /// constant, "trash X programs, where X is the number of advancement
    /// counters on this card" (Aggressive Secretary class) is a selector.
    /// CR 1.15.2e caps it at the number of distinct valid targets available.
    Choose { count: Quantity, criteria: Vec<TargetFilter>, up_to: bool },
    /// The top N cards of a deck (Breached Dome-style).
    TopOfDeck(Side, u32),
    /// CR 8.7.4: the cards found by this ability's search, still set aside
    /// facedown (4.8.4). This is how an install/play/add-to-hand instruction
    /// refers to them without a per-card hook.
    FoundBySearch,
    /// CR 1.15.2: ONE instruction that requires SEVERAL announcements —
    /// "Trash 1 installed program and 1 installed resource" (Colossus
    /// class). Each element is announced in turn, as its own Decision, and
    /// the instruction acts on the union when it resolves (9.12.2a: one
    /// effect over a set).
    Each(Vec<TargetSpec>),
    /// CR 1.15.4: a target this ABILITY already announced, addressed by a
    /// later instruction without re-announcing it (Howler class). `nth` is
    /// 0-based over the ability's announcements in order.
    EarlierTarget { nth: usize },
}

/// Which subroutines of the encountered ice a break ability acts on
/// (§9.8.6). The count is a quantity position (§12 rule 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubroutineSpec {
    /// "Break up to N subroutines." (Cleaver class.) CR 9.8.6: only
    /// UNBROKEN subroutines can be chosen; `up_to` is the "up to" of the
    /// printed text, since 1.15.2e otherwise forces as many as possible.
    Chosen { count: Quantity, up_to: bool },
    /// CR 9.8.6a: "break all subroutines" targets nothing at all, and can
    /// be used while the ice has at least 1 unbroken subroutine.
    All,
    /// CR 9.8.6b: "break all but N subroutines" — the announced targets are
    /// the ones that will NOT be broken, so already-broken subroutines are
    /// valid targets (Grappling Hook).
    AllBut { count: Quantity },
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
    /// CR 6.1.2: ice protecting the server currently under attack (Na'Not'K
    /// class). Empty when no run is in progress, which is what makes the
    /// modification it feeds lapse the moment the run ends.
    IceProtectingAttackedServer,
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
    /// CR 8.1.2: "a rezzed piece of ice", "a rezzed card" — an installed
    /// faceup Corp card.
    Rezzed,
    /// CR 9.5.5 / 4.8.3: a card SET ASIDE by the trigger cost of the ability
    /// making this selection — the only kind of ability that can see the
    /// set-aside zone at all (Street Peddler class). A zone-naming criterion,
    /// so 1.15.2c's play-area restriction lifts for it.
    SetAsideByThisAbility,
    /// CR 8.4.2a: the cards a player has DRAWN and that are still set aside —
    /// "abilities with a trigger condition that refers to cards being drawn
    /// can see the drawn cards in the set-aside zone. This is an exception to
    /// rule 4.8.3." A zone-naming criterion, so 1.15.2c's play-area
    /// restriction lifts for it.
    DrawnCards,
    /// CR 1.12.3 / 1.21.2: a card THIS ability is currently looking at. An
    /// entry whose object has been re-made — a shuffle or a rearrangement
    /// moves cards to an unknown location, and 1.12.3 makes each a NEW object
    /// — no longer matches, so the ability can no longer act on it. A
    /// zone-naming criterion, so 1.15.2c's play-area restriction lifts.
    LookedAtByThisAbility,
    /// CR 4.5: "an agenda in the Runner's score area".
    InScoreAreaOf(Side),
    /// CR 4.4: "a card in Archives" / "a card in your heap" — a criterion
    /// that names a HIDDEN-capable zone, which is what makes 4.1.2a's reveal
    /// necessary when the criteria also stipulate a characteristic.
    InDiscardOf(Side),
    /// "…a card in the root of another server" (Pinhole class): installed in
    /// the root of a server OTHER than the attacked one.
    InRootOfServerOtherThanAttacked,
    /// CR 4.2.2: "1 of the top N cards of R&D" (Top Hat class) — a criterion
    /// that explicitly specifies the zone, which is what lets 1.15.2c's
    /// play-area restriction lift for it.
    TopOfDeckOf { side: Side, n: u32 },
    /// CR 6.2.3: "a piece of ice in the same position" — ice whose server has
    /// the same number of positions inward from it as the reference position
    /// has from its own. Which position is the reference is the content
    /// (§12 rule 2), so the Rook class and the Slipstream class are one atom.
    IceInSamePositionAs(PositionRef),
    /// CR 2.1 / 10.1.5: "a copy of <name>" — a card's NAME, which is not
    /// self-referential language: it describes every card with that name,
    /// including but not limited to the ability's own source.
    HasName(&'static str),
    /// "this card" as a criterion — the ability's own source, and only it.
    /// Self-referential language (10.1.4), the complement of `OtherThanSource`.
    IsSource,
    /// CR 1.13.2: "cards hosted on this card" — the source's hosted cards,
    /// installed or not (1.13.2a).
    HostedOnSource,
    /// CR 1.12.6: "a card you did not install this turn" (Seamless Launch) /
    /// "an agenda installed during this turn" (Clot's prohibition). A GAME
    /// HISTORY query, not a state one — the change log since the turn began,
    /// which 10.2.1 makes open information to both players. The polarity is
    /// content (§12 rule 2), so one atom says both sentences.
    InstalledThisTurn(bool),
    /// CR 1.18.3: "a card you can advance" (AstroScript Pilot Program, Slot
    /// Machine). The PERMISSION side of advancing — an agenda always, and any
    /// other installed card while an active ability says so — read through
    /// the same derivation the basic advance action uses, so the criterion
    /// and the action can never disagree. Names the play area (1.15.2c).
    CanBeAdvanced,
    /// "each **other** rezzed piece of ice", "another installed program" —
    /// the word "other" in a description, which excludes the ability's own
    /// source from the set it describes (Mother Goddess and Warden Fatuma
    /// both need it, and a swap that must name "another piece of ice" is the
    /// same atom — see deviation 30).
    OtherThanSource,
}

impl TargetFilter {
    /// CR 4.1.2a: does this criterion stipulate a CHARACTERISTIC of the card
    /// (rather than merely where it is)? A stipulation of this kind has to be
    /// demonstrated when the chosen card is not otherwise visible, which is
    /// what forces the reveal.
    pub fn stipulates_characteristic(self) -> bool {
        matches!(
            self,
            TargetFilter::CardTypeIs(_)
                | TargetFilter::HasSubtype(_)
                | TargetFilter::PrintedCostAtMost(_)
                | TargetFilter::HasName(_)
        )
    }
}

/// CR 6.2.3: what a "same position" criterion is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionRef {
    /// The position of the ability's source — or of its HOST, when the source
    /// is hosted on a piece of ice (6.2.1a: a hosted card occupies no position
    /// of its own, so a hosted program's "same position" reads its host's).
    Source,
    /// The Runner's current position in the run (6.2.5).
    Runner,
}

impl TargetFilter {
    /// CR 1.15.2c: does this criterion "explicitly specify the zone from
    /// which an object must be selected"? When NO criterion of an
    /// announcement does, only installed cards (and counters in the play
    /// area) are valid targets — "the Runner trashes 1 program" cannot
    /// reach the grip or the stack.
    ///
    /// The installed-ness atoms are listed too: they already restrict to
    /// the play area, so the implicit restriction is a no-op for them.
    pub fn names_zone(self) -> bool {
        matches!(
            self,
            TargetFilter::InstalledCorpCard
                | TargetFilter::InstalledRunnerCard
                | TargetFilter::InstalledResource
                | TargetFilter::Rezzed
                | TargetFilter::IceProtectingSourceServer
                | TargetFilter::IceProtectingAttackedServer
                | TargetFilter::CardsInHandOf(_)
                | TargetFilter::InScoreAreaOf(_)
                | TargetFilter::InDiscardOf(_)
                | TargetFilter::SetAsideByThisAbility
                | TargetFilter::DrawnCards
                | TargetFilter::LookedAtByThisAbility
                | TargetFilter::TopOfDeckOf { .. }
                // 6.2.1: only ice PROTECTING a server occupies a position, so
                // this criterion already names the play area.
                | TargetFilter::IceInSamePositionAs(_)
                // 1.18.3: only an INSTALLED card can be advanced, so this
                // criterion already names the play area.
                | TargetFilter::CanBeAdvanced
                // 1.13.2: a host relationship IS a location — "an agenda
                // hosted on Film Critic" says where the card is as precisely
                // as "a card in HQ" does. Without this the restriction never
                // lifts for a card that is hosted but NOT installed (Film
                // Critic's agenda, Bookmark's facedown cards), and such a
                // card could never be targeted by the very ability that put
                // it there.
                | TargetFilter::HostedOnSource
        )
    }
}

/// CR 8.2.2 / 9.9.8b: where a trashed card goes when a replacement effect has
/// modified the trash movement without replacing it by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashDestination {
    /// CR 4.9: the removed-from-game zone (Skorpios class).
    RemovedFromGame,
    /// CR 8.1.4/8.1.4d: the installed Runner card is turned facedown and
    /// stays where it is — "a Runner card turned facedown is not considered
    /// to be uninstalled and simply remains in the play area" (Harbinger
    /// class).
    FacedownInPlay,
}

/// CR 8.5.16b: the install destination, declared as part of installing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDest {
    /// Corp: the root of an existing server (8.5.2b/c).
    Root(ServerId),
    /// Corp: create a new remote server (8.5.2a).
    NewRemoteRoot,
    /// Corp: ice protecting a NEW remote server (8.5.2a + 8.5.2d) — the
    /// server is created at step 8.5.16e, exactly as for `NewRemoteRoot`.
    NewRemoteProtecting,
    /// Corp: protecting a server, outermost position (8.5.2d).
    Protecting(ServerId),
    /// "directly inward" from the ability's source ice (Brân class). If the
    /// source is not protecting a server, the destination cannot be
    /// identified and no installation takes place (8.5.14).
    InwardFromSource,
    /// Runner: the rig (8.5.4).
    Rig,
    /// Hosted on a specific card (8.5.1a). The card is installed into the
    /// host's zone (1.13.12).
    HostedOn(ObjectId),
    /// The root of the server currently being breached (Ganked/Drafter
    /// class; resolved when the destination is declared).
    BreachedServerRoot,
    /// CR 8.5.16b: the effect states NO destination, so the installing player
    /// chooses and declares one at step 8.5.16b — every location the card
    /// could legally occupy, "including any host relationships". This is the
    /// destination of the basic install action (5.2.6d/5.2.7d), where the
    /// player picks the server; `Vm::install_destinations_for` computes the
    /// list and the answer replaces this variant with the declared one.
    DeclaredByInstaller,
    /// Runner installs with no stated destination: the rig (8.5.4). Named
    /// for the 1.13.6a choice every install offers — a card whose ability
    /// describes what it can host is an eligible destination, so the
    /// installer is asked to pick one or take the default. That choice is
    /// NOT special to this variant: it is offered for every destination
    /// (see `Vm::eligible_hosts_for`).
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

/// CR 9.5.6a: "A paid ability that contains an instruction that could break 1
/// or more subroutines can only be used during an encounter."
///
/// The restriction is a property of the INSTRUCTIONS, not of the card, so it
/// is derived here and applied wherever a paid ability is offered. A card that
/// additionally *refers* to a stated piece of ice ("break 1 **barrier**
/// subroutine") carries 9.5.6c's separate restriction as a
/// [`crate::ability::TimingRestriction`]; this one holds even for a card that
/// names no ice at all.
pub fn could_break_subroutines(instrs: &[Instruction]) -> bool {
    cite!("rule_paid_ability_breaks_subroutines");
    instrs.iter().any(|i| match i {
        Instruction::BreakSubroutines { .. } => true,
        Instruction::PerformedBy { instr, .. } | Instruction::DeclineableChoice(instr) => {
            could_break_subroutines(std::slice::from_ref(instr))
        }
        Instruction::NestedCostThen { effect, .. } | Instruction::NestedCostUnless { effect, .. } => {
            could_break_subroutines(std::slice::from_ref(effect))
        }
        Instruction::Combined(list) => could_break_subroutines(list),
        Instruction::ChooseOne { options } => {
            options.iter().any(|(_, is)| could_break_subroutines(is))
        }
        _ => false,
    })
}

/// CR 7.1.5b: "The Runner cannot trash or pay the trash cost of a card in the
/// Corp's discard pile, either with the basic trash ability or with other
/// mid-access abilities."
///
/// The "other mid-access abilities" half needs to know which abilities would
/// trash the card being accessed, which — like [`could_break_subroutines`] —
/// is a property of the instructions rather than of the card.
pub fn could_trash_accessed_card(instrs: &[Instruction]) -> bool {
    cite!("rule_trash_in_archives");
    instrs.iter().any(|i| match i {
        Instruction::TrashCards(TargetSpec::AccessedCard) => true,
        Instruction::MustTrashAccessedCard { .. } => true,
        Instruction::PerformedBy { instr, .. } | Instruction::DeclineableChoice(instr) => {
            could_trash_accessed_card(std::slice::from_ref(instr))
        }
        Instruction::NestedCostThen { effect, .. } | Instruction::NestedCostUnless { effect, .. } => {
            could_trash_accessed_card(std::slice::from_ref(effect))
        }
        Instruction::Combined(list) => could_trash_accessed_card(list),
        Instruction::ChooseOne { options } => {
            options.iter().any(|(_, is)| could_trash_accessed_card(is))
        }
        _ => false,
    })
}
