//! Plans: the player seam as DATA (ARCHITECTURE §12 rule 5).
//!
//! A [`Plan`] is an ordered list of `when <`[`Match`]`> → <`[`Reply`]`>` rules
//! plus a fallback policy — pure data, no closures, so it can be inspected,
//! diffed and (later) serialised for replay. ONE driver ([`Script`]) folds
//! `Vm × Plan(Corp) × Plan(Runner) → `[`Transcript`], answering every
//! [`DecisionSpec`] the coroutine yields. Hand-rolled `while vm.step()` loops
//! in tests are the named defect that this module retires: a test declares
//! setup, two plans, and assertions, and the driver is shared.
//!
//! The player algebra now has three honest interpreters — the scripted plan
//! here, `default_answer`'s neutral policy, and (at cutover) the server/human
//! driver. That is the final-tagless boundary done at a seam with real second
//! interpreters, rather than as ceremony (§12).
//!
//! Rules are tried in order and the FIRST applicable rule answers; a rule
//! whose [`Ordinal`] is exhausted falls through to the next rule (and finally
//! to the fallback), which is exactly the `if !already_used { … } else { … }`
//! shape the hand-rolled loops used.
//!
//! Everything the machine offered is recorded in the [`Transcript`], so
//! assertions about *what was offered* — the count of pending instances in a
//! reaction window, "Tori was never usable" — are made after the fold instead
//! of from inside a loop. That is strictly stronger: the loop could only see
//! what it thought to look at.

use crate::decision::{
    ActionOption, DecisionAnswer, DecisionSpec, GameResult, WindowOption, Yield,
};
use crate::frames::Frame;
use crate::object::{ObjectId, ServerId, Side};
use crate::timing::StructKind;
use crate::vm::Vm;

// ---------------------------------------------------------------------------
// Decision kinds
// ---------------------------------------------------------------------------

/// The decision families a [`Match`] can key on — the five priority windows
/// (9.2.5) plus the non-window choices the instruction vocabulary asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Mulligan,
    /// 9.2.6 action window.
    Action,
    /// 9.2.7 paid ability window.
    Paid,
    /// 9.2.8 reaction window.
    Reaction,
    /// 9.2.9 interrupt window.
    Interrupt,
    /// 9.2.10 mid-access window.
    MidAccess,
    /// 9.3.4b target announcement.
    Targets,
    /// 9.11.4g optioned effects.
    Options,
    /// 9.11.4f / 1.16.10-11 nested or additional cost.
    NestedCost,
    /// 9.6.9c optional part.
    Optional,
    /// 11.5 step 4a candidate choice.
    Candidate,
    /// 10.3.1j breach-candidacy declaration.
    DeclareCandidate,
    /// 6.9.4c jack out.
    JackOut,
    /// 5.5.4c discard to hand size.
    Discard,
    /// 10.3.1e minimal appropriate set.
    MinimalSet,
    /// 10.8.6c/d open trace spend.
    TraceSpend,
    /// 10.14.6b sealed psi bid.
    PsiBid,
}

impl Kind {
    pub fn of(spec: &DecisionSpec) -> Kind {
        cite!("rule_priority_window_types");
        match spec {
            DecisionSpec::Mulligan => Kind::Mulligan,
            DecisionSpec::TakeAction { .. } => Kind::Action,
            DecisionSpec::PaidWindow { .. } => Kind::Paid,
            DecisionSpec::ReactionWindow { .. } => Kind::Reaction,
            DecisionSpec::InterruptWindow { .. } => Kind::Interrupt,
            DecisionSpec::MidAccessWindow { .. } => Kind::MidAccess,
            DecisionSpec::ChooseTargets { .. } => Kind::Targets,
            DecisionSpec::ChooseOption { .. } => Kind::Options,
            DecisionSpec::NestedCost { .. } => Kind::NestedCost,
            DecisionSpec::OptionalEffect { .. } => Kind::Optional,
            DecisionSpec::ChooseCandidate { .. } => Kind::Candidate,
            DecisionSpec::DeclareBreachCandidate { .. } => Kind::DeclareCandidate,
            DecisionSpec::JackOut => Kind::JackOut,
            DecisionSpec::DiscardCards { .. } => Kind::Discard,
            DecisionSpec::MinimalSet { .. } => Kind::MinimalSet,
            DecisionSpec::TraceSpend { .. } => Kind::TraceSpend,
            DecisionSpec::PsiBid { .. } => Kind::PsiBid,
        }
    }
}

// ---------------------------------------------------------------------------
// Option selectors
// ---------------------------------------------------------------------------

/// Names ONE offered option, in a window or an action window, as data. The
/// same selector language does double duty: it is both how a rule *matches*
/// ("when a window offering `decoy` opens") and how a rule *answers* ("take
/// `decoy`") — §12 rule 6's discipline applied to option positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// Any labelled option (pending instance, paid ability, card action)
    /// whose label contains this needle.
    Labeled(&'static str),
    /// (R) rez this card (9.2.7c).
    Rez(ObjectId),
    /// 9.2.7e: rez the approached ice (whichever it is).
    RezApproachedIce,
    /// (S) score this card (9.2.7d).
    Score(ObjectId),
    /// 7.1.5 basic trash of the accessed card.
    BasicTrash,
    /// The first pending instance whose controller must trigger it (9.2.8e).
    Mandatory,
    /// The nth offered option, 0-based (the neutral "just do something").
    Index(usize),
    /// 5.2.6b/5.2.7b basic credit action.
    Credit,
    /// 5.2.6c/5.2.7c basic draw action.
    Draw,
    /// 5.2.7f basic run on this server.
    Run(ServerId),
    /// 5.2.7g basic remove-tag action.
    RemoveTag,
}

/// Does a window option carry a label containing `needle`?
pub fn labelled(o: &WindowOption, needle: &str) -> bool {
    match o {
        WindowOption::TriggerInstance { label, .. } | WindowOption::TriggerPaid { label, .. } => {
            label.contains(needle)
        }
        _ => false,
    }
}

/// How many of these options carry a label containing `needle` — the
/// multiplicity assertions (9.6.4b one instance per occurrence) read this.
pub fn count_labelled(options: &[WindowOption], needle: &str) -> usize {
    options.iter().filter(|o| labelled(o, needle)).count()
}

impl Pick {
    /// Resolve against a priority window's options.
    pub fn find_window(&self, options: &[WindowOption]) -> Option<WindowOption> {
        match self {
            Pick::Labeled(n) => options.iter().find(|o| labelled(o, n)).cloned(),
            Pick::Rez(c) => options
                .iter()
                .find(|o| matches!(o, WindowOption::Rez { card } if card == c))
                .cloned(),
            Pick::RezApproachedIce => options
                .iter()
                .find(|o| matches!(o, WindowOption::RezApproachedIce { .. }))
                .cloned(),
            Pick::Score(c) => options
                .iter()
                .find(|o| matches!(o, WindowOption::Score { card } if card == c))
                .cloned(),
            Pick::BasicTrash => options
                .iter()
                .find(|o| matches!(o, WindowOption::BasicTrash { .. }))
                .cloned(),
            Pick::Mandatory => options
                .iter()
                .find(|o| matches!(o, WindowOption::TriggerInstance { mandatory: true, .. }))
                .cloned(),
            Pick::Index(i) => options.get(*i).cloned(),
            Pick::Credit | Pick::Draw | Pick::Run(_) | Pick::RemoveTag => None,
        }
    }

    /// Resolve against an action window's options (5.2).
    pub fn find_action(&self, options: &[ActionOption]) -> Option<ActionOption> {
        match self {
            Pick::Credit => options.iter().find(|o| **o == ActionOption::BasicCredit).cloned(),
            Pick::Draw => options.iter().find(|o| **o == ActionOption::BasicDraw).cloned(),
            Pick::RemoveTag => {
                options.iter().find(|o| **o == ActionOption::BasicRemoveTag).cloned()
            }
            Pick::Run(s) => options
                .iter()
                .find(|o| matches!(o, ActionOption::BasicRun { server } if server == s))
                .cloned(),
            Pick::Labeled(n) => options
                .iter()
                .find(|o| matches!(o, ActionOption::CardAction { label, .. } if label.contains(n)))
                .cloned(),
            Pick::Index(i) => options.get(*i).cloned(),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Matchers
// ---------------------------------------------------------------------------

/// How often a rule applies once its shape matches.
///
/// The count is PER RULE and only advances on decisions the driver actually
/// evaluates this rule against: rules are tried in order and the first
/// applicable one answers, so a decision claimed by an earlier rule never
/// reaches — and never counts towards — a later one. "The second action
/// window" therefore means `nth(2)` only when no earlier rule consumed an
/// action window; where one did, the later rule wants `once()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ordinal {
    /// Every matching decision.
    #[default]
    Every,
    /// Only the nth matching decision (1-based) — "the second PaidWindow".
    Nth(usize),
    /// The first n matching decisions.
    UpTo(usize),
}

/// The decision shape a rule keys on. Every field is an independent
/// conjunct; `None`/`false` means "don't care".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Match {
    /// Decision family (None = any).
    pub kind: Option<Kind>,
    /// The decision must offer this option.
    pub offers: Option<Pick>,
    /// A structure of this kind must be in progress (anywhere on the frame
    /// stack): "while a run is in progress", "during a breach".
    pub during: Option<StructKind>,
    /// No structure of this kind may be in progress.
    pub outside: Option<StructKind>,
    /// The innermost structure frame is at this §11 step id.
    pub at_step: Option<&'static str>,
    /// Paid window with the approach-ice class open (9.2.7e).
    pub approaching_ice: bool,
    /// Paid window with the (S) class open (9.2.7d).
    pub can_score: bool,
    pub ordinal: Ordinal,
}

impl Match {
    pub fn any() -> Match {
        Match::default()
    }
    fn of(kind: Kind) -> Match {
        Match { kind: Some(kind), ..Match::default() }
    }
    pub fn mulligan() -> Match {
        Match::of(Kind::Mulligan)
    }
    pub fn action() -> Match {
        Match::of(Kind::Action)
    }
    pub fn paid() -> Match {
        Match::of(Kind::Paid)
    }
    pub fn reaction() -> Match {
        Match::of(Kind::Reaction)
    }
    pub fn interrupt() -> Match {
        Match::of(Kind::Interrupt)
    }
    pub fn mid_access() -> Match {
        Match::of(Kind::MidAccess)
    }
    pub fn targets() -> Match {
        Match::of(Kind::Targets)
    }
    pub fn options() -> Match {
        Match::of(Kind::Options)
    }
    pub fn nested_cost() -> Match {
        Match::of(Kind::NestedCost)
    }
    pub fn optional() -> Match {
        Match::of(Kind::Optional)
    }
    pub fn candidate() -> Match {
        Match::of(Kind::Candidate)
    }
    pub fn declare_candidate() -> Match {
        Match::of(Kind::DeclareCandidate)
    }
    pub fn jack_out() -> Match {
        Match::of(Kind::JackOut)
    }
    pub fn discard() -> Match {
        Match::of(Kind::Discard)
    }
    pub fn minimal_set() -> Match {
        Match::of(Kind::MinimalSet)
    }
    pub fn trace_spend() -> Match {
        Match::of(Kind::TraceSpend)
    }
    pub fn psi_bid() -> Match {
        Match::of(Kind::PsiBid)
    }
    /// Any priority window (the five 9.2.5 kinds).
    pub fn window() -> Match {
        Match::any()
    }

    pub fn offering(mut self, needle: &'static str) -> Match {
        self.offers = Some(Pick::Labeled(needle));
        self
    }
    pub fn offering_pick(mut self, p: Pick) -> Match {
        self.offers = Some(p);
        self
    }
    pub fn during(mut self, k: StructKind) -> Match {
        self.during = Some(k);
        self
    }
    pub fn outside(mut self, k: StructKind) -> Match {
        self.outside = Some(k);
        self
    }
    pub fn at_step(mut self, id: &'static str) -> Match {
        self.at_step = Some(id);
        self
    }
    pub fn approaching_ice(mut self) -> Match {
        self.approaching_ice = true;
        self
    }
    pub fn can_score(mut self) -> Match {
        self.can_score = true;
        self
    }
    /// Only the nth matching decision (1-based).
    pub fn nth(mut self, n: usize) -> Match {
        self.ordinal = Ordinal::Nth(n);
        self
    }
    /// At most the first n matching decisions.
    pub fn times(mut self, n: usize) -> Match {
        self.ordinal = Ordinal::UpTo(n);
        self
    }
    /// Exactly the first matching decision.
    pub fn once(self) -> Match {
        self.times(1)
    }
    pub fn first(self) -> Match {
        self.nth(1)
    }

    /// Shape test, ignoring the ordinal (which is counted by the driver).
    fn shape_matches(&self, ctx: &Context, spec: &DecisionSpec, reply: &Reply) -> bool {
        if let Some(k) = self.kind {
            if k != Kind::of(spec) {
                return false;
            }
        }
        if let Some(k) = self.during {
            if !ctx.stack.contains(&k) {
                return false;
            }
        }
        if let Some(k) = self.outside {
            if ctx.stack.contains(&k) {
                return false;
            }
        }
        if let Some(id) = self.at_step {
            if ctx.step.as_deref() != Some(id) {
                return false;
            }
        }
        if self.approaching_ice || self.can_score {
            match spec {
                DecisionSpec::PaidWindow { classes, .. } => {
                    if self.approaching_ice && !classes.rez_approached_ice {
                        return false;
                    }
                    if self.can_score && !classes.score {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        // An explicit `offering` guard, or the implicit one carried by a
        // `Take` reply: "take X" applies exactly where X is on offer.
        let guard = self.offers.clone().or(match reply {
            Reply::Take(p) => Some(p.clone()),
            _ => None,
        });
        if let Some(p) = guard {
            return offer_present(spec, &p);
        }
        true
    }
}

fn offer_present(spec: &DecisionSpec, p: &Pick) -> bool {
    match spec {
        DecisionSpec::TakeAction { options } => p.find_action(options).is_some(),
        _ => p.find_window(window_options(spec)).is_some(),
    }
}

/// The options of any priority-window decision (empty for the rest).
pub fn window_options(spec: &DecisionSpec) -> &[WindowOption] {
    match spec {
        DecisionSpec::PaidWindow { options, .. }
        | DecisionSpec::ReactionWindow { options, .. }
        | DecisionSpec::InterruptWindow { options, .. }
        | DecisionSpec::MidAccessWindow { options } => options,
        _ => &[],
    }
}

/// The labelled options of a 9.11.4g optioned-effect decision.
pub fn choices(spec: &DecisionSpec) -> &[&'static str] {
    match spec {
        DecisionSpec::ChooseOption { options } => options,
        _ => &[],
    }
}

/// The options of an action-window decision (empty for the rest).
pub fn action_options(spec: &DecisionSpec) -> &[ActionOption] {
    match spec {
        DecisionSpec::TakeAction { options } => options,
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Replies
// ---------------------------------------------------------------------------

/// What the plan answers with. `Take` covers both priority-window options and
/// action-window actions; the driver dispatches on the decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// Take the named option. Carries an implicit match guard: the rule only
    /// applies where the option is actually offered.
    Take(Pick),
    /// 9.2.4b: decline to act. Where the CR forbids passing (9.2.8e mandatory
    /// pendings) the obligation is discharged first.
    Pass,
    /// The neutral policy: pass/decline where legal, discharge obligations
    /// otherwise. The second interpreter of the player algebra.
    Default,
    Targets(Vec<ObjectId>),
    /// 9.11.4g: choose the nth option.
    Choose(usize),
    /// 9.11.4g / 9.9.11: choose the option whose label contains this needle
    /// (option lists are labelled, e.g. the replacement-ordering Decision).
    ChooseNamed(&'static str),
    /// 1.16.10-11: pay (true) or decline (false).
    PayCost(bool),
    /// 9.6.9c: resolve (true) or decline (false) an optional part; also
    /// 10.3.1j candidacy declaration.
    Optional(bool),
    Candidate(ObjectId),
    JackOut(bool),
    Discard(Vec<ObjectId>),
    ChooseSet(usize),
    /// 10.8.6c/d: spend n credits openly.
    Spend(u32),
    /// 10.14.6b: bid n.
    Bid(u32),
    Keep,
    Mulligan,
    /// Suspend the driver here, leaving the decision UNANSWERED so the test
    /// can assert mid-flight and resume with `Script::run` again.
    Halt,
    /// Declare this decision impossible: reaching it fails the test.
    Forbid,
}

impl Reply {
    pub fn take(needle: &'static str) -> Reply {
        Reply::Take(Pick::Labeled(needle))
    }
    pub fn run(server: ServerId) -> Reply {
        Reply::Take(Pick::Run(server))
    }
    pub fn credit() -> Reply {
        Reply::Take(Pick::Credit)
    }
    pub fn draw() -> Reply {
        Reply::Take(Pick::Draw)
    }
    pub fn rez(card: ObjectId) -> Reply {
        Reply::Take(Pick::Rez(card))
    }
    pub fn score(card: ObjectId) -> Reply {
        Reply::Take(Pick::Score(card))
    }
    pub fn trash_accessed() -> Reply {
        Reply::Take(Pick::BasicTrash)
    }
    pub fn target(obj: ObjectId) -> Reply {
        Reply::Targets(vec![obj])
    }
}

/// One `when → then` rule. Data: no closures, no interior state (the
/// ordinal bookkeeping lives in the driver).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub when: Match,
    pub then: Reply,
}

/// One player's plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub side: Side,
    pub rules: Vec<Rule>,
    /// Answer for decisions no rule claims.
    pub fallback: Reply,
}

impl Plan {
    pub fn for_side(side: Side) -> Plan {
        Plan { side, rules: Vec::new(), fallback: Reply::Default }
    }
    /// The Corp's plan: neutral by default (pass, decline, discharge).
    pub fn corp() -> Plan {
        Plan::for_side(Side::Corp)
    }
    /// The Runner's plan: neutral by default.
    pub fn runner() -> Plan {
        Plan::for_side(Side::Runner)
    }
    /// Append a rule (order is significant: first applicable rule answers).
    pub fn when(mut self, when: Match, then: Reply) -> Plan {
        self.rules.push(Rule { when, then });
        self
    }
    /// Stop the driver at the first action window this player is given —
    /// the end-of-script marker most example tests want ("play out the
    /// consequences, then let me assert").
    pub fn stop_at_action(self) -> Plan {
        self.when(Match::action(), Reply::Halt)
    }
    /// Stop at the nth action window (1-based).
    pub fn stop_at_action_nth(self, n: usize) -> Plan {
        self.when(Match::action().nth(n), Reply::Halt)
    }
    /// Spend every action window on a basic credit (the "drain the turn"
    /// idiom): appended after the plan's own action rules.
    pub fn otherwise_click_credit(self) -> Plan {
        self.when(Match::action(), Reply::credit())
    }
    /// Any decision this plan does not claim fails the test — the strict
    /// script discipline the playable slice asserts with.
    pub fn forbidding_the_rest(mut self) -> Plan {
        self.fallback = Reply::Forbid;
        self
    }
    /// Any unclaimed decision stops the driver.
    pub fn stopping_at_the_rest(mut self) -> Plan {
        self.fallback = Reply::Halt;
        self
    }
    pub fn with_fallback(mut self, r: Reply) -> Plan {
        self.fallback = r;
        self
    }
}

// ---------------------------------------------------------------------------
// Transcript
// ---------------------------------------------------------------------------

/// One recorded interaction: what the machine asked, in what context, and how
/// the plan answered.
#[derive(Debug, Clone)]
pub struct Entry {
    pub seq: usize,
    pub side: Side,
    pub spec: DecisionSpec,
    /// `None` when the plan halted here (the decision is still pending).
    pub answer: Option<DecisionAnswer>,
    /// Index of the rule that answered, if any.
    pub rule: Option<usize>,
    /// Timing structures in progress, innermost first.
    pub stack: Vec<StructKind>,
    /// §11 step id of the innermost structure frame.
    pub step: Option<String>,
}

impl Entry {
    pub fn kind(&self) -> Kind {
        Kind::of(&self.spec)
    }
    pub fn options(&self) -> &[WindowOption] {
        window_options(&self.spec)
    }
    pub fn actions(&self) -> &[ActionOption] {
        action_options(&self.spec)
    }
    /// The labelled options of a 9.11.4g optioned-effect decision.
    pub fn choices(&self) -> &[&'static str] {
        choices(&self.spec)
    }
    /// The objects put to the player at a target/candidate choice.
    pub fn candidates(&self) -> &[ObjectId] {
        match &self.spec {
            DecisionSpec::ChooseTargets { candidates, .. }
            | DecisionSpec::ChooseCandidate { candidates } => candidates,
            DecisionSpec::DiscardCards { hand, .. } => hand,
            _ => &[],
        }
    }
    /// The cost put to the player at a nested/additional-cost decision.
    pub fn cost(&self) -> Option<&crate::ability::Cost> {
        match &self.spec {
            DecisionSpec::NestedCost { cost } => Some(cost),
            _ => None,
        }
    }
    /// How many offered options carry this label (9.6.4b multiplicity).
    pub fn count(&self, needle: &str) -> usize {
        count_labelled(self.options(), needle)
    }
    pub fn offered(&self, needle: &str) -> bool {
        self.count(needle) > 0
    }
    pub fn took(&self, needle: &str) -> bool {
        match &self.answer {
            Some(DecisionAnswer::Take(o)) => labelled(o, needle),
            Some(DecisionAnswer::Action(ActionOption::CardAction { label, .. })) => {
                label.contains(needle)
            }
            _ => false,
        }
    }
}

/// Everything the fold observed. Assertions about offers are made here, after
/// the fact, rather than from inside a loop.
#[derive(Debug, Clone, Default)]
pub struct Transcript {
    pub entries: Vec<Entry>,
    pub result: Option<GameResult>,
    /// The driver stopped on a `Halt` (the decision is still pending).
    pub halted: bool,
}

impl Transcript {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn last(&self) -> Option<&Entry> {
        self.entries.last()
    }
    /// Every decision of this kind put to this player, in order.
    pub fn windows(&self, kind: Kind, side: Side) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.side == side && e.kind() == kind).collect()
    }
    /// The nth (1-based) decision of this kind put to this player.
    pub fn nth_window(&self, kind: Kind, side: Side, n: usize) -> &Entry {
        let ws = self.windows(kind, side);
        ws.get(n - 1).copied().unwrap_or_else(|| {
            panic!("no {n}. {kind:?} window for {side:?} (saw {})", ws.len())
        })
    }
    /// The first decision of this kind put to this player.
    pub fn first_window(&self, kind: Kind, side: Side) -> &Entry {
        self.nth_window(kind, side, 1)
    }
    /// Was this option offered anywhere, to anyone?
    pub fn ever_offered(&self, needle: &str) -> bool {
        self.entries.iter().any(|e| e.offered(needle))
    }
    /// Was this option ever offered to this player?
    pub fn ever_offered_to(&self, side: Side, needle: &str) -> bool {
        self.entries.iter().any(|e| e.side == side && e.offered(needle))
    }
    /// How many decisions offered this option.
    pub fn offers(&self, needle: &str) -> usize {
        self.entries.iter().filter(|e| e.offered(needle)).count()
    }
    /// How many times the plans took this option.
    pub fn times_taken(&self, needle: &str) -> usize {
        self.entries.iter().filter(|e| e.took(needle)).count()
    }
    pub fn took(&self, needle: &str) -> bool {
        self.times_taken(needle) > 0
    }
    /// Decisions of this kind, either side.
    pub fn of_kind(&self, kind: Kind) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.kind() == kind).collect()
    }
    /// A compact tail for panic messages.
    pub fn tail(&self, n: usize) -> String {
        let start = self.entries.len().saturating_sub(n);
        self.entries[start..]
            .iter()
            .map(|e| format!("  #{} {:?} {:?} -> {:?}", e.seq, e.side, e.spec, e.answer))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

struct Context {
    stack: Vec<StructKind>,
    step: Option<String>,
}

fn context(vm: &Vm) -> Context {
    let mut stack = Vec::new();
    let mut step = None;
    for f in vm.frames.iter().rev() {
        if let Frame::Structure(sf) = f {
            if step.is_none() {
                step = vm
                    .tables
                    .iter()
                    .find(|t| t.kind == sf.kind)
                    .and_then(|t| t.steps.get(sf.cursor))
                    .map(|s| s.id.clone());
            }
            stack.push(sf.kind);
        }
    }
    Context { stack, step }
}

/// THE shared driver: folds `Vm × Plan(Corp) × Plan(Runner)` into a
/// [`Transcript`]. Holds the per-rule ordinal counters so a test can stop,
/// assert, and resume without a rule's `once()` firing twice.
pub struct Script {
    corp: Plan,
    runner: Plan,
    corp_counts: Vec<usize>,
    runner_counts: Vec<usize>,
    budget: usize,
    seq: usize,
    transcript: Transcript,
}

impl Script {
    pub fn new(corp: Plan, runner: Plan) -> Script {
        assert_eq!(corp.side, Side::Corp, "the Corp plan must be Plan::corp()");
        assert_eq!(runner.side, Side::Runner, "the Runner plan must be Plan::runner()");
        let corp_counts = vec![0; corp.rules.len()];
        let runner_counts = vec![0; runner.rules.len()];
        Script {
            corp,
            runner,
            corp_counts,
            runner_counts,
            budget: 600,
            seq: 0,
            transcript: Transcript::default(),
        }
    }

    /// Maximum decisions answered per `run` (a livelock guard).
    pub fn budget(mut self, n: usize) -> Script {
        self.budget = n;
        self
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// Run to the next halt / game over, then hand back the transcript so
    /// far. The transcript accumulates across calls.
    pub fn run(&mut self, vm: &mut Vm) -> &Transcript {
        self.transcript.halted = false;
        for _ in 0..self.budget {
            match vm.step() {
                Yield::Progressed => continue,
                Yield::GameOver(r) => {
                    self.transcript.result = Some(r);
                    return &self.transcript;
                }
                Yield::Decision(side, spec) => {
                    let ctx = context(vm);
                    let (rule, reply) = self.choose(side, &ctx, &spec);
                    let answer = resolve(&reply, &spec, &self.transcript);
                    self.seq += 1;
                    let seq = self.seq;
                    self.transcript.entries.push(Entry {
                        seq,
                        side,
                        spec: spec.clone(),
                        answer: answer.clone(),
                        rule,
                        stack: ctx.stack,
                        step: ctx.step,
                    });
                    match answer {
                        Some(a) => vm.answer(a),
                        None => {
                            self.transcript.halted = true;
                            return &self.transcript;
                        }
                    }
                }
            }
        }
        panic!(
            "plan driver exceeded its budget of {} decisions; last decisions:\n{}",
            self.budget,
            self.transcript.tail(8)
        );
    }

    /// Run once and take the transcript (the single-segment idiom).
    pub fn play(mut self, vm: &mut Vm) -> Transcript {
        self.run(vm);
        self.transcript
    }

    fn choose(
        &mut self,
        side: Side,
        ctx: &Context,
        spec: &DecisionSpec,
    ) -> (Option<usize>, Reply) {
        let (plan, counts) = match side {
            Side::Corp => (&self.corp, &mut self.corp_counts),
            Side::Runner => (&self.runner, &mut self.runner_counts),
        };
        for (i, rule) in plan.rules.iter().enumerate() {
            if !rule.when.shape_matches(ctx, spec, &rule.then) {
                continue;
            }
            counts[i] += 1;
            let n = counts[i];
            let applies = match rule.when.ordinal {
                Ordinal::Every => true,
                Ordinal::Nth(k) => n == k,
                Ordinal::UpTo(k) => n <= k,
            };
            if applies {
                return (Some(i), rule.then.clone());
            }
        }
        (None, plan.fallback.clone())
    }
}

/// Turn a [`Reply`] into the concrete answer for this decision. `None` means
/// halt.
fn resolve(reply: &Reply, spec: &DecisionSpec, t: &Transcript) -> Option<DecisionAnswer> {
    let a = match reply {
        Reply::Halt => return None,
        Reply::Forbid => panic!(
            "the plan declared this decision impossible: {spec:?}\nrecent:\n{}",
            t.tail(6)
        ),
        Reply::Take(p) => match spec {
            DecisionSpec::TakeAction { options } => DecisionAnswer::Action(
                p.find_action(options)
                    .unwrap_or_else(|| panic!("plan wanted {p:?}; offered: {options:?}")),
            ),
            _ => {
                let options = window_options(spec);
                DecisionAnswer::Take(
                    p.find_window(options)
                        .unwrap_or_else(|| panic!("plan wanted {p:?}; offered: {options:?}")),
                )
            }
        },
        Reply::Pass => {
            cite!("rule_pass");
            match spec {
                // 9.2.8e: mandatory pendings must be discharged before their
                // controller may pass — "pass" means "nothing of my own".
                DecisionSpec::ReactionWindow { can_pass: false, .. }
                | DecisionSpec::InterruptWindow { can_pass: false, .. } => default_answer(spec),
                _ => DecisionAnswer::Pass,
            }
        }
        Reply::Default => default_answer(spec),
        Reply::Targets(t) => DecisionAnswer::Targets(t.clone()),
        Reply::Choose(i) => DecisionAnswer::Option(*i),
        Reply::ChooseNamed(n) => {
            let opts = choices(spec);
            DecisionAnswer::Option(
                opts.iter()
                    .position(|l| l.contains(n))
                    .unwrap_or_else(|| panic!("plan wanted option {n:?}; offered: {opts:?}")),
            )
        }
        Reply::PayCost(b) => DecisionAnswer::PayNestedCost(*b),
        Reply::Optional(b) => DecisionAnswer::ResolveOptional(*b),
        Reply::Candidate(o) => DecisionAnswer::Candidate(*o),
        Reply::JackOut(b) => DecisionAnswer::JackOut(*b),
        Reply::Discard(v) => DecisionAnswer::Discard(v.clone()),
        Reply::ChooseSet(i) => DecisionAnswer::ChooseSet(*i),
        Reply::Spend(n) => DecisionAnswer::SpendCredits(*n),
        Reply::Bid(n) => DecisionAnswer::Bid(*n),
        Reply::Keep => DecisionAnswer::KeepHand,
        Reply::Mulligan => DecisionAnswer::TakeMulligan,
    };
    Some(a)
}

/// The neutral policy — the second interpreter of the player algebra, and the
/// meaning of [`Reply::Default`]: pass or decline wherever the CR allows it,
/// and discharge mandatory obligations (9.2.8e) where it does not.
pub fn default_answer(spec: &DecisionSpec) -> DecisionAnswer {
    match spec {
        DecisionSpec::Mulligan => DecisionAnswer::KeepHand,
        DecisionSpec::TakeAction { options } => DecisionAnswer::Action(
            options.first().cloned().unwrap_or(ActionOption::BasicCredit),
        ),
        DecisionSpec::PaidWindow { .. } => DecisionAnswer::Pass,
        DecisionSpec::ReactionWindow { options, can_pass } => {
            cite!("rule_reaction_window_priority");
            if *can_pass {
                DecisionAnswer::Pass
            } else {
                let mandatory = options
                    .iter()
                    .find(|o| matches!(o, WindowOption::TriggerInstance { mandatory: true, .. }))
                    .or(options.first())
                    .cloned()
                    .expect("mandatory option");
                DecisionAnswer::Take(mandatory)
            }
        }
        DecisionSpec::InterruptWindow { options, can_pass } => {
            if *can_pass {
                DecisionAnswer::Pass
            } else {
                DecisionAnswer::Take(options.first().cloned().unwrap())
            }
        }
        DecisionSpec::MidAccessWindow { .. } => {
            cite!("rule_mid_access_window_one_ability");
            DecisionAnswer::Pass
        }
        DecisionSpec::ChooseTargets { candidates, count, .. } => {
            DecisionAnswer::Targets(candidates.iter().take(*count as usize).copied().collect())
        }
        DecisionSpec::ChooseOption { .. } => DecisionAnswer::Option(0),
        DecisionSpec::NestedCost { .. } => DecisionAnswer::PayNestedCost(false),
        DecisionSpec::OptionalEffect { .. } => DecisionAnswer::ResolveOptional(false),
        DecisionSpec::ChooseCandidate { candidates } => DecisionAnswer::Candidate(candidates[0]),
        // 10.3.1j: the neutral policy declines candidacy; plans opt in.
        DecisionSpec::DeclareBreachCandidate { .. } => DecisionAnswer::ResolveOptional(false),
        DecisionSpec::JackOut => DecisionAnswer::JackOut(false),
        DecisionSpec::DiscardCards { count, hand } => {
            DecisionAnswer::Discard(hand.iter().take(*count as usize).copied().collect())
        }
        DecisionSpec::MinimalSet { .. } => DecisionAnswer::ChooseSet(0),
        DecisionSpec::TraceSpend { .. } => DecisionAnswer::SpendCredits(0),
        DecisionSpec::PsiBid { .. } => DecisionAnswer::Bid(0),
    }
}

/// Fold with two plans, from wherever the VM currently is, to the first halt
/// or game over. The one-liner most tests use.
pub fn play(vm: &mut Vm, corp: Plan, runner: Plan) -> Transcript {
    Script::new(corp, runner).play(vm)
}
