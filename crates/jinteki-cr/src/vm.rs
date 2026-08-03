//! The CR virtual machine: a sans-IO coroutine. Timing structures are
//! programs whose steps are data (§11); abilities are procedures of
//! instructions (§9); priority windows are the scheduler (§9.2); the
//! checkpoint (§10.3) is the innermost loop watching the change buffer; and
//! interrupts/replacements (§9.9) rewrite an instruction's expected effects
//! between imminence and resolution. Every player decision suspends the
//! machine: `step()` yields typed [`Yield`] values and never blocks.

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::ability::{
    StaticCond,
    ability_active, is_corp_card, AbilityDef, AbilityFlag, AbilityInstance, AbilityKind,
    AbilityRef, Condition, Cost, StaticDecl, TriggerCond,
};
use crate::checkpoint;
use crate::decision::{
    ActionOption, DecisionAnswer, DecisionSpec, GameResult, WindowOption, Yield,
};
use crate::effects::{DamageKind, EffectAtom, EffectClass, WouldCounters, WouldScope};
use crate::frames::{
    AbilityFrame, AbilityPhase, AccessCtx, BreachCtx, Frame, ResolutionKind, RunCtx, StepPhase,
    StructCtx, StructureFrame,
};
pub use crate::ability::SubKey;
use crate::instr::{Instruction, TargetFilter, TargetSpec};
use crate::lingering::{Duration, LingeringEffect, Payload};
use crate::change::{ActionIdentity, BasicAction, ChangeBuffer, GameChange};
use crate::object::{
    card_active, CardType, CounterKind, IcePosition, Object, ObjectId, PrintedCard, ServerId, Side,
    Zone,
};
use crate::timing::{load_tables, step_op, BranchPred, StepBody, StepKind, StepOp, StepTable};
use crate::window::{PawClasses, WindowFrame, WindowKind};

/// Per-player mutable resources.
#[derive(Debug, Clone)]
pub struct PlayerState {
    pub credits: u32,
    pub clicks: u32,
    /// CR 1.11.2: allotted clicks (Corp 3, Runner 4 by default).
    pub allotted_clicks: u32,
    pub tags: u32,
    pub bad_publicity: u32,
    pub core_damage: u32,
    pub max_hand_size_base: i32,
    pub memory_limit_base: i32,
}

impl PlayerState {
    fn new(side: Side) -> Self {
        cite!("rule_allotted_clicks");
        PlayerState {
            credits: 0,
            clicks: 0,
            allotted_clicks: match side {
                Side::Corp => 3,
                Side::Runner => 4,
            },
            tags: 0,
            bad_publicity: 0,
            core_damage: 0,
            // CR 5.5.3a: players begin with maximum hand size five.
            max_hand_size_base: 5,
            // CR 1.20.2: the Runner starts with a memory limit of 4.
            memory_limit_base: 4,
        }
    }
}

/// The active-encounter record (§6.5). Encounters can exist without a run
/// (6.1.4 / `rule_end_encounter_outside_run`).
#[derive(Debug, Clone)]
pub struct EncounterState {
    pub id: u64,
    pub ice: ObjectId,
    /// CR 9.8.4: per-encounter broken status, keyed by stable subroutine
    /// identity so gains/losses mid-encounter preserve statuses.
    pub broken: std::collections::BTreeSet<SubKey>,
    /// Subroutines already resolved this encounter (6.9.3c loop).
    pub resolved: std::collections::BTreeSet<SubKey>,
    /// CR 9.12.2d: "all subroutines broken" has been noted for this
    /// encounter (at most once) — vacuously true for zero-sub ice as soon
    /// as step 6.9.3b begins.
    pub all_broken_noted: bool,
}


/// The pure game state (cloneable for the 9.6.6a snapshot).
#[derive(Debug, Clone)]
pub struct CoreState {
    pub objects: BTreeMap<ObjectId, Object>,
    pub deck: BTreeMap<Side, Vec<ObjectId>>,
    pub hand: BTreeMap<Side, Vec<ObjectId>>,
    pub discard: BTreeMap<Side, Vec<ObjectId>>,
    pub score_area: BTreeMap<Side, Vec<ObjectId>>,
    /// CR 6.2.1: each server's sequence of positions, INNERMOST FIRST. The
    /// ice protecting a server is what occupies those positions; the
    /// positions themselves are the ordered thing (6.2.6), so a vacated
    /// position survives here until 10.3.1i destroys it.
    pub ice: BTreeMap<ServerId, Vec<crate::object::IcePosition>>,
    /// Root cards per server.
    pub root: BTreeMap<ServerId, Vec<ObjectId>>,
    pub corp: PlayerState,
    pub runner: PlayerState,
    /// Whose turn it is (CR 9.2.1: the active player).
    pub turn_side: Side,
    pub turn_seq: u64,
    /// CR 6.4 bad publicity fund (`rule_bad_publicity_fund`).
    pub bp_fund: u32,
    pub encounter: Option<EncounterState>,
    /// The card currently being accessed, if any (7.1).
    pub accessed: Option<ObjectId>,
    /// CR 7.3.6: the run instance and the number of accesses ACTUALLY
    /// PERFORMED during it. Set when a run begins and kept after it ends, so
    /// a "when this run ends" ability — which resolves after the run frame
    /// has popped (6.9.6d / 10.3.6) — can still count them. An access
    /// replaced by another effect never reaches `CardBecomesAccessed` and so
    /// never counts, which is the whole of the rule.
    pub run_accesses: Option<(u64, u32)>,
    /// CR 5.2.5 / 1.16.4d: the action in progress and how many [click] have
    /// been spent to take it.
    pub current_action: Option<(crate::change::ActionIdentity, u32)>,
    /// CR 5.2.5: where in the change log this turn began — the window a
    /// "different actions this turn" query reviews.
    pub turn_log_start: usize,
    /// CR 9.12.3e: the "must make a run with your first [click]" requirement
    /// has been discharged this turn — by making the run, or by being offered
    /// its additional cost and declining it.
    pub run_requirement_discharged: bool,
    /// CR 1.12.6: where in the change log the run in progress (or the one
    /// that just ended) began — the window a history query about "this run"
    /// reviews.
    pub run_log_start: usize,
    /// CR 10.2.2b: the hidden information each player has been SHOWN by a game
    /// effect — looking (1.21.2), revealing (1.21.3), exposing (1.21.4). Every
    /// other kind of visibility is derived from the state by
    /// `Vm::identity_visible_to`, so this holds only what a rule or an ability
    /// actively disclosed.
    pub seen: crate::view::Sightings,
    /// CR 4.8.7: the next distinct facedown set-aside group.
    pub next_set_aside_group: u64,
    /// Move stamps: bumped on every zone change (9.1.4 stranding checks).
    pub move_seq: u64,
    /// Active-since stamps (10.3.1d).
    pub active_seq: u64,
}

impl CoreState {
    pub fn player(&self, s: Side) -> &PlayerState {
        match s {
            Side::Corp => &self.corp,
            Side::Runner => &self.runner,
        }
    }
    pub fn player_mut(&mut self, s: Side) -> &mut PlayerState {
        match s {
            Side::Corp => &mut self.corp,
            Side::Runner => &mut self.runner,
        }
    }
}

/// What the pending decision is *for* — answer routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionCtx {
    Mulligan(Side),
    Window(u64),
    Targets,
    NestedCost,
    Optional,
    JackOut,
    Discard(Side),
    Candidate,
    MinimalSet,
    /// Additional-cost-to-steal decision for the accessed agenda (1.16.10).
    StealCost(ObjectId),
    /// CR 1.16.10c: pay or decline the additional cost to SCORE this agenda.
    ScoreCost(ObjectId),
    /// CR 6.3.4 / 1.16.10a: pay or decline the additional cost to make a run.
    RunActionCost(ServerId),
    /// 10.8.6c/d trace spends.
    TraceSpend(Side),
    /// CR 9.8.2c: the granting player declares where the subroutines just
    /// granted go.
    SubroutineOrder,
    /// CR 8.3.3: the arranging player declares, secretly, the order the
    /// set-aside cards go back on top of this deck.
    Arrange { to_top_of: Side },
    /// CR 10.1.6a: the player resolving a mandatory infinite loop chooses how
    /// many times it resolves.
    LoopCount,
    /// CR 10.4.3a: which cards the selecting player trashes for this damage.
    DamageSelection { kind: crate::effects::DamageKind, amount: u32 },
    /// 10.14.6 sealed psi bids.
    PsiBid(Side),
    /// 10.3.1j: the Runner declares candidacy of a mid-breach root entry.
    BreachCandidacy(ObjectId),
    /// 8.5.13d/1.16.4c: pay or decline the additional rez cost during an
    /// "install and rez" effect.
    RezAdditionalCost,
    /// 1.16.2f: the Corp divides an install-and-rez "total N less" modifier
    /// between the install cost and the rez cost.
    CostDivision,
    /// 9.9.11: choose the order in which replacement effects apply.
    ReplacementOrder,
    /// CR 6.7.4c: apply an optional replacement effect, or decline it.
    OptionalReplacement,
    /// 7.4.3 example 2 (Gagarin class): pay or decline an additional cost
    /// to access the chosen candidate.
    AccessCost(ObjectId),
    /// 10.12.2: the Corp chooses which cards to trash from HQ for a
    /// "sabotage N"; the remainder comes off the top of R&D.
    Sabotage { count: u32 },
    /// CR 1.16: one of the choices the payment in progress needs — the
    /// 1.16.2c value of X, a 1.16.2e alternate payment, the 1.10.3c division
    /// of the credits, or which cards/agendas are spent.
    Payment,
    /// CR 8.7.2: which cards the searching player FINDS. Asked while the
    /// search instruction resolves, never at announce time — found cards are
    /// not targets (`rule_searching_does_not_target`). Carries the searched
    /// zone so the 8.7.3 shuffle happens on the answer.
    SearchFind { zone: Zone },
}

/// CR 8.5.16: one installation in progress. Installing is a procedure, not
/// a timing structure (9.2.2e); the VM expands it into step instructions and
/// this record carries the state between them.
#[derive(Debug, Clone)]
pub struct InstallProgress {
    pub card: ObjectId,
    pub dest: crate::instr::InstallDest,
    pub and_rez: bool,
    pub ignore_costs: bool,
    pub reveal_check: Option<crate::instr::RevealCheck>,
    /// The card came from a hidden/secret zone or was facedown (8.5.13
    /// reveal relevance).
    pub was_hidden: bool,
    /// 8.5.14: the destination was invalid — remaining steps are no-ops.
    pub aborted: bool,
    /// 8.5.13d: the card cannot be rezzed — the rez steps are skipped.
    pub rez_skipped: bool,
    /// 8.5.13: the card has already been revealed (at most once).
    pub revealed: bool,
    /// Destination resolved at step 8.5.16b.
    pub resolved_zone: Option<Zone>,
    /// CR 6.2.2: the position created for this ice when its destination was
    /// declared (8.5.16b); the ice occupies it at 8.5.16e.
    pub ice_position: Option<u64>,
    /// CR 1.16.2f: the "total N[credit] less" modifier this install-and-rez
    /// carries, and how much of it the Corp declared against the INSTALL cost
    /// at the beginning of step 8.5.16d. The remainder goes to the rez cost.
    pub reduce_total: u32,
    pub reduce_install: u32,
    /// CR 4.8.3: the zone the card is treated as having been installed FROM
    /// — its location at step 8.5.16a, with a set-aside card reported as
    /// coming from wherever it was before it was set aside.
    pub from_zone: Zone,
}

/// CR 8.7.2b: the instruction a search is "followed by", when it refers to
/// the found cards — the only thing that restricts what may be found beyond
/// the search's own criteria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FollowOn {
    Install { dest: crate::instr::InstallDest, ignore_costs: bool },
    Play { ignore_costs: bool },
}

/// CR 8.6.7: one play (event/operation) in progress.
#[derive(Debug, Clone)]
pub struct PlayProgress {
    pub card: ObjectId,
    pub ignore_costs: bool,
}

/// An in-progress trace attempt (10.8.6): shared state across the expanded
/// step instructions.
#[derive(Debug, Clone)]
pub struct TraceState {
    pub trace_strength: i64,
    pub link_strength: i64,
}

/// Setup progress (§1.6 is a procedure, not a timing structure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupPhase {
    CorpMulligan,
    RunnerMulligan,
    Done,
}

pub struct Vm {
    pub st: CoreState,
    pub rng: ChaCha8Rng,
    pub tables: Vec<StepTable>,
    pub frames: Vec<Frame>,
    pub imminents: Vec<ImminentWrap>,
    pub instances: BTreeMap<u64, AbilityInstance>,
    pub lingering: Vec<LingeringEffect>,
    pub changes: ChangeBuffer,
    pub would: WouldCounters,
    /// CR 9.6.7d: static-condition abilities throttled until a timing
    /// structure step completes.
    pub throttled: HashSet<AbilityRef>,
    /// CR 9.3.6g: once-per-turn uses this turn.
    /// CR 9.3.6g once-per-turn use, keyed by the OBJECT (1.12.2): a card
    /// that changed zones and came back is a new object, so its
    /// once-per-turn ability is available again.
    pub once_per_turn_used: HashSet<(AbilityRef, u32)>,
    /// CR 9.6.6a: game state as of the previous checkpoint's step (a).
    pub snapshot: Option<Box<CoreState>>,
    /// Scan window captured by the last checkpoint's step (a) (for 10.3.1j).
    pub last_scan_window: Vec<(GameChange, u64)>,
    /// Sets offered by a suspended 10.3.1e minimal-set Decision.
    pub last_minimal_sets: Option<Vec<Vec<ObjectId>>>,
    /// CR 9.5.5: set-aside counters left over after their ability finished —
    /// returned to the bank at checkpoint step 10.3.1f/g.
    pub orphan_set_aside_counters: Vec<(CounterKind, u32)>,
    /// CR 9.5.5: set-aside cards left over — trashed at step 10.3.1g.
    pub set_aside_card_cleanup: Vec<ObjectId>,
    /// In-progress trace attempt (10.8; NOT a timing structure, 9.2.2e).
    pub trace: Option<TraceState>,
    /// In-progress installations (8.5.16), innermost last. Installing is a
    /// procedure (9.2.2e); nested installs stack.
    pub installs: Vec<InstallProgress>,
    /// In-progress plays (8.6.7), innermost last.
    pub plays: Vec<PlayProgress>,
    /// 10.3.1j: mid-breach root entries awaiting the Runner's candidacy
    /// declaration.
    pub pending_candidacy: Vec<ObjectId>,
    /// CR 9.6.14d: instances marked pending by an EFFECT rather than by a
    /// stipulation occurring. They are ordinary pendings — they just are not
    /// discovered by the checkpoint's step (a) scan, so the next checkpoint
    /// drains them into its newly-pending set and 10.3.2 opens the reaction
    /// window that offers them.
    pub pending_from_effect: Vec<u64>,
    /// Sealed first bid of an in-progress Psi Game (10.14.6).
    psi_first_bid: Option<u32>,
    pub pending_decision: Option<(Side, DecisionSpec, DecisionCtx)>,
    answer: Option<DecisionAnswer>,
    pub game_over: Option<GameResult>,
    setup: SetupPhase,
    next_turn_side: Side,
    // id fountains
    next_object: u32,
    next_instance: u64,
    next_window: u64,
    next_structure: u64,
    next_lingering: u64,
    next_encounter: u64,
    next_run: u64,
    next_remote: u32,
    next_position: u64,
    /// CR 6.2.2: the position created when an install destination was
    /// declared (step 8.5.16b), waiting for the ice to occupy it once it
    /// becomes installed (step 8.5.16e) — and the same channel by which a
    /// swap (6.2.2f) or a move (6.2.7d) puts a piece of ice into an
    /// already-existing position instead of making a new one.
    pending_ice_position: Option<(ServerId, u64)>,
    /// Run context mirror for conditions when the run frame is deep in the
    /// stack: (run_id, server, reached_success) while a run is in progress.
    pub current_run: Option<(u64, ServerId, bool)>,
    /// CR 9.8.2c: the ice and grant stamp a `DeclareSubroutineOrder` Decision
    /// is about, while it is outstanding.
    pub pending_sub_order: Option<(ObjectId, u64)>,
    /// CR 6.7.4c: the optional replacement effect whose apply-or-decline
    /// Decision is outstanding.
    pub pending_optional_replacement: Option<u64>,
    /// CR 9.9.9c: counters a Project-Vacheron-class replacement said the
    /// stolen agenda arrives in the score area WITH.
    pub pending_steal_counters: Vec<(CounterKind, u32)>,
    /// CR 10.1.6a: how many more times a mandatory infinite loop resolves
    /// before it ends. `None` while no loop is in progress.
    pub loop_budget: Option<u32>,
    /// CR 1.16: the payment in progress, if any (`Vm::begin_payment`).
    pub payment: Option<Payment>,
    /// Trace of resolutions for tests: labels of resolved ability frames.
    pub resolution_log: Vec<String>,
}

/// CR 1.16.1: paying a cost is a PROCEDURE, not a single act. Everything the
/// payer gets to choose — the 1.16.2c value of X, which 1.16.2e alternate
/// payments to use, the 1.10.3c division of the credits among the locations
/// they may come from, and which cards/agendas are spent — is decided first,
/// one Decision at a time, and only then is the whole cost paid at once. That
/// is what makes cost payment a suspendable phase of the ability frame: the
/// frame stays where it is while the payment gathers its choices.
#[derive(Debug, Clone)]
pub struct Payment {
    pub side: Side,
    pub source: ObjectId,
    pub cost: Cost,
    /// CR 1.16.2c: the value announced for X, once announced.
    pub announced_x: Option<u32>,
    /// CR 1.16.2e: alternate payments elected, as (credits covered, what is
    /// paid instead). Indices into `alternates_offered` already decided.
    pub alternates_offered: usize,
    pub alternate_covers: u32,
    pub alternate_cost: Cost,
    /// CR 1.10.3c: how many credits come from each allowed location, in the
    /// order `credit_locations` lists them.
    pub division: Option<Vec<u32>>,
    /// Cards chosen for a `trash_from_hand` component.
    pub from_hand: Option<Vec<ObjectId>>,
    /// Agendas chosen for a `forfeit_agenda` component (8.2.5).
    pub forfeited: Option<Vec<ObjectId>>,
    /// Installed cards chosen for a `trash_matching` component.
    pub trashed: Option<Vec<ObjectId>>,
    /// CR 1.16.1c: a restriction on the effect being paid for, which the
    /// payment must not break.
    pub restriction: Option<PaymentRestriction>,
    /// What the payment was for: resumed once it is committed.
    pub cont: PaymentCont,
}

/// CR 6.7.4: "Many abilities that initiate a run contain a conditional
/// ability with the trigger condition 'If successful'. This means, 'After the
/// run created this way becomes successful'." The clause belongs to the
/// initiating EFFECT, so it travels with the run rather than being an
/// ordinary delayed conditional (9.6.13d would refuse one: no run is in
/// progress when the initiating instruction resolves).
#[derive(Debug, Clone)]
pub struct IfSuccessful {
    pub source: AbilityRef,
    pub controller: Side,
    /// CR 6.7.4a: the servers the initiating effect allowed.
    pub allowed: crate::instr::RunServerSet,
    pub effects: Vec<Instruction>,
}

/// CR 1.16.1c: "if triggering an ability or resolving an effect is subject to
/// both costs and other restrictions, the cost … cannot be paid in a way that
/// would result in any restriction no longer being met." The restriction is
/// DATA (§12 rule 2), consulted while the payment filters its candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentRestriction {
    /// CR 1.17.3a: this agenda must still have advancement counters at least
    /// equal to its advancement requirement.
    ScoreRequirement(ObjectId),
}

/// What a completed payment goes on to do. Every payment has one, because a
/// payment that can suspend cannot simply return to its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentCont {
    /// Nothing: the caller had no work left after the payment (an ability
    /// frame's 9.5.7b trigger cost, an install/play cost step, a nested cost
    /// whose branch was already spliced in).
    None,
    /// 8.1.2: the rez procedure continues — the card turns faceup.
    Rez(ObjectId),
    /// 7.4.3: the additional cost to access the chosen candidate was paid.
    Access(ObjectId),
    /// 7.1.5: the basic trash ability's cost was paid.
    BasicTrash { card: ObjectId, window: u64 },
    /// CR 9.5.7b: a paid ability's TRIGGER cost. Nothing follows it — the
    /// ability frame carries on by itself — but the payment's announced X
    /// (1.16.2c) belongs to this use of the ability and is kept on the frame.
    TriggerCost,
}

// A tiny wrapper so effects.rs stays free of frame types.
pub use imminent::ImminentWrap;
mod imminent {
    use super::*;
    /// One imminent instruction with its continuously-updated expected
    /// effects (9.9.2).
    #[derive(Debug, Clone)]
    pub struct ImminentWrap {
        pub instr: Instruction,
        pub atoms: Vec<EffectAtom>,
        pub controller: Side,
        pub targets: Vec<ObjectId>,
        /// CR 1.15.1: announced SUBROUTINE targets (9.8.6).
        pub sub_targets: Vec<crate::ability::SubKey>,
        /// CR 1.15.1 / 1.12.1: announced COUNTER targets.
        pub counter_targets: Vec<crate::object::CounterRef>,
        /// Ordinals per class at imminence time (9.9.5a), per-run scope.
        pub run_ordinal: BTreeMap<u64, u32>,
        /// Same, per-turn scope ("the first time each turn…").
        pub turn_ordinal: BTreeMap<u64, u32>,
        /// Imminence sequence number (replacement once-per-effect keys).
        pub seq: u64,
    }
}

/// Configuration for a full game (§1.6 setup path).
pub struct GameSetup {
    pub corp_deck: Vec<PrintedCard>,
    pub runner_deck: Vec<PrintedCard>,
    pub corp_identity: Option<PrintedCard>,
    pub runner_identity: Option<PrintedCard>,
    pub seed: u64,
    /// Test-harness determinism hook: skip the 1.6.5 shuffle so decks stay
    /// in the given order (SYS-F-8 stack-deck semantics).
    pub shuffle: bool,
}

impl Vm {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Full game start per §1.6: credits (1.6.4), shuffle (1.6.5), draw 5
    /// (1.6.6), then the mulligan decisions (1.6.6a) and the Corp's first
    /// turn (1.6.7).
    pub fn new_game(setup: GameSetup) -> Vm {
        let mut vm = Vm::empty(setup.seed);
        for (side, deck, identity) in [
            (Side::Corp, setup.corp_deck, setup.corp_identity),
            (Side::Runner, setup.runner_deck, setup.runner_identity),
        ] {
            if let Some(idc) = identity {
                let id = vm.new_object(idc, Zone::PlayArea(side));
                vm.st.objects.get_mut(&id).unwrap().faceup = true;
            }
            for card in deck {
                let id = vm.new_object(card, Zone::Deck(side));
                vm.st.deck.get_mut(&side).unwrap().push(id);
            }
        }
        cite!("rule_start_credits");
        vm.st.corp.credits = 5;
        vm.st.runner.credits = 5;
        cite!("rule_start_shuffle");
        if setup.shuffle {
            for side in [Side::Corp, Side::Runner] {
                let deck = vm.st.deck.get_mut(&side).unwrap();
                deck.shuffle(&mut vm.rng);
            }
        }
        cite!("rule_start_hand");
        for side in [Side::Corp, Side::Runner] {
            let n = vm.starting_hand_size(side);
            for _ in 0..n {
                vm.draw_card_silent(side);
            }
        }
        vm.setup = SetupPhase::CorpMulligan;
        vm
    }

    /// Bare machine for scripted example states (test support): empty zones,
    /// no setup procedure, corp turn about to start unless a test drives
    /// otherwise.
    pub fn empty(seed: u64) -> Vm {
        let mut deck = BTreeMap::new();
        let mut hand = BTreeMap::new();
        let mut discard = BTreeMap::new();
        let mut score = BTreeMap::new();
        for s in [Side::Corp, Side::Runner] {
            deck.insert(s, Vec::new());
            hand.insert(s, Vec::new());
            discard.insert(s, Vec::new());
            score.insert(s, Vec::new());
        }
        Vm {
            st: CoreState {
                objects: BTreeMap::new(),
                deck,
                hand,
                discard,
                score_area: score,
                ice: BTreeMap::new(),
                root: BTreeMap::new(),
                corp: PlayerState::new(Side::Corp),
                runner: PlayerState::new(Side::Runner),
                turn_side: Side::Corp,
                turn_seq: 0,
                bp_fund: 0,
                encounter: None,
                accessed: None,
                run_accesses: None,
                run_log_start: 0,
            run_requirement_discharged: false,
            current_action: None,
            turn_log_start: 0,
                seen: Default::default(),
                next_set_aside_group: 1,
                move_seq: 0,
                active_seq: 0,
            },
            rng: ChaCha8Rng::seed_from_u64(seed),
            tables: load_tables(),
            frames: Vec::new(),
            imminents: Vec::new(),
            instances: BTreeMap::new(),
            lingering: Vec::new(),
            changes: ChangeBuffer::default(),
            would: WouldCounters::default(),
            throttled: HashSet::new(),
            once_per_turn_used: HashSet::new(),
            pending_sub_order: None,
            pending_optional_replacement: None,
            pending_steal_counters: Vec::new(),
            loop_budget: None,
            payment: None,
            snapshot: None,
            last_scan_window: Vec::new(),
            last_minimal_sets: None,
            orphan_set_aside_counters: Vec::new(),
            set_aside_card_cleanup: Vec::new(),
            trace: None,
            installs: Vec::new(),
            plays: Vec::new(),
            pending_candidacy: Vec::new(),
            pending_from_effect: Vec::new(),
            psi_first_bid: None,
            pending_decision: None,
            answer: None,
            game_over: None,
            setup: SetupPhase::Done,
            next_turn_side: Side::Corp,
            next_object: 1,
            next_instance: 1,
            next_window: 1,
            next_structure: 1,
            next_lingering: 1,
            next_encounter: 1,
            next_run: 1,
            next_remote: 100,
            next_position: 1,
            pending_ice_position: None,
            current_run: None,
            resolution_log: Vec::new(),
        }
    }

    pub fn new_object(&mut self, printed: PrintedCard, zone: Zone) -> ObjectId {
        let id = ObjectId(self.next_object);
        self.next_object += 1;
        let owner = printed.side;
        self.st.objects.insert(
            id,
            Object {
                id,
                printed,
                flipped: false,
                zone,
                faceup: false,
                owner,
                controller: owner,
                host: None,
                hosted: Vec::new(),
                hosted_not_installed: false,
                counters: BTreeMap::new(),
                active_since: 0,
                set_aside_for_ability: false,
                loaded_kinds: Default::default(),
                set_aside_from: None,
                staged: false,
                generation: 0,
                scored_snapshot: None,
                last_server: None,
                set_aside_group: None,
                converted_agenda: None,
            },
        );
        id
    }

    // ------------------------------------------------------------------
    // The coroutine surface
    // ------------------------------------------------------------------

    /// Advance the machine until it needs a decision, progresses notably, or
    /// the game ends. Never blocks.
    pub fn step(&mut self) -> Yield {
        for _ in 0..100_000 {
            if let Some(r) = self.game_over {
                return Yield::GameOver(r);
            }
            if self.pending_decision.is_some() {
                if self.answer.is_some() {
                    self.apply_answer();
                    continue;
                }
                let (side, spec, _) = self.pending_decision.clone().unwrap();
                return Yield::Decision(side, spec);
            }
            self.tick();
        }
        Yield::Progressed
    }

    /// Provide the answer to the currently pending decision.
    pub fn answer(&mut self, a: DecisionAnswer) {
        assert!(self.pending_decision.is_some(), "no pending decision");
        self.answer = Some(a);
    }

    /// Convenience: answer then step.
    pub fn answer_step(&mut self, a: DecisionAnswer) -> Yield {
        self.answer(a);
        self.step()
    }

    fn ask(&mut self, side: Side, spec: DecisionSpec, ctx: DecisionCtx) {
        self.pending_decision = Some((side, spec, ctx));
    }

    pub fn next_instance_id(&mut self) -> u64 {
        let id = self.next_instance;
        self.next_instance += 1;
        id
    }

    pub fn next_lingering_id(&mut self) -> u64 {
        let id = self.next_lingering;
        self.next_lingering += 1;
        id
    }

    /// 10.3.1e: multiple appropriate sets — the choice is a Decision.
    pub fn suspend_for_minimal_set(&mut self, chooser: Side, sets: Vec<Vec<ObjectId>>) {
        self.last_minimal_sets = Some(sets.clone());
        self.ask(
            chooser,
            DecisionSpec::MinimalSet { sets },
            DecisionCtx::MinimalSet,
        );
    }

    pub fn breach_server(&self) -> Option<ServerId> {
        self.breach_ctx().map(|b| b.server)
    }

    /// 10.3.1j support: is this root entry eligible for a candidacy
    /// declaration (not already a candidate/accessed/declined)?
    pub(crate) fn run_breach_bookkeeping(&self, obj: ObjectId) -> bool {
        let Some(b) = self.breach_ctx() else { return false };
        !(b.candidates.contains(&obj)
            || b.accessed.contains(&obj)
            || b.declined.contains(&obj))
    }

    /// 10.3.1j: suspend for the Runner's candidacy declaration.
    pub(crate) fn ask_breach_candidacy(&mut self, card: ObjectId) {
        self.ask(
            Side::Runner,
            DecisionSpec::DeclareBreachCandidate { card },
            DecisionCtx::BreachCandidacy(card),
        );
    }

    pub fn add_breach_candidate(&mut self, obj: ObjectId) {
        if let Some(b) = self.breach_ctx_mut() {
            // 7.4.6a: cards the Runner declared non-candidates stay out.
            if !b.candidates.contains(&obj)
                && !b.accessed.contains(&obj)
                && !b.declined.contains(&obj)
            {
                b.candidates.push(obj);
            }
        }
    }

    // ------------------------------------------------------------------
    // Main dispatch
    // ------------------------------------------------------------------

    fn tick(&mut self) {
        match self.frames.last() {
            None => self.tick_toplevel(),
            Some(Frame::Structure(_)) => self.tick_structure(),
            Some(Frame::Ability(_)) => self.tick_ability(),
            Some(Frame::Window(_)) => self.tick_window(),
        }
    }

    fn tick_toplevel(&mut self) {
        match self.setup {
            SetupPhase::CorpMulligan => {
                cite!("rule_mulligan");
                self.ask(
                    Side::Corp,
                    DecisionSpec::Mulligan,
                    DecisionCtx::Mulligan(Side::Corp),
                );
            }
            SetupPhase::RunnerMulligan => {
                self.ask(
                    Side::Runner,
                    DecisionSpec::Mulligan,
                    DecisionCtx::Mulligan(Side::Runner),
                );
            }
            SetupPhase::Done => {
                // CR 1.6.7: the game begins and the Corp takes their first
                // turn; afterwards turns alternate (5.1).
                cite!("rule_start_corp_turn");
                if self.changes.log.is_empty() {
                    self.changes.record(GameChange::GameBegan);
                }
                let side = self.next_turn_side;
                self.push_turn(side);
                self.next_turn_side = side.other();
            }
        }
    }

    /// Test-support entry: begin a specific player's turn structure directly
    /// (example states start mid-game).
    pub fn start_turn(&mut self, side: Side) {
        self.push_turn(side);
        self.next_turn_side = side.other();
    }

    fn push_turn(&mut self, side: Side) {
        let kind = match side {
            Side::Corp => crate::timing::StructKind::CorpTurn,
            Side::Runner => crate::timing::StructKind::RunnerTurn,
        };
        self.st.turn_side = side;
        self.st.turn_seq += 1;
        self.once_per_turn_used.clear();
        // 9.12.3a: a "first [click] each turn" requirement is fresh each turn.
        self.st.run_requirement_discharged = false;
        self.st.current_action = None;
        self.st.turn_log_start = self.changes.log.len();
        self.would.reset_scope(WouldScope::Turn);
        let id = self.next_structure;
        self.next_structure += 1;
        self.frames.push(Frame::Structure(StructureFrame {
            kind,
            instance_id: id,
            cursor: 0,
            phase: StepPhase::Enter,
            pending_jump: None,
            ctx: StructCtx::Turn { side },
        }));
    }

    fn table(&self, kind: crate::timing::StructKind) -> &StepTable {
        self.tables.iter().find(|t| t.kind == kind).unwrap()
    }

    // ------------------------------------------------------------------
    // Structure frames: §11 tables as data
    // ------------------------------------------------------------------

    fn tick_structure(&mut self) {
        // CR 6.5.8a / 6.2.7c / 6.1.4b: an aborted Encounter Ice Phase ends
        // "without following any of its remaining steps" — the instruction
        // that aborted it resolved to completion first (9.11.2), and the
        // phase completes as soon as it is on top of the stack again.
        let aborted = matches!(
            self.frames.last(),
            Some(Frame::Structure(StructureFrame { ctx: StructCtx::Encounter(e), .. }))
                if e.aborted
        );
        if aborted {
            cite!("rule_bypass");
            cite!("rule_ice_change_encounter_uninstall_derez");
            self.complete_structure();
            self.checkpoint_and_react(None);
            return;
        }
        let (kind, cursor, phase) = {
            let Frame::Structure(sf) = self.frames.last().unwrap() else { unreachable!() };
            (sf.kind, sf.cursor, sf.phase)
        };
        let step_id = self.table(kind).steps[cursor].id.clone();
        let op = step_op(kind, &step_id);

        match phase {
            StepPhase::Enter => {
                // CR 9.11.2: each step is a single instruction, preceded by
                // an interrupt window…
                cite!("rule_step_in_timing_structure_is_instruction");
                match op {
                    StepOp::Instr(k) | StepOp::InstrThenGoto(k, _) => {
                        let instr = self.step_instruction(k);
                        let atoms = self.expected_atoms(&instr, self.st.turn_side, &[], None);
                        let asked =
                            self.push_imminent(instr, self.st.turn_side, Vec::new(), Vec::new(), Vec::new(), atoms);
                        self.set_structure_phase(StepPhase::Exec);
                        if asked {
                            // 9.9.11 order Decision pending; the answer path
                            // reopens the interrupt window before Exec runs.
                            return;
                        }
                        if self.open_interrupt_window_if_relevant() {
                            return; // window frame now on top
                        }
                    }
                    _ => {
                        // Window steps / branch steps: no imminence of their
                        // own; the windows themselves handle checkpoints on
                        // priority (9.2.4e).
                        self.set_structure_phase(StepPhase::Exec);
                    }
                }
            }
            StepPhase::Exec => match op {
                StepOp::Instr(k) => {
                    // The step's own frame index: executing the step may push
                    // child frames (breach → access), which must not swallow
                    // this frame's phase transition.
                    let me = self.frames.len() - 1;
                    self.exec_step_instruction(k);
                    self.set_structure_phase_at(me, StepPhase::Checkpoint);
                }
                StepOp::InstrThenGoto(k, target) => {
                    let me = self.frames.len() - 1;
                    self.exec_step_instruction(k);
                    let idx = self.table(kind).index_of(target);
                    self.set_structure_jump_at(me, idx);
                    self.set_structure_phase_at(me, StepPhase::Checkpoint);
                }
                StepOp::OpenPhase { phase, then } => {
                    // 9.2.2b: the phase runs as its own structure. This frame
                    // is parked at `then` — the step the parent continues from
                    // when the phase completes — and the phase's frame goes on
                    // top of it.
                    cite!("rule_run_timing_structure");
                    let me = self.frames.len() - 1;
                    let idx = self.table(kind).index_of(then);
                    self.set_structure_jump_at(me, idx);
                    self.set_structure_phase_at(me, StepPhase::Checkpoint);
                    debug_assert_eq!(phase, crate::timing::StructKind::Encounter);
                    // The ice in the Runner's position is the one encountered
                    // (6.5.1); with the position vacated there is nothing to
                    // encounter and the run moves on.
                    if let Some(ice) = self.run_ctx().and_then(|r| self.approached_ice(r)) {
                        self.open_encounter_phase(ice, false);
                    }
                }
                StepOp::Paw(classes) => {
                    // 9.12.2d: for zero-sub ice, "all subroutines broken"
                    // is satisfied as soon as step 6.9.3b begins.
                    if self.st.encounter.is_some() {
                        self.check_all_subs_broken();
                    }
                    self.open_paid_window(classes);
                    // Window frames run above; when the window closes the
                    // structure resumes in Checkpoint phase.
                }
                StepOp::MidAccessWindow => {
                    self.open_mid_access_window();
                }
                StepOp::BodyOrGoto { pred, body, skip_to } => {
                    if self.eval_pred(pred) {
                        match body {
                            StepBody::ActionWindow => self.open_action_window(),
                            StepBody::ChooseCandidate => self.choose_candidate_body(),
                            StepBody::ResolveNextSubroutine => self.resolve_next_subroutine_body(),
                        }
                    } else {
                        let idx = self.table(kind).index_of(skip_to);
                        self.set_structure_jump(idx);
                        self.set_structure_phase(StepPhase::Checkpoint);
                    }
                }
                StepOp::BranchGoto { pred, yes, no } => {
                    let target = if self.eval_pred(pred) { yes } else { no };
                    let idx = self.table(kind).index_of(target);
                    self.set_structure_jump(idx);
                    self.set_structure_phase(StepPhase::Checkpoint);
                }
                StepOp::Goto(target) => {
                    let idx = self.table(kind).index_of(target);
                    self.set_structure_jump(idx);
                    self.set_structure_phase(StepPhase::Checkpoint);
                }
                StepOp::Complete => {
                    // CR 10.3.6: the checkpoint following the last step of a
                    // timing structure takes place OUTSIDE of it: pop first.
                    cite!("rule_checkpoint_after_timing_structure");
                    self.complete_structure();
                    self.checkpoint_and_react(None);
                }
            },
            StepPhase::Checkpoint => {
                // …and followed by a checkpoint (9.11.2).
                cite!("rule_checkpoint_after_instruction_resolution");
                self.checkpoint_and_react(None);
                // CR 9.6.7d: a timing structure step completed — clear the
                // static-condition no-effect throttle.
                cite!("rule_conditional_ability_static_condition_no_effect");
                self.throttled.clear();
                let Some(Frame::Structure(sf)) = self.frames.last_mut() else { return };
                if let Some(j) = sf.pending_jump.take() {
                    sf.cursor = j;
                } else {
                    sf.cursor += 1;
                }
                sf.phase = StepPhase::Enter;
            }
        }
    }

    fn set_structure_phase(&mut self, p: StepPhase) {
        if let Some(Frame::Structure(sf)) = self.frames.last_mut() {
            sf.phase = p;
        }
    }
    fn set_structure_phase_at(&mut self, idx: usize, p: StepPhase) {
        if let Some(Frame::Structure(sf)) = self.frames.get_mut(idx) {
            sf.phase = p;
        }
    }
    fn set_structure_jump_at(&mut self, idx: usize, j: usize) {
        if let Some(Frame::Structure(sf)) = self.frames.get_mut(idx) {
            sf.pending_jump = Some(j);
        }
    }
    fn set_structure_jump(&mut self, idx: usize) {
        if let Some(Frame::Structure(sf)) = self.frames.last_mut() {
            sf.pending_jump = Some(idx);
        }
    }

    fn eval_pred(&self, pred: BranchPred) -> bool {
        match pred {
            BranchPred::ActivePlayerHasClicks => {
                self.st.player(self.st.turn_side).clicks > 0
            }
            // 6.9.1f: "Does the Runner have a position corresponding to a
            // piece of ice?" — a vacated position (6.2.4) is a position with
            // no ice, and answers no.
            BranchPred::RunnerHasIcePosition => self
                .run_ctx()
                .map(|r| r.position.and_then(|p| self.ice_in_position(r.server, p)).is_some())
                .unwrap_or(false),
            BranchPred::ApproachedIceRezzed => self
                .run_ctx()
                .and_then(|r| self.approached_ice(r))
                .map(|ice| self.st.objects[&ice].faceup)
                .unwrap_or(false),
            BranchPred::UnbrokenSubsRemain => self.next_unbroken_sub().is_some(),
            BranchPred::MovedToNewPosition => {
                self.run_ctx().map(|r| r.moved_to_new_position).unwrap_or(false)
            }
            BranchPred::CandidatesRemain => {
                !self.restrict_candidates(self.breach_candidates_now()).is_empty()
            }
        }
    }

    pub fn run_ctx(&self) -> Option<&RunCtx> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Run(r), .. }) => Some(r),
            _ => None,
        })
    }
    fn run_ctx_mut(&mut self) -> Option<&mut RunCtx> {
        self.frames.iter_mut().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Run(r), .. }) => Some(r),
            _ => None,
        })
    }
    fn breach_ctx(&self) -> Option<&BreachCtx> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Breach(b), .. }) => Some(b),
            _ => None,
        })
    }
    fn breach_ctx_mut(&mut self) -> Option<&mut BreachCtx> {
        self.frames.iter_mut().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Breach(b), .. }) => Some(b),
            _ => None,
        })
    }

    /// CR 6.2.1: the ice protecting a server, INNERMOST FIRST. Vacant
    /// positions (6.2.4, awaiting 10.3.1i) contribute nothing.
    pub fn ice_at(&self, server: ServerId) -> Vec<ObjectId> {
        self.positions_at(server).iter().filter_map(|p| p.ice).collect()
    }

    /// CR 6.2.1: the server's sequence of positions, innermost first.
    pub fn positions_at(&self, server: ServerId) -> &[IcePosition] {
        self.st.ice.get(&server).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Which position (and whose server) a piece of ice occupies (6.2.1:
    /// exactly 1 at a time). `None` for ice that is not protecting a server —
    /// including hosted ice (6.2.1a).
    pub fn position_of_ice(&self, ice: ObjectId) -> Option<(ServerId, u64)> {
        cite!("rule_hosted_ice_has_no_position");
        self.st.ice.iter().find_map(|(&s, v)| {
            v.iter().find(|p| p.ice == Some(ice)).map(|p| (s, p.id))
        })
    }

    /// The ice occupying a named position, if any.
    pub fn ice_in_position(&self, server: ServerId, pos: u64) -> Option<ObjectId> {
        self.positions_at(server).iter().find(|p| p.id == pos).and_then(|p| p.ice)
    }

    /// CR 6.2.3: how many positions lie inward from this one — the number
    /// that decides whether two pieces of ice protecting different servers
    /// occupy the "same position".
    pub fn positions_inward_of(&self, server: ServerId, pos: u64) -> Option<usize> {
        cite!("rule_count_positions");
        self.positions_at(server).iter().position(|p| p.id == pos)
    }

    /// CR 6.2.3: the reference a "same position" criterion is measured
    /// against, as a count of positions inward.
    fn reference_position(
        &self,
        r: crate::instr::PositionRef,
        source: Option<ObjectId>,
    ) -> Option<usize> {
        let (server, pos) = match r {
            crate::instr::PositionRef::Source => {
                let src = source?;
                // 6.2.1a: a hosted card occupies no position of its own, so
                // the reference is its host's (1.13.12 puts it in the same
                // zone, which is how a hosted program names ice positions).
                self.position_of_ice(src).or_else(|| {
                    self.st.objects.get(&src).and_then(|o| o.host).and_then(|h| {
                        self.position_of_ice(h)
                    })
                })?
            }
            crate::instr::PositionRef::Runner => {
                let r = self.run_ctx()?;
                (r.server, r.position?)
            }
        };
        self.positions_inward_of(server, pos)
    }

    /// CR 6.2.2: create a new position in a server's sequence at `at`
    /// (innermost-first index), and return its identity. `at = len` is
    /// 6.2.2a's outermost, `at = 0` is 6.2.2b's innermost, and an index
    /// found from another ice's position is 6.2.2c's "directly inward".
    fn create_position(&mut self, server: ServerId, at: usize) -> u64 {
        cite!("rule_create_position");
        let id = self.next_position;
        self.next_position += 1;
        let v = self.st.ice.entry(server).or_default();
        let at = at.min(v.len());
        v.insert(at, IcePosition { id, ice: None });
        id
    }

    /// A piece of ice leaves whatever position it occupies. CR 6.2.4: the
    /// position does NOT cease here — it ceases at the next checkpoint's step
    /// 10.3.1i, and not even then if the Runner is standing in it.
    fn vacate_ice(&mut self, ice: ObjectId) {
        cite!("rule_destroy_position");
        for v in self.st.ice.values_mut() {
            for p in v.iter_mut() {
                if p.ice == Some(ice) {
                    p.ice = None;
                }
            }
        }
    }

    /// CR 6.2.2a: a piece of ice comes to protect `server` in a NEW outermost
    /// position — the position an instruction that does not explicitly
    /// specify one creates.
    pub fn place_ice_outermost(&mut self, ice: ObjectId, server: ServerId) {
        self.occupy_ice_position(ice, server, None);
    }

    /// A piece of ice comes to protect `server`. CR 6.2.2: it occupies the
    /// position `reserved` for it — by the install's destination declaration
    /// (8.5.16b), by a swap (6.2.2f) or by a move — and otherwise a NEW
    /// outermost position is created for it (6.2.2a, the default an
    /// instruction that does not explicitly specify a position gets).
    fn occupy_ice_position(
        &mut self,
        ice: ObjectId,
        server: ServerId,
        reserved: Option<(ServerId, u64)>,
    ) {
        // CR 6.2.1: a POSITION is occupied by a piece of ice. A card that is
        // merely in a server's ice zone because it is hosted on a piece of ice
        // there (1.13.5: a hosted card is in its host's zone) protects
        // nothing — it is not ice, so it never occupies a position, and the
        // Runner never approaches it.
        cite!("rule_position");
        if self.st.objects[&ice].printed.card_type != CardType::Ice {
            return;
        }
        let slot = match reserved {
            Some((s, id)) if s == server => Some(id),
            _ => None,
        };
        let existing = slot
            .filter(|&id| self.positions_at(server).iter().any(|p| p.id == id));
        let id = match existing {
            Some(id) => id,
            None => {
                cite!("rule_create_position_outermost");
                let at = self.positions_at(server).len();
                self.create_position(server, at)
            }
        };
        if let Some(v) = self.st.ice.get_mut(&server) {
            if let Some(p) = v.iter_mut().find(|p| p.id == id) {
                p.ice = Some(ice);
            }
        }
    }

    fn approached_ice(&self, r: &RunCtx) -> Option<ObjectId> {
        self.ice_in_position(r.server, r.position?)
    }

    /// Redirect the innermost run structure to another of its §11 steps.
    fn jump_run_to(&mut self, step: &str) {
        let idx = self.table(crate::timing::StructKind::Run).index_of(step);
        for f in self.frames.iter_mut().rev() {
            if let Frame::Structure(sf) = f {
                if matches!(sf.ctx, StructCtx::Run(_)) {
                    sf.pending_jump = Some(idx);
                    break;
                }
            }
        }
    }

    /// The §11 step id the innermost run structure is currently at.
    fn run_step_id(&self) -> Option<&str> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Structure(sf) if matches!(sf.ctx, StructCtx::Run(_)) => {
                self.table(sf.kind).steps.get(sf.cursor).map(|s| s.id.as_str())
            }
            _ => None,
        })
    }

    /// CR 6.2.7: changes to the piece of ice in the Runner's CURRENT position
    /// affect the progression of the run — differently during an encounter
    /// than outside one. These are consequences of state changes, not steps of
    /// any procedure, so the VM notices them where it notices every state
    /// change: at the checkpoint, before its own steps run (so that an
    /// encounter ending here is in the scan window step 10.3.1a reads, and so
    /// that a position the Runner has just left is vacant for 10.3.1i).
    pub(crate) fn apply_ice_change_to_run(&mut self) {
        cite!("rule_ice_change_current_position");
        let Some(enc) = self.st.encounter.as_ref().map(|e| e.ice) else {
            // 6.2.7a: outside an encounter, an approached piece of ice that
            // is uninstalled or moved to another position ends the approach
            // and the run continues to the Movement Phase. 6.2.7e: during the
            // Initiation and Movement Phases the same change does nothing —
            // the Runner is not moved and the timing step does not change,
            // which is what 6.2.6's position-as-element gives for free.
            cite!("rule_ice_change_during_movement");
            let approaching = matches!(
                self.run_step_id(),
                Some("step_approach_begins" | "step_approach_paw" | "step_approach_complete")
            );
            if approaching {
                let vacant = self
                    .run_ctx()
                    .map(|r| self.approached_ice(r).is_none())
                    .unwrap_or(false);
                if vacant {
                    cite!("rule_ice_change_approach_uninstall_move");
                    self.jump_run_to("step_pass_ice");
                }
            }
            return;
        };
        // 6.2.7 governs "the piece of ice in the Runner's CURRENT POSITION",
        // which is the ice the run's own Encounter Ice Phase was opened for.
        // A forced encounter (6.5.9a) is resolved outside the run's
        // progression — its ice need not be in the Runner's position and need
        // not be installed at all (9.1.8h) — so none of 6.2.7 applies to it,
        // and 6.5.9d says as much from the other direction.
        if self.encounter_ctx().map(|c| c.forced).unwrap_or(false) {
            cite!("rule_move_during_forced");
            return;
        }
        // 6.2.7c: uninstalled or derezzed while being encountered — the
        // Encounter Phase ends and the run continues to the Movement Phase.
        let gone = self
            .st
            .objects
            .get(&enc)
            .map(|o| !o.zone.is_installed() || !o.faceup)
            .unwrap_or(true);
        if gone {
            // The Encounter Ice Phase ends; the run (if there is one) is
            // already parked at its Movement Phase, and a FORCED encounter
            // ending here leaves the underlying run's timing alone (6.5.9d).
            cite!("rule_ice_change_encounter_uninstall_derez");
            self.abort_encounter_phase();
            return;
        }
        // 6.2.7d: the encountered ice moved to another position, or was
        // swapped with installed ice in another position — the Runner stays
        // WITH THE ICE and continues the run from its new position, which can
        // make another server the attacked one (6.1.2).
        let Some((server, pos)) = self.position_of_ice(enc) else { return };
        let stale = self.run_ctx().map(|r| (r.server, r.position) != (server, Some(pos)));
        if stale == Some(true) {
            cite!("rule_ice_change_encounter_move_swap");
            if let Some(r) = self.run_ctx_mut() {
                r.server = server;
                r.position = Some(pos);
            }
            if let Some((_, s, _)) = self.current_run.as_mut() {
                *s = server;
            }
        }
    }

    /// Build the contextual instruction for a step kind.
    fn step_instruction(&self, k: StepKind) -> Instruction {
        let side = self.st.turn_side;
        match k {
            StepKind::GainAllottedClicks => Instruction::GainAllottedClicks(side),
            StepKind::RefillRecurring => Instruction::RefillRecurring(side),
            StepKind::TurnFormallyBegins => Instruction::TurnFormallyBegins(side),
            StepKind::MandatoryDraw => Instruction::MandatoryDraw,
            StepKind::ActionPhaseEnds => Instruction::TurnComplete(side), // marker; real op in exec
            StepKind::DiscardToHandSize => Instruction::DiscardToHandSize(side),
            StepKind::LoseUnspentClicks => Instruction::LoseUnspentClicks(side),
            StepKind::TurnFormallyEnds => Instruction::TurnFormallyEnds(side),
            StepKind::AnnounceAttackedServer => {
                let server = self.run_ctx().map(|r| r.server).unwrap_or(ServerId::Hq);
                Instruction::AnnounceAttackedServer(server)
            }
            StepKind::FillBadPubFund => Instruction::FillBadPubFund,
            StepKind::RunFormallyBegins => Instruction::RunFormallyBegins,
            StepKind::SetPositionOutermost => Instruction::SetPositionOutermost,
            StepKind::ApproachIce => Instruction::ApproachIce,
            StepKind::EncounterIce => Instruction::EncounterIce,
            StepKind::PassIce => Instruction::PassIce,
            StepKind::JackOutChoice => Instruction::JackOutChoice,
            StepKind::MovePositionInward => Instruction::MovePositionInward,
            StepKind::ApproachServer => Instruction::ApproachServer,
            StepKind::DeclareRunSuccessful => Instruction::DeclareRunSuccessful,
            StepKind::BreachAttackedServer => {
                let server = self.run_ctx().map(|r| r.server).unwrap_or(ServerId::Hq);
                Instruction::BreachServer(server)
            }
            StepKind::CloseRunPriorityWindows => Instruction::CloseRunPriorityWindows,
            StepKind::EmptyBadPubFund => Instruction::EmptyBadPubFund,
            StepKind::DeclareRunUnsuccessfulIfApplicable => {
                Instruction::DeclareRunUnsuccessfulIfApplicable
            }
            StepKind::BreachBegins => Instruction::BreachBegins,
            StepKind::FlipArchivesFaceup => Instruction::FlipArchivesFaceup,
            StepKind::DetermineAccessLimit => Instruction::DetermineAccessLimit,
            StepKind::AccessChosenCandidate => Instruction::AccessChosenCandidate,
            StepKind::CardBecomesAccessed => Instruction::CardBecomesAccessed,
            StepKind::StealIfAgenda => Instruction::StealIfAgenda,
        }
    }

    /// Execute a structure step: pop the imminence (its atoms may have been
    /// modified by interrupts) and perform the step's state change.
    fn exec_step_instruction(&mut self, k: StepKind) {
        let imm = self.imminents.pop();
        self.changes.bump_group();
        let side = self.st.turn_side;
        match k {
            StepKind::GainAllottedClicks => {
                let n = self.st.player(side).allotted_clicks;
                self.st.player_mut(side).clicks += n;
                self.changes.record(GameChange::ClicksGained { side, amount: n });
            }
            StepKind::RefillRecurring => {
                // CR 1.10.5a/c: at step 5.6.1c/5.7.1c, before any ability
                // meets a turn-begins condition, every recurring card of the
                // active player is topped up to its printed number.
                cite!("rule_recurring_credits");
                cite!("rule_refill_recurring_credits");
                let ids: Vec<ObjectId> = self
                    .st
                    .objects
                    .values()
                    .filter(|o| {
                        o.controller == side
                            && card_active(o)
                            && o.printed.recurring_credits.is_some()
                    })
                    .map(|o| o.id)
                    .collect();
                for id in ids {
                    // 1.10.5d: recurring credits do not accumulate — the card
                    // is refilled UP TO the printed number, never past it, so
                    // the top-up is a set, not an add.
                    cite!("rule_recurring_credits_do_not_accumulate");
                    let n = self.st.objects[&id].printed.recurring_credits.unwrap();
                    let have = self.st.objects[&id].counter(CounterKind::Credit);
                    if have >= n {
                        continue;
                    }
                    self.st
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .counters
                        .insert(CounterKind::Credit, n);
                    self.changes.record(GameChange::CounterPlaced {
                        obj: id,
                        kind: CounterKind::Credit,
                        amount: n - have,
                    });
                }
            }
            StepKind::TurnFormallyBegins => {
                self.changes.record(GameChange::TurnBegan { side });
            }
            StepKind::MandatoryDraw => {
                cite!("rule_mandatory_draw");
                self.resolve_atoms_then(imm, |vm, _| {
                    vm.draw_cards(Side::Corp, 1, true);
                });
            }
            StepKind::ActionPhaseEnds => {
                self.changes.record(GameChange::ActionPhaseEnded { side });
            }
            StepKind::DiscardToHandSize => {
                self.discard_step(side);
            }
            StepKind::LoseUnspentClicks => {
                let n = self.st.player(side).clicks;
                self.st.player_mut(side).clicks = 0;
                if n > 0 {
                    self.changes.record(GameChange::ClicksLost { side, amount: n });
                }
            }
            StepKind::TurnFormallyEnds => {
                self.changes.record(GameChange::TurnEnded { side });
            }
            StepKind::AnnounceAttackedServer => {
                cite!("rule_attacked_server");
                let server = self.run_ctx().unwrap().server;
                let _ = server; // announced at initiation; recorded at RunBegan
            }
            StepKind::FillBadPubFund => {
                // CR 6.4.2 (`rule_bad_publicity_beginning_run`): 1 credit per
                // Corp bad publicity.
                cite!("rule_bad_publicity_fund");
                cite!("rule_bad_publicity_beginning_run");
                self.st.bp_fund = self.st.corp.bad_publicity;
            }
            StepKind::RunFormallyBegins => {
                let (run_id, server) = {
                    let r = self.run_ctx().unwrap();
                    (r.run_id, r.server)
                };
                self.current_run = Some((run_id, server, false));
                self.changes.record(GameChange::RunBegan { server });
            }
            StepKind::SetPositionOutermost => {
                cite!("rule_position_initial");
                let server = self.run_ctx().unwrap().server;
                let outermost = self.positions_at(server).last().map(|p| p.id);
                let r = self.run_ctx_mut().unwrap();
                r.position = outermost;
            }
            StepKind::ApproachIce => {
                let (server, pos) = {
                    let r = self.run_ctx().unwrap();
                    (r.server, r.position)
                };
                if let Some(ice) = pos.and_then(|p| self.ice_in_position(server, p)) {
                    self.changes.record(GameChange::IceApproached { ice });
                }
                if let Some(r) = self.run_ctx_mut() {
                    r.came_from_ice = true;
                    // 6.1.3e: a new approach starts a new "direct sequence";
                    // anything the run reached this phase from is behind it.
                    r.last_encounter = None;
                }
            }
            StepKind::EncounterIce => {
                // 6.9.3a: the ice this phase was opened for — the one in the
                // Runner's position for a run encounter, the named one for a
                // forced encounter (6.5.9a).
                let ice = self
                    .encounter_ctx()
                    .map(|c| c.ice)
                    .expect("the Encounter Ice Phase carries the ice it was opened for");
                self.begin_encounter(ice);
            }
            StepKind::PassIce => {
                let (came, server, pos, last) = {
                    let r = self.run_ctx().unwrap();
                    (r.came_from_ice, r.server, r.position, r.last_encounter)
                };
                if came {
                    if let Some(ice) = pos.and_then(|p| self.ice_in_position(server, p)) {
                        cite!("rule_pass_ice");
                        // 6.1.3e: the pass is "after" an encounter only when
                        // the two phases occurred in direct sequence, with
                        // this ice — an unrezzed ice passed straight out of
                        // the Approach Ice Phase is passed after no encounter
                        // at all, and 6.1.3f then has nothing to be true of.
                        cite!("rule_run_phase_after");
                        cite!("rule_pass_after_breaking");
                        let (after_encounter, fully_broken, subs_resolved) = match last {
                            Some((i, b, r)) if i == ice => (true, b, r),
                            _ => (false, false, false),
                        };
                        self.changes.record(GameChange::IcePassed {
                            ice,
                            after_encounter,
                            fully_broken,
                            subs_resolved,
                        });
                    }
                }
            }
            StepKind::JackOutChoice => {
                cite!("rule_jack_out_after_passing_ice");
                self.ask(Side::Runner, DecisionSpec::JackOut, DecisionCtx::JackOut);
            }
            StepKind::MovePositionInward => {
                cite!("rule_position_progression");
                // 6.2.5b/c: the next position moving inward is the element
                // before this one in the server's sequence; leaving the
                // innermost position leaves the Runner with none.
                let (server, pos) = {
                    let r = self.run_ctx().unwrap();
                    (r.server, r.position)
                };
                let inward = pos
                    .and_then(|p| self.positions_inward_of(server, p))
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| self.positions_at(server).get(i).map(|p| p.id));
                let had_position = pos.is_some();
                let r = self.run_ctx_mut().unwrap();
                match inward {
                    Some(next) => {
                        r.position = Some(next);
                        r.moved_to_new_position = true;
                    }
                    None => {
                        cite!("rule_no_position_after_innermost_ice");
                        if had_position {
                            r.position = None;
                        }
                        r.moved_to_new_position = false;
                    }
                }
            }
            StepKind::ApproachServer => {
                cite!("rule_no_position_after_approach_server");
                let server = self.run_ctx().unwrap().server;
                let r = self.run_ctx_mut().unwrap();
                r.position = None;
                r.came_from_ice = false;
                self.changes.record(GameChange::ServerApproached { server });
            }
            StepKind::DeclareRunSuccessful => {
                cite!("rule_successful_run");
                let server = self.run_ctx().unwrap().server;
                // 6.8.4a-adjacent: reaching the Success Phase at all is
                // remembered regardless of the declaration.
                if let Some(r) = self.run_ctx_mut() {
                    r.reached_success = true;
                }
                if let Some((_, _, s)) = self.current_run.as_mut() {
                    *s = false;
                }
                let prohibited = self.run_success_prohibited(server);
                if !prohibited {
                    if let Some(r) = self.run_ctx_mut() {
                        r.declared_successful = true;
                    }
                    if let Some((_, _, s)) = self.current_run.as_mut() {
                        *s = true;
                    }
                    self.changes.record(GameChange::RunDeclaredSuccessful { server });
                    // CR 6.7.4: "If successful" means "after the run created
                    // this way becomes successful", so the clause the
                    // initiating effect carried becomes pending HERE — an
                    // ordinary conditional instance, offered by the reaction
                    // window this step's checkpoint opens.
                    self.pend_if_successful(server);
                }
            }
            StepKind::BreachAttackedServer => {
                let server = self.run_ctx().unwrap().server;
                // A replaced breach (Security-Testing class, 9.9.11a) has
                // its atom removed — the breach never happens.
                self.resolve_atoms_then(imm, |vm, _| vm.push_breach(server));
                return; // imm consumed by resolve_atoms_then
            }
            StepKind::CloseRunPriorityWindows => {
                // 6.8.2: windows from before "end the run" were closed when
                // the ETR effect unwound the stack; nothing remains here.
                cite!("rule_run_ends_process_priority_windows");
            }
            StepKind::EmptyBadPubFund => {
                cite!("rule_bad_publicity_gone_in_run_ends_phase");
                self.st.bp_fund = 0;
            }
            StepKind::DeclareRunUnsuccessfulIfApplicable => {
                cite!("rule_unsuccessful_run");
                cite!("rule_not_unsuccessful_when_reached_success_phase");
                let (server, reached) = {
                    let r = self.run_ctx().unwrap();
                    (r.server, r.reached_success)
                };
                if !reached {
                    self.changes.record(GameChange::RunDeclaredUnsuccessful { server });
                }
            }
            StepKind::BreachBegins => {
                let server = self.breach_ctx().unwrap().server;
                self.changes.record(GameChange::BreachBegan { server });
                self.compute_candidates();
            }
            StepKind::FlipArchivesFaceup => {
                cite!("rule_breaching_archives");
                let server = self.breach_ctx().unwrap().server;
                if server == ServerId::Archives {
                    let ids: Vec<ObjectId> =
                        self.st.discard[&Side::Corp].iter().copied().collect();
                    for id in ids {
                        self.st.objects.get_mut(&id).unwrap().faceup = true;
                    }
                }
            }
            StepKind::DetermineAccessLimit => {
                cite!("rule_number_of_accesses");
                let server = self.breach_ctx().unwrap().server;
                let mut n = match server {
                    ServerId::Hq | ServerId::Rnd => 1,
                    _ => 0,
                };
                // Maker's-Eye-class additional accesses.
                for l in &self.lingering {
                    if let Payload::AdditionalAccess { server: s, extra } = l.payload {
                        if s == server {
                            n += extra;
                        }
                    }
                }
                if let Some(b) = self.breach_ctx_mut() {
                    b.remaining_from_zone = n;
                }
                // Populate the first hand/deck candidate (7.4.6/7.4.7).
                self.refresh_candidates_after_access();
            }
            StepKind::AccessChosenCandidate => {
                let card = self.breach_ctx().unwrap().chosen.expect("candidate chosen");
                // 7.4.3 example 1: a replaced access never happens — but the
                // chosen candidate stays consumed.
                let suppressed = imm
                    .as_ref()
                    .map(|im| {
                        !im.atoms.is_empty()
                            && !im.atoms.iter().any(|a| a.occurs_at_resolution())
                    })
                    .unwrap_or(false);
                if suppressed {
                    cite!("rule_candidates_already_accessed");
                    if let Some(b) = self.breach_ctx_mut() {
                        b.chosen = None;
                    }
                    self.refresh_candidates_after_access();
                    return;
                }
                // 7.4.3 example 2 (Gagarin class): an additional cost to
                // access — declining means the access does not occur, but
                // the card still ceased to be a candidate.
                let access_cost = self.additional_access_cost(card);
                if !access_cost.is_free() {
                    cite!("rule_candidates_already_accessed");
                    self.ask(
                        Side::Runner,
                        DecisionSpec::NestedCost { cost: access_cost },
                        DecisionCtx::AccessCost(card),
                    );
                    return;
                }
                self.push_access(card);
            }
            StepKind::CardBecomesAccessed => {
                cite!("rule_accessing");
                let card = self.access_card().unwrap();
                self.st.accessed = Some(card);
                self.changes.record(GameChange::CardAccessed { obj: card });
                // 7.3.6: an ability counting accesses "only includes accesses
                // that are actually performed" — this is the only place one
                // is, so it is the only place the count moves.
                cite!("rule_number_of_accesses");
                if let (Some(run), Some((r, n))) = (self.current_run, self.st.run_accesses) {
                    if run.0 == r {
                        self.st.run_accesses = Some((r, n + 1));
                    }
                }
            }
            StepKind::StealIfAgenda => {
                cite!("rule_after_mid_access_agenda");
                let card = self.access_card().unwrap();
                if self.st.objects[&card].printed.card_type == CardType::Agenda {
                    let total = self.steal_cost_of(card);
                    // 1.16.1b: a cost that cannot be paid is not a choice —
                    // it is never put to the Runner, and the agenda is not
                    // stolen (Obokata vs Guru Davinder).
                    if !total.is_free() && !self.cost_payable(Side::Runner, card, &total) {
                        cite!("rule_cost_interrupt_static_mandatory");
                    } else if total.is_free() {
                        // 7.2.3/1.17.3: stealing is mandatory with no
                        // additional cost.
                        cite!("rule_decline_to_steal");
                        self.steal_agenda(card);
                    } else {
                        // 1.16.10a: the Runner may pay or decline.
                        self.ask(
                            Side::Runner,
                            DecisionSpec::NestedCost { cost: total },
                            DecisionCtx::StealCost(card),
                        );
                    }
                }
            }
        }
    }

    /// Run a step's effect unless its imminence was fully prevented: with no
    /// atoms it proceeds structurally; with atoms, at least one must occur at
    /// resolution (9.9.7d).
    fn resolve_atoms_then(
        &mut self,
        imm: Option<ImminentWrap>,
        f: impl FnOnce(&mut Vm, Option<&ImminentWrap>),
    ) {
        let proceed = imm
            .as_ref()
            .map(|im| im.atoms.is_empty() || im.atoms.iter().any(|a| a.occurs_at_resolution()))
            .unwrap_or(true);
        if proceed {
            f(self, imm.as_ref());
        }
    }

    fn access_card(&self) -> Option<ObjectId> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Access(a), .. }) => Some(a.card),
            _ => None,
        })
    }

    /// CR 1.16.10: aggregate the additional costs to steal an agenda —
    /// printed plus active statics — into one all-at-once payment.
    pub fn steal_cost_of(&self, card: ObjectId) -> Cost {
        cite!("rule_additional_cost");
        let mut total = self.st.objects[&card]
            .printed
            .additional_steal_cost
            .clone()
            .unwrap_or_default();
        for (_, d) in self.active_statics() {
            if let StaticDecl::AdditionalStealCost(c) = d {
                total = total.plus(&c);
            }
        }
        total
    }

    /// CR 1.16.10: the additional cost to SCORE this agenda — printed plus
    /// any active declaration — as one all-at-once payment.
    pub fn score_cost_of(&self, card: ObjectId) -> Cost {
        cite!("rule_additional_cost");
        self.st.objects[&card].printed.additional_score_cost.clone().unwrap_or_default()
    }

    /// CR 1.16.1/1.16.1c: is the (S) option available for this agenda? An
    /// additional cost to score that cannot be paid — or cannot be paid
    /// without breaking the restriction that made the agenda scorable in the
    /// first place — means the Corp cannot score it.
    fn score_cost_payable(&self, card: ObjectId) -> bool {
        let cost = self.score_cost_of(card);
        cost.is_free()
            || self.cost_payable_under(
                Side::Corp,
                card,
                &cost,
                Some(&PaymentRestriction::ScoreRequirement(card)),
            )
    }

    /// CR 1.2.2 / 1.17.3: is the Corp prohibited from scoring this agenda?
    /// "If a rule or ability directs something to happen, but another effect
    /// states that it cannot happen, the 'cannot' ability takes precedence" —
    /// so a Clot-class declaration removes the (S) option rather than
    /// competing with it. The description is re-read every time the window
    /// opens, which is what makes "during the same turn they installed that
    /// agenda" lift by itself at the start of the next turn.
    fn score_prohibited(&self, card: ObjectId) -> bool {
        cite!("rule_cannot_precedence");
        cite!("rule_score_not_an_action");
        let Some(o) = self.st.objects.get(&card) else { return false };
        self.active_statics().iter().any(|(obj, d)| match d {
            StaticDecl::CannotScoreMatching { criteria } => {
                criteria.iter().all(|f| self.filter_matches(o, *f, Some(*obj)))
            }
            _ => false,
        })
    }

    fn run_success_prohibited(&self, server: ServerId) -> bool {
        // Crisium-class static: "Runs on this server cannot be declared
        // successful."
        self.active_statics().iter().any(|(obj, d)| {
            matches!(d, StaticDecl::RunsNotDeclaredSuccessful)
                && self.server_of(*obj) == Some(server)
        })
    }

    pub fn server_of(&self, obj: ObjectId) -> Option<ServerId> {
        match self.st.objects.get(&obj)?.zone {
            Zone::Root(s) | Zone::Ice(s) => Some(s),
            _ => None,
        }
    }

    /// A **citation anchor**: the rule below is realised structurally — by the
    /// shape of the types and the records, not at one call site — so this is
    /// where the traceability registry records it.
    ///
    /// A **citation anchor**: CR 5.2's action model.
    ///
    /// 5.2.1: "an action is any paid ability where the cost begins with a
    /// [click] symbol" — `AbilityDef::is_action`. 5.2.2a: once initiated, an
    /// action must be completed before the game advances; 5.2.2b: a timing
    /// structure initiated during it keeps it incomplete until that structure
    /// finishes; 5.2.2c/d: an ability meeting its condition because of the
    /// action resolves after the action, which is where the action window's
    /// own checkpoint puts it. 5.2.3: each player has basic actions they can
    /// always perform. 5.2.4: actions are taken only during the action phase,
    /// which is where the §11 turn table opens the 9.2.6 window. 5.2.5: what
    /// makes two actions the same or different — `ActionIdentity`.
    pub fn action_model() {
        cite!("rule_action_definition");
        cite!("rule_action_completion");
        cite!("rule_action_timing_structure_completion");
        cite!("rule_action_conditional_ability_trigger");
        cite!("rule_finish_action_trigger_condition");
        cite!("rule_basic_actions");
        cite!("rule_actions_outside_action_phase");
        cite!("rule_same_different_actions");
        cite!("runner_basic_action_credit");
        cite!("runner_basic_action_card");
    }

    /// CR 9.1.6: a player USES an ability whenever they choose to resolve an
    /// optional ability or an optional part of one, and 9.1.6b puts the moment
    /// at the end of the relevant optional effects — which is where
    /// `AbilityUsed` is recorded.
    pub fn ability_use_model() {
        cite!("rule_using");
        cite!("rule_conditional_ability_used_condition");
    }

    /// CR 4.6.6: a SERVER is a set of locations the Corp installs into and the
    /// Runner runs against. 4.6.6c: the two types are central (4.6.7a: three
    /// of them, one per Corp zone) and remote (4.6.8a: the only kind whose
    /// root can hold an asset or agenda; 4.6.8b: created by declaring one as
    /// an install destination; 4.6.8e: ceases to exist at a checkpoint once
    /// nothing is installed in or protecting it). 4.6.6d: every server can
    /// have ice protecting it; 4.6.6f: the cards in a root are kept together
    /// and their order carries no meaning; 4.6.6b: an installed Corp card
    /// cannot leave its server without explicit direction.
    pub fn remote_servers(&self) -> std::collections::BTreeSet<ServerId> {
        cite!("rule_server");
        cite!("rule_server_types");
        cite!("rule_server_ice");
        cite!("rule_server_root_order");
        cite!("rule_corp_cards_cannot_be_moved");
        cite!("rule_three_central_servers");
        cite!("rule_remote_server");
        cite!("rule_creating_remote_servers");
        cite!("rule_remote_server_cease_to_exist");
        cite!("rule_play_area_corp_cards_distinct_server");
        self.remote_servers_inner()
    }

    /// CR 9.10.3: the choice `source` is currently maintaining under `key`,
    /// if the lingering effect that remembers it is still alive.
    pub fn maintained_choice(
        &self,
        source: ObjectId,
        key: &str,
    ) -> Option<crate::lingering::ChoiceValue> {
        cite!("rule_lingering_effect_maintaining_choice_duration_other_cases");
        self.lingering.iter().rev().find_map(|l| match &l.payload {
            Payload::MaintainedChoice { key: k, choice } if *k == key && l.source == source => {
                Some(*choice)
            }
            _ => None,
        })
    }

    // ------------------------------------------------------------------
    // §10.2 information: what each player is entitled to know
    // ------------------------------------------------------------------

    /// CR 10.2.2a / 10.2.3a: is this card's IDENTITY — its front face —
    /// available to `side`?
    ///
    /// The answer is derived from the state, in the order the CR gives the
    /// entitlements:
    ///
    /// * 10.2.2b — a game effect has SHOWN it to this player (looking 1.21.2,
    ///   revealing 1.21.3, exposing 1.21.4), recorded in `CoreState::seen`.
    /// * 7.3.1a — the Runner accessed it during the breach in progress, so it
    ///   "remains visible to the Runner for the remainder of the breach".
    ///   That entitlement survives the card MOVING, which is the exception
    ///   1.21.6 points at: the Runner watched the card leave.
    /// * otherwise the zone it is in says, per §4.
    pub fn identity_visible_to(&self, id: ObjectId, side: Side) -> bool {
        cite!("rule_hidden_information");
        cite!("rule_open_information");
        let Some(o) = self.st.objects.get(&id) else { return false };
        if self.st.seen.shown(side, id) {
            cite!("rule_bluffing");
            return true;
        }
        // 7.1.2: "While the Runner is accessing a card, the Runner is allowed
        // to look at that card, even if it would normally not be visible to
        // them" — and the Corp may too, EXCEPT from R&D, which falls out of
        // 4.2.2 below without a clause of its own.
        if side == Side::Runner && self.st.accessed == Some(id) {
            cite!("rule_accessing");
            cite!("rule_accessing_who_can_look");
            return true;
        }
        // 7.3.1a: an accessed card remains visible to the Runner for the
        // remainder of the breach — including after it has been moved out of
        // the zone it was accessed in. That is the exception 1.21.6 points to
        // when it ends a disclosure at the card's next move: the Runner
        // watched this one leave.
        if side == Side::Runner && self.breach_ctx().is_some_and(|b| b.accessed.contains(&id)) {
            cite!("rule_visibility_after_access");
            return true;
        }
        match o.zone {
            // 4.3.2: "A player may look at the cards in their own hand, but
            // not at any of the cards in their opponent's hands."
            Zone::Hand(s) => {
                cite!("rule_hand_secret");
                s == side
            }
            // 4.2.2: decks are kept hidden from BOTH players — the Corp may
            // not look at R&D any more than the Runner may.
            Zone::Deck(_) => {
                cite!("rule_deck_hidden");
                false
            }
            // 4.4.6c: the faceup cards in Archives are open information; the
            // facedown ones are visible only to the Corp. 4.4.7b: the whole
            // heap is open information.
            Zone::Discard(Side::Corp) => {
                cite!("rule_archives_faceup_open_info");
                o.faceup || side == Side::Corp
            }
            Zone::Discard(Side::Runner) => {
                cite!("rule_heap_open_info");
                true
            }
            // 4.5: score areas are open — an agenda's points are the score.
            Zone::ScoreArea(_) => true,
            // 1.21.1: faceup cards in the play area are freely visible to all
            // players; 1.21.2a: "a player may look at facedown cards they
            // control at any time".
            Zone::Root(_) | Zone::Ice(_) | Zone::Rig | Zone::PlayArea(_) => {
                cite!("rule_faceup_facedown");
                cite!("rule_look_at_controlled_facedown");
                o.faceup || o.controller == side
            }
            // 4.8.6: cards set aside by an ability are faceup unless the
            // ability said facedown; 4.8.7/8.3.3a: a facedown group belongs to
            // the player carrying that effect out, and the opponent "cannot
            // look at the set-aside cards during this process".
            Zone::SetAside => {
                cite!("rule_set_aside_default_faceup");
                cite!("rule_arrange_opponent");
                match o.set_aside_group {
                    Some(g) => g.by == side,
                    None => o.faceup || o.controller == side,
                }
            }
            Zone::RemovedFromGame => o.faceup || o.controller == side,
            Zone::Bank => true,
        }
    }

    /// CR 10.2: one card as `side` sees it (`Unseen` for hidden identities).
    pub fn card_view(&self, id: ObjectId, side: Side) -> crate::view::CardView {
        if self.identity_visible_to(id, side) {
            crate::view::CardView::Seen(id)
        } else {
            crate::view::CardView::Unseen
        }
    }

    /// CR 4.8.7: the facedown set-aside groups in existence, in creation
    /// order, each with the cards it holds.
    pub fn set_aside_groups(&self) -> Vec<(crate::view::SetAsideGroup, Vec<ObjectId>)> {
        cite!("rule_facedown_set_aside_distinct_groups");
        cite!("rule_hosted_cards_treated_as_group");
        let mut out: Vec<(crate::view::SetAsideGroup, Vec<ObjectId>)> = Vec::new();
        for (id, o) in &self.st.objects {
            if o.zone != Zone::SetAside {
                continue;
            }
            let Some(g) = o.set_aside_group else { continue };
            match out.iter_mut().find(|(gg, _)| gg.id == g.id) {
                Some((_, v)) => v.push(*id),
                None => out.push((g, vec![*id])),
            }
        }
        out.sort_by_key(|(g, _)| g.id);
        out
    }

    /// CR §10.2: the whole game state as `side` is entitled to see it.
    ///
    /// Everything here is derived; a `View` is a snapshot for asserting on,
    /// never a second copy of the state. The number of cards in every zone is
    /// present for both players because 10.2.3a makes counts open information
    /// even where the identities are not.
    pub fn view_of(&self, side: Side) -> crate::view::View {
        cite!("sec_information");
        cite!("rule_hidden_or_open_information");
        cite!("rule_hand_size_open_info");
        cite!("rule_deck_size_open_info");
        cite!("rule_discard_pile_open_info");
        let remotes: Vec<ServerId> = self.remote_servers().into_iter().collect();
        let zones = crate::view::viewable_zones(&remotes)
            .into_iter()
            .map(|z| {
                let cards: Vec<crate::view::CardView> = self
                    .cards_in_zone(z)
                    .into_iter()
                    .map(|c| self.card_view(c, side))
                    .collect();
                (z, cards)
            })
            .collect();
        let groups = self
            .set_aside_groups()
            .into_iter()
            .map(|(g, cards)| {
                (g.id, cards.into_iter().map(|c| self.card_view(c, side)).collect())
            })
            .collect();
        // 10.2.3b: a maintained choice was announced (1.15.2) and stays
        // available to both players — "open information cannot be hidden from
        // an opponent", and the opponent may ask again later.
        cite!("rule_cannot_hide_open_info");
        let choices = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::MaintainedChoice {
                    key,
                    choice: crate::lingering::ChoiceValue::Object(o),
                } => Some((*key, *o)),
                _ => None,
            })
            .collect();
        crate::view::View {
            side,
            zones,
            groups,
            choices,
            credits: vec![
                (Side::Corp, self.st.corp.credits),
                (Side::Runner, self.st.runner.credits),
            ],
        }
    }

    /// CR 4.6.6i: what an ability on `obj` means by **"this server"**.
    ///
    /// Three readings, in the order the rule gives them:
    /// 1. 4.6.6k — a hosted object reads its HOST's server.
    /// 2. The card is in a server, its root, or protecting it: that server.
    /// 3. The card has LEFT one: "the server associated with the previous
    ///    location of the card" (`last_server`) — which is why a trashed
    ///    Warroid Tracker still means the server it was trashed from and not
    ///    Archives, and why a Border-Control-class subroutine resolved after
    ///    the ice was trashed counts the ice of the server it protected
    ///    (without counting itself, since it no longer protects it).
    /// 4. Otherwise the central server corresponding to the zone the card is
    ///    in, which is the parenthesis in 4.6.6i's first sentence — a card in
    ///    Archives that never left a server says "Archives".
    ///
    /// Approximation (deviation 38): the rule scopes reading 3 to abilities
    /// *initiated by* the move. The kernel applies it to any ability of a card
    /// that has left a server, because the only abilities that resolve from
    /// such a card are the ones 9.1.8 keeps active across the move — which
    /// are exactly the move-initiated ones.
    pub fn this_server(&self, obj: ObjectId) -> Option<ServerId> {
        cite!("rule_this_server");
        let o = self.st.objects.get(&obj)?;
        if let Some(h) = o.host {
            cite!("rule_host_server");
            return self.this_server(h);
        }
        match o.zone {
            Zone::Root(s) | Zone::Ice(s) => Some(s),
            _ if o.last_server.is_some() => o.last_server,
            Zone::Hand(Side::Corp) => Some(ServerId::Hq),
            Zone::Deck(Side::Corp) => Some(ServerId::Rnd),
            Zone::Discard(Side::Corp) => Some(ServerId::Archives),
            _ => None,
        }
    }

    /// CR 9.6.5c: do the additional requirements listed inside a trigger
    /// condition hold right now? They are part of the CONDITION, so they gate
    /// the pending instance being created at all — both when the stipulation
    /// really occurs and when 9.6.14d resolves the ability by class.
    pub fn trigger_requirements_met(&self, cond: &crate::ability::TriggerCond) -> bool {
        self.trigger_requirements_met_for(cond, None)
    }

    pub fn trigger_requirements_met_for(
        &self,
        cond: &crate::ability::TriggerCond,
        source: Option<ObjectId>,
    ) -> bool {
        cite!("rule_condition_requirements_part_of_condition");
        crate::ability::trigger_requirements(cond)
            .iter()
            .all(|r| self.state_requirement_holds_for(r, source))
    }

    /// CR 9.6.14d: mark the abilities of `obj` in the named class pending, as
    /// though the class's stipulation had occurred. Returns the instances
    /// created — empty when the card has no ability in that class, or when an
    /// additional requirement of its trigger condition is not met by the game
    /// state (in which case the ability cannot become pending at all).
    fn pend_abilities_by_class(
        &mut self,
        obj: ObjectId,
        class: crate::ability::AbilityClass,
    ) -> Vec<u64> {
        cite!("rule_instructed_to_resolve_conditional_ability");
        let Some(o) = self.st.objects.get(&obj) else { return Vec::new() };
        let controller = o.controller;
        let threat = self.threat_level();
        let encountered = self.st.encounter.as_ref().map(|e| e.ice);
        let accessed = self.st.accessed;
        let matching: Vec<(usize, AbilityDef)> = o
            .printed
            .abilities
            .iter()
            .enumerate()
            .filter(|(_, a)| crate::ability::ability_in_class(a, class))
            .map(|(i, a)| (i, a.clone()))
            .collect();
        let mut out = Vec::new();
        for (index, def) in matching {
            // 9.1.7/9.1.9: an ability that is not active, or that the card no
            // longer has, does nothing — an effect naming its class cannot
            // resurrect it.
            if !self.ability_present(obj, index)
                || !ability_active(&self.st.objects[&obj], &def, encountered, accessed, threat)
            {
                continue;
            }
            // 9.6.14d: "Any additional requirements of the trigger condition
            // in question must still be met by the game state."
            if let Some(crate::ability::Condition::Trigger(cond)) = &def.condition {
                if !self.trigger_requirements_met_for(cond, Some(obj)) {
                    continue;
                }
            }
            let mandatory = !def.optional;
            let id = self.next_instance_id();
            cite!("rule_pending_instances");
            self.instances.insert(
                id,
                AbilityInstance {
                    id,
                    ability: AbilityRef { obj, index },
                    def,
                    controller,
                    mandatory,
                    window: None,
                    hangover: false,
                    independent: false,
                    source_generation: self.generation(obj),
                    occurrence_group: 0,
                    from_lingering: None,
                    run_id: self.current_run.map(|(r, _, _)| r),
                },
            );
            out.push(id);
        }
        out
    }

    /// CR 6.7.4: the "If successful" ability the effect that initiated this
    /// run carried becomes pending, as a conditional instance, exactly when
    /// the run is declared successful. CR 6.7.4a: it is tied to the servers
    /// that effect allowed, so a run moved (6.1.2d) outside that set does not
    /// meet the condition, while a move WITHIN it does.
    fn pend_if_successful(&mut self, server: ServerId) {
        let Some(c) = self.run_ctx().and_then(|r| r.if_successful.clone()) else { return };
        cite!("rule_if_successful");
        cite!("rule_if_successful_tied_to_server");
        if !c.allowed.allows(server) {
            return;
        }
        let label = self
            .st
            .objects
            .get(&c.source.obj)
            .map(|o| o.printed.name)
            .unwrap_or("if successful");
        let def = AbilityDef {
            kind: AbilityKind::Conditional,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: c.effects.clone(),
            statics: Vec::new(),
            optional: false,
            timing: None,
            label,
        };
        let id = self.next_instance_id();
        cite!("rule_pending_instances");
        let gen = self.generation(c.source.obj);
        self.instances.insert(
            id,
            AbilityInstance {
                id,
                ability: c.source,
                def,
                controller: c.controller,
                mandatory: true,
                window: None,
                hangover: false,
                independent: false,
                source_generation: gen,
                occurrence_group: 0,
                from_lingering: None,
                run_id: self.current_run.map(|(r, _, _)| r),
            },
        );
        self.pending_from_effect.push(id);
        // 6.7.4: the clause belongs to the run, and it fires once.
        if let Some(r) = self.run_ctx_mut() {
            r.if_successful = None;
        }
    }

    /// CR 4.6.8f: may the Corp create a new remote server right now? An
    /// active limit forbids it once the limit is reached. The declaration is
    /// a restriction (9.3.4), so it applies to the *destination declaration*
    /// at step 8.5.16b: an install that names a new remote when the limit is
    /// reached has no identifiable destination and does nothing (8.5.14).
    pub fn can_create_new_remote(&self) -> bool {
        cite!("rule_limit_remote_servers");
        let remotes = self.remote_servers().len() as i64;
        self.active_statics()
            .iter()
            .filter_map(|(_, d)| match d {
                StaticDecl::RemoteServerLimit(n) => Some(*n as i64),
                _ => None,
            })
            .all(|limit| remotes < limit)
    }

    /// CR 4.6.8d: the remote servers that exist — those with at least one card
    /// in their root or protecting them.
    fn remote_servers_inner(&self) -> std::collections::BTreeSet<ServerId> {
        cite!("rule_remote_server_existence");
        self.st
            .objects
            .values()
            .filter_map(|o| match o.zone {
                Zone::Root(s @ ServerId::Remote(_)) | Zone::Ice(s @ ServerId::Remote(_))
                    if self.is_installed(o) =>
                {
                    Some(s)
                }
                _ => None,
            })
            .collect()
    }

    /// All (source object, declaration) pairs of active static abilities.
    pub fn active_statics(&self) -> Vec<(ObjectId, StaticDecl)> {
        cite!("rule_static_ability");
        let mut out = Vec::new();
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Static {
                    continue;
                }
                // 9.3.7a: a static ability's declarations apply while any
                // condition stated in the ability holds (9.1.2b scoping, the
                // Attini class).
                if let Some(Condition::Static(sc)) = &a.condition {
                    if !self.static_cond_holds(o.id, sc) {
                        continue;
                    }
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                {
                    continue;
                }
                for d in &a.statics {
                    out.push((o.id, d.clone()));
                }
            }
        }
        out
    }

    /// CR 9.6.7 / 9.3.7a: does the static condition stated by an ability of
    /// `obj` hold right now?
    pub fn static_cond_holds(&self, obj: ObjectId, cond: &StaticCond) -> bool {
        cite!("rule_conditional_ability_with_static_condition");
        match cond {
            StaticCond::HostStrengthAtMost(n) => self
                .st
                .objects
                .get(&obj)
                .and_then(|o| o.host)
                .and_then(|h| self.effective_strength(h))
                .map(|s| s <= *n)
                .unwrap_or(false),
            // 9.1.2b: an ability of this card "is resolving" — its frame is
            // on the stack, from its first instruction becoming imminent
            // until its last has finished resolving. Any interrupt window
            // opened for one of its instructions is nested ABOVE that frame,
            // so it is inside the scope, which is the whole point of the rule.
            StaticCond::SourceAbilityResolving => {
                cite!("rule_is_resolving");
                self.frames
                    .iter()
                    .any(|f| matches!(f, Frame::Ability(af) if af.source.obj == obj))
            }
            // 7.4.2b: only during the run in progress, and only once an
            // access has actually been performed in it.
            StaticCond::RunnerHasAccessedCardThisRun => {
                cite!("rule_prohibiting_access_to_1");
                self.current_run.is_some() && self.accesses_this_run() > 0
            }
            // 4.5: the score area is a zone, one per player; 9.1.8a keeps a
            // card there active whichever score area it is in, so which one
            // is the whole content of the condition.
            StaticCond::SourceInScoreAreaOf(side) => {
                cite!("sec_score_area");
                self.st.objects.get(&obj).map(|o| o.zone == Zone::ScoreArea(*side)).unwrap_or(false)
            }
        }
    }

    /// CR 10.11.1/10.11.1a: the server designated as the mark, if any. There
    /// is only ever one, shared by every card that refers to it.
    pub fn mark(&self) -> Option<(ServerId, usize)> {
        cite!("rule_mark");
        cite!("rule_only_one_mark");
        self.lingering.iter().find_map(|l| match l.payload {
            Payload::MarkDesignation { server, since } => Some((server, since)),
            _ => None,
        })
    }

    /// CR 7.3.8: is a breach in progress? A breach that would begin now takes
    /// place when the current one ends instead.
    fn breach_in_progress(&self) -> bool {
        self.frames
            .iter()
            .any(|f| matches!(f, Frame::Structure(StructureFrame { ctx: StructCtx::Breach(_), .. })))
    }

    /// Is ability index `i` on `obj` present after gains/losses (9.1.9)?
    pub fn ability_present(&self, obj: ObjectId, i: usize) -> bool {
        let effects = self.char_effects();
        let eff = crate::object::compute_effective(&self.st.objects, &effects, obj);
        eff.ability_present.get(i).copied().unwrap_or(true)
    }

    /// Gather characteristic effects from active statics + lingering value
    /// modifiers (the 9.12.1d/e pipeline input).
    pub fn char_effects(&self) -> Vec<crate::object::CharEffect> {
        use crate::object::{CharEffect, CharOp};
        let mut out = Vec::new();
        let threat = self.gather_threat_level();
        for o in self.st.objects.values() {
            for a in &o.face().abilities {
                if a.kind != AbilityKind::Static {
                    continue;
                }
                // 9.1.7/9.1.8 + 9.3.6f: a static ability contributes
                // characteristic modifications only while it is ACTIVE, which
                // `active_statics` has always checked and this pass did not —
                // it gathered behind `card_active` alone, so a `[threat N]`
                // ability modified strength and subtypes at threat 0. Every
                // 9.1.8 exception now reaches characteristics too.
                cite!("rule_threat_flag");
                if !ability_active(
                    o,
                    a,
                    self.st.encounter.as_ref().map(|e| e.ice),
                    self.st.accessed,
                    threat,
                ) {
                    continue;
                }
                if let Some(Condition::Static(sc)) = &a.condition {
                    if !self.static_cond_holds(o.id, sc) {
                        continue;
                    }
                }
                for d in &a.statics {
                    match d {
                        StaticDecl::StrengthMod { target_self, delta } => {
                            let target = if *target_self { Some(o.id) } else { o.host };
                            if let Some(t) = target {
                                out.push(CharEffect {
                                    source: o.id,
                                    target: t,
                                    op: if *delta >= 0 {
                                        CharOp::IncreaseStrength(*delta)
                                    } else {
                                        CharOp::DecreaseStrength(-*delta)
                                    },
                                });
                            }
                        }
                        StaticDecl::SubtypeModSelf { add, remove } => {
                            cite!("rule_add_remove_subtypes");
                            for t in add {
                                out.push(CharEffect {
                                    source: o.id,
                                    target: o.id,
                                    op: CharOp::AddSubtype(t),
                                });
                            }
                            for t in remove {
                                out.push(CharEffect {
                                    source: o.id,
                                    target: o.id,
                                    op: CharOp::RemoveSubtype(t),
                                });
                            }
                        }
                        StaticDecl::RemoveAbilitiesOf(rel) => {
                            cite!("rule_lose_ability");
                            let targets: Vec<ObjectId> = match rel {
                                crate::ability::HostRelation::Host => o.host.into_iter().collect(),
                                // 1.13.9: hosting is not transitive, so only
                                // the cards hosted DIRECTLY on the source.
                                crate::ability::HostRelation::Hosted => o.hosted.clone(),
                            };
                            for t in targets {
                                out.push(CharEffect {
                                    source: o.id,
                                    target: t,
                                    op: CharOp::RemoveAllAbilities,
                                });
                            }
                        }
                        StaticDecl::RemoveAbilitiesOfMatching { criteria } => {
                            // 9.1.9a: every card the description reaches loses
                            // all of its abilities. The criteria are read
                            // shallowly here for the same reason
                            // `GainSubtypesOf`'s are (deviation 2b).
                            cite!("rule_lose_ability");
                            for other in self.st.objects.values() {
                                if other.id != o.id
                                    && criteria
                                        .iter()
                                        .all(|f| self.filter_matches_shallow(other, *f, Some(o.id)))
                                {
                                    out.push(CharEffect {
                                        source: o.id,
                                        target: other.id,
                                        op: CharOp::RemoveAllAbilities,
                                    });
                                }
                            }
                        }
                        StaticDecl::GainSubtypesOf { criteria } => {
                            // 9.12.1b through 9.12.1d: one add per subtype of
                            // each described card, resolved when the pipeline
                            // applies the effect (see `CharOp::CopySubtypesFrom`).
                            cite!("rule_add_remove_subtypes");
                            for other in self.st.objects.values() {
                                if criteria
                                    .iter()
                                    .all(|f| self.filter_matches_shallow(other, *f, Some(o.id)))
                                {
                                    out.push(CharEffect {
                                        source: o.id,
                                        target: o.id,
                                        op: CharOp::CopySubtypesFrom(other.id),
                                    });
                                }
                            }
                        }
                        StaticDecl::SelfAgendaPointsMod(q) => {
                            // 2.5 through 9.12.1a: an increase or a decrease
                            // of the source's own agenda point value,
                            // evaluated continuously like every other
                            // characteristic modification (so Project Beale's
                            // "for each hosted agenda counter" tracks the
                            // counters it actually has).
                            cite!("rule_agenda_points_citation");
                            let n = self.eval_quantity(q, Some(o.id)) as i32;
                            out.push(CharEffect {
                                source: o.id,
                                target: o.id,
                                op: if n >= 0 {
                                    CharOp::IncreaseAgendaPoints(n)
                                } else {
                                    CharOp::DecreaseAgendaPoints(-n)
                                },
                            });
                        }
                        StaticDecl::SelfStrength(q) => {
                            // 9.12.2e: the strength-X selector, evaluated
                            // continuously through the characteristics
                            // pipeline; while the defining ability is lost
                            // (Hush) the 9.12.1d pipeline skips the effect
                            // and X is treated as 0.
                            cite!("rule_values_defined_by_x");
                            let x = self.eval_quantity(q, Some(o.id));
                            out.push(CharEffect {
                                source: o.id,
                                target: o.id,
                                op: CharOp::SetStrength(x as i32),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
        for l in &self.lingering {
            if let Payload::SubtypeMod { target, add, remove } = &l.payload {
                cite!("rule_add_remove_subtypes");
                for t in add {
                    out.push(crate::object::CharEffect {
                        source: l.source,
                        target: *target,
                        op: crate::object::CharOp::AddSubtype(t),
                    });
                }
                for t in remove {
                    out.push(crate::object::CharEffect {
                        source: l.source,
                        target: *target,
                        op: crate::object::CharOp::RemoveSubtype(t),
                    });
                }
            }
            if let Payload::StrengthMod { target, delta } = l.payload {
                out.push(crate::object::CharEffect {
                    source: l.source,
                    target,
                    op: if delta >= 0 {
                        crate::object::CharOp::IncreaseStrength(delta)
                    } else {
                        crate::object::CharOp::DecreaseStrength(-delta)
                    },
                });
            }
        }
        out
    }

    pub fn effective_strength(&self, obj: ObjectId) -> Option<i32> {
        let effects = self.char_effects();
        crate::object::compute_effective(&self.st.objects, &effects, obj).strength
    }

    /// CR 2.16 through the 9.12.1b pipeline: does this object currently have
    /// the named subtype?
    pub fn has_subtype(&self, obj: ObjectId, s: &str) -> bool {
        let effects = self.char_effects();
        crate::object::compute_effective(&self.st.objects, &effects, obj).subtypes.contains(s)
    }

    // ------------------------------------------------------------------
    // Encounters and subroutines
    // ------------------------------------------------------------------

    /// CR 6.5.1 + 9.2.2b: open an Encounter Ice Phase as its own timing
    /// structure, on top of whatever is resolving. `forced` marks 6.5.9a's
    /// forced encounter — one resolved outside the run's normal progression,
    /// which does not change the Runner's position (and 6.5.9c: the
    /// instruction that created it is not finished until it completes, which
    /// is what putting the frame above that ability's frame means).
    fn open_encounter_phase(&mut self, ice: ObjectId, forced: bool) {
        cite!("rule_encounter_ice_phase");
        // 6.8.2c: a window completing after the run was ended cannot initiate
        // a new timing structure, and 9.2.2b makes the Encounter Ice Phase
        // one — the Corp may still rez and move the ice, but the encounter
        // does not begin.
        if self.timing_structures_blocked() {
            cite!("rule_run_ends_other_priority_windows");
            return;
        }
        if forced {
            cite!("rule_forced_encounter");
            cite!("rule_forced_encounter_end");
        }
        let id = self.next_structure;
        self.next_structure += 1;
        self.frames.push(Frame::Structure(StructureFrame {
            kind: crate::timing::StructKind::Encounter,
            instance_id: id,
            cursor: 0,
            phase: StepPhase::Enter,
            pending_jump: None,
            ctx: StructCtx::Encounter(crate::frames::EncounterCtx {
                ice,
                forced,
                outer: None,
                imminents_at_open: self.imminents.len(),
                aborted: false,
            }),
        }));
    }

    /// The innermost Encounter Ice Phase in progress.
    fn encounter_ctx(&self) -> Option<&crate::frames::EncounterCtx> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Encounter(e), .. }) => Some(e),
            _ => None,
        })
    }

    fn begin_encounter(&mut self, ice: ObjectId) {
        cite!("rule_subroutines_initial_status_in_encounter");
        let id = self.next_encounter;
        self.next_encounter += 1;
        // 6.5.9a: a forced encounter can begin while another encounter is in
        // progress (Shiro → Chrysalis). Everything that reads "the encounter"
        // reads the innermost one; the interrupted one is put back when this
        // phase completes.
        let outer = self.st.encounter.take();
        if let Some(Frame::Structure(sf)) = self.frames.last_mut() {
            if let StructCtx::Encounter(e) = &mut sf.ctx {
                e.outer = outer;
            }
        }
        self.st.encounter = Some(EncounterState {
            id,
            ice,
            broken: std::collections::BTreeSet::new(),
            resolved: std::collections::BTreeSet::new(),
            all_broken_noted: false,
        });
        self.changes.record(GameChange::EncounterBegan { ice, encounter_id: id });
    }

    /// CR 9.12.2d: note "all subroutines broken" for this encounter as soon
    /// as it could be satisfied — vacuously for zero-sub ice (checked when
    /// step 6.9.3b begins), or when the last subroutine is broken.
    fn check_all_subs_broken(&mut self) {
        let Some(e) = self.st.encounter.as_ref() else { return };
        if e.all_broken_noted {
            return;
        }
        let ice = e.ice;
        let subs = self.current_subs(ice);
        let all = {
            let e = self.st.encounter.as_ref().unwrap();
            subs.iter().all(|(k, _)| e.broken.contains(k))
        };
        if all {
            cite!("rule_vacuous_truth");
            if let Some(e) = self.st.encounter.as_mut() {
                e.all_broken_noted = true;
            }
            self.changes.record(GameChange::AllSubsBroken { ice });
        }
    }

    /// CR 9.8.9: the subroutine an active replacement effect says resolves
    /// instead of the imminent one.
    fn subroutine_replacement(&self) -> Option<Vec<Instruction>> {
        for (_, d) in self.active_statics() {
            if let StaticDecl::ReplaceSubroutineResolution { instead } = d {
                return Some(instead);
            }
        }
        None
    }

    /// CR 6.1.3f / 9.8.9: what happened during the encounter that is ending —
    /// `(ice, every subroutine it had was broken (6.5.7), any of its
    /// subroutines resolved)`. Read before the encounter state is cleared.
    fn encounter_outcome(&self, ice: ObjectId) -> (ObjectId, bool, bool) {
        cite!("rule_pass_after_breaking");
        let Some(e) = self.st.encounter.as_ref() else { return (ice, false, false) };
        let fully_broken = e.all_broken_noted;
        let subs_resolved = !e.resolved.is_empty();
        (ice, fully_broken, subs_resolved)
    }

    /// The encounter STATE ends (the change is recorded). The phase's frame is
    /// wound up separately — by [`Vm::complete_encounter_phase`] at step
    /// 6.9.3e, or by [`Vm::abort_encounter_phase`] where an effect ends it
    /// early.
    fn end_encounter(&mut self) {
        if let Some(e) = self.st.encounter.take() {
            self.changes.record(GameChange::EncounterEnded {
                ice: e.ice,
                encounter_id: e.id,
            });
        }
    }

    /// CR 6.5.8a / 6.2.7c: the Encounter Ice Phase is ABORTED. The encounter
    /// itself ends now (so nothing else treats it as in progress); the phase's
    /// frame is flagged and completes as soon as the instruction that aborted
    /// it has finished resolving, without following any of its remaining steps
    /// — so no further subroutine resolves (9.8.7c).
    fn abort_encounter_phase(&mut self) {
        self.end_encounter();
        let Some(pos) = self.frames.iter().rposition(|f| {
            matches!(f, Frame::Structure(StructureFrame { ctx: StructCtx::Encounter(_), .. }))
        }) else {
            return;
        };
        if let Some(Frame::Structure(sf)) = self.frames.get_mut(pos) {
            if let StructCtx::Encounter(e) = &mut sf.ctx {
                e.aborted = true;
                // The interrupted encounter comes back with the phase (6.5.9a).
                self.st.encounter = e.outer.take();
            }
        }
    }

    /// The ordered subroutine list of a piece of ice (9.8.2), computed from
    /// the 9.8.3 origin categories:
    /// (a) external "before" grants, newest first; (b) self-static "before"
    /// (none in the vocabulary yet); (c) printed, in printed order;
    /// (d) self-static "after"/unspecified (count-linked, lose last-first);
    /// (e) external "after"/unspecified grants, oldest first.
    pub fn current_subs(&self, ice: ObjectId) -> Vec<(SubKey, AbilityDef)> {
        cite!("rule_subroutines_ordered");
        let mut out: Vec<(SubKey, AbilityDef)> = Vec::new();
        // (a) external before, newest first (9.8.3a).
        cite!("rule_subroutine_origin_external_before");
        let mut befores: Vec<(u64, u32, AbilityDef)> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::GrantedSubroutine {
                    to,
                    sub,
                    before: true,
                    seq,
                    ord,
                    placement: None,
                } if *to == ice => Some((*seq, *ord, sub.clone())),
                _ => None,
            })
            .collect();
        befores.extend(self.static_subroutine_grants(ice, true));
        // 9.8.3a: "the most recently added subroutines first" orders the
        // GRANTS; several subroutines added by ONE effect keep the order they
        // had where they came from (Loki copying another ice's subroutines).
        befores.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
        for (seq, ord, def) in befores {
            out.push((SubKey { category: 1, src: seq, ord }, def));
        }
        // (c) printed, in printed order (9.8.3c), honoring 9.1.9 losses — and
        // 9.1.7: only ACTIVE abilities do anything. For a piece of ice that is
        // not installed, that activity comes from 9.1.8h and lasts exactly as
        // long as the encounter with it, which is what makes a forced
        // encounter with a card in HQ resolve its subroutines at all.
        cite!("rule_subroutine_origin_printed");
        cite!("rule_ability_active");
        cite!("rule_active_exception_encounter_not_installed");
        let threat = self.threat_level();
        let encountered = self.st.encounter.as_ref().map(|e| e.ice);
        for (i, a) in self.st.objects[&ice].face().abilities.iter().enumerate() {
            if a.kind == AbilityKind::Subroutine
                && self.ability_present(ice, i)
                && ability_active(&self.st.objects[&ice], a, encountered, self.st.accessed, threat)
            {
                out.push((SubKey { category: 3, src: 0, ord: i as u32 }, a.clone()));
            }
        }
        // (d) self-static count-linked (9.8.3d): Ashigaru class.
        cite!("rule_subroutine_origin_static_after");
        for a in &self.st.objects[&ice].face().abilities {
            if a.kind != AbilityKind::Static {
                continue;
            }
            if !crate::object::card_active(&self.st.objects[&ice]) {
                continue;
            }
            for d in &a.statics {
                if let StaticDecl::GainSubroutines { sub, count } = d {
                    let n = self.eval_quantity(count, Some(ice)).max(0) as u32;
                    for k in 0..n {
                        out.push((SubKey { category: 4, src: 0, ord: k }, (**sub).clone()));
                    }
                }
            }
        }
        // (e) external after/unspecified, oldest first (9.8.3e).
        cite!("rule_subroutine_origin_external_after");
        let mut afters: Vec<(u64, u32, AbilityDef)> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::GrantedSubroutine {
                    to,
                    sub,
                    before: false,
                    seq,
                    ord,
                    placement: None,
                } if *to == ice => Some((*seq, *ord, sub.clone())),
                _ => None,
            })
            .collect();
        afters.extend(self.static_subroutine_grants(ice, false));
        afters.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)));
        for (seq, ord, def) in afters {
            out.push((SubKey { category: 5, src: seq, ord }, def));
        }
        // 9.8.2c: subroutines granted "in the order of your choice" sit where
        // the granting player declared, relative to every subroutine the ice
        // had at that time, REGARDLESS OF CATEGORIES — so the declaration is
        // applied after the category sort, to the list it produced.
        cite!("rule_gain_subroutines_in_any_order");
        let mut placed: Vec<(usize, u64, u32, AbilityDef)> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::GrantedSubroutine { to, sub, seq, ord, placement: Some(at), .. }
                    if *to == ice =>
                {
                    Some((*at, *seq, *ord, sub.clone()))
                }
                _ => None,
            })
            .collect();
        placed.sort_by_key(|(at, seq, ord, _)| (*at, *seq, *ord));
        for (k, (at, seq, ord, def)) in placed.into_iter().enumerate() {
            let idx = (at + k).min(out.len());
            out.insert(idx, (SubKey { category: 2, src: seq, ord }, def));
        }
        out
    }

    /// CR 9.8.2c: ask the granting player where the subroutines just granted
    /// to `ice` go, relative to every subroutine the ice has at this time.
    fn ask_subroutine_placement(&mut self, ice: ObjectId) {
        cite!("rule_gain_subroutines_in_any_order");
        let pending: Vec<(u64, u32, &'static str)> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::GrantedSubroutine { to, sub, seq, ord, placement: None, .. }
                    if *to == ice =>
                {
                    Some((*seq, *ord, sub.label))
                }
                _ => None,
            })
            .collect();
        // The declaration is made against the list WITHOUT the new ones.
        let newest = pending.iter().map(|(s, _, _)| *s).max();
        let Some(newest) = newest else { return };
        let mut granted: Vec<(u64, u32, &'static str)> =
            pending.into_iter().filter(|(s, _, _)| *s == newest).collect();
        granted.sort_by_key(|(_, o, _)| *o);
        if granted.is_empty() {
            return;
        }
        self.pending_sub_order = Some((ice, newest));
        // "relative to each subroutine the ice has AT THAT TIME": the list the
        // declaration is made against is the one WITHOUT the subroutines being
        // placed.
        let existing: Vec<(SubKey, &'static str)> = self
            .current_subs(ice)
            .into_iter()
            .filter(|(k, _)| k.src != newest)
            .map(|(k, d)| (k, d.label))
            .collect();
        let side = self.st.objects[&ice].controller;
        self.ask(
            side,
            DecisionSpec::DeclareSubroutineOrder {
                existing,
                granted: granted.into_iter().map(|(_, _, l)| l).collect(),
            },
            DecisionCtx::SubroutineOrder,
        );
    }

    /// CR 9.8.3a/e: subroutines granted to `ice` by a static ability on ANOTHER
    /// card (Warden Fatuma class). Because the granting ability is not on the
    /// ice itself, the grant is external, and its "when the effect began to
    /// apply" stamp is the moment its source became active — the same clock
    /// `Payload::GrantedSubroutine`'s `seq` is drawn from, so the two kinds of
    /// external grant sort against each other.
    fn static_subroutine_grants(&self, ice: ObjectId, before: bool) -> Vec<(u64, u32, AbilityDef)> {
        cite!("rule_subroutine_origins");
        let threat = self.threat_level();
        let encountered = self.st.encounter.as_ref().map(|e| e.ice);
        let mut out = Vec::new();
        for o in self.st.objects.values() {
            if o.id == ice || !card_active(o) {
                continue;
            }
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Static
                    || !self.ability_present(o.id, i)
                    || !ability_active(o, a, encountered, self.st.accessed, threat)
                {
                    continue;
                }
                for d in &a.statics {
                    let StaticDecl::GrantSubroutinesTo { criteria, sub, before: b } = d else {
                        continue;
                    };
                    if *b != before {
                        continue;
                    }
                    let target = &self.st.objects[&ice];
                    if criteria.iter().all(|f| self.filter_matches(target, *f, Some(o.id))) {
                        out.push((o.active_since, 0, (**sub).clone()));
                    }
                }
            }
        }
        out
    }

    /// The next unbroken, unresolved subroutine in order (6.9.3c).
    fn next_unbroken_sub(&self) -> Option<(SubKey, AbilityDef, usize)> {
        let e = self.st.encounter.as_ref()?;
        // 9.8.4b: newly gained subroutines arrive unbroken.
        cite!("rule_new_subroutines_during_encounter");
        self.current_subs(e.ice)
            .into_iter()
            .enumerate()
            .find(|(_, (k, _))| !e.broken.contains(k) && !e.resolved.contains(k))
            .map(|(pos, (k, d))| (k, d, pos))
    }

    fn resolve_next_subroutine_body(&mut self) {
        cite!("rule_resolve_subroutines_mandatory");
        cite!("rule_resolve_subroutines_in_order");
        let Some((key, def, pos)) = self.next_unbroken_sub() else {
            self.set_structure_phase(StepPhase::Checkpoint);
            return;
        };
        let ice = self.st.encounter.as_ref().unwrap().ice;
        if let Some(e) = self.st.encounter.as_mut() {
            e.resolved.insert(key);
        }
        let ability_index = if key.category == 3 { key.ord as usize } else { usize::MAX };
        self.changes.record(GameChange::SubroutineResolved { ice, index: pos });
        self.push_ability_frame(
            ResolutionKind::Subroutine,
            AbilityRef { obj: ice, index: ability_index },
            Side::Corp,
            def.instructions,
            None,
            Some(pos),
        );
    }

    // ------------------------------------------------------------------
    // Runs, breaches, accesses
    // ------------------------------------------------------------------

    /// Initiate a run (basic action 5.2.7f or card effect).
    pub fn initiate_run(&mut self, server: ServerId) {
        cite!("rule_run_timing_structure");
        // 6.8.2c: no new timing structure while a window left open by the end
        // of a run is being completed.
        if self.timing_structures_blocked() {
            cite!("rule_run_ends_other_priority_windows");
            return;
        }
        let run_id = self.next_run;
        self.next_run += 1;
        let id = self.next_structure;
        self.next_structure += 1;
        self.would.reset_scope(WouldScope::Run);
        // 7.3.6: start counting the accesses performed during THIS run.
        self.st.run_accesses = Some((run_id, 0));
        // 1.12.6: history queries about "this run" review from here.
        self.st.run_log_start = self.changes.log.len();
        self.frames.push(Frame::Structure(StructureFrame {
            kind: crate::timing::StructKind::Run,
            instance_id: id,
            cursor: 0,
            phase: StepPhase::Enter,
            pending_jump: None,
            ctx: StructCtx::Run(RunCtx {
                run_id,
                server,
                position: None,
                came_from_ice: false,
                moved_to_new_position: false,
                reached_success: false,
                declared_successful: false,
                jump_to_run_ends: false,
                if_successful: None,
                last_encounter: None,
            }),
        }));
    }

    fn push_breach(&mut self, server: ServerId) {
        // 6.8.2c names the delayed breach of 7.3.8 explicitly.
        if self.timing_structures_blocked() {
            cite!("rule_run_ends_other_priority_windows");
            cite!("rule_consecutive_breaches");
            return;
        }
        let id = self.next_structure;
        self.next_structure += 1;
        self.frames.push(Frame::Structure(StructureFrame {
            kind: crate::timing::StructKind::Breach,
            instance_id: id,
            cursor: 0,
            phase: StepPhase::Enter,
            pending_jump: None,
            ctx: StructCtx::Breach(BreachCtx {
                server,
                candidates: Vec::new(),
                zone_candidate: None,
                chosen: None,
                accessed: Vec::new(),
                remaining_from_zone: 0,
                declined: Vec::new(),
                chosen_ever: Vec::new(),
            }),
        }));
    }

    fn push_access(&mut self, card: ObjectId) {
        let id = self.next_structure;
        self.next_structure += 1;
        self.frames.push(Frame::Structure(StructureFrame {
            kind: crate::timing::StructKind::Access,
            instance_id: id,
            cursor: 0,
            phase: StepPhase::Enter,
            pending_jump: None,
            ctx: StructCtx::Access(AccessCtx { card, must_trash: None }),
        }));
    }

    /// CR 7.4: candidates per server type.
    fn compute_candidates(&mut self) {
        cite!("sec_determining_candidates");
        let server = self.breach_ctx().unwrap().server;
        // 7.4.1a: each card in the ROOT of the breached server is a
        // candidate — for every server, central ones included. Ice
        // protecting the server is not in its root and is not a candidate.
        cite!("rule_candidates_in_server_root");
        let mut cands: Vec<ObjectId> = self.st.root.get(&server).cloned().unwrap_or_default();
        cands.reverse();
        match server {
            // 7.4.1d: plus every card in the Corp's discard pile.
            ServerId::Archives => {
                cite!("rule_candidates_in_archives");
                cands.extend(self.st.discard[&Side::Corp].iter().copied());
            }
            // 7.4.1b/c: filled per access at random / from the top.
            ServerId::Hq | ServerId::Rnd | ServerId::Remote(_) => {}
        }
        if let Some(b) = self.breach_ctx_mut() {
            b.candidates = cands;
        }
    }

    /// CR 1.12.3: the current existence of this card as an object.
    pub fn generation(&self, id: ObjectId) -> u32 {
        self.st.objects.get(&id).map(|o| o.generation).unwrap_or(0)
    }

    /// Is this OBJECT (card + generation) in the list? A card that has since
    /// changed zones is a different object and is not (1.12.3).
    fn same_object_listed(&self, c: ObjectId, list: &[(ObjectId, u32)]) -> bool {
        list.iter().any(|(o, g)| *o == c && *g == self.generation(c))
    }

    /// CR 7.4.2: apply active access prohibitions to a candidate list.
    ///
    /// The prohibitions are read AT THE MOMENT the candidates are wanted, so
    /// 7.4.2a's re-evaluation ("if an ability prohibiting some or all accesses
    /// becomes inactive during a breach, candidates are reevaluated") needs no
    /// code of its own: a card that stopped being a candidate only because of
    /// a prohibition is back in the list as soon as the prohibition is gone.
    /// A card kept out for any OTHER reason (7.4.3 already chosen, 7.4.6a
    /// declined) was removed from the maintained list and does not come back,
    /// which is the rule's "if no other rule or effect is applicable".
    fn restrict_candidates(&self, list: Vec<ObjectId>) -> Vec<ObjectId> {
        // 7.4.2b: "the Runner cannot access more than N cards during this
        // run" — the ability has no effect on candidates until the Runner has
        // actually accessed that many (7.3.6: accesses actually performed),
        // and then it prohibits every other access for the rest of the run.
        let performed = self.accesses_this_run();
        if self.lingering.iter().any(|l| match &l.payload {
            Payload::AccessLimitThisRun { limit } => performed >= *limit,
            _ => false,
        }) {
            cite!("rule_prohibiting_access_to_1");
            cite!("rule_prohibiting_access");
            return Vec::new();
        }
        let mut only: Vec<ObjectId> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::RestrictCandidatesTo(x) => Some(*x),
                _ => None,
            })
            .collect();
        // The same prohibition declared by a STATIC ability (Flagship class):
        // it applies exactly while the ability is active and its stated
        // condition holds, so uninstalling or derezzing the source lifts it
        // mid-breach — 7.4.2a's case.
        for (obj, d) in self.active_statics() {
            if matches!(d, StaticDecl::RestrictCandidatesToSelf) {
                only.push(obj);
            }
        }
        if only.is_empty() {
            list
        } else {
            cite!("rule_prohibiting_access");
            list.into_iter().filter(|c| only.contains(c)).collect()
        }
    }

    /// CR 7.3.6: the number of accesses ACTUALLY PERFORMED during the run in
    /// progress — or, once it has ended, during the run that just ended, so
    /// that a "when this run ends" ability can still count them (6.9.6d puts
    /// those abilities after the run frame has popped).
    pub fn accesses_this_run(&self) -> u32 {
        cite!("rule_number_of_accesses");
        self.st.run_accesses.map(|(_, n)| n).unwrap_or(0)
    }

    /// The candidates as of RIGHT NOW. Archives candidates derive
    /// continuously from the discard pile (7.4.6d: cards entering Archives
    /// during the breach become candidates), excluding everything already
    /// chosen (7.4.3) or declined (7.4.6a); other servers track a
    /// maintained list.
    fn breach_candidates_now(&self) -> Vec<ObjectId> {
        let Some(b) = self.breach_ctx() else { return Vec::new() };
        let mut v = match b.server {
            ServerId::Archives => {
                // 7.4.1a + 7.4.6d: the root list is maintained (root entries
                // go through the 10.3.1j declaration), while the discard-pile
                // half is derived continuously — a card entering Archives
                // during the breach becomes a candidate.
                cite!("rule_candidates_entering_archives");
                let mut v = b.candidates.clone();
                for c in self.st.discard[&Side::Corp].iter().copied() {
                    if !v.contains(&c) {
                        v.push(c);
                    }
                }
                v.retain(|c| !self.same_object_listed(*c, &b.chosen_ever) && !b.declined.contains(c));
                v
            }
            _ => b.candidates.clone(),
        };
        // 7.4.1b/c: the card presented from the corresponding zone is a
        // candidate ALONGSIDE the root cards. It leads the list so that a
        // driver taking the first candidate accesses the central server the
        // Runner came for.
        if let Some(z) = b.zone_candidate {
            if !v.contains(&z) {
                v.insert(0, z);
            }
        }
        v
    }

    /// CR 7.4.3 / 7.3.5c: record that the Runner has chosen `card`. It ceases
    /// to be a candidate for the remainder of the breach — as THAT object
    /// (1.12.3) — whether or not it is ultimately accessed; and if it was the
    /// candidate presented from the breached central server's zone, the choice
    /// counts towards the random access limit, again whether or not the access
    /// happens.
    fn take_candidate(&mut self, card: ObjectId) {
        cite!("rule_candidates_already_accessed");
        let g = self.generation(card);
        if let Some(b) = self.breach_ctx_mut() {
            b.chosen = Some(card);
            b.chosen_ever.push((card, g));
            b.candidates.retain(|&c| c != card);
            if b.zone_candidate == Some(card) {
                cite!("rule_counting_random_access_limit");
                b.zone_candidate = None;
                b.remaining_from_zone = b.remaining_from_zone.saturating_sub(1);
            }
        }
    }

    fn choose_candidate_body(&mut self) {
        cite!("step_choose_candidate");
        let candidates = self.restrict_candidates(self.breach_candidates_now());
        if candidates.len() == 1 {
            let only = candidates[0];
            self.take_candidate(only);
            self.set_structure_phase(StepPhase::Checkpoint);
        } else {
            // CR 7.5 step 4a: the Runner chooses a candidate.
            self.ask(
                Side::Runner,
                DecisionSpec::ChooseCandidate { candidates },
                DecisionCtx::Candidate,
            );
        }
    }

    /// Candidates are recomputed as accesses happen: R&D presents the
    /// TOPMOST ELIGIBLE card (7.4.7a — eligible = not already chosen, not
    /// prohibited); HQ picks at random (CONVENTION → RNG per the digest
    /// §13).
    pub fn refresh_candidates_after_access(&mut self) {
        let server = match self.breach_ctx() {
            Some(b) => b.server,
            None => return,
        };
        let (remaining, chosen_ever) = {
            let b = self.breach_ctx().unwrap();
            (b.remaining_from_zone, b.chosen_ever.clone())
        };
        if remaining == 0 {
            return;
        }
        let pick = match server {
            ServerId::Rnd => {
                cite!("rule_rnd_candidates_1_at_a_time");
                cite!("rule_rnd_topmost_eligibile_candidate");
                // All deck cards cease to be candidates, then the topmost
                // eligible one becomes the candidate.
                self.st.deck[&Side::Corp]
                    .iter()
                    .copied()
                    .find(|c| !chosen_ever.iter().any(|(o, _)| o == c))
            }
            ServerId::Hq => {
                cite!("rule_candidates_in_hq");
                let pool: Vec<ObjectId> = self.st.hand[&Side::Corp]
                    .iter()
                    .copied()
                    .filter(|c| !chosen_ever.iter().any(|(o, _)| o == c))
                    .collect();
                if pool.is_empty() {
                    None
                } else {
                    let i = self.rng.random_range(0..pool.len());
                    Some(pool[i])
                }
            }
            _ => return,
        };
        if let Some(b) = self.breach_ctx_mut() {
            b.zone_candidate = pick;
        }
    }

    fn steal_agenda(&mut self, card: ObjectId) {
        cite!("rule_score_steal");
        self.capture_scored_snapshot(card);
        self.move_card(card, Zone::ScoreArea(Side::Runner));
        self.st.objects.get_mut(&card).unwrap().faceup = true;
        // 9.9.9c: a replacement may have said the agenda arrives in the score
        // area WITH hosted counters (Project Vacheron class).
        for (kind, amount) in std::mem::take(&mut self.pending_steal_counters) {
            cite!("rule_replacement_effect_only_applies_once_per_effect");
            let have = self.st.objects[&card].counter(kind);
            self.st.objects.get_mut(&card).unwrap().counters.insert(kind, have + amount);
            self.changes.record(GameChange::CounterPlaced { obj: card, kind, amount });
        }
        // 2.5: the value recorded is the one the agenda has WHERE IT NOW IS —
        // a Merger stolen by the Runner is worth 3 from the moment it arrives.
        let points = self.effective_agenda_points(card).unwrap_or(0);
        self.changes.record(GameChange::AgendaStolen { obj: card, points });
    }

    /// CR 1.17.4-adjacent scoring via the (S) window option.
    fn score_agenda(&mut self, card: ObjectId) {
        cite!("rule_score");
        self.capture_scored_snapshot(card);
        self.move_card(card, Zone::ScoreArea(Side::Corp));
        self.st.objects.get_mut(&card).unwrap().faceup = true;
        let points = self.effective_agenda_points(card).unwrap_or(0);
        self.changes.record(GameChange::AgendaScored { obj: card, points });
    }

    /// CR 1.17.8 / 10.13.2: capture what an agenda's advancement counters and
    /// advancement requirement WERE, at the moment it began to be scored or
    /// stolen — before 1.17.5 returns the counters to the bank and before the
    /// declarations modifying its requirement stop applying to it.
    fn capture_scored_snapshot(&mut self, card: ObjectId) {
        cite!("rule_advancement_counters_reference");
        cite!("rule_dividends_timing");
        let adv = self.st.objects[&card].counter(CounterKind::Advancement);
        let req = self.advancement_requirement(card);
        self.st.objects.get_mut(&card).unwrap().scored_snapshot = Some((adv, req));
    }

    /// CR 6.3.2a: is the Runner prohibited from ANNOUNCING this server as the
    /// attacked server? An Off-the-Grid-class declaration protects the server
    /// its source is in, and reaches no further than the announcement: a run
    /// already in progress can still be moved onto that server (6.1.2d).
    pub fn run_initiation_prohibited(&self, server: ServerId) -> bool {
        cite!("rule_cannot_run_abilities");
        self.active_statics().iter().any(|(src, d)| {
            matches!(d, StaticDecl::CannotInitiateRunOnSourceServer)
                && self.st.objects.get(src).is_some_and(|o| match o.zone {
                    Zone::Root(s) | Zone::Ice(s) => s == server,
                    _ => false,
                })
        })
    }

    /// CR 1.17.3a: the advancement requirement that governs scoring this
    /// agenda — its printed one as modified by active declarations (a
    /// SanSan-class upgrade in the same server lowers it). Never below 0.
    /// 1.17.8/10.13.2: once the agenda has been scored or stolen, the last
    /// known requirement is what any ability of that scoring still reads.
    pub fn advancement_requirement(&self, card: ObjectId) -> u32 {
        self.advancement_requirement_without(card, &[])
    }

    /// CR 1.16.1c: the same requirement, as it WOULD be if the listed cards
    /// were no longer there — which is how the payment procedure tells
    /// whether spending a card would break the restriction to score.
    pub fn advancement_requirement_without(&self, card: ObjectId, gone: &[ObjectId]) -> u32 {
        cite!("rule_advancement_requirement");
        let Some(o) = self.st.objects.get(&card) else { return u32::MAX };
        if let Some((_, req)) = o.scored_snapshot {
            return req;
        }
        let Some(base) = o.printed.advancement_requirement else { return u32::MAX };
        let mut req = base as i64;
        let server = match o.zone {
            Zone::Root(s) | Zone::Ice(s) => Some(s),
            _ => None,
        };
        for (src, d) in self.active_statics() {
            if gone.contains(&src) {
                continue;
            }
            if let StaticDecl::ScoreRequirementModInSourceServer(delta) = d {
                cite!("rule_active_exception_advancement_requirement");
                let src_server = self.st.objects.get(&src).and_then(|s| match s.zone {
                    Zone::Root(sv) | Zone::Ice(sv) => Some(sv),
                    _ => None,
                });
                if src_server.is_some() && src_server == server {
                    req += delta as i64;
                }
            }
        }
        req.max(0) as u32
    }

    /// "End the run" (6.8.1/`rule_end_the_run`): unwind every frame above the
    /// run structure (windows drop their pendings — 9.2.8f/6.8.2), end any
    /// encounter, and jump to the Run Ends Phase.
    pub fn end_the_run(&mut self) {
        cite!("rule_end_the_run");
        let Some(run_pos) = self.frames.iter().rposition(|f| {
            matches!(f, Frame::Structure(StructureFrame { ctx: StructCtx::Run(_), .. }))
        }) else {
            // CR 6.1.4b: no run, but an encounter in progress — THAT encounter
            // ends without resolving any more of its steps, open priority
            // windows are processed as in 6.8.2, and no step of the Run Ends
            // Phase runs. Anything begun inside the encounter (a breach, an
            // access, the ability resolving right now) is above the phase's
            // frame and ends with it; anything already in progress when the
            // encounter began is below it and is untouched. 6.1.4c: with no
            // encounter either, the effect does nothing.
            cite!("rule_end_run_no_run_or_encounter");
            self.end_run_ends_encounter_phase();
            return;
        };
        // 6.8.2: each priority window that was open when the run was ended is
        // PROCESSED, not simply discarded. (a) a paid ability window closes;
        // (b) a reaction window opened because a timing structure began closes
        // as per 9.2.8f — in this kernel that binding IS
        // `WindowFrame::originating_structure`; (c) any OTHER open priority
        // window "is completed normally, except that new timing structures …
        // cannot be initiated", and several of them keep their usual order
        // (9.1.2), which is the order they sit in on the frame stack.
        let mut kept: Vec<Frame> = Vec::new();
        while self.frames.len() > run_pos + 1 {
            let f = self.frames.pop().unwrap();
            match f {
                Frame::Window(mut w) => {
                    let completes_normally =
                        w.kind == WindowKind::Reaction && w.originating_structure.is_none();
                    if completes_normally {
                        cite!("rule_run_ends_other_priority_windows");
                        w.no_new_timing_structures = true;
                        kept.push(Frame::Window(w));
                    } else {
                        // 6.8.2a/6.8.2b: closed; pendings die untriggered.
                        cite!("rule_run_ends_close_paws");
                        cite!("rule_run_ends_close_reaction_window");
                        self.drop_window_pendings(&w);
                    }
                }
                // 6.5.9b: during a forced encounter, "end the run" applies to
                // the Encounter Ice Phase being resolved AND to the phase it
                // was initiated from — every encounter frame above the run
                // unwinds, innermost first.
                Frame::Structure(StructureFrame { ctx: StructCtx::Encounter(e), .. }) => {
                    cite!("rule_forced_encounter_during_run");
                    self.end_encounter();
                    self.st.encounter = e.outer;
                    // Anything collected so far was inside the structure that
                    // just ended, so 9.2.8f closes it after all.
                    for k in kept.drain(..) {
                        if let Frame::Window(w) = k {
                            self.drop_window_pendings(&w);
                        }
                    }
                }
                _ => {}
            }
        }
        self.imminents.clear();
        self.end_encounter();
        let table_idx = self.table(crate::timing::StructKind::Run)
            .index_of("step_open_priority_windows_closed");
        if let Some(Frame::Structure(sf)) = self.frames.last_mut() {
            sf.cursor = table_idx;
            sf.phase = StepPhase::Enter;
            sf.pending_jump = None;
            if let StructCtx::Run(r) = &mut sf.ctx {
                r.jump_to_run_ends = true;
            }
        }
        // 6.8.2c: the windows that complete normally go back on the stack, in
        // the order they had (they were popped innermost-first).
        kept.reverse();
        self.frames.extend(kept);
        // The ETR instruction finished resolving: checkpoint (10.3.5).
        self.checkpoint_and_react(None);
    }

    /// CR 6.8.2c: is a priority window that survived "end the run" open above
    /// us? While one is, "new timing structures (including a breach that was
    /// delayed according to rule 7.3.8) cannot be initiated".
    pub fn timing_structures_blocked(&self) -> bool {
        self.frames.iter().any(|f| {
            matches!(f, Frame::Window(w) if w.no_new_timing_structures)
        })
    }

    /// CR 6.1.4b: "end the run" resolving with no run in progress but an
    /// encounter in progress. The encounter ends without resolving any more of
    /// its steps: every frame above the phase unwinds (open priority windows
    /// are closed as in 6.8.2, dropping their pendings; a breach or access
    /// begun inside the encounter ends with it), and then the phase itself
    /// completes.
    fn end_run_ends_encounter_phase(&mut self) {
        let Some(pos) = self.frames.iter().rposition(|f| {
            matches!(f, Frame::Structure(StructureFrame { ctx: StructCtx::Encounter(_), .. }))
        }) else {
            return;
        };
        cite!("rule_end_encounter_outside_run");
        let depth = match &self.frames[pos] {
            Frame::Structure(StructureFrame { ctx: StructCtx::Encounter(e), .. }) => {
                e.imminents_at_open
            }
            _ => 0,
        };
        while self.frames.len() > pos + 1 {
            let f = self.frames.pop().unwrap();
            match f {
                Frame::Window(w) => {
                    cite!("rule_run_ends_close_paws");
                    cite!("rule_run_ends_close_reaction_window");
                    self.drop_window_pendings(&w);
                }
                Frame::Structure(StructureFrame { ctx: StructCtx::Access(a), .. }) => {
                    self.st.accessed = None;
                    self.changes.record(GameChange::AccessEnded { obj: a.card });
                }
                _ => {}
            }
        }
        self.imminents.truncate(depth);
        // 10.3.6: the phase's frame pops before its closing checkpoint.
        self.complete_structure();
        self.checkpoint_and_react(None);
    }

    fn complete_structure(&mut self) {
        let Some(Frame::Structure(sf)) = self.frames.pop() else { unreachable!() };
        match sf.ctx {
            StructCtx::Turn { .. } => {
                // "…is complete, and the game moves to the other turn."
            }
            StructCtx::Encounter(e) => {
                // 6.5.6 / 6.9.3e: the phase is over. An ABORTED phase already
                // ended its encounter (and put back the one it interrupted);
                // a phase reaching step (e) ends it here.
                cite!("rule_encounter_ice_next_phase");
                if !e.aborted {
                    // 6.1.3e: the run is now coming DIRECTLY from an encounter
                    // with this ice, and what happened in it is what 6.1.3f
                    // and 9.8.9 ask about. 6.1.3c: a FORCED encounter does not
                    // change the run's timing point, so it is not a phase the
                    // run can pass "after".
                    if !e.forced {
                        cite!("rule_run_phase_after");
                        let outcome = self.encounter_outcome(e.ice);
                        if let Some(r) = self.run_ctx_mut() {
                            r.last_encounter = Some(outcome);
                        }
                    }
                    self.end_encounter();
                    cite!("rule_forced_encounter");
                    self.st.encounter = e.outer;
                }
            }
            StructCtx::Run(r) => {
                cite!("step_run_complete");
                self.current_run = None;
                self.changes.record(GameChange::RunEnded {
                    server: r.server,
                    run_id: r.run_id,
                });
            }
            StructCtx::Breach(b) => {
                self.changes.record(GameChange::BreachEnded { server: b.server });
            }
            StructCtx::Access(a) => {
                cite!("step_access_complete");
                self.st.accessed = None;
                self.changes.record(GameChange::AccessEnded { obj: a.card });
                // Bookkeeping for the enclosing breach.
                let card = a.card;
                if let Some(b) = self.breach_ctx_mut() {
                    b.accessed.push(card);
                    b.chosen = None;
                }
                self.refresh_candidates_after_access();
            }
        }
    }

    // ------------------------------------------------------------------
    // Imminence, expected effects, interrupt windows (§9.9)
    // ------------------------------------------------------------------

    /// Evaluate a quantity selector (§12 rule 5) against the current state.
    /// This is THE evaluation point for calculated quantities (9.12.2):
    /// callers choose WHEN to evaluate (imminence for effect values, trace
    /// initiation for X-traces, continuously for characteristics), and the
    /// selector says WHAT is counted.
    pub fn eval_quantity(&self, q: &crate::instr::Quantity, source: Option<ObjectId>) -> i64 {
        use crate::instr::Quantity as Q;
        cite!("rule_calculated_quantity");
        match q {
            Q::Const(n) => *n,
            Q::Count(f) => self.count_filter(*f, source),
            Q::CreditsLostThisAbility(side) => {
                // "…for each credit lost" — credits the named player ACTUALLY
                // lost during the resolution of the ability now resolving
                // (the observed 1.10.3b loss, not the requested amount).
                // Scope: changes recorded since this ability frame began.
                let mark = self
                    .frames
                    .iter()
                    .rev()
                    .find_map(|f| match f {
                        Frame::Ability(af) => Some(af.log_mark),
                        _ => None,
                    })
                    .unwrap_or(0);
                self.changes.log[mark..]
                    .iter()
                    .map(|c| match c {
                        GameChange::CreditsLost { side: s, amount } if s == side => {
                            *amount as i64
                        }
                        _ => 0,
                    })
                    .sum()
            }
            Q::AccessesThisRun => self.accesses_this_run() as i64,
            Q::DistinctIcePassedThisRun => {
                // 1.12.6: the game history, not the present game state — an
                // ice trashed after being passed is still one of the distinct
                // objects the Runner passed.
                cite!("rule_previous_object");
                let mut seen: Vec<ObjectId> = Vec::new();
                for c in &self.changes.log[self.st.run_log_start..] {
                    if let GameChange::IcePassed { ice, .. } = c {
                        if !seen.contains(ice) {
                            seen.push(*ice);
                        }
                    }
                }
                seen.len() as i64
            }
            Q::CountersOnSource(kind) => {
                // CR 1.17.8: an ability that met its condition from its source
                // agenda being scored or stolen reads that agenda's LAST KNOWN
                // number of advancement counters — the real ones went back to
                // the bank with the move (1.17.5).
                if *kind == CounterKind::Advancement {
                    if let Some((adv, _)) =
                        source.and_then(|s| self.st.objects.get(&s)).and_then(|o| o.scored_snapshot)
                    {
                        cite!("rule_advancement_counters_reference");
                        return adv as i64;
                    }
                }
                // CR 9.5.5: counters set aside by a [trash] trigger cost
                // still count as hosted for this ability.
                cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                let on_card = source
                    .and_then(|s| self.st.objects.get(&s))
                    .map(|o| o.counter(*kind))
                    .unwrap_or(0);
                let set_aside: u32 = self
                    .frames
                    .iter()
                    .rev()
                    .find_map(|f| match f {
                        Frame::Ability(af) if Some(af.source.obj) == source => Some(
                            af.set_aside_counters
                                .iter()
                                .filter(|(k, _)| k == kind)
                                .map(|(_, n)| *n)
                                .sum::<u32>(),
                        ),
                        _ => None,
                    })
                    .unwrap_or(0);
                (on_card + set_aside) as i64
            }
            Q::AnnouncedX => {
                // 1.16.2c: the value announced for the payment in progress —
                // or, once that payment has committed, the value announced for
                // the trigger cost of the ability now resolving, because the
                // announcement belongs to that use of the ability.
                // 1.16.2d: 0 wherever neither exists.
                cite!("rule_cost_x");
                cite!("rule_cost_x_out_of_context");
                self.payment
                    .as_ref()
                    .and_then(|p| p.announced_x)
                    .or_else(|| {
                        self.frames.iter().rev().find_map(|f| match f {
                            Frame::Ability(af) => Some(af.announced_x),
                            _ => None,
                        })?
                    })
                    .unwrap_or(0) as i64
            }
            Q::RunnerTags => {
                cite!("rule_tag");
                self.st.runner.tags as i64
            }
            Q::Plus(a, b) => self.eval_quantity(a, source) + self.eval_quantity(b, source),
            Q::Minus(a, b) => self.eval_quantity(a, source) - self.eval_quantity(b, source),
            Q::RequirementOfSource => {
                source.map(|s| self.advancement_requirement(s) as i64).unwrap_or(0)
            }
            // 1.10.1: the named player's credit POOL — 1.13.3 keeps credits
            // hosted on cards out of it.
            Q::CreditsInPoolOf(side) => {
                cite!("rule_credit_pool");
                self.st.player(*side).credits as i64
            }
            Q::Times(n, inner) => n * self.eval_quantity(inner, source),
            // 9.12.2a: "1 for every N" — the count of complete groups, so a
            // remainder buys nothing and there is never a negative count.
            Q::PerEvery(inner, per) => {
                if *per <= 0 {
                    0
                } else {
                    self.eval_quantity(inner, source).max(0) / per
                }
            }
            Q::XOfSource(inner) => {
                // CR 9.12.2e: X is defined by an ability of the source; while
                // that defining ability is inactive (source in Archives —
                // the ZATO example) or lost, X is treated as 0.
                cite!("rule_values_defined_by_x");
                let defined = source.is_some_and(|s| {
                    self.st.objects.get(&s).is_some_and(|o| {
                        card_active(o)
                            && o.face().abilities.iter().enumerate().any(|(i, a)| {
                                a.statics.iter().any(|d| {
                                    matches!(d, StaticDecl::SelfStrength(_))
                                }) && self.ability_present(s, i)
                            })
                    })
                });
                if defined {
                    self.eval_quantity(inner, source)
                } else {
                    0
                }
            }
        }
    }

    /// Count objects matching a filter of the shared filter language,
    /// relative to `source` where the filter is source-relative.
    fn count_filter(&self, f: TargetFilter, source: Option<ObjectId>) -> i64 {
        match f {
            // 4.6.6i: "ice protecting this server" reads the server the
            // source means by "this server" — which is the one it LEFT when
            // it is no longer installed (and it is no longer among the ice
            // counted there).
            TargetFilter::IceProtectingSourceServer => source
                .and_then(|s| self.this_server(s))
                .map(|sv| self.ice_at(sv).len() as i64)
                .unwrap_or(0),
            TargetFilter::IceProtectingAttackedServer => self
                .current_run
                .map(|(_, sv, _)| self.ice_at(sv).len() as i64)
                .unwrap_or(0),
            TargetFilter::CardsInHandOf(side) => self.st.hand[&side].len() as i64,
            other => self.filter_candidates_from(&[other], source).len() as i64,
        }
    }

    /// CR 9.9.2: compute the initial expected effects of an instruction,
    /// modified by active static abilities.
    pub fn expected_atoms(
        &mut self,
        instr: &Instruction,
        controller: Side,
        targets: &[ObjectId],
        source: Option<ObjectId>,
    ) -> Vec<EffectAtom> {
        cite!("rule_expected_effects");
        match instr {
            // 1.14.5: the named player carries out the effect, so everything
            // the atoms attribute to "the controller" is attributed to them.
            Instruction::PerformedBy { side, instr } => {
                cite!("rule_controller_choices");
                self.expected_atoms(instr, *side, targets, source)
            }
            Instruction::GainCredits(side, q) => {
                // 9.12.2b/c: credits are an aggregated class — one atom with
                // the aggregated value.
                let n = self.eval_quantity(q, source);
                vec![EffectAtom::new(EffectClass::GainCredits, n, *side)]
            }
            Instruction::LoseCredits(side, q) => {
                let n = self.eval_quantity(q, source);
                vec![EffectAtom::new(EffectClass::LoseCredits, n, *side)]
            }
            // 1.11.3a/b + 9.12.2c: clicks are an aggregated class exactly as
            // credits are, so a "lose [click]" carries one atom whose value
            // is the whole amount.
            Instruction::GainClicks(side, q) => {
                cite!("rule_gain_clicks");
                let n = self.eval_quantity(q, source);
                vec![EffectAtom::new(EffectClass::GainClicks, n, *side)]
            }
            Instruction::LoseClicks(side, q) => {
                cite!("rule_lose_spend_clicks");
                let n = self.eval_quantity(q, source);
                vec![EffectAtom::new(EffectClass::LoseClicks, n, *side)]
            }
            // 8.4.5a: setting the cards aside IS the draw — "the cards are
            // now considered drawn" — so this is the step that carries the
            // draw's expected effect and that a WouldDraw interrupt modifies.
            Instruction::Draw(side, n) | Instruction::DrawStepSetAside { side, n, .. } => {
                // 9.9.2: statics modify expected effects — a Lockdown-class
                // "cannot draw" removes the draw entirely.
                if self.draw_prohibited(*side) {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::Draw, *n as i64, *side)]
                }
            }
            // 8.4.5c: adding the set-aside cards to the hand completes the
            // procedure; the draw already happened at (a).
            Instruction::DrawStepAddToHand { side, .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, *side)]
            }
            // 8.3.3: the two halves of arranging — setting the cards aside and
            // putting them back. Nothing about either is a modifiable value.
            Instruction::SetAsideTopOfDeck { .. } | Instruction::ArrangeSetAside { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::DamageUnpreventable { kind, amount, responsible } => {
                cite!("rule_static_modification_keep_restrictions");
                let mut v = self.eval_quantity(amount, source);
                for (_, d) in self.active_statics() {
                    if let StaticDecl::DamageBonus { kind: k, responsible: r, amount: b } = d {
                        if k == *kind && r == *responsible {
                            v += b;
                        }
                    }
                }
                let mut atom = EffectAtom::new(EffectClass::Damage(*kind), v, Side::Runner);
                atom.unpreventable = true;
                vec![atom]
            }
            Instruction::Damage { kind, amount, responsible } => {
                // 6.8.5 / The Noble Path: a prevent-all-damage lingering
                // effect removes damage from the expected effects entirely
                // while it lives (its run-bound duration expires at 6.9.6d).
                if self.damage_shield_active() {
                    cite!("rule_run_ends_condition");
                    return vec![];
                }
                // 9.12.2b/c: damage is an aggregated class — a computed
                // selector ("2 net plus 1 per advancement counter") yields
                // ONE atom with the aggregated value, so Prāna-class
                // interrupts apply once.
                cite!("rule_calculated_quantity");
                cite!("rule_aggregated_instructions");
                let mut v = self.eval_quantity(amount, source);
                // 9.9.2: statics modify expected effects (The Cleaners as a
                // static formulation).
                for (_, d) in self.active_statics() {
                    if let StaticDecl::DamageBonus { kind: k, responsible: r, amount: b } = d {
                        if k == *kind && r == *responsible {
                            v += b;
                        }
                    }
                }
                vec![EffectAtom::new(EffectClass::Damage(*kind), v, Side::Runner)]
            }
            Instruction::GainTags(n) => {
                vec![EffectAtom::new(EffectClass::TakeTags, *n as i64, Side::Runner)]
            }
            Instruction::TrashCards(spec) => {
                let resolved = self.resolve_targets(spec, source, targets);
                // Statics can remove parts of the expected effects (9.9.2 —
                // Architect's "cannot be trashed" leaves nothing expected),
                // and a card already in a discard pile cannot be trashed
                // again (1.19.1: trashing moves it there; Golden Rule 1.2.4).
                let filtered: Vec<ObjectId> = resolved
                    .into_iter()
                    .filter(|t| !self.trash_prohibited(*t))
                    .filter(|t| {
                        !matches!(self.st.objects[t].zone, Zone::Discard(_))
                    })
                    .collect();
                if filtered.is_empty() {
                    vec![]
                } else {
                    let n = filtered.len() as i64;
                    vec![EffectAtom::new(EffectClass::TrashCards, n, controller)
                        .with_targets(filtered)]
                }
            }
            Instruction::EndTheRun => {
                vec![EffectAtom::new(EffectClass::EndTheRun, 1, controller)]
            }
            Instruction::DeclineableChoice(inner) => {
                self.expected_atoms(inner, controller, targets, source)
            }
            Instruction::Combined(list) => {
                let mut out = Vec::new();
                for i in list {
                    out.extend(self.expected_atoms(i, controller, targets, source));
                }
                out
            }
            Instruction::NestedCostThen { .. } | Instruction::NestedCostUnless { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 0, controller)]
            }
            Instruction::MoveSetAsideCounters { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::PreventDamage { .. }
            | Instruction::PreventAllDamage { .. }
            | Instruction::AvoidTags(_)
            | Instruction::IncreaseImminentDamage { .. }
            | Instruction::PreventTrashOf(_)
            | Instruction::BypassEncounteredIce
            | Instruction::ForceEncounter { .. }
            | Instruction::ModifyStrength { .. }
            | Instruction::ModifySubtypes { .. }
            | Instruction::Derez { .. }
            | Instruction::ExposeCards { .. }
            | Instruction::LookAtCards { .. }
            | Instruction::EndActionPhase(_)
            | Instruction::RezCard { .. }
            | Instruction::ResolveAbilityOf { .. }
            | Instruction::BreakSubroutines { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::PlaceCounters { amount, .. }
            | Instruction::LoadCounters { amount, .. } => {
                // 9.12.2: the count is a selector; the atom's value is what it
                // evaluates to when the instruction becomes imminent.
                let n = self.eval_quantity(amount, source);
                vec![EffectAtom::new(EffectClass::Structural, n, controller)]
            }
            Instruction::AdvanceCard { .. } | Instruction::ChangeAttackedServer { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::TrashSelf => {
                let tgt: Vec<ObjectId> = source
                    .into_iter()
                    .filter(|t| !self.trash_prohibited(*t))
                    .collect();
                if tgt.is_empty() {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::TrashCards, 1, controller).with_targets(tgt)]
                }
            }
            Instruction::StealSelfAgenda | Instruction::ScoreSelfAgenda => {
                vec![EffectAtom::new(EffectClass::StealAgenda, 1, controller)]
            }
            Instruction::StealIfAgenda => {
                // 7.2.3 as an expected effect: adding the accessed agenda to
                // the Runner's score area is what a Project-Vacheron-class
                // replacement modifies (9.9.9c).
                let is_agenda = self
                    .access_card()
                    .and_then(|c| self.st.objects.get(&c))
                    .is_some_and(|o| o.printed.card_type == CardType::Agenda);
                if is_agenda {
                    vec![EffectAtom::new(EffectClass::StealAgenda, 1, Side::Runner)]
                } else {
                    vec![]
                }
            }
            Instruction::MandatoryDraw => {
                if self.draw_prohibited(Side::Corp) {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::Draw, 1, Side::Corp)]
                }
            }
            Instruction::ReplaceImminentDamageKind { .. }
            | Instruction::InitiateRun { .. }
            | Instruction::ShuffleCardsIntoDeck { .. }
            | Instruction::RemoveCardsFromGame { .. }
            | Instruction::FlipIdentity(_) => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::BreachServer(_) => {
                // 6.9.5b as an expected effect: the Security-Testing class
                // replaces it (9.9.11a).
                vec![EffectAtom::new(EffectClass::Breach, 1, controller)]
            }
            Instruction::AccessCards { .. } => {
                vec![EffectAtom::new(EffectClass::AccessCard, 1, controller)]
            }
            Instruction::AccessChosenCandidate => {
                // 7.5.5 as an expected effect: the Immolation-Script class
                // replaces it (7.4.3 example 1).
                vec![EffectAtom::new(EffectClass::AccessCard, 1, controller)]
            }
            Instruction::TraceInitiate { base } => {
                // 9.9.6d: the base trace strength is a modifiable value (it
                // need not be positive).
                cite!("rule_modifiable_value_base_trace_strength");
                vec![EffectAtom::new(EffectClass::Structural, *base, controller)]
            }
            Instruction::TrashRandomFromHand { side, count } => {
                // 9.12.2c: trashing a number of cards from a specified
                // location is an aggregated class — one atom, one value.
                cite!("rule_aggregated_instructions");
                let n = self.eval_quantity(count, source).min(self.st.hand[side].len() as i64);
                if n <= 0 {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::TrashCards, n, *side)]
                }
            }
            Instruction::Sabotage { count } => {
                // 10.12.1: the trashes are one aggregated set (9.12.2c), and
                // there is nothing to trash when both zones are empty.
                let n = self.eval_quantity(count, source);
                let have = (self.st.hand[&Side::Corp].len() + self.st.deck[&Side::Corp].len()) as i64;
                if have == 0 {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::TrashCards, n.min(have), Side::Corp)]
                }
            }
            Instruction::Search { .. }
            | Instruction::AddCardsToHand { .. }
            | Instruction::AddToScoreArea { .. }
            | Instruction::HostCards { .. }
            | Instruction::SwapCards { .. }
            | Instruction::MoveIce { .. }
            | Instruction::MoveRunnerToIce { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::RemoveCountersFromPlayer { side, amount, .. }
            | Instruction::TakeBadPublicity { side, amount } => {
                let n = self.eval_quantity(amount, source);
                vec![EffectAtom::new(EffectClass::Structural, n, *side)]
            }
            // 9.9.6c: the install/play cost payment step carries a VALUE —
            // the credits that would be paid — which an interrupt can modify.
            Instruction::InstallStepPayCost | Instruction::PlayStepPayCost => {
                cite!("rule_modifiable_value_cost");
                let n = self.imminent_cost_credits();
                vec![EffectAtom::new(EffectClass::PayCost, n, controller)]
            }
            Instruction::TraceCorpSpend
            | Instruction::TraceRunnerSpend
            | Instruction::TraceDetermine { .. }
            | Instruction::PsiGame { .. }
            | Instruction::GrantSubroutines { .. }
            | Instruction::CorpDiscards { .. }
            | Instruction::RestrictAccessToSelf
            | Instruction::CreateDelayedConditional { .. }
            | Instruction::CreateLingeringEffect { .. }
            | Instruction::ReduceRunnerMemoryThisTurn(_)
            | Instruction::ChooseOne { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::GainAllottedClicks(side) => {
                let n = self.st.player(*side).allotted_clicks as i64;
                vec![EffectAtom::new(EffectClass::GainClicks, n, *side)]
            }
            // Structure-internal instructions carry structural atoms.
            _ => vec![EffectAtom::new(EffectClass::Structural, 1, controller)],
        }
    }

    /// Gagarin class: total additional cost to access an installed card in
    /// a remote server's root.
    fn additional_access_cost(&self, card: ObjectId) -> Cost {
        let in_remote_root = matches!(
            self.st.objects.get(&card).map(|o| o.zone),
            Some(Zone::Root(ServerId::Remote(_)))
        );
        if !in_remote_root {
            return Cost::free();
        }
        let mut total = Cost::free();
        for (_, d) in self.active_statics() {
            if let StaticDecl::AdditionalAccessCost(c) = d {
                total = total.plus(&c);
            }
        }
        total
    }

    /// The Noble Path class: a live prevent-all-damage lingering effect.
    fn damage_shield_active(&self) -> bool {
        self.lingering
            .iter()
            .any(|l| matches!(l.payload, Payload::DamagePreventionAll))
    }

    /// Lockdown-class static: "<side> cannot draw cards" (9.9.2 example 2).
    pub fn draw_prohibited(&self, side: Side) -> bool {
        cite!("rule_expected_effects");
        self.active_statics()
            .iter()
            .any(|(_, d)| matches!(d, StaticDecl::CannotDraw(s) if *s == side))
    }

    fn trash_prohibited(&self, target: ObjectId) -> bool {
        self.active_statics()
            .iter()
            .any(|(obj, d)| *obj == target && matches!(d, StaticDecl::CannotBeTrashed))
    }

    /// Push an imminence record; bump ordinal-would counters (9.9.5a) and
    /// apply pre-existing replacement effects (9.9.9b). Returns `true` if a
    /// 9.9.11 order Decision was asked — the caller must suspend; the answer
    /// path finishes replacement application and reopens the flow.
    #[must_use]
    fn push_imminent(
        &mut self,
        instr: Instruction,
        controller: Side,
        targets: Vec<ObjectId>,
        sub_targets: Vec<crate::ability::SubKey>,
        counter_targets: Vec<crate::object::CounterRef>,
        atoms: Vec<EffectAtom>,
    ) -> bool {
        let seq = self.changes.next_group + 1_000_000; // distinct key-space
        // CR 9.9.5a: ordinal trackers count imminences.
        let mut run_ordinal = BTreeMap::new();
        let mut turn_ordinal = BTreeMap::new();
        for a in &atoms {
            if a.expected() {
                self.would.bump(a.class);
                run_ordinal.insert(
                    class_key(a.class),
                    self.would.count(WouldScope::Run, a.class),
                );
                turn_ordinal.insert(
                    class_key(a.class),
                    self.would.count(WouldScope::Turn, a.class),
                );
            }
        }
        self.imminents.push(ImminentWrap {
            instr,
            atoms,
            controller,
            targets,
            sub_targets,
            counter_targets,
            run_ordinal,
            turn_ordinal,
            seq,
        });
        // CR 9.9.9b: active replacement effects apply as the window opens,
        // before pending interrupts are determined.
        cite!("rule_replacement_effects_apply_as_interrupt_window_opens");
        self.resolve_replacements_or_ask()
    }

    /// Replacement effects applicable to the top imminence RIGHT NOW:
    /// unapplied for this effect (9.9.9c) and with their target effect still
    /// expected (9.9.11a — a replacement cannot apply without something to
    /// replace).
    fn applicable_replacements(&self) -> Vec<u64> {
        cite!("rule_replacement_effect_only_applies_once_per_effect");
        cite!("rule_replacement_effect_must_have_something_to_replace");
        let Some(imm) = self.imminents.last() else { return Vec::new() };
        self.lingering
            .iter()
            .filter(|l| match &l.payload {
                Payload::ReplacementEffect { applies_to, .. } => {
                    !l.applied_to.contains(&imm.seq)
                        && imm.atoms.iter().any(|a| a.expected() && a.class == *applies_to)
                }
                _ => false,
            })
            .map(|l| l.id)
            .collect()
    }

    /// CR 6.7.4c: is this replacement one its controller may decline?
    fn replacement_is_optional(&self, lid: u64) -> bool {
        self.lingering.iter().any(|l| {
            l.id == lid
                && matches!(l.payload, Payload::ReplacementEffect { optional: true, .. })
        })
    }

    fn replacement_label(&self, lid: u64) -> &'static str {
        self.lingering
            .iter()
            .find(|l| l.id == lid)
            .and_then(|l| self.st.objects.get(&l.source))
            .map(|o| o.printed.name)
            .unwrap_or("replacement")
    }

    /// Apply one replacement effect to the top imminence (marks it applied
    /// for this effect — 9.9.9c).
    fn apply_replacement(&mut self, lid: u64) {
        let Some(imm_seq) = self.imminents.last().map(|i| i.seq) else { return };
        let Some(l) = self.lingering.iter_mut().find(|l| l.id == lid) else { return };
        let src_obj = l.source;
        let Payload::ReplacementEffect { applies_to, replace_with, .. } = &l.payload else { return };
        let applies_to = *applies_to;
        let replace_with = replace_with.clone();
        let mut resolve_instead: Option<Vec<Instruction>> = None;
        l.applied_to.push(imm_seq);
        let controller = self.imminents.last().map(|i| i.controller).unwrap_or(Side::Runner);
        let Some(imm) = self.imminents.last_mut() else { return };
        let Some(atom) = imm.atoms.iter_mut().find(|a| a.expected() && a.class == applies_to)
        else {
            return;
        };
        match replace_with {
            crate::lingering::ReplacementTransform::Suppress => atom.removed = true,
            crate::lingering::ReplacementTransform::ChangeDamageKind(k) => {
                if let EffectClass::Damage(_) = atom.class {
                    atom.class = EffectClass::Damage(k);
                }
            }
            crate::lingering::ReplacementTransform::SuppressAndResolve(instrs) => {
                // 9.9.2b: the replacing effect's instructions resolve in
                // place of what they replaced.
                cite!("rule_applying_replacement_effects");
                atom.removed = true;
                resolve_instead = Some(instrs);
            }
            crate::lingering::ReplacementTransform::SuppressAndGainCredits(n) => {
                atom.removed = true;
                self.st.player_mut(controller).credits += n;
                self.changes
                    .record(GameChange::CreditsGained { side: controller, amount: n });
            }
            crate::lingering::ReplacementTransform::BreachFromBottom => {
                // The breach is replaced but STILL EXPECTED — a later
                // replacement can act on it (9.9.11a example 2). The atom
                // stays in place.
            }
            crate::lingering::ReplacementTransform::SuppressAccessAndTrashOther(t) => {
                cite!("rule_candidates_already_accessed");
                atom.removed = true;
                self.trash_card(t, Side::Runner);
            }
            crate::lingering::ReplacementTransform::StealWithHostedCounters { kind, amount } => {
                // 9.9.9c: the agenda still goes to the score area — what the
                // replacement changes is that it arrives with counters on it.
                cite!("rule_replacement_effect_only_applies_once_per_effect");
                self.pending_steal_counters.push((kind, amount));
            }
            crate::lingering::ReplacementTransform::SuppressAccessAndRemoveChosen => {
                // 7.3.6: the access never happens, so it is never counted.
                cite!("rule_number_of_accesses");
                atom.removed = true;
                if let Some(c) = self.breach_ctx().and_then(|b| b.chosen) {
                    self.move_card(c, Zone::RemovedFromGame);
                }
            }
        }
        // Kernel-wave replacements are one-shot effects (Security Testing,
        // Account Siphon, Showing Off, Immolation Script): applying consumes
        // the lingering effect. Multi-application replacement durations
        // arrive with the card layer.
        self.lingering.retain(|l| l.id != lid);
        if let Some(instrs) = resolve_instead {
            self.push_ability_frame(
                ResolutionKind::Conditional,
                AbilityRef { obj: src_obj, index: 0 },
                controller,
                instrs,
                None,
                None,
            );
        }
    }

    /// Apply replacements one at a time; when several could apply, the order
    /// is a Decision (9.9.11: the base effect's controller chooses).
    /// Returns `true` if a Decision was asked.
    fn resolve_replacements_or_ask(&mut self) -> bool {
        loop {
            let appl = self.applicable_replacements();
            match appl.len() {
                0 => return false,
                1 => {
                    // CR 6.7.4c: an OPTIONAL replacement is not applied until
                    // its controller says so, and the decision is made where
                    // the effect it replaces would happen — for a breach,
                    // step 6.9.5b, by which time everything the 6.9.5a
                    // reaction window held has already resolved.
                    if self.replacement_is_optional(appl[0]) {
                        cite!("rule_if_successful_ability_optional");
                        let label = self.replacement_label(appl[0]);
                        let chooser = self
                            .lingering
                            .iter()
                            .find(|l| l.id == appl[0])
                            .map(|l| self.st.objects[&l.source].controller)
                            .unwrap_or(Side::Runner);
                        self.pending_optional_replacement = Some(appl[0]);
                        self.ask(
                            chooser,
                            DecisionSpec::OptionalEffect { label },
                            DecisionCtx::OptionalReplacement,
                        );
                        return true;
                    }
                    self.apply_replacement(appl[0])
                }
                _ => {
                    cite!("rule_order_of_replacement_effects");
                    let labels: Vec<&'static str> = appl
                        .iter()
                        .map(|lid| {
                            let src = self
                                .lingering
                                .iter()
                                .find(|l| l.id == *lid)
                                .map(|l| l.source)
                                .unwrap_or(ObjectId(0));
                            self.st
                                .objects
                                .get(&src)
                                .map(|o| o.printed.name)
                                .unwrap_or("replacement")
                        })
                        .collect();
                    let chooser =
                        self.imminents.last().map(|i| i.controller).unwrap_or(Side::Runner);
                    self.ask(
                        chooser,
                        DecisionSpec::ChooseOption { options: labels },
                        DecisionCtx::ReplacementOrder,
                    );
                    return true;
                }
            }
        }
    }

    /// CR 9.9.4: as an interrupt window opens — expected effects were
    /// computed, replacements applied — mark relevant conditional interrupts
    /// pending (a FIXED set, 9.9.4b) and open the window if anyone could act.
    fn open_interrupt_window_if_relevant(&mut self) -> bool {
        cite!("rule_interrupt_window_opening");
        let mut pending_ids = Vec::new();
        let atoms_snapshot: Vec<EffectAtom> =
            self.imminents.last().map(|i| i.atoms.clone()).unwrap_or_default();
        let ordinals = self
            .imminents
            .last()
            .map(|i| i.run_ordinal.clone())
            .unwrap_or_default();

        // Conditional-ability interrupts: fixed pending set at open (9.9.4b).
        cite!("rule_pending_status_for_interrupt_windows");
        let mut to_pend: Vec<(ObjectId, usize, AbilityDef, Side)> = Vec::new();
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Conditional || !a.is_interrupt() {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                {
                    continue;
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                if self.interrupt_relevant(a, &atoms_snapshot, &ordinals, o.id) {
                    to_pend.push((o.id, i, a.clone(), o.controller));
                }
            }
        }
        for (obj, index, def, controller) in to_pend {
            let id = self.next_instance;
            self.next_instance += 1;
            let mandatory = !def.optional;
            self.instances.insert(
                id,
                AbilityInstance {
                    id,
                    ability: AbilityRef { obj, index },
                    def,
                    controller,
                    mandatory,
                    window: None,
                    hangover: false,
                    independent: false,
                    source_generation: self.generation(obj),
                    occurrence_group: 0,
                    from_lingering: None,
                    run_id: self.current_run.map(|(r, _, _)| r),
                },
            );
            pending_ids.push(id);
        }

        // Paid interrupts participate openly (9.9.4d) — window opens if any
        // player has any relevant option now.
        let anyone = !pending_ids.is_empty()
            || self.any_relevant_paid_interrupt(Side::Corp, &atoms_snapshot, &ordinals)
            || self.any_relevant_paid_interrupt(Side::Runner, &atoms_snapshot, &ordinals);

        if !anyone {
            for id in pending_ids {
                self.instances.remove(&id);
            }
            return false;
        }

        cite!("rule_interrupt_window");
        let wid = self.next_window;
        self.next_window += 1;
        let mut w = WindowFrame::new(wid, WindowKind::Interrupt, self.st.turn_side);
        w.imminent_index = Some(self.imminents.len() - 1);
        w.pending = pending_ids.clone();
        for id in &pending_ids {
            if let Some(inst) = self.instances.get_mut(id) {
                inst.window = Some(wid);
            }
        }
        self.frames.push(Frame::Window(w));
        true
    }

    /// CR 9.9.3: relevance of an interrupt to the imminent instruction.
    pub fn interrupt_relevant(
        &self,
        def: &AbilityDef,
        atoms: &[EffectAtom],
        run_ordinals: &BTreeMap<u64, u32>,
        source: ObjectId,
    ) -> bool {
        cite!("sec_relevant_interrupts");
        // (d) "would" trigger conditions met by the expected effects.
        if let Some(Condition::Trigger(t)) = &def.condition {
            match t {
                TriggerCond::WouldDamage { kind, first_each_run } => {
                    cite!("rule_would_relevant");
                    let hit = atoms.iter().any(|a| {
                        a.expected()
                            && matches!(a.class, EffectClass::Damage(k)
                                if kind.map(|kk| kk == k).unwrap_or(true))
                    });
                    if !hit {
                        return false;
                    }
                    if *first_each_run {
                        // 9.9.5a: only the FIRST imminence counts.
                        let ord = atoms
                            .iter()
                            .filter(|a| matches!(a.class, EffectClass::Damage(_)))
                            .filter_map(|a| run_ordinals.get(&class_key(a.class)))
                            .min()
                            .copied()
                            .unwrap_or(u32::MAX);
                        return ord == 1;
                    }
                    return true;
                }
                TriggerCond::WouldDraw { first_each_turn } => {
                    cite!("rule_would_relevant");
                    let hit = atoms
                        .iter()
                        .any(|a| a.expected() && a.class == EffectClass::Draw);
                    if !hit {
                        return false;
                    }
                    if *first_each_turn {
                        // Turn-scope ordinal: only the first draw imminence.
                        let ord = self
                            .imminents
                            .last()
                            .and_then(|i| i.turn_ordinal.get(&class_key(EffectClass::Draw)))
                            .copied()
                            .unwrap_or(u32::MAX);
                        return ord == 1;
                    }
                    return true;
                }
                TriggerCond::WouldPayCost => {
                    // 9.9.6c's example: the interrupt modifies a play cost or
                    // an install cost, so it is relevant to any instruction
                    // where a card will be played or installed AND the
                    // corresponding cost paid — which is exactly the
                    // instruction carrying a `PayCost` value.
                    cite!("rule_modifiable_value_cost");
                    cite!("rule_modify_value_relevant");
                    return atoms.iter().any(|a| a.expected() && a.class == EffectClass::PayCost);
                }
                TriggerCond::SelfWouldBeTrashed => {
                    // Harbinger class: relevant while the expected effects
                    // still include this source being trashed (9.9.4c).
                    return atoms.iter().any(|a| {
                        a.expected()
                            && a.class == EffectClass::TrashCards
                            && a.targets.contains(&source)
                    });
                }
                TriggerCond::WouldStealSelfAgenda => {
                    cite!("rule_would_relevant");
                    return self.st.accessed == Some(source)
                        && atoms
                            .iter()
                            .any(|a| a.expected() && a.class == EffectClass::StealAgenda);
                }
                TriggerCond::WouldTakeTags { during_run } => {
                    let hit = atoms
                        .iter()
                        .any(|a| a.expected() && a.class == EffectClass::TakeTags && a.value > 0);
                    if !hit {
                        return false;
                    }
                    if *during_run && self.current_run.is_none() {
                        // Jesminder-class: no run in progress → not relevant
                        // (10.3.6 example).
                        return false;
                    }
                    return true;
                }
                _ => return false,
            }
        }
        // (a)/(b): effects that could prevent/avoid or modify a value.
        for i in &def.instructions {
            match i {
                Instruction::PreventDamage { kind, .. } => {
                    // A numeric prevention needs something to subtract from:
                    // at 0 it could not change the expected effects.
                    cite!("rule_prevent_relevant");
                    if atoms.iter().any(|a| {
                        a.expected()
                            && a.class == EffectClass::Damage(*kind)
                            && a.value > 0
                            && !a.unpreventable
                    }) {
                        return true;
                    }
                }
                Instruction::PreventAllDamage { kind } => {
                    // 9.9.7b: "prevent all damage" REMOVES the damage from the
                    // expected effects, so it changes them even when the value
                    // has already been reduced to 0 — and that removal is
                    // observable, because there is then no longer a value for
                    // a Cleaners-class ability to modify (the 9.9.7f example).
                    cite!("rule_prevent_relevant");
                    cite!("rule_prevent_all");
                    if atoms.iter().any(|a| {
                        a.expected() && a.class == EffectClass::Damage(*kind) && !a.unpreventable
                    }) {
                        return true;
                    }
                }
                Instruction::AvoidTags(_) => {
                    if atoms.iter().any(|a| {
                        a.expected()
                            && a.class == EffectClass::TakeTags
                            && a.value > 0
                            && !a.unpreventable
                    }) {
                        return true;
                    }
                }
                Instruction::ReduceImminentCost { .. } => {
                    // 9.9.6c's example: the interrupt modifies a play cost or
                    // an install cost, so it is relevant to any instruction
                    // where a card will be played or installed AND the
                    // corresponding cost paid — which is exactly the
                    // instruction carrying a `PayCost` value.
                    cite!("rule_modify_value_relevant");
                    cite!("rule_modifiable_value_cost");
                    if atoms.iter().any(|a| a.expected() && a.class == EffectClass::PayCost) {
                        return true;
                    }
                }
                Instruction::IncreaseImminentDamage { kind, .. } => {
                    cite!("rule_modify_value_relevant");
                    // 9.9.7a: values ≤ 0 are still modifiable while imminent.
                    if atoms
                        .iter()
                        .any(|a| a.expected() && a.class == EffectClass::Damage(*kind))
                    {
                        return true;
                    }
                }
                Instruction::ReplaceImminentDamageKind { to } => {
                    cite!("rule_replacement_effect_relevant");
                    if atoms.iter().any(|a| {
                        a.expected() && matches!(a.class, EffectClass::Damage(k) if k != *to)
                    }) {
                        return true;
                    }
                }
                // 9.9.8c: an interrupt that CREATES a replacement effect is
                // relevant exactly when the effect it would replace is among
                // the imminent instruction's expected effects (9.9.10 then
                // applies it immediately).
                Instruction::CreateLingeringEffect {
                    payload: crate::instr::LingeringSpec::Replacement { applies_to, .. },
                    ..
                } => {
                    cite!("rule_replacement_effect_relevant");
                    if atoms.iter().any(|a| a.expected() && a.class == *applies_to) {
                        return true;
                    }
                }
                Instruction::PreventTrashOf(t) => {
                    if atoms.iter().any(|a| {
                        a.expected()
                            && a.class == EffectClass::TrashCards
                            && a.targets.contains(t)
                    }) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn any_relevant_paid_interrupt(
        &self,
        side: Side,
        atoms: &[EffectAtom],
        ordinals: &BTreeMap<u64, u32>,
    ) -> bool {
        !self.paid_interrupt_options(side, atoms, ordinals).is_empty()
    }

    /// CR 9.9.4d: paid-ability interrupts a player could use right now.
    fn paid_interrupt_options(
        &self,
        side: Side,
        atoms: &[EffectAtom],
        ordinals: &BTreeMap<u64, u32>,
    ) -> Vec<WindowOption> {
        cite!("rule_trigger_paid_ability_interrupt");
        let mut out = Vec::new();
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Paid || !a.is_interrupt() {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                {
                    continue;
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                if !self.break_ability_timing_ok(a) {
                    continue;
                }
                if !self.cost_payable(side, o.id, a.cost.as_ref().unwrap_or(&Cost::default())) {
                    continue;
                }
                if a.has_flag(AbilityFlag::OncePerTurn)
                    && self.once_per_turn_used.contains(&(AbilityRef { obj: o.id, index: i }, o.generation))
                {
                    continue;
                }
                if self.interrupt_relevant(a, atoms, ordinals, o.id) {
                    out.push(WindowOption::TriggerPaid {
                        ability: AbilityRef { obj: o.id, index: i },
                        label: a.label,
                    });
                }
            }
        }
        out
    }

    // ------------------------------------------------------------------
    // Ability frames: the ONE shared resolution loop
    // (9.5.7 / 9.6.15 / 9.7.2 / 9.8.10)
    // ------------------------------------------------------------------

    pub fn push_ability_frame(
        &mut self,
        kind: ResolutionKind,
        source: AbilityRef,
        controller: Side,
        instructions: Vec<Instruction>,
        instance: Option<u64>,
        subroutine_index: Option<usize>,
    ) {
        self.push_ability_frame_cost(kind, source, controller, instructions, instance, subroutine_index, None)
    }

    /// CR 10.1.6a: has the resolution stack begun to REPEAT? An ability
    /// frame's signature is the ability it resolves, and a loop is a suffix of
    /// that sequence made of one block appearing twice — the same abilities
    /// resolving each other, with nothing on the way out. Returns the length
    /// of one turn of the loop.
    ///
    /// Only MANDATORY loops are detected here: an optional one (10.1.6b) has a
    /// priority window in it, and a priority window is a frame of another kind,
    /// so the ability-frame suffix never repeats.
    fn loop_period(&self, next: AbilityRef) -> Option<usize> {
        cite!("sec_infinite_loops");
        // Only the CONTIGUOUS ability-frame suffix counts: a frame of any
        // other kind on the way — a timing structure, a window — means this
        // is not "the same abilities resolving each other with nothing on
        // the way out". (An event whose run contains an if-successful
        // ability of the same card is nesting, not looping.)
        let mut seq: Vec<AbilityRef> = self
            .frames
            .iter()
            .rev()
            .map_while(|f| match f {
                Frame::Ability(af) => Some(af.source),
                _ => None,
            })
            .collect();
        seq.reverse();
        seq.push(next);
        for k in 1..=4usize {
            if seq.len() < 2 * k {
                break;
            }
            let n = seq.len();
            if seq[n - k..] == seq[n - 2 * k..n - k] {
                return Some(k);
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push_ability_frame_cost(
        &mut self,
        kind: ResolutionKind,
        source: AbilityRef,
        controller: Side,
        instructions: Vec<Instruction>,
        instance: Option<u64>,
        subroutine_index: Option<usize>,
        cost: Option<Cost>,
    ) {
        // CR 10.1.6a: "if a mandatory infinite loop is created (a player
        // cannot choose to stop resolving the loop) then the player who is
        // resolving the loop chooses a number. The loop instantaneously
        // resolves that many times, and then ends."
        if let Some(period) = self.loop_period(source) {
            cite!("rule_mandatory_infinite_loop");
            match self.loop_budget {
                None => {
                    if self.pending_decision.is_none() {
                        self.ask(
                            controller,
                            DecisionSpec::LoopCount { period },
                            DecisionCtx::LoopCount,
                        );
                    }
                }
                Some(0) => {
                    // The chosen number of iterations is spent: the loop ends,
                    // so this turn of it is not resolved at all and the stack
                    // unwinds normally.
                    self.loop_budget = None;
                    return;
                }
                Some(n) => self.loop_budget = Some(n - 1),
            }
        }
        let phase = match (kind, &cost) {
            // 9.5.7b: pay the trigger cost first (paid abilities).
            (_, Some(_)) => AbilityPhase::PayCost,
            // 9.8.10a: the subroutine itself becomes imminent first.
            (ResolutionKind::Subroutine, None) => AbilityPhase::SubImminent,
            _ => AbilityPhase::Targets,
        };
        self.frames.push(Frame::Ability(AbilityFrame {
            kind,
            source,
            controller,
            instructions,
            idx: 0,
            phase,
            targets: Vec::new(),
            sub_targets: Vec::new(),
            counter_targets: Vec::new(),
            announce_slot: 0,
            ability_targets: Vec::new(),
            imminent_index: None,
            instance,
            // CR 9.1.4 via 1.12.3: the ability's source is an OBJECT, i.e. an
            // (id, generation) pair. A conditional instance remembers the
            // generation it came into being with (9.6.2), and the frame
            // inherits it — so an ability whose source moved between the
            // condition being met and the ability resolving is stranded even
            // though the frame was pushed after the move (the Compile/Mayfly
            // example).
            log_mark: self.changes.log.len(),
            source_generation: instance
                .and_then(|i| self.instances.get(&i))
                .map(|i| i.source_generation)
                .unwrap_or_else(|| self.generation(source.obj)),
            any_expected_effects: false,
            subroutine_index,
            declined: false,
            cost,
            cost_restriction: None,
            set_aside_counters: Vec::new(),
            set_aside_cards: Vec::new(),
            found_cards: Vec::new(),
            looked_at: Vec::new(),
            set_aside_group: None,
            announced_x: None,
        }));
    }

    fn tick_ability(&mut self) {
        let (phase, idx, len) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (af.phase, af.idx, af.instructions.len())
        };
        match phase {
            AbilityPhase::PayCost => {
                // 9.5.7b: pay the trigger cost; used; "when used" conditions
                // met; the cost-paid checkpoint occurs — and any reaction
                // window it opens resolves BEFORE our instructions become
                // imminent (the Geist/Decoy chain, 9.1.2a).
                cite!("step_paid_ability_condition");
                let (source, controller, cost, restriction) = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { unreachable!() };
                    af.phase = AbilityPhase::Targets;
                    (
                        af.source,
                        af.controller,
                        af.cost.clone().unwrap_or_default(),
                        af.cost_restriction,
                    )
                };
                // CR 9.5.5: if the trigger cost uninstalls the source, set
                // aside its hosted counters and cards as the cost is paid.
                // They still count as "hosted" for this ability and are
                // invisible to everything else (4.8.3).
                if cost.trash_self || cost.remove_self_from_game {
                    cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                    let counters: Vec<(CounterKind, u32)> = self.st.objects[&source.obj]
                        .counters
                        .iter()
                        .map(|(k, n)| (*k, *n))
                        .collect();
                    self.st.objects.get_mut(&source.obj).unwrap().counters.clear();
                    let hosted: Vec<ObjectId> =
                        self.st.objects[&source.obj].hosted.clone();
                    for h in &hosted {
                        let o = self.st.objects.get_mut(h).unwrap();
                        // 4.8.3: "they are treated as having been installed
                        // or trashed from their previous location in the
                        // play area" (the 9.5.5 example's own words).
                        cite!("rule_set_aside_zone_passthrough");
                        o.set_aside_from = Some(o.zone);
                        o.set_aside_for_ability = true;
                        o.zone = Zone::SetAside;
                    }
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        af.set_aside_counters = counters;
                        af.set_aside_cards = hosted;
                    }
                }
                cite!("rule_paid_ability_used_condition");
                self.changes.record(GameChange::AbilityUsed { source: source.obj });
                self.begin_payment(
                    controller,
                    source.obj,
                    &cost,
                    PaymentCont::TriggerCost,
                    restriction,
                );
            }
            AbilityPhase::SubImminent => {
                cite!("step_subroutine_becomes_imminent");
                cite!("step_subroutine_interrupt_subroutine_resolution");
                // 9.8.9: a replacement effect can apply WHILE THE SUBROUTINE
                // IS IMMINENT to resolve a different subroutine instead. The
                // frame keeps its source — "the replaced subroutine is treated
                // as having the same source as the original imminent
                // subroutine" — so the substitution is of the instruction list
                // alone, and everything downstream (9.8.8 independence, the
                // `SubroutineResolved` record naming the ice) is unchanged.
                let replacement = self.subroutine_replacement();
                if let Some(instrs) = replacement {
                    cite!("rule_replace_subroutine_resolution");
                    cite!("rule_replacement_effect_from_static_ability");
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        af.instructions = instrs;
                        af.idx = 0;
                    }
                }
                self.set_ability_phase(AbilityPhase::Targets);
            }
            AbilityPhase::SubInterrupt => {
                self.set_ability_phase(AbilityPhase::Targets);
            }
            AbilityPhase::Targets => {
                if idx >= len {
                    self.finish_ability_frame();
                    return;
                }
                // 9.5.7c/9.6.15b/9.7.2a/9.8.10c: announce targets, then the
                // instruction becomes imminent.
                cite!("step_paid_ability_target_first_instruction");
                cite!("step_conditional_ability_target_first_instruction");
                cite!("step_play_ability_target_first_instruction");
                cite!("step_subroutine_target_first_instruction");
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                    af.instructions[idx].clone()
                };
                // 10.8.6: a Trace instruction expands into the step sequence
                // of resolving a trace attempt (a procedure, not a timing
                // structure — 9.2.2e; its checkpoints come from 10.8.6b and
                // the cost payments in (c)/(d)). The base quantity selector
                // is evaluated HERE, when the trace initiates (9.12.2e:
                // X-based traces read X at initiation; an orphaned XOfSource
                // selector yields 0 — the ZATO example).
                let instr = match &instr {
                    Instruction::PerformedBy { instr: inner, .. }
                        if matches!(**inner, Instruction::Trace { .. }) =>
                    {
                        // "the Corp must trace[1]" (Citadel Sanctuary class):
                        // 10.8.6's steps name their own actors, so the
                        // 1.14.5 wrapper is inert once the trace expands.
                        cite!("rule_controller_choices");
                        (**inner).clone()
                    }
                    other => other.clone(),
                };
                if let Instruction::Trace { base, if_successful, if_unsuccessful, determined_min } =
                    &instr
                {
                    cite!("rule_steps_of_resolving_trace_attempt");
                    cite!("rule_not_timing_structures");
                    let src = {
                        let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                        af.source.obj
                    };
                    let (b, isucc, iunsucc, dmin) = (
                        self.eval_quantity(base, Some(src)),
                        if_successful.clone(),
                        if_unsuccessful.clone(),
                        determined_min.clone(),
                    );
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        af.instructions[af.idx] = Instruction::TraceInitiate { base: b };
                        af.instructions.insert(af.idx + 1, Instruction::TraceCorpSpend);
                        af.instructions.insert(af.idx + 2, Instruction::TraceRunnerSpend);
                        af.instructions.insert(
                            af.idx + 3,
                            Instruction::TraceDetermine {
                                if_successful: isucc,
                                if_unsuccessful: iunsucc,
                                determined_min: dmin,
                            },
                        );
                    }
                    // Re-enter Targets with the expanded first step.
                    return;
                }
                // Nested costs: an unpayable cost forces the branch without
                // a decision (1.16.1b — the choice cannot be taken).
                match &instr {
                    Instruction::NestedCostThen { cost, .. } => {
                        let (payer, source) = self.nested_cost_payer(&instr);
                        if !self.cost_payable(payer, source, cost) {
                            // Cannot pay: the effect never happens; the
                            // choice-instruction completes with no effect.
                            self.set_ability_phase(AbilityPhase::Checkpoint);
                            return;
                        }
                    }
                    Instruction::NestedCostUnless { cost, effect, .. } => {
                        let (payer, source) = self.nested_cost_payer(&instr);
                        if !self.cost_payable(payer, source, cost) {
                            cite!("rule_cost_interrupt_static_mandatory");
                            // Cannot pay: the "unless" effect is forced.
                            let eff = (**effect).clone();
                            let idx_now = {
                                let Some(Frame::Ability(af)) = self.frames.last_mut() else {
                                    unreachable!()
                                };
                                af.instructions.insert(af.idx + 1, eff);
                                af.idx
                            };
                            let _ = idx_now;
                            self.set_ability_phase(AbilityPhase::Checkpoint);
                            return;
                        }
                    }
                    _ => {}
                }
                // 1.14.5: a choice named for a player is made by that
                // player; unwrapped, the controller makes it.
                let (chooser_override, peeled) = match &instr {
                    Instruction::PerformedBy { side, instr } => (Some(*side), (**instr).clone()),
                    other => (None, other.clone()),
                };
                if let Instruction::ChooseOne { options } = &peeled {
                    // 9.11.4g: the choice ends an instruction; 9.12.3c: a
                    // "must" choice is restricted to fully-resolvable
                    // options — if none is resolvable, nothing happens.
                    cite!("rule_choice_instruction");
                    cite!("rule_mandatory_choice");
                    let controller = chooser_override.unwrap_or({
                        let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                        af.controller
                    });
                    let resolvable: Vec<usize> = options
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, instrs))| self.option_resolvable(instrs))
                        .map(|(i, _)| i)
                        .collect();
                    match resolvable.len() {
                        0 => {
                            // "If none of the choices can be fully resolved,
                            // the ability does nothing."
                            self.set_ability_phase(AbilityPhase::Checkpoint);
                            return;
                        }
                        1 => {
                            let only = resolvable[0];
                            let inject = wrap_all(options[only].1.clone(), chooser_override);
                            if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                                for (k, ins) in inject.into_iter().enumerate() {
                                    af.instructions.insert(af.idx + 1 + k, ins);
                                }
                            }
                            self.set_ability_phase(AbilityPhase::Checkpoint);
                            return;
                        }
                        _ => {
                            let labels: Vec<&'static str> = resolvable
                                .iter()
                                .map(|&i| options[i].0)
                                .collect();
                            self.ask(
                                controller,
                                DecisionSpec::ChooseOption { options: labels },
                                DecisionCtx::Targets,
                            );
                            return;
                        }
                    }
                }
                if let Some((side, spec)) = self.targets_needed(&instr) {
                    self.ask(side, spec, DecisionCtx::Targets);
                    return;
                }
                self.begin_imminence(instr);
            }
            AbilityPhase::Imminent => {
                // 9.6.15c: the interrupt window during which abilities can
                // modify, prevent or avoid the imminent effects.
                cite!("step_conditional_ability_interrupt_window");
                // The interrupt window above us closed → resolve.
                self.set_ability_phase(AbilityPhase::Resolve);
            }
            AbilityPhase::Resolve => {
                // 9.1.2: to resolve an ability is to resolve each of its
                // instructions in the order they appear; 9.1.2a: an ability
                // that meets its condition while this one resolves starts a
                // chain reaction, which the checkpoint below discovers.
                cite!("rule_resolve_ability");
                cite!("rule_chain_reaction");
                cite!("step_paid_ability_resolution");
                cite!("step_conditional_ability_resolution");
                cite!("step_play_ability_interrupt_resolution");
                cite!("step_subroutine_resolution");
                self.resolve_current_instruction();
            }
            AbilityPhase::Checkpoint => {
                cite!("step_paid_ability_checkpoint");
                cite!("step_conditional_ability_checkpoint");
                cite!("step_play_ability_checkpoint");
                cite!("step_subroutine_checkpoint");
                // CR 9.11.2a: "the steps of installing a card are not separate
                // instructions… the only checkpoint that occurs during the
                // procedure of installing a card is at step 8.5.16d,
                // immediately after the install cost is paid." The kernel
                // expands the procedure into steps so its decisions and
                // interrupt points land where the CR puts them, but a step is
                // not an instruction: 8.5.16a is followed by no checkpoint,
                // and the one after 8.5.16f IS the instruction's own
                // post-instruction checkpoint (9.11.2).
                let procedural = matches!(
                    self.frames.last(),
                    Some(Frame::Ability(af))
                        if matches!(af.instructions.get(af.idx), Some(Instruction::InstallStepPlace))
                );
                if procedural {
                    cite!("rule_step_sequences");
                } else {
                    self.checkpoint_and_react(None);
                }
                let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                af.idx += 1;
                af.phase = AbilityPhase::Targets;
                // 9.6.15f (and its 9.5.7/9.7.2/9.8.10 counterparts): if there
                // are more instructions, announce targets for the next one and
                // return to the interrupt window.
                cite!("step_conditional_ability_loop");
                // 1.15.2: the next instruction announces its own targets
                // from scratch; 1.15.4 keeps the ability-wide list.
                af.targets.clear();
                af.sub_targets.clear();
                af.counter_targets.clear();
                af.announce_slot = 0;
            }
        }
    }

    fn set_ability_phase(&mut self, p: AbilityPhase) {
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.phase = p;
        }
    }

    /// CR 1.15.4: every target the innermost resolving ability has
    /// announced, in announcement order.
    fn ability_targets(&self) -> Vec<ObjectId> {
        self.frames
            .iter()
            .rev()
            .find_map(|f| match f {
                Frame::Ability(af) => Some(af.ability_targets.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The source of the innermost resolving ability, if any — the object a
    /// quantity selector reads "this card" from (9.12.2).
    fn current_source(&self) -> Option<ObjectId> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Ability(af) => Some(af.source.obj),
            _ => None,
        })
    }

    /// 9.12.3c: can an option's effects be fully resolved right now?
    fn option_resolvable(&self, instrs: &[Instruction]) -> bool {
        instrs.iter().all(|i| match i {
            Instruction::PerformedBy { instr, .. } => self.option_resolvable(std::slice::from_ref(instr)),
            Instruction::LoseCredits(side, q) => {
                self.st.player(*side).credits as i64
                    >= self.eval_quantity(q, self.current_source())
            }
            Instruction::TrashCards(TargetSpec::Choose { count, criteria, up_to: false }) => {
                let want = self.eval_quantity(count, self.current_source()).max(0) as usize;
                self.filter_candidates(criteria, Side::Runner).len() >= want
            }
            // Tag costs blocked by mandatory avoiders are unpayable
            // (1.16.1b), mirrored for choice options.
            Instruction::GainTags(_) => !self.tag_cost_blocked(),
            _ => true,
        })
    }

    /// The payer of a nested cost: tag/damage components are always paid by
    /// the Runner (they are things the Runner suffers); otherwise the
    /// ability's controller pays.
    fn nested_cost_payer(&self, instr: &Instruction) -> (Side, ObjectId) {
        let Some(Frame::Ability(af)) = self.frames.last() else {
            return (Side::Runner, ObjectId(0));
        };
        let (cost, explicit) = match instr {
            Instruction::NestedCostThen { cost, payer, .. }
            | Instruction::NestedCostUnless { cost, payer, .. } => (cost, *payer),
            _ => return (af.controller, af.source.obj),
        };
        let payer = explicit.unwrap_or(if cost.tags > 0 || cost.net_damage > 0 {
            // Tag/damage components are things the Runner suffers.
            Side::Runner
        } else {
            af.controller
        });
        (payer, af.source.obj)
    }

    /// CR 1.15.2: how many separate announcements this instruction requires
    /// ("for each time the instruction requires a player to choose 1 or more
    /// objects"). Every instruction in the vocabulary requires at most one
    /// except a `TargetSpec::Each`, which requires one per element.
    fn announcements_required(&self, instr: &Instruction) -> usize {
        match instr {
            Instruction::PerformedBy { instr, .. } => self.announcements_required(instr),
            Instruction::TrashCards(TargetSpec::Each(specs)) => specs.len(),
            // 1.15.2: "for each time the instruction requires a player to
            // choose 1 or more objects" — a swap has TWO target positions and
            // asks once for each that is actually a choice.
            Instruction::SwapCards { a, b } => {
                [a, b].iter().filter(|s| matches!(s, TargetSpec::Choose { .. })).count()
            }
            // 1.15.1: "the targets of this operation are the advancement
            // counters to be moved AND the destination card" — two
            // announcements, the destination first so the counters can be
            // required to come from another card.
            Instruction::MoveCounters { .. } => 2,
            _ => 1,
        }
    }

    /// Compute targets that need a Decision (9.3.4b).
    fn targets_needed(&self, instr: &Instruction) -> Option<(Side, DecisionSpec)> {
        let Some(Frame::Ability(af)) = self.frames.last() else { return None };
        // 1.15.2: one Decision per announcement the instruction requires,
        // and no more — once every slot is filled the instruction becomes
        // imminent and 1.15.2f closes target selection for good.
        if af.announce_slot >= self.announcements_required(instr) {
            cite!("rule_targeting_only_once");
            return None;
        }
        match instr {
            // 1.14.5: the named player makes the choices this instruction
            // requires, in place of the ability's controller.
            Instruction::PerformedBy { side, instr } => {
                cite!("rule_controller_choices");
                self.targets_needed(instr).map(|(_, spec)| (*side, spec))
            }
            // 9.10.3: "choose an installed piece of ice" is a 1.15.2
            // announcement like any other; the choice is then remembered.
            Instruction::MaintainChoice { of: crate::instr::ChoiceSpec::Object(spec), .. } => {
                self.announcement_for(spec).map(|s| (af.controller, s))
            }
            // 9.8.3a: "choose another rezzed piece of ice" — the ice whose
            // subroutines are copied is a 1.15.2 target announcement.
            Instruction::GrantSubroutines {
                grant: crate::instr::SubroutineGrant::CopiedFrom(spec),
                ..
            } => self.announcement_for(spec).map(|s| (af.controller, s)),
            Instruction::TrashCards(spec)
            | Instruction::ShuffleCardsIntoDeck { targets: spec, .. }
            | Instruction::RemoveCardsFromGame { targets: spec }
            | Instruction::LookAtCards { cards: spec, .. }
            | Instruction::AccessCards { cards: spec }
            | Instruction::ModifySubtypes { target: spec, .. }
            | Instruction::MoveIce { ice: spec, .. }
            | Instruction::ForceEncounter { ice: spec }
            | Instruction::RezCard { target: spec, .. }
            | Instruction::ExposeCards { cards: spec }
            | Instruction::ResolveAbilityOf { source: spec, .. }
            | Instruction::Derez { target: spec }
            // 8.2/1.15.1: "add 1 of the drawn cards to the bottom of R&D"
            // chooses its card, so the card position announces like any other.
            | Instruction::MoveToDeck { card: spec, .. }
            // 1.15.1: "place 2 advancement counters on 1 installed card" —
            // the CARD is the target of the instruction, so every counter
            // instruction whose card position is a choice announces it. (For
            // the usual `SelfSource` position `announcement_for` returns
            // `None` and nothing changes.)
            | Instruction::PlaceCounters { target: spec, .. }
            | Instruction::LoadCounters { target: spec, .. }
            | Instruction::RemoveCounters { target: spec, .. }
            | Instruction::TakeHostedCredits { from: spec, .. }
            | Instruction::AdvanceCard { target: spec }
            // 1.15.1: "Choose 1 rezzed piece of ice protecting this server.
            // That ice gets +X strength" — 9.11.4c makes the choose sentence
            // and the modifying sentence ONE instruction, and the ice is its
            // target.
            | Instruction::ModifyStrength { target: spec, .. }
            | Instruction::MoveRunnerToIce { ice: spec, .. } => {
                self.announcement_for(spec).map(|s| (af.controller, s))
            }
            // CR 1.15.1 / 9.8.6: a break ability announces the subroutines
            // it acts on. "All subroutines" (9.8.6a) announces nothing.
            Instruction::BreakSubroutines { subs } => {
                use crate::instr::SubroutineSpec as S;
                let ice = self.st.encounter.as_ref()?.ice;
                let all = self.current_subs(ice);
                let e = self.st.encounter.as_ref()?;
                let (count, up_to, candidates): (_, _, Vec<(SubKey, &'static str)>) = match subs {
                    S::All => return None,
                    // 9.8.6: only unbroken subroutines can be chosen as
                    // targets for an ability that would break them.
                    S::Chosen { count, up_to } => (
                        count,
                        *up_to,
                        all.iter()
                            .filter(|(k, _)| !e.broken.contains(k))
                            .map(|(k, d)| (*k, d.label))
                            .collect(),
                    ),
                    // 9.8.6b: the target is the subroutine that will NOT be
                    // broken, so a broken one is a legal choice.
                    S::AllBut { count } => (
                        count,
                        false,
                        all.iter().map(|(k, d)| (*k, d.label)).collect(),
                    ),
                };
                cite!("rule_unbroken_subroutines_target_for_break_abilities");
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((
                    af.controller,
                    DecisionSpec::ChooseSubroutines {
                        count: want.min(candidates.len() as u32),
                        up_to,
                        candidates,
                    },
                ))
            }
            // 1.15.1: the destination card, then the counters.
            Instruction::MoveCounters { kind, count, up_to, to, from_criteria } => {
                if af.announce_slot == 0 {
                    return self.announcement_for(to).map(|s| (af.controller, s));
                }
                cite!("rule_target");
                cite!("rule_object");
                let dest = af.targets.first().copied();
                let mut candidates: Vec<crate::object::CounterRef> = Vec::new();
                for (id, o) in &self.st.objects {
                    if Some(*id) == dest {
                        continue;
                    }
                    if !from_criteria.iter().all(|f| self.filter_matches(o, *f, Some(af.source.obj)))
                    {
                        continue;
                    }
                    for index in 0..o.counter(*kind) {
                        candidates.push(crate::object::CounterRef {
                            host: *id,
                            kind: *kind,
                            index,
                        });
                    }
                }
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                // 1.15.2b: the announcement cannot ask for more distinct
                // targets than exist.
                let n = want.min(candidates.len() as u32);
                Some((
                    af.controller,
                    DecisionSpec::ChooseCounters { candidates, count: n, up_to: *up_to },
                ))
            }
            Instruction::NestedCostThen { cost, .. }
            | Instruction::NestedCostUnless { cost, .. } => {
                let (payer, _) = self.nested_cost_payer(instr);
                Some((payer, DecisionSpec::NestedCost { cost: cost.clone() }))
            }
            Instruction::MoveSetAsideCounters {
                target: TargetSpec::Choose { count, criteria, up_to: false }, ..
            } => {
                let candidates = self.filter_candidates(criteria, af.controller);
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((af.controller, self.announcement(candidates, want)))
            }
            Instruction::DeclineableChoice(_) => Some((
                af.controller,
                DecisionSpec::OptionalEffect { label: "optional effect" },
            )),
            // 1.13.1: "host <cards> on this card" / "add <a card> to your
            // grip" announce which cards they act on (9.3.4b).
            Instruction::HostCards { cards: TargetSpec::Choose { count, criteria, up_to: false }, .. }
            | Instruction::AddToScoreArea { cards: TargetSpec::Choose { count, criteria, up_to: false }, .. }
            | Instruction::AddCardsToHand { cards: TargetSpec::Choose { count, criteria, up_to: false } } => {
                let candidates = self.filter_candidates_from(criteria, Some(af.source.obj));
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((
                    af.controller,
                    DecisionSpec::ChooseTargets {
                        candidates,
                        count: want,
                        up_to: true,
                        min: 0,
                    },
                ))
            }
            // 8.8.1/8.8.2: a swap announces both cards it exchanges, one
            // announcement per position (1.15.2), and the SECOND is filtered
            // by 8.8.2 — only cards each of which may occupy the other's
            // location. The first is filtered the same way against the whole
            // field, since a card with no legal partner is not a choice the
            // swap could ever complete ("if a swap effect would resolve while
            // there are no legal exchanges possible, then that effect does
            // nothing").
            Instruction::SwapCards { a, b } => {
                let slot = af.announce_slot;
                let specs: Vec<&TargetSpec> = [a, b]
                    .into_iter()
                    .filter(|s| matches!(s, TargetSpec::Choose { .. }))
                    .collect();
                let TargetSpec::Choose { count, criteria, up_to: false } = specs.get(slot)? else {
                    return None;
                };
                let all = self.filter_candidates_from(criteria, Some(af.source.obj));
                let chosen = af.targets.first().copied();
                let mut candidates: Vec<ObjectId> = all
                    .iter()
                    .copied()
                    .filter(|&c| Some(c) != chosen)
                    .filter(|&c| match chosen {
                        Some(first) => self.swap_legal(first, c),
                        None => all.iter().any(|&o| o != c && self.swap_legal(c, o)),
                    })
                    .collect();
                candidates.retain(|c| !af.targets.contains(c));
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((af.controller, self.announcement(candidates, want)))
            }
            // 1.13.1: the other target position of a hosting instruction —
            // WHICH CARD becomes the host (a Rook-class ability moving itself
            // onto another piece of ice). 1.15.2e applies: as many distinct
            // valid targets as the instruction asks for, or as many as exist.
            Instruction::HostCards { host: TargetSpec::Choose { count, criteria, up_to: false }, .. } => {
                let candidates = self.filter_candidates_from(criteria, Some(af.source.obj));
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((af.controller, self.announcement(candidates, want)))
            }
            // 8.5.5: multi-installs choose ONE card at a time.
            Instruction::InstallCards {
                count,
                from_hand_of,
                filter,
                and_rez_if_able,
                ..
            } if *count > 0 => {
                cite!("rule_install_one_at_a_time");
                let candidates = self.install_pick_candidates(
                    *from_hand_of,
                    *filter,
                    *and_rez_if_able,
                );
                if candidates.is_empty() {
                    None
                } else {
                    Some((
                        af.controller,
                        DecisionSpec::ChooseTargets { candidates, count: 1, up_to: true, min: 0 },
                    ))
                }
            }
            // 8.6.3: multi-plays choose ONE card at a time; affordability is
            // evaluated per pick, so credits gained by the first play can
            // fund the second (Subcontract).
            Instruction::PlayCards { count, from_hand_of, ignore_costs } if *count > 0 => {
                cite!("rule_playing_one_at_a_time");
                let candidates = self.play_pick_candidates(*from_hand_of, *ignore_costs);
                if candidates.is_empty() {
                    None
                } else {
                    Some((
                        af.controller,
                        DecisionSpec::ChooseTargets { candidates, count: 1, up_to: true, min: 0 },
                    ))
                }
            }
            // 8.5.16b: the Runner declares a host or defaults to the rig.
            Instruction::InstallCard {
                card: TargetSpec::Choose { count, criteria, up_to: false },
                ..
            } => {
                let candidates = self.filter_candidates(criteria, af.controller);
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((af.controller, self.announcement(candidates, want)))
            }
            // 6.9.1a: the effect initiated a run without naming a server, so
            // the Runner announces the attacked server. 6.7.4a fixes the set
            // it may be chosen from, and 6.3.2a removes the servers a run
            // cannot be initiated on.
            Instruction::InitiateRun { server: None, allowed, .. } => {
                cite!("step_initiation_announce");
                cite!("rule_if_successful_tied_to_server");
                let options: Vec<ServerId> = self
                    .all_servers()
                    .into_iter()
                    .filter(|s| allowed.allows(*s))
                    .filter(|s| !self.run_initiation_prohibited(*s))
                    .collect();
                if options.is_empty() {
                    None
                } else {
                    Some((Side::Runner, DecisionSpec::DeclareAttackedServer { options }))
                }
            }
            // 8.5.16b: the effect named no destination, so the installing
            // player chooses and declares one — every location the card may
            // legally occupy, "including any host relationships".
            Instruction::InstallCard {
                card,
                dest: crate::instr::InstallDest::DeclaredByInstaller,
                ..
            } => {
                cite!("rule_steps_installing_destination");
                let c = self
                    .resolve_targets(card, Some(af.source.obj), &af.targets)
                    .first()
                    .copied();
                let options =
                    c.map(|c| self.install_destinations_for(c, af.controller)).unwrap_or_default();
                if options.is_empty() {
                    None
                } else {
                    Some((af.controller, DecisionSpec::DeclareInstallDestination { options }))
                }
            }
            // 1.13.6a/8.5.16b: with the card known, whoever is installing it
            // declares the destination — and any eligible host is one of the
            // destinations on offer, whatever destination the effect named.
            // `up_to: true` is the "or as normal" half of the choice.
            Instruction::InstallCard { card, dest, .. }
                if !matches!(dest, crate::instr::InstallDest::HostedOn(_)) =>
            {
                cite!("rule_host_via_install");
                cite!("rule_steps_installing_destination");
                let c = self
                    .resolve_targets(card, Some(af.source.obj), &af.targets)
                    .first()
                    .copied();
                let hosts = c.map(|c| self.eligible_hosts_for(c)).unwrap_or_default();
                if hosts.is_empty() {
                    None
                } else {
                    // 1.13.6c: where the installee's own ability names its
                    // hosts, one of them MUST be chosen; a 1.13.6a host is
                    // one destination among the ones normally available.
                    let optional =
                        c.map(|c| self.install_only_hosted_on(c).is_none()).unwrap_or(true);
                    Some((
                        af.controller,
                        DecisionSpec::ChooseTargets {
                            candidates: hosts,
                            count: 1,
                            up_to: optional, min: 0,
                        },
                    ))
                }
            }
            _ => None,
        }
    }

    /// Cards a multi-install effect may choose from (8.5.5); the Ad Blitz
    /// "if able" stipulation excludes unrezzable cards (8.5.13d).
    fn install_pick_candidates(
        &self,
        from_hand_of: Side,
        filter: crate::instr::InstallFilter,
        and_rez_if_able: bool,
    ) -> Vec<ObjectId> {
        use crate::instr::InstallFilter as F;
        self.st.hand[&from_hand_of]
            .iter()
            .copied()
            .filter(|id| {
                let o = &self.st.objects[id];
                let t = o.printed.card_type;
                let class_ok = match filter {
                    F::Program => t == CardType::Program,
                    F::Ice => t == CardType::Ice,
                    F::Any => true,
                };
                let rez_ok = !and_rez_if_able
                    || matches!(t, CardType::Asset | CardType::Ice | CardType::Upgrade);
                // 8.7.2b-adjacent: a player must be able to install what
                // they choose — printed-cost affordability gate (hosting
                // discounts are not anticipated here; the kernel's tests
                // fund installs fully).
                let afford = match t {
                    CardType::Program | CardType::Hardware | CardType::Resource => {
                        self.st.player(from_hand_of).credits >= o.printed.cost.unwrap_or(0)
                    }
                    _ => true,
                };
                // 1.13.6c: a card that can only be installed hosted onto
                // another card cannot be chosen while no valid destination
                // exists — the illegality is checked before the installation
                // process begins.
                let destination_ok = self.install_destination_available(*id);
                class_ok && rez_ok && afford && destination_ok
            })
            .collect()
    }

    /// CR 1.13.6a: a card with an ability describing the types and numbers of
    /// cards it can host — and NO ability that hosts cards onto itself
    /// (1.13.6b) — is an eligible installation destination for the cards it
    /// describes, up to the number it specifies (1.13.5). Only cards in a
    /// score area or the play area can host at all (1.13.1a).
    pub fn eligible_hosts_for(&self, card: ObjectId) -> Vec<ObjectId> {
        cite!("rule_host_via_install");
        cite!("rule_host_relationship");
        let Some(installee) = self.st.objects.get(&card) else { return Vec::new() };
        // 1.13.6c: a card stipulating that it can only be installed hosted
        // onto another card names its own destinations, and must use them.
        if let Some(hosts) = self.install_only_hosted_on(card) {
            return hosts;
        }
        self.st
            .objects
            .values()
            .filter(|o| o.id != card)
            .filter(|o| self.can_host_location(o))
            .filter(|o| !self.hosts_onto_itself(o.id))
            .filter(|o| self.host_accepts(o, installee))
            .map(|o| o.id)
            .collect()
    }

    /// CR 1.13.1a: only cards in score areas and the play area can host
    /// objects — and only while they are actually there (an inactive but
    /// installed Corp card is in the play area).
    fn can_host_location(&self, o: &Object) -> bool {
        cite!("rule_valid_hosts");
        !o.hosted_not_installed
            && !o.staged
            && (o.zone.is_installed() || matches!(o.zone, Zone::ScoreArea(_) | Zone::PlayArea(_)))
    }

    /// CR 1.13.1: create a host relationship. The hosted object moves to the
    /// host's zone (1.13.12) and any previous relationship ends (1.13.4:
    /// an object is hosted on a single card at a time). `installed` says
    /// whether the ability that created it was installing the card: if not,
    /// the card does not become installed (1.13.2a) and is therefore not
    /// active (4.6.5h) — and an installed Corp card hosted on a Runner card
    /// becomes uninstalled (1.13.2b).
    pub fn create_host_relationship(&mut self, guest: ObjectId, host: ObjectId, installed: bool) {
        cite!("rule_placed_loaded");
        cite!("rule_hosted_limit");
        if let Some(old) = self.st.objects[&guest].host {
            if let Some(h) = self.st.objects.get_mut(&old) {
                h.hosted.retain(|&x| x != guest);
            }
        }
        let host_zone = self.st.objects[&host].zone;
        let was_installed = self.is_installed(&self.st.objects[&guest]);
        let was_zone = self.st.objects[&guest].zone;
        if self.st.objects[&guest].zone != host_zone {
            cite!("rule_hosted_object_same_zone_as_host");
            self.move_card(guest, host_zone);
        }
        {
            let g = self.st.objects.get_mut(&guest).unwrap();
            g.host = Some(host);
            if !installed {
                cite!("rule_host_without_install");
                g.hosted_not_installed = true;
                // 1.13.7b: hosted without being installed → hosted faceup.
                cite!("rule_hosted_when_not_installed");
                g.faceup = true;
            }
        }
        self.st.objects.get_mut(&host).unwrap().hosted.push(guest);
        // CR 6.2.1a: hosted ice has no position — becoming hosted takes a
        // piece of ice out of the sequence it was protecting a server in.
        cite!("rule_hosted_ice_has_no_position");
        self.vacate_ice(guest);
        self.changes.record(GameChange::CardHosted { obj: guest, host });
        // 1.13.2b: an installed Corp card that becomes hosted on a Runner
        // card becomes uninstalled.
        let corp_on_runner = is_corp_card(self.st.objects[&guest].printed.card_type)
            && self.st.objects[&host].printed.side == Side::Runner;
        if was_installed && (!installed || corp_on_runner) {
            if corp_on_runner {
                cite!("rule_host_corp_card_uninstall");
                self.st.objects.get_mut(&guest).unwrap().hosted_not_installed = true;
            }
            self.changes
                .record(GameChange::CardUninstalled { obj: guest, was_zone });
        }
    }

    /// CR 8.8.1/8.8.4: swap two cards — they exchange locations
    /// simultaneously, and whatever is hosted on either of them stays hosted
    /// on it (8.8.3a / 8.8.4c: swapping agendas between score areas leaves
    /// hosted cards and counters where they are).
    ///
    /// KERNEL SLICE: the exchange is implemented for two cards that are in
    /// the same KIND of location (both in score areas, both installed in the
    /// same kind of zone). The 8.8.4b mixed installed/uninstalled case — the
    /// only one where hosted objects are trashed and install/uninstall
    /// conditions are met — belongs to the §8.8 wave and is not implemented.
    /// CR 4.6.6e / 8.5.6a: are these two cards "like cards" that cannot share
    /// a server's root — an asset-or-agenda pair, or two regions?
    fn like_cards(&self, a: ObjectId, b: ObjectId) -> bool {
        cite!("rule_server_root");
        cite!("rule_region_one_root");
        let (Some(x), Some(y)) = (self.st.objects.get(&a), self.st.objects.get(&b)) else {
            return false;
        };
        let both_assetish = matches!(x.printed.card_type, CardType::Asset | CardType::Agenda)
            && matches!(y.printed.card_type, CardType::Asset | CardType::Agenda);
        let both_regions = x.printed.subtypes.contains(&"region")
            && y.printed.subtypes.contains(&"region");
        both_assetish || both_regions
    }

    /// CR 8.8.2: a card can only ever be swapped into a location it is
    /// normally allowed to occupy, and the player "must observe any applicable
    /// game rules or card abilities that would affect that card in its final
    /// destination". Both halves of the exchange are tested, each against the
    /// other's location as it will be once the other card has left it.
    pub fn swap_legal(&self, x: ObjectId, y: ObjectId) -> bool {
        cite!("rule_swap_only_to_valid_location");
        let (Some(a), Some(b)) = (self.st.objects.get(&x), self.st.objects.get(&y)) else {
            return false;
        };
        self.may_occupy(x, b.zone, y) && self.may_occupy(y, a.zone, x)
    }

    /// May this card occupy that location, given that `vacating` is leaving it
    /// in the same instant (8.8.3: the exchange is simultaneous)?
    fn may_occupy(&self, card: ObjectId, dest: Zone, vacating: ObjectId) -> bool {
        let Some(o) = self.st.objects.get(&card) else { return false };
        let t = o.printed.card_type;
        match dest {
            // 6.2.1: only a piece of ice protects a server.
            Zone::Ice(_) => t == CardType::Ice,
            Zone::Root(s) => {
                // 3.6.1 / 4.6.6e: a central server's root takes upgrades only;
                // a remote's takes 1 asset-or-agenda and any number of
                // upgrades.
                cite!("rule_upgrade_install");
                let type_ok = if s.is_central() {
                    t == CardType::Upgrade
                } else {
                    matches!(t, CardType::Asset | CardType::Agenda | CardType::Upgrade)
                };
                type_ok
                    && !self
                        .st
                        .root
                        .get(&s)
                        .map(|v| {
                            v.iter().any(|&other| other != vacating && self.like_cards(card, other))
                        })
                        .unwrap_or(false)
            }
            // 8.5.4: the rig holds the Runner's installed cards.
            Zone::Rig => !is_corp_card(t),
            // 4.5: score areas hold agendas.
            Zone::ScoreArea(_) => t == CardType::Agenda,
            _ => true,
        }
    }

    pub fn swap_cards(&mut self, x: ObjectId, y: ObjectId) {
        cite!("rule_swap_installed_cards");
        cite!("rule_swap_installed_cards_preserves_hosting");
        cite!("rule_swap_score_areas");
        let (zx, zy) = (self.st.objects[&x].zone, self.st.objects[&y].zone);
        // 6.2.2f: ice protecting a server occupies a POSITION, so two pieces
        // of ice protecting the SAME server are in different locations even
        // though they are in the same zone — and the swap re-occupies the two
        // existing positions rather than creating any.
        let (px, py) = (self.position_of_ice(x), self.position_of_ice(y));
        if zx == zy && px == py {
            return;
        }
        // 8.8.2: with no legal exchange available, the effect does nothing.
        if !self.swap_legal(x, y) {
            return;
        }
        cite!("rule_create_position_swap");
        // 8.8.4d / 8.4.3b: a card swapped INTO the set-aside zone takes the
        // place of the one that left — it joins that card's 4.8.7 group, so
        // "the card swapped into the set-aside zone is now considered drawn,
        // can be manipulated by other abilities, and will be added to the hand
        // with the other drawn cards".
        let (gx, gy) = (
            self.st.objects[&x].set_aside_group,
            self.st.objects[&y].set_aside_group,
        );
        // Simultaneous exchange: neither card's own move can be observed
        // trashing what is hosted on it (8.8.3a), so the host relationships
        // are simply carried along with the cards.
        let (xi, yi) = (zx.is_installed(), zy.is_installed());
        self.pending_ice_position = py;
        self.move_card(x, zy);
        self.pending_ice_position = px;
        self.move_card(y, zx);
        self.pending_ice_position = None;
        if zy == Zone::SetAside {
            cite!("rule_drawn_card_swapped");
            cite!("rule_state_of_swap_into_zone");
            self.st.objects.get_mut(&x).unwrap().set_aside_group = gy;
        }
        if zx == Zone::SetAside {
            cite!("rule_drawn_card_swapped");
            self.st.objects.get_mut(&y).unwrap().set_aside_group = gx;
        }
        // 8.8.4b: exactly one of the two was installed. It becomes
        // uninstalled — and everything hosted on it is trashed, since nothing
        // followed it out of the play area — while the other becomes
        // installed in the exact position the first occupied, WITHOUT the
        // 8.5.16 install procedure: no cost is paid and no like cards are
        // trashed. The uninstall/install trigger conditions are met at the
        // next checkpoint, which is what the `Card{Un,}Installed` records do.
        if xi != yi {
            cite!("rule_swap_become_installed");
            let (left, joined) = if xi { (x, y) } else { (y, x) };
            let guests: Vec<ObjectId> = self.st.objects[&left].hosted.clone();
            for g in guests {
                cite!("rule_swap_become_installed");
                let owner = self.st.objects[&g].owner;
                self.trash_card(g, owner);
            }
            let counters: Vec<(CounterKind, u32)> = self.st.objects[&left]
                .counters
                .iter()
                .map(|(k, n)| (*k, *n))
                .collect();
            self.st.objects.get_mut(&left).unwrap().counters.clear();
            for (kind, amount) in counters {
                self.changes.record(GameChange::CounterRemoved {
                    obj: Some(left),
                    kind,
                    amount,
                });
            }
            // 8.8.4a: each card enters its destination in the state a card
            // would normally enter it — a Corp card entering the play area
            // enters unrezzed.
            cite!("rule_state_of_swap_into_zone");
            let side = self.st.objects[&joined].printed.side;
            {
                self.st.active_seq += 1;
                let seq = self.st.active_seq;
                let o = self.st.objects.get_mut(&joined).unwrap();
                o.faceup = side == Side::Runner;
                if o.faceup {
                    o.active_since = seq;
                }
            }
            self.changes.record(GameChange::CardInstalled {
                obj: joined,
                side,
                // 8.8.4b: the joining card was not installed a moment ago; it
                // comes from wherever the swap took it from.
                from: zx,
            });
        }
        if let (Zone::ScoreArea(sx), Zone::ScoreArea(sy)) = (zx, zy) {
            // 4.5: a card in a score area is controlled by its owner of that
            // area — the swap changes who has scored it.
            cite!("rule_score_area");
            self.st.objects.get_mut(&x).unwrap().controller = sy;
            self.st.objects.get_mut(&y).unwrap().controller = sx;
        }
    }

    /// CR 1.9.5: remove up to `n` counters of `kind` from a PLAYER (their
    /// tags, their bad publicity). Hosted counters are never reachable here
    /// (1.13.3).
    fn remove_player_counters(&mut self, side: Side, kind: CounterKind, n: u32) {
        let removed = match kind {
            CounterKind::BadPublicity => {
                cite!("rule_bad_publicity");
                let have = self.st.player(side).bad_publicity;
                let take = have.min(n);
                self.st.player_mut(side).bad_publicity -= take;
                take
            }
            _ => 0,
        };
        if removed > 0 {
            self.changes
                .record(GameChange::CounterRemoved { obj: None, kind, amount: removed });
        }
    }

    /// CR 1.13.6b: does this card have an ability that creates a host
    /// relationship onto itself? If so, it hosts only through those
    /// abilities and is not an installation destination.
    fn hosts_onto_itself(&self, host: ObjectId) -> bool {
        cite!("rule_host_via_ability");
        let Some(o) = self.st.objects.get(&host) else { return false };
        o.face().abilities.iter().any(|a| {
            a.instructions.iter().any(|i| {
                matches!(i, Instruction::HostCards { host: TargetSpec::SelfSource, .. })
            })
        })
    }

    /// Does `host`'s hosting declaration accept `installee`, with room left
    /// (1.13.5: any number unless the ability says otherwise)?
    fn host_accepts(&self, host: &Object, installee: &Object) -> bool {
        host.face().abilities.iter().enumerate().any(|(i, a)| {
            a.kind == AbilityKind::Static
                && self.ability_present(host.id, i)
                && a.statics.iter().any(|d| match d {
                    StaticDecl::CanHost { criteria, capacity } => {
                        cite!("rule_hosting_limit");
                        let room = match capacity {
                            None => true,
                            Some(q) => (host.hosted.len() as i64) < self.eval_quantity(q, Some(host.id)),
                        };
                        room && criteria
                            .iter()
                            .all(|f| self.filter_matches(installee, *f, Some(host.id)))
                    }
                    _ => false,
                })
        })
    }

    /// CR 1.13.6c: "install only on <description>" — the cards this one may
    /// be installed onto. `None` means the card carries no such restriction.
    pub fn install_only_hosted_on(&self, card: ObjectId) -> Option<Vec<ObjectId>> {
        let o = self.st.objects.get(&card)?;
        // 9.1.8c: an ability restricting where its source may be installed is
        // active even while that source is inactive (it is in a hand).
        cite!("rule_active_exception_modify_play_install_rez");
        let criteria: Vec<TargetFilter> = o
            .printed
            .abilities
            .iter()
            .filter(|a| a.kind == AbilityKind::Static)
            .flat_map(|a| a.statics.iter())
            .find_map(|d| match d {
                StaticDecl::InstallOnlyHostedOn(c) => Some(c.clone()),
                _ => None,
            })?;
        cite!("rule_host_on_ability");
        cite!("rule_host_restriction");
        Some(
            self.st
                .objects
                .values()
                .filter(|h| h.id != card && self.can_host_location(h))
                .filter(|h| criteria.iter().all(|f| self.filter_matches(h, *f, Some(card))))
                .map(|h| h.id)
                .collect(),
        )
    }

    /// CR 1.13.6c: could this card legally be installed right now? A card
    /// that may only be installed hosted onto another card cannot be
    /// installed at all while no valid destination exists.
    pub fn install_destination_available(&self, card: ObjectId) -> bool {
        match self.install_only_hosted_on(card) {
            None => true,
            Some(hosts) => !hosts.is_empty(),
        }
    }

    /// ONE predicate for the shared filter vocabulary (§12 rule 5): the same
    /// atoms decide announce-time candidacy, `Quantity::Count` membership and
    /// 8.7.2a search criteria. Location atoms read the object's zone;
    /// card-characteristic atoms read the card itself, so a search supplies
    /// the zone separately.
    pub fn filter_matches(&self, o: &Object, f: TargetFilter, source: Option<ObjectId>) -> bool {
        match f {
            TargetFilter::IceProtectingAttackedServer => {
                matches!((o.zone, self.current_run), (Zone::Ice(a), Some((_, b, _))) if a == b)
            }
            // 6.2.3: "the same position" is the same number of positions
            // inward, counted in each ice's own server.
            TargetFilter::IceInSamePositionAs(r) => {
                let mine = self.position_of_ice(o.id).and_then(|(s, p)| {
                    self.positions_inward_of(s, p)
                });
                let theirs = self.reference_position(r, source);
                mine.is_some() && mine == theirs
            }
            // 9.5.5: the cards this ability's own trigger cost set aside.
            // 4.8.3: no other ability can see them, which is why the criterion
            // reads the RESOLVING ability's set-aside list rather than the
            // set-aside zone.
            // 8.4.2a: the drawn cards, while they are still set aside — an
            // explicit exception to 4.8.3, so this criterion reads the
            // set-aside ZONE rather than any one ability's list.
            TargetFilter::DrawnCards => {
                cite!("rule_draw_relevant_abilities_see_set_aside");
                o.zone == Zone::SetAside && o.set_aside_group.is_some_and(|g| g.drawn)
            }
            TargetFilter::SetAsideByThisAbility => {
                cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                cite!("rule_set_aside_zone_passthrough");
                self.frames
                    .iter()
                    .rev()
                    .find_map(|fr| match fr {
                        Frame::Ability(af) => Some(
                            af.set_aside_cards.contains(&o.id)
                                // 8.3.3b: the cards this ability set aside from
                                // the top of a deck, while it performs "other
                                // effects on cards in a deck before arranging
                                // them".
                                || (af.set_aside_group.is_some()
                                    && o.set_aside_group.map(|g| g.id) == af.set_aside_group),
                        ),
                        _ => None,
                    })
                    .unwrap_or(false)
            }
            // 1.12.3: the cards this ability is looking at, minus any whose
            // object has ceased to exist because a shuffle or a rearrangement
            // moved it to an unknown location.
            TargetFilter::LookedAtByThisAbility => {
                cite!("rule_object_move_location");
                cite!("rule_look");
                let gen = self.generation(o.id);
                self.frames
                    .iter()
                    .rev()
                    .find_map(|fr| match fr {
                        Frame::Ability(af) => {
                            Some(af.looked_at.iter().any(|(id, g)| *id == o.id && *g == gen))
                        }
                        _ => None,
                    })
                    .unwrap_or(false)
            }
            // 1.12.6: the game history since the turn began, which 10.2.1
            // makes open information — an install that happened this turn is
            // a fact about the past, so it is read from the change log and
            // not from any state the object carries.
            TargetFilter::InstalledThisTurn(want) => {
                cite!("rule_previous_object");
                cite!("rule_open_information");
                let installed = self.changes.log[self.st.turn_log_start..].iter().any(|c| {
                    matches!(c, GameChange::CardInstalled { obj, .. } if *obj == o.id)
                });
                installed == want
            }
            // 1.18.3: the same permission the basic advance action reads, so
            // a criterion and the action can never disagree about it.
            TargetFilter::CanBeAdvanced => {
                cite!("rule_you_can_advance");
                self.advanceable_cards().contains(&o.id)
            }
            TargetFilter::InstalledCorpCard => self.is_installed(o) && is_corp_card(o.printed.card_type),
            TargetFilter::InstalledRunnerCard => {
                self.is_installed(o) && !is_corp_card(o.printed.card_type)
            }
            TargetFilter::InstalledResource => {
                self.is_installed(o)
                    && o.zone == Zone::Rig
                    && o.printed.card_type == CardType::Resource
            }
            // 8.1.2: a rezzed card is an installed faceup Corp card.
            TargetFilter::Rezzed => {
                cite!("rule_rezzed_unrezzed");
                self.is_installed(o) && is_corp_card(o.printed.card_type) && o.faceup
            }
            TargetFilter::IceProtectingSourceServer => source
                .and_then(|s| self.this_server(s))
                .map(|sv| self.ice_at(sv).contains(&o.id))
                .unwrap_or(false),
            TargetFilter::CardsInHandOf(side) => o.zone == Zone::Hand(side),
            TargetFilter::CardTypeIs(t) => o.printed.card_type == t,
            // 9.12.1b: subtypes come from the characteristics pipeline, so a
            // subtype granted or removed by an active effect counts.
            TargetFilter::HasSubtype(s) => {
                cite!("rule_modify_subtypes");
                crate::object::compute_effective(&self.st.objects, &self.char_effects(), o.id)
                    .subtypes
                    .contains(&s)
            }
            TargetFilter::PrintedCostAtMost(n) => o.printed.cost.unwrap_or(0) <= n,
            TargetFilter::InScoreAreaOf(side) => {
                cite!("rule_score_area");
                o.zone == Zone::ScoreArea(side)
            }
            TargetFilter::InDiscardOf(side) => {
                cite!("rule_discard_pile");
                o.zone == Zone::Discard(side)
            }
            // CR 4.2.2: the deck is ordered; "the top N cards" are its first
            // N, and a card must still be there to be a valid target.
            TargetFilter::TopOfDeckOf { side, n } => {
                cite!("rule_deck_ordered");
                self.st.deck[&side].iter().take(n as usize).any(|c| *c == o.id)
            }
            // "each OTHER rezzed piece of ice": the description excludes the
            // describing ability's own source.
            TargetFilter::OtherThanSource => source != Some(o.id),
            // 10.1.4: "this card" — the ability's own source, and only it.
            TargetFilter::IsSource => source == Some(o.id),
            // 1.13.2: hosted ON the source — a host relationship, which is
            // what "all hosted cards" names.
            TargetFilter::HostedOnSource => {
                cite!("rule_hosted_word_meaning");
                o.host.is_some() && o.host == source
            }
            // 10.1.5: naming a card is not self-reference — "a copy of X"
            // matches every card named X, the source included.
            TargetFilter::HasName(n) => {
                cite!("sec_old_self_reference_rules");
                o.printed.name == n
            }
        }
    }

    /// The same predicate, evaluated without re-entering the 9.12.1
    /// characteristics pipeline. Used only while GATHERING that pipeline's
    /// input (`char_effects`), where a subtype atom would recurse forever:
    /// there, `HasSubtype` reads the printed subtypes (2.16) instead of the
    /// effective ones. Every other atom is unchanged.
    fn filter_matches_shallow(
        &self,
        o: &Object,
        f: TargetFilter,
        source: Option<ObjectId>,
    ) -> bool {
        match f {
            TargetFilter::HasSubtype(s) => o.printed.subtypes.contains(&s),
            other => self.filter_matches(o, other, source),
        }
    }

    /// CR 1.13.2: installed is distinct from hosted — a card hosted without
    /// being installed (1.13.2a) sits in the play area but is not installed,
    /// and neither is a card merely staged there mid-install (8.5.16a).
    pub fn is_installed(&self, o: &Object) -> bool {
        cite!("rule_hosted_installed_state");
        o.zone.is_installed() && !o.hosted_not_installed && !o.staged
    }

    /// Candidates for an announced choice: every object matching ALL the
    /// criteria (the conjunction the CR writes as "a rezzed piece of ice"),
    /// restricted to the play area unless a criterion names a zone (1.15.2c).
    fn filter_candidates(&self, criteria: &[TargetFilter], _controller: Side) -> Vec<ObjectId> {
        self.filter_candidates_from(criteria, None)
    }

    /// The objects a description picks out, read from `source` — the public
    /// face of the shared criteria vocabulary (1.15.2's candidate set).
    pub fn candidates_matching(
        &self,
        criteria: &[TargetFilter],
        source: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        self.filter_candidates_from(criteria, source)
    }

    fn filter_candidates_from(
        &self,
        criteria: &[TargetFilter],
        source: Option<ObjectId>,
    ) -> Vec<ObjectId> {
        // CR 1.15.2c: "unless an instruction explicitly specifies the zone
        // from which an object must be selected as a target, only counters
        // in the play area and installed cards are valid targets". The
        // criteria ARE the instruction's specification, so the restriction
        // lifts exactly when one of them names a zone.
        cite!("rule_targets_must_be_in_play_area");
        let zoned = criteria.iter().any(|f| f.names_zone());
        self.st
            .objects
            .values()
            .filter(|o| zoned || self.is_installed(o))
            .filter(|o| criteria.iter().all(|f| self.filter_matches(o, *f, source)))
            .map(|o| o.id)
            .collect()
    }

    /// CR 1.15.2e: the announcement asks for as many DISTINCT valid targets
    /// as the instruction wants, or as many as exist — "the remaining
    /// targets are not announced". Both the ceiling and the floor are that
    /// number, so a plan cannot under-announce.
    fn announcement(&self, candidates: Vec<ObjectId>, want: u32) -> DecisionSpec {
        cite!("rule_distinct_targets");
        let n = want.min(candidates.len() as u32);
        DecisionSpec::ChooseTargets { candidates, count: n, up_to: false, min: n }
    }

    /// CR 1.15.2: the announcement a target spec still owes, for the
    /// announcement slot the frame is on — `None` once every slot is filled
    /// (or for a spec that names its objects outright). A `Each` spec is
    /// "each time the instruction requires a player to choose": one Decision
    /// per element, in order, and already-announced targets of THIS
    /// instruction are out of the running (1.15.2e's distinctness applies
    /// per announcement; Colossus's program and resource cannot collide
    /// anyway, but a "2 different pieces of ice" spec relies on it).
    fn announcement_for(&self, spec: &TargetSpec) -> Option<DecisionSpec> {
        let Some(Frame::Ability(af)) = self.frames.last() else { return None };
        let slot = af.announce_slot;
        let (count, criteria, up_to) = match spec {
            TargetSpec::Choose { count, criteria, up_to } if slot == 0 => (count, criteria, *up_to),
            TargetSpec::Each(specs) => match specs.get(slot) {
                Some(TargetSpec::Choose { count, criteria, up_to }) => (count, criteria, *up_to),
                Some(_) | None => return None,
            },
            _ => return None,
        };
        cite!("rule_announce_targets");
        let mut candidates = self.filter_candidates_from(criteria, Some(af.source.obj));
        candidates.retain(|c| !af.targets.contains(c));
        let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
        if up_to {
            // "up to N": the floor is zero (1.15.2e's completion rule applies
            // only to the ceiling the player chose to reach for).
            let n = want.min(candidates.len() as u32);
            return Some(DecisionSpec::ChooseTargets { candidates, count: n, up_to: true, min: 0 });
        }
        Some(self.announcement(candidates, want))
    }

    /// Make the current instruction imminent: compute expected effects, open
    /// the interrupt window if relevant.
    fn begin_imminence(&mut self, instr: Instruction) {
        // §8.5: install instructions expand into the 8.5.16 step sequence
        // (installing is a procedure, not a timing structure — 9.2.2e), the
        // same way Trace expands into the 10.8.6 steps.
        match &instr {
            Instruction::InstallCard { .. } => {
                self.expand_install_card(instr);
                return;
            }
            Instruction::InstallCards { .. } => {
                self.expand_install_cards(instr);
                return;
            }
            Instruction::PlayCard { .. } => {
                self.expand_play_card(instr);
                return;
            }
            Instruction::PlayCards { .. } => {
                self.expand_play_cards(instr);
                return;
            }
            // §8.4: drawing is a PROCEDURE too (8.4.2/8.4.5) — the cards are
            // set aside facedown, a checkpoint happens while they are there,
            // and only then do they reach the hand.
            Instruction::Draw(..) if matches!(self.frames.last(), Some(Frame::Ability(_))) => {
                self.expand_draw(instr);
                return;
            }
            _ => {}
        }
        let (controller, targets, sub_targets, counter_targets) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (
                af.controller,
                af.targets.clone(),
                af.sub_targets.clone(),
                af.counter_targets.clone(),
            )
        };
        let source_obj = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            Some(af.source.obj)
        };
        let atoms = self.expected_atoms(&instr, controller, &targets, source_obj);
        let has_effects = atoms.iter().any(|a| a.expected());
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.any_expected_effects |= has_effects;
            af.phase = AbilityPhase::Imminent;
        }
        // CR 9.6.12/9.8.8: independence at first-instruction imminence.
        cite!("rule_conditional_ability_independent");
        cite!("rule_subroutine_independent");
        let asked =
            self.push_imminent(instr, controller, targets, sub_targets, counter_targets, atoms);
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.imminent_index = Some(0);
        }
        if asked {
            // 9.9.11 order Decision pending; the answer path reopens the
            // interrupt window (phase stays Imminent).
            return;
        }
        if !self.open_interrupt_window_if_relevant() {
            self.set_ability_phase(AbilityPhase::Resolve);
        }
    }

    /// §8.5: expand an InstallCard into the 8.5.16 step sequence. The
    /// announced targets carry either the chosen card (TargetSpec::Choose)
    /// or the chosen host (InstallDest::RunnerChoiceHostOrRig).
    fn expand_install_card(&mut self, instr: Instruction) {
        let Instruction::InstallCard {
            card,
            dest,
            and_rez,
            ignore_costs,
            reveal_check,
            reduce_total,
        } = instr
        else {
            unreachable!()
        };
        cite!("rule_installing");
        cite!("sec_steps_installing");
        let (announced, source_obj) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (af.targets.clone(), af.source.obj)
        };
        let picked: Option<ObjectId> = match &card {
            TargetSpec::Choose { .. } => announced.first().copied(),
            spec => self
                .resolve_targets(spec, Some(source_obj), &announced)
                .first()
                .copied(),
        };
        // 8.7.4: a found card is consumed by the instruction that refers to
        // it, so it is no longer "still set aside" when the ability ends.
        if matches!(card, TargetSpec::FoundBySearch) {
            self.take_found_cards();
        }
        let Some(c) = picked else {
            // Nothing to install: the instruction completes with no effect.
            // 8.7.4: with nothing found, effects referencing found cards fail
            // to resolve and the rest of the ability carries on.
            cite!("rule_continue_after_search");
            self.set_ability_phase(AbilityPhase::Checkpoint);
            return;
        };
        // 1.13.6a / 8.5.16b: if the installer declared an eligible host as
        // the destination, that is the destination; otherwise the one the
        // effect named ("or as normal"). `RunnerChoiceHostOrRig` names the
        // rig (8.5.4).
        let dest = match dest {
            crate::instr::InstallDest::HostedOn(h) => crate::instr::InstallDest::HostedOn(h),
            d => match announced.first() {
                Some(&h) if h != c && self.eligible_hosts_for(c).contains(&h) => {
                    cite!("rule_steps_installing_destination");
                    crate::instr::InstallDest::HostedOn(h)
                }
                _ => match d {
                    crate::instr::InstallDest::RunnerChoiceHostOrRig => {
                        crate::instr::InstallDest::Rig
                    }
                    other => other,
                },
            },
        };
        let was_hidden = {
            let o = &self.st.objects[&c];
            matches!(o.zone, Zone::Hand(_) | Zone::Deck(_))
                // 4.8.4: a card found by a search sits facedown in the
                // set-aside zone — still hidden provenance for 8.5.13.
                || (!o.faceup && matches!(o.zone, Zone::Discard(_) | Zone::SetAside))
        };
        self.installs.push(InstallProgress {
            card: c,
            dest,
            and_rez,
            ignore_costs,
            reveal_check,
            was_hidden,
            aborted: false,
            rez_skipped: false,
            revealed: false,
            resolved_zone: None,
            ice_position: None,
            // 1.16.2f: the "total" modifier is inert without a second cost
            // to divide it with.
            reduce_total: if and_rez {
                self.eval_quantity(&reduce_total, Some(source_obj)).max(0) as u32
            } else {
                0
            },
            reduce_install: 0,
            // 4.8.3: where the card is treated as coming from.
            from_zone: {
                let o = &self.st.objects[&c];
                if o.zone == Zone::SetAside {
                    cite!("rule_set_aside_zone_passthrough");
                    o.set_aside_from.unwrap_or(Zone::SetAside)
                } else {
                    o.zone
                }
            },
        });
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.instructions[af.idx] = Instruction::InstallStepPlace;
            let mut at = af.idx + 1;
            af.instructions.insert(at, Instruction::InstallStepPayCost);
            at += 1;
            af.instructions.insert(at, Instruction::InstallStepComplete);
            at += 1;
            if and_rez {
                cite!("rule_install_and_rez");
                af.instructions.insert(at, Instruction::InstallRezPayCost);
                at += 1;
                af.instructions.insert(at, Instruction::InstallRezFinish);
            }
            af.targets.clear();
            af.announce_slot = 0;
            // Phase stays Targets: the next tick makes InstallStepPlace
            // imminent.
        }
    }

    /// CR 8.3.3: the cards the resolving ability set aside to arrange.
    fn ability_set_aside_group_cards(&self) -> Vec<ObjectId> {
        let Some(group) = self.frames.iter().rev().find_map(|f| match f {
            Frame::Ability(af) => Some(af.set_aside_group),
            _ => None,
        }).flatten() else {
            return Vec::new();
        };
        self.st
            .objects
            .iter()
            .filter(|(_, o)| {
                o.zone == Zone::SetAside && o.set_aside_group.is_some_and(|g| g.id == group)
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// CR 8.3.3: "…and returns them to the top of that deck", in the declared
    /// order. "All of the arranged cards become new objects" (1.12.3), which
    /// is what strands any ability still referring to them; and the arranging
    /// player knows what they put where — 4.2.3 keeps decks ordered — so the
    /// arrangement leaves them seeing those cards, while 8.3.3a keeps the
    /// other player from seeing them at all.
    fn finish_arrangement(&mut self, to_top_of: Side, order: Vec<ObjectId>) {
        cite!("rule_arrange_secretly");
        cite!("rule_deck_ordered");
        for (i, c) in order.iter().enumerate() {
            self.move_card(*c, Zone::Deck(to_top_of));
            let deck = self.st.deck.get_mut(&to_top_of).unwrap();
            deck.retain(|x| x != c);
            deck.insert(i, *c);
        }
        // 8.3.3: every arranged card becomes a new object.
        self.new_objects_for_unknown_location(&order);
        let arranger = self.current_controller().unwrap_or(to_top_of);
        for c in &order {
            self.st.seen.show(arranger, *c);
        }
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.set_aside_group = None;
        }
    }

    /// The controller of the innermost resolving ability.
    fn current_controller(&self) -> Option<Side> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Ability(af) => Some(af.controller),
            _ => None,
        })
    }

    /// CR 8.4.2 / 8.4.5: expand a draw into its step sequence. Drawing is a
    /// procedure, not a timing structure (9.2.2e), so the steps become
    /// instructions in the resolving ability's own list — the same shape
    /// installing (8.5.16) and playing (8.6.7) take. The checkpoint 8.4.5b
    /// calls for is the ordinary post-instruction one between them, and it is
    /// what lets a "whenever you draw" ability resolve while the cards are
    /// still set aside (8.4.2/8.4.2a).
    fn expand_draw(&mut self, instr: Instruction) {
        let Instruction::Draw(side, n) = instr else { unreachable!() };
        cite!("rule_drawing");
        cite!("rule_draw_procedure");
        cite!("sec_steps_of_drawing_n_cards");
        let group = self.st.next_set_aside_group;
        self.st.next_set_aside_group += 1;
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.instructions[af.idx] = Instruction::DrawStepSetAside { side, n, group };
            af.instructions
                .insert(af.idx + 1, Instruction::DrawStepAddToHand { side, group });
        }
    }

    /// CR 8.4.5a: set aside N cards from the top of the drawing player's deck,
    /// facedown, as ONE 4.8.7 group. "The cards are now considered drawn and
    /// can be looked at by their controller" — so `CardDrawn` is recorded here
    /// and 8.4.2a's exception can name the group.
    fn set_aside_drawn(&mut self, side: Side, n: u32, group: u64) {
        if self.draw_prohibited(side) {
            return;
        }
        cite!("rule_facedown_set_aside_distinct_groups");
        for _ in 0..n {
            if self.st.deck[&side].is_empty() {
                return;
            }
            let id = self.st.deck.get_mut(&side).unwrap().remove(0);
            {
                let o = self.st.objects.get_mut(&id).unwrap();
                // 4.8.3: where the card is treated as coming from, so an
                // ability that does not refer to drawn cards sees the move
                // deck -> hand and nothing else (8.4.2b).
                o.set_aside_from = Some(Zone::Deck(side));
                o.zone = Zone::SetAside;
                o.faceup = false;
                o.set_aside_group =
                    Some(crate::view::SetAsideGroup { id: group, by: side, drawn: true });
            }
            self.changes.record(GameChange::CardDrawn { side, obj: id });
        }
    }

    /// CR 8.4.5c: add whatever is still in the drawn group to the hand.
    /// 8.4.3a — a drawn card that LEFT the set-aside zone is no longer drawn
    /// and stays where it went; 8.4.3b — a card swapped INTO the group is now
    /// drawn and goes to the hand with the rest.
    fn add_drawn_set_to_hand(&mut self, side: Side, group: u64) {
        cite!("rule_modify_drawn_cards");
        cite!("rule_card_leaves_drawn_set");
        cite!("rule_drawn_card_swapped");
        let cards: Vec<ObjectId> = self
            .st
            .objects
            .iter()
            .filter(|(_, o)| {
                o.zone == Zone::SetAside && o.set_aside_group.is_some_and(|g| g.id == group)
            })
            .map(|(id, _)| *id)
            .collect();
        for c in cards {
            // 4.2.1/4.3.1: a drawn card goes to the drawing player's hand.
            self.move_card(c, Zone::Hand(side));
        }
    }

    /// CR 8.5.5: an effect installing several cards — each chosen and
    /// installed one at a time, as separate instructions (9.11.4b).
    fn expand_install_cards(&mut self, instr: Instruction) {
        let Instruction::InstallCards {
            count,
            from_hand_of,
            filter,
            dest,
            and_rez,
            and_rez_if_able,
            ignore_costs,
        } = instr
        else {
            unreachable!()
        };
        cite!("rule_install_one_at_a_time");
        cite!("rule_split_up_instruction");
        let announced = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            af.targets.clone()
        };
        let Some(&c) = announced.first().filter(|_| count > 0) else {
            // Declined or exhausted: the multi-install completes.
            self.set_ability_phase(AbilityPhase::Checkpoint);
            return;
        };
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.instructions[af.idx] = Instruction::InstallCard {
                card: TargetSpec::Objects(vec![c]),
                dest,
                and_rez,
                ignore_costs,
                reveal_check: None,
                reduce_total: crate::instr::Quantity::c(0),
            };
            af.instructions.insert(
                af.idx + 1,
                Instruction::InstallCards {
                    count: count - 1,
                    from_hand_of,
                    filter,
                    dest,
                    and_rez,
                    and_rez_if_able,
                    ignore_costs,
                },
            );
            af.targets.clear();
            af.announce_slot = 0;
            // Re-enter Targets: the InstallCard may itself need a
            // destination choice before expanding.
        }
    }

    /// §8.6: expand a PlayCard into the 8.6.7 step sequence.
    fn expand_play_card(&mut self, instr: Instruction) {
        let Instruction::PlayCard { card, ignore_costs } = instr else { unreachable!() };
        cite!("rule_playing");
        cite!("sec_steps_playing");
        let (announced, source_obj) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (af.targets.clone(), af.source.obj)
        };
        let picked: Option<ObjectId> = match &card {
            TargetSpec::Choose { .. } => announced.first().copied(),
            spec => self
                .resolve_targets(spec, Some(source_obj), &announced)
                .first()
                .copied(),
        };
        if matches!(card, TargetSpec::FoundBySearch) {
            self.take_found_cards();
        }
        let Some(c) = picked else {
            cite!("rule_continue_after_search");
            self.set_ability_phase(AbilityPhase::Checkpoint);
            return;
        };
        self.plays.push(PlayProgress { card: c, ignore_costs });
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.instructions[af.idx] = Instruction::PlayStepPlace;
            af.instructions.insert(af.idx + 1, Instruction::PlayStepPayCost);
            af.instructions.insert(af.idx + 2, Instruction::PlayStepActivate);
            af.instructions.insert(af.idx + 3, Instruction::PlayStepResolve);
            af.instructions.insert(af.idx + 4, Instruction::PlayStepFinish);
            af.targets.clear();
            af.announce_slot = 0;
        }
    }

    /// CR 8.6.3: an effect playing several cards — one at a time, each as a
    /// separate instruction; the state between plays is real (Subcontract:
    /// credits from the first operation pay for the second).
    fn expand_play_cards(&mut self, instr: Instruction) {
        let Instruction::PlayCards { count, from_hand_of, ignore_costs } = instr else {
            unreachable!()
        };
        cite!("rule_playing_one_at_a_time");
        cite!("rule_split_up_instruction");
        let announced = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            af.targets.clone()
        };
        let Some(&c) = announced.first().filter(|_| count > 0) else {
            self.set_ability_phase(AbilityPhase::Checkpoint);
            return;
        };
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.instructions[af.idx] = Instruction::PlayCard {
                card: TargetSpec::Objects(vec![c]),
                ignore_costs,
            };
            af.instructions.insert(
                af.idx + 1,
                Instruction::PlayCards { count: count - 1, from_hand_of, ignore_costs },
            );
            af.targets.clear();
            af.announce_slot = 0;
        }
    }

    /// Cards a multi-play effect may choose from: events/operations in hand
    /// whose play cost is affordable NOW (8.6.2; evaluated per pick).
    fn play_pick_candidates(&self, from_hand_of: Side, ignore_costs: bool) -> Vec<ObjectId> {
        self.st.hand[&from_hand_of]
            .iter()
            .copied()
            .filter(|id| self.play_permitted(*id))
            .filter(|id| {
                let o = &self.st.objects[id];
                let playable_type = match from_hand_of {
                    Side::Corp => o.printed.card_type == CardType::Operation,
                    Side::Runner => o.printed.card_type == CardType::Event,
                };
                let afford = ignore_costs
                    || self.st.player(from_hand_of).credits >= o.printed.cost.unwrap_or(0);
                playable_type && afford
            })
            .collect()
    }

    /// CR 9.1.8c: does every "Play only if <state>" requirement printed on
    /// this card hold right now? A card with no such declaration is always
    /// permitted; the declaration is read from the card itself because
    /// `active_statics` only gathers statics of ACTIVE cards and a card in
    /// hand is inactive — which is exactly the case 9.1.8c exists for.
    pub fn play_permitted(&self, card: ObjectId) -> bool {
        cite!("rule_active_exception_modify_play_install_rez");
        let Some(o) = self.st.objects.get(&card) else { return false };
        o.face().abilities.iter().all(|a| {
            a.statics.iter().all(|d| match d {
                StaticDecl::PlayOnlyIf(reqs) => {
                    reqs.iter().all(|r| self.state_requirement_holds(r))
                }
                _ => true,
            })
        })
    }

    /// CR 1.6.6 / rule_start_hand: the starting hand size — 5 unless this
    /// side's identity prints another number (Andromeda class).
    pub fn starting_hand_size(&self, side: Side) -> u32 {
        self.st
            .objects
            .values()
            .find(|o| o.printed.card_type == CardType::Identity && o.printed.side == side)
            .and_then(|o| o.face().starting_hand_size)
            .unwrap_or(5)
    }

    /// One state requirement of the shared predicate vocabulary
    /// (`TriggerRequirement`), evaluated against the present state and the
    /// public game history (10.2.1).
    pub fn state_requirement_holds(&self, req: &crate::ability::TriggerRequirement) -> bool {
        self.state_requirement_holds_for(req, None)
    }

    pub fn state_requirement_holds_for(
        &self,
        req: &crate::ability::TriggerRequirement,
        source: Option<ObjectId>,
    ) -> bool {
        use crate::ability::TriggerRequirement as R;
        match req {
            R::SelfScoredThisTurn => {
                cite!("rule_hidden_or_open_information");
                let Some(src) = source else { return false };
                let log = &self.changes.log;
                let start = log
                    .iter()
                    .rposition(|c| matches!(c, GameChange::TurnBegan { .. }))
                    .unwrap_or(0);
                log[start..]
                    .iter()
                    .any(|c| matches!(c, GameChange::AgendaScored { obj, .. } if *obj == src))
            }
            R::SelfInstalledThisTurn => {
                cite!("rule_hidden_or_open_information");
                let Some(src) = source else { return false };
                let log = &self.changes.log;
                let start = log
                    .iter()
                    .rposition(|c| matches!(c, GameChange::TurnBegan { .. }))
                    .unwrap_or(0);
                log[start..]
                    .iter()
                    .any(|c| matches!(c, GameChange::CardInstalled { obj, .. } if *obj == src))
            }
            R::RunnerTagsAtLeast(n) => {
                cite!("rule_tagged");
                self.st.runner.tags >= *n
            }
            // "…during their last turn": the most recently COMPLETED Runner
            // turn in the change log. During the Runner's own turn that is the
            // previous one, which is what the Corp's cards ask about.
            R::PlayedOperationThisTurn(side) => {
                cite!("rule_hidden_or_open_information");
                let log = &self.changes.log;
                let start = log
                    .iter()
                    .rposition(|c| matches!(c, GameChange::TurnBegan { .. }))
                    .unwrap_or(0);
                log[start..].iter().any(|c| {
                    matches!(c, GameChange::CardPlayed { obj, side: s } if s == side
                        && self.st.objects.get(obj).is_some_and(|o| o.printed.card_type == CardType::Operation))
                })
            }
            R::RunnerMadeRunLastTurn { successful_only } => {
                cite!("rule_hidden_or_open_information");
                let log = &self.changes.log;
                let ends: Vec<usize> = log
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| matches!(c, GameChange::TurnEnded { side: Side::Runner }))
                    .map(|(i, _)| i)
                    .collect();
                let Some(&end) = ends.last() else { return false };
                let start = log[..end]
                    .iter()
                    .rposition(|c| matches!(c, GameChange::TurnBegan { side: Side::Runner }))
                    .unwrap_or(0);
                log[start..end].iter().any(|c| {
                    if *successful_only {
                        matches!(c, GameChange::RunDeclaredSuccessful { .. })
                    } else {
                        matches!(c, GameChange::RunBegan { .. })
                    }
                })
            }
        }
    }

    /// CR 1.20-adjacent: the Runner's link strength (base + active statics).
    pub fn runner_link(&self) -> i32 {
        self.active_statics()
            .iter()
            .filter_map(|(_, d)| match d {
                StaticDecl::LinkBonus(n) => Some(*n),
                _ => None,
            })
            .sum()
    }

    /// CR 8.5.11 / 1.16.6: the install cost of `card` for a destination,
    /// before any Patchwork-class cost-reducing ability. `resolved` is the
    /// zone already declared at step 8.5.16b, when there is one; the 8.7.2b
    /// legality query has only the destination.
    fn install_cost_at(
        &self,
        card: ObjectId,
        dest: crate::instr::InstallDest,
        resolved: Option<Zone>,
    ) -> u32 {
        cite!("sec_install_cost");
        cite!("rule_install_cost_link");
        let o = &self.st.objects[&card];
        let base = match o.printed.card_type {
            // 1 credit per ice already protecting the destination server.
            CardType::Ice => {
                let server = match (resolved, dest) {
                    (Some(Zone::Ice(s)), _) => Some(s),
                    (_, crate::instr::InstallDest::Protecting(s)) => Some(s),
                    (_, crate::instr::InstallDest::InwardFromSource) => None,
                    _ => None,
                };
                server.map(|s| self.ice_at(s).len() as u32).unwrap_or(0)
            }
            CardType::Program | CardType::Hardware | CardType::Resource => {
                o.printed.cost.unwrap_or(0)
            }
            // Assets, agendas, upgrades have no install cost.
            _ => 0,
        };
        let discount = match dest {
            crate::instr::InstallDest::HostedOn(h) => self.host_install_discount(h),
            _ => 0,
        };
        base.saturating_sub(discount)
    }

    /// CR 1.16.6 (Patchwork class): install-cost reductions a player controls
    /// that must themselves be paid for, as `(source, own cost, amount)`.
    fn install_discounts(&self, side: Side) -> Vec<(ObjectId, Cost, u32)> {
        self.active_statics()
            .into_iter()
            .filter(|(src, _)| {
                self.st.objects.get(src).map(|o| o.controller) == Some(side)
            })
            .filter_map(|(src, d)| match d {
                StaticDecl::InstallDiscount { cost, amount } => Some((src, cost, amount)),
                _ => None,
            })
            .collect()
    }

    /// CR 8.5.11 + 1.16.6: what installing `card` actually costs — the credit
    /// amount left after applying as many cost-reducing abilities as are
    /// needed to bring it within reach, plus the combined cost of the
    /// reductions used (paid all at once with the install cost, 1.16.10b).
    ///
    /// KERNEL APPROXIMATION: reductions are applied only when the player
    /// could not otherwise pay ("they must use Patchwork" — the 8.7.2b
    /// example's own words), largest first, and the choice of whether to use
    /// an affordable-anyway reduction is not offered.
    /// Step 8.5.16d proper: pay the install cost, net of the reductions the
    /// installer must use (1.16.6) and of the share of a 1.16.2f "total"
    /// modifier the Corp declared against it.
    /// CR 9.9.6c: the credits an install or play cost payment step is about
    /// to pay — the VALUE the interrupt window is opened over.
    fn imminent_cost_credits(&self) -> i64 {
        cite!("rule_modifiable_value_cost");
        if let Some(p) = self.installs.last() {
            if p.aborted || p.ignore_costs {
                return 0;
            }
            let payer = self.st.objects[&p.card].printed.side;
            let (net, _) = self.install_payment(p.card, p.dest, p.resolved_zone, payer);
            return net.saturating_sub(p.reduce_install) as i64;
        }
        if let Some(pl) = self.plays.last() {
            if pl.ignore_costs {
                return 0;
            }
            return self.st.objects[&pl.card].printed.cost.unwrap_or(0) as i64;
        }
        0
    }

    fn pay_install_cost(&mut self, value: Option<u32>) {
        let Some(p) = self.installs.last().cloned() else { return };
        // 1.16.5c: "ignoring all costs" reduces the cost to 0, but the step
        // still happens and is still followed by a checkpoint (1.16.3a).
        let payer = self.st.objects[&p.card].printed.side;
        // 1.16.6: a Patchwork-class reduction the player needs is used here,
        // and its own cost is part of the same all-at-once payment (1.16.10b).
        let cost = if p.ignore_costs {
            Cost::free()
        } else {
            let (net, extra) = self.install_payment(p.card, p.dest, p.resolved_zone, payer);
            // 1.16.2a: apply the lowering effect, then floor at 0. 9.9.6c: an
            // interrupt that modified the value while the instruction was
            // imminent has already produced the final number.
            cite!("rule_cost_calculation");
            cite!("rule_modifiable_value_cost");
            let credits = value.unwrap_or_else(|| net.saturating_sub(p.reduce_install));
            extra.plus(&Cost::credits(credits))
        };
        self.pay_cost(payer, p.card, &cost);
    }

    fn install_payment(
        &self,
        card: ObjectId,
        dest: crate::instr::InstallDest,
        resolved: Option<Zone>,
        payer: Side,
    ) -> (u32, Cost) {
        cite!("rule_install_cost");
        let mut net = self.install_cost_at(card, dest, resolved);
        let mut extra = Cost::free();
        let mut pool: Vec<(ObjectId, Cost, u32)> = self.install_discounts(payer);
        pool.sort_by_key(|(_, _, amount)| std::cmp::Reverse(*amount));
        for (src, cost, amount) in pool {
            if self.st.player(payer).credits >= net {
                break;
            }
            let combined = extra.plus(&cost);
            if !self.cost_payable(payer, src, &combined.plus(&Cost::credits(net.saturating_sub(amount))))
            {
                continue;
            }
            net = net.saturating_sub(amount);
            extra = combined;
        }
        (net, extra)
    }

    /// CR 8.7.2b: when a search is followed by an install instruction
    /// referring to the found cards, a card can only be FOUND if the
    /// searching player would actually be able to install it.
    ///
    /// A pure LEGALITY QUERY: nothing moves, no cost is paid. It answers
    /// "could that install instruction resolve for this candidate?" by the
    /// two things the CR names — the card must be of an installable type
    /// (8.5.1/8.5.3: events and operations never are), and the install cost
    /// (8.5.11) must be payable INCLUDING the cost-reducing abilities the
    /// player would have to use (the Patchwork example).
    ///
    /// It deliberately does NOT require the card to be rezzable: an
    /// "install and rez" follow-on still permits the find, and 8.5.13d makes
    /// the Corp reveal the card it cannot rez (the Tucana example).
    ///
    /// APPROXIMATION (recorded in docs/vm/WAVES.md): the destination's own
    /// legality (8.5.14 invalid destinations, 8.5.2 server limits) is not
    /// re-derived here; no example turns on it.
    pub fn could_install_found_card(
        &self,
        card: ObjectId,
        dest: crate::instr::InstallDest,
        ignore_costs: bool,
    ) -> bool {
        cite!("rule_valid_search_target_install_play");
        let Some(o) = self.st.objects.get(&card) else { return false };
        // 8.5.1/8.5.3: only these card types are ever installed.
        cite!("rule_installing");
        let installable = match o.printed.card_type {
            CardType::Agenda | CardType::Asset | CardType::Ice | CardType::Upgrade => {
                o.printed.side == Side::Corp
            }
            CardType::Program | CardType::Hardware | CardType::Resource => {
                o.printed.side == Side::Runner
            }
            // Events, operations and identities can never be installed.
            CardType::Event | CardType::Operation | CardType::Identity => false,
        };
        if !installable {
            return false;
        }
        // 1.13.6c: no valid host means the card cannot be installed at all.
        if !self.install_destination_available(card) {
            return false;
        }
        if ignore_costs {
            cite!("rule_ignore_all_costs");
            return true;
        }
        let payer = o.printed.side;
        let (net, extra) = self.install_payment(card, dest, None, payer);
        self.cost_payable(payer, card, &extra.plus(&Cost::credits(net)))
    }

    /// CR 8.7.2b, play branch: "The same is true when the search is followed
    /// by a play instruction." Only events/operations are ever played
    /// (8.6.1) and the play cost (8.6.2) must be payable.
    pub fn could_play_found_card(&self, card: ObjectId, ignore_costs: bool) -> bool {
        cite!("rule_valid_search_target_install_play");
        cite!("rule_playing");
        let Some(o) = self.st.objects.get(&card) else { return false };
        let playable = match o.printed.card_type {
            CardType::Event => o.printed.side == Side::Runner,
            CardType::Operation => o.printed.side == Side::Corp,
            _ => false,
        };
        if !playable {
            return false;
        }
        // 1.16.10a/b: an additional cost to play the card is part of the one
        // payment, so it is part of "could this card be played".
        let mut cost = Cost::credits(o.printed.cost.unwrap_or(0));
        if let Some(extra) = &o.printed.additional_play_cost {
            cite!("rule_additional_cost");
            cost = cost.plus(extra);
        }
        ignore_costs || self.cost_payable(o.printed.side, card, &cost)
    }

    /// Dhegdheer-class hosted-install discount.
    fn host_install_discount(&self, host: ObjectId) -> u32 {
        let Some(o) = self.st.objects.get(&host) else { return 0 };
        if !card_active(o) {
            return 0;
        }
        o.printed
            .abilities
            .iter()
            .enumerate()
            .filter(|(i, a)| a.kind == AbilityKind::Static && self.ability_present(host, *i))
            .flat_map(|(_, a)| a.statics.iter())
            .filter_map(|d| match d {
                // 1.13.9: the discount belongs to the card the installee is
                // hosted ON, and host relationships are not transitive.
                StaticDecl::HostedInstallDiscount(q) => {
                    cite!("rule_host_transitivity");
                    Some(self.eval_quantity(q, Some(host)).max(0) as u32)
                }
                _ => None,
            })
            .sum()
    }

    // ------------------------------------------------------------------
    // §8.7 searching, finding, shuffling
    // ------------------------------------------------------------------

    /// The cards in a searched zone (8.7.1: the player looks at ALL of them),
    /// in whatever order that zone maintains (4.2.3: decks are ordered).
    pub fn cards_in_zone(&self, zone: Zone) -> Vec<ObjectId> {
        match zone {
            Zone::Deck(s) => self.st.deck[&s].clone(),
            Zone::Hand(s) => self.st.hand[&s].clone(),
            Zone::Discard(s) => self.st.discard[&s].clone(),
            Zone::ScoreArea(s) => self.st.score_area[&s].clone(),
            Zone::Root(s) => self.st.root.get(&s).cloned().unwrap_or_default(),
            Zone::Ice(s) => self.ice_at(s),
            other => self
                .st
                .objects
                .values()
                .filter(|o| o.zone == other)
                .map(|o| o.id)
                .collect(),
        }
    }

    /// CR 8.7.2a + 8.7.2b: the cards a search may legally FIND — those in the
    /// zone matching every criterion, further restricted by what the
    /// follow-on instruction of this same ability could do with them.
    fn valid_find_targets(&self, zone: Zone, criteria: &[TargetFilter]) -> Vec<ObjectId> {
        cite!("rule_search");
        cite!("rule_search_hidden_secret_zone");
        cite!("rule_deck_order_while_searching");
        let source = self.frames.iter().rev().find_map(|f| match f {
            Frame::Ability(af) => Some(af.source.obj),
            _ => None,
        });
        let follow_on = self.search_follow_on();
        self.cards_in_zone(zone)
            .into_iter()
            .filter(|id| {
                let Some(o) = self.st.objects.get(id) else { return false };
                cite!("rule_valid_search_target_criteria");
                criteria.iter().all(|f| self.filter_matches(o, *f, source))
            })
            .filter(|id| match follow_on {
                Some(FollowOn::Install { dest, ignore_costs }) => {
                    self.could_install_found_card(*id, dest, ignore_costs)
                }
                Some(FollowOn::Play { ignore_costs }) => {
                    self.could_play_found_card(*id, ignore_costs)
                }
                None => true,
            })
            .collect()
    }

    /// CR 8.7.2b: does a later instruction of the resolving ability install
    /// or play the cards this search finds? Read off the instruction list —
    /// no card identity is involved.
    fn search_follow_on(&self) -> Option<FollowOn> {
        let Some(Frame::Ability(af)) = self.frames.last() else { return None };
        af.instructions[af.idx + 1..].iter().find_map(|i| match i {
            Instruction::InstallCard {
                card: TargetSpec::FoundBySearch,
                dest,
                ignore_costs,
                ..
            } => Some(FollowOn::Install { dest: *dest, ignore_costs: *ignore_costs }),
            Instruction::PlayCard { card: TargetSpec::FoundBySearch, ignore_costs } => {
                Some(FollowOn::Play { ignore_costs: *ignore_costs })
            }
            _ => None,
        })
    }

    /// CR 8.7.2: take the found cards from the searched zone and set them
    /// aside facedown (4.8.4), then complete the search — 8.7.3 reshuffles a
    /// searched deck IMMEDIATELY, before any remaining effect of the ability
    /// and before any chain reaction, and only then is the search recorded as
    /// complete (8.7.5).
    fn complete_search(&mut self, zone: Zone, found: &[ObjectId], searcher: Side) {
        cite!("rule_find");
        for &c in found {
            // 4.8.3: remember where the card came from, so a later move out
            // of the set-aside zone is reported as coming from there.
            cite!("rule_set_aside_zone_passthrough");
            let was = self.st.objects[&c].zone;
            self.st.objects.get_mut(&c).unwrap().set_aside_from = Some(was);
            self.st.objects.get_mut(&c).unwrap().zone = Zone::SetAside;
            self.st.objects.get_mut(&c).unwrap().set_aside_for_ability = true;
            match zone {
                Zone::Deck(s) => self.st.deck.get_mut(&s).unwrap().retain(|&x| x != c),
                Zone::Hand(s) => self.st.hand.get_mut(&s).unwrap().retain(|&x| x != c),
                Zone::Discard(s) => self.st.discard.get_mut(&s).unwrap().retain(|&x| x != c),
                Zone::Root(s) => {
                    self.st.root.entry(s).or_default().retain(|&x| x != c)
                }
                // 6.2.4: taken out of its position, which then awaits
                // 10.3.1i like any other vacated position.
                Zone::Ice(_) => self.vacate_ice(c),
                _ => {}
            }
            cite!("rule_searched_cards_set_aside");
        }
        // 8.7.2c: found cards are NOT revealed unless the ability says so,
        // and not until resolution resumes — nothing is recorded here.
        cite!("rule_reveal_for_search");
        if let Zone::Deck(side) = zone {
            cite!("rule_shuffle_deck_after_search");
            self.shuffle_deck(side);
        }
        // 9.11.4d: ending the search and performing any necessary shuffling
        // IS the end of an instruction — the post-instruction checkpoint that
        // follows is the one at which a search-involving condition becomes
        // pending (8.7.5), while the found cards are still set aside.
        cite!("rule_search_instruction");
        self.changes.record(GameChange::ZoneSearched { by: searcher, zone });
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.found_cards.extend(found.iter().map(|&c| (c, zone)));
        }
    }

    /// CR 8.7.3: shuffle a deck, recording the change so its position in the
    /// log witnesses that it happened before anything else continued.
    fn shuffle_deck(&mut self, side: Side) {
        let deck = self.st.deck.get_mut(&side).unwrap();
        deck.shuffle(&mut self.rng);
        let ids: Vec<ObjectId> = deck.clone();
        // 1.12.3: a card moved to an UNKNOWN LOCATION becomes a new object,
        // even though it did not change zones — nobody can say which card
        // corresponds to which of the objects that were there before.
        self.new_objects_for_unknown_location(&ids);
        self.changes.record(GameChange::DeckShuffled { side });
    }

    /// CR 1.12.3: these cards were moved to an unknown location — each of
    /// them is now a NEW object, so every reference to the objects that were
    /// there is stranded (9.1.4) and every "this object already" bookkeeping
    /// (7.4.3/7.4.7a) forgets them.
    fn new_objects_for_unknown_location(&mut self, ids: &[ObjectId]) {
        cite!("rule_object_move_location");
        for id in ids {
            if let Some(o) = self.st.objects.get_mut(id) {
                o.generation += 1;
            }
        }
        self.st.move_seq += 1;
    }

    /// 8.5.13: reveal the installing card once, if not already revealed.
    fn install_reveal(&mut self, card: ObjectId) {
        cite!("rule_install_from_hidden_or_secret_zone");
        if let Some(p) = self.installs.last_mut() {
            if p.revealed {
                return;
            }
            p.revealed = true;
        }
        // 1.21.3 / 10.2.2b: revealing shows the front face to ALL players,
        // which is one of the game effects by which a player learns hidden
        // information.
        self.st.seen.show_all(card);
        self.changes.record(GameChange::CardRevealed { obj: card });
    }

    /// Terminal 8.5.13c check: a hidden-provenance card that ends the
    /// process facedown, installed by an ability imposing requirements, is
    /// revealed to verify the installation.
    fn install_terminal_reveal(&mut self, p: &InstallProgress) {
        if p.aborted || p.revealed {
            return;
        }
        if p.was_hidden && p.reveal_check.is_some() && !self.st.objects[&p.card].faceup {
            cite!("rule_reveal_for_ability_limitations");
            self.st.seen.show_all(p.card);
            self.changes.record(GameChange::CardRevealed { obj: p.card });
        }
    }

    /// Resolve the imminent instruction of the top ability frame.
    fn resolve_current_instruction(&mut self) {
        let imm = self.imminents.pop().expect("imminent instruction to resolve");
        self.changes.bump_group();
        let (frame_idx, controller, source, stamp) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (self.frames.len() - 1, af.controller, af.source, af.source_generation)
        };
        // CR 9.1.4: if the source changed zones after independence, the
        // ability cannot act on the source.
        cite!("rule_abilities_resolution_independent");
        let source_moved =
            self.st.objects.get(&source.obj).map(|o| o.active_since).is_none()
                || self.source_moved_since(source.obj, stamp);
        self.apply_imminent(imm, controller, source, source_moved);
        // Advance THIS frame's phase by index — resolution may have pushed
        // frames above us (a nested run, 9.2.4d) or unwound us (ETR).
        if let Some(Frame::Ability(af)) = self.frames.get_mut(frame_idx) {
            if af.source == source && af.phase == AbilityPhase::Resolve {
                af.phase = AbilityPhase::Checkpoint;
            }
        }
    }

    /// CR 9.1.4 + 1.12.3: has the ability's source changed zones since the
    /// ability became independent of it? The card that was the source is one
    /// OBJECT; a zone change makes the card a NEW object (1.12.3), which the
    /// generation stamp records. An ability whose source generation no longer
    /// matches cannot act on the source — there is nothing there to act on.
    fn source_moved_since(&self, obj: ObjectId, generation: u32) -> bool {
        cite!("rule_abilities_resolution_independent");
        cite!("rule_object_move_location");
        self.st.objects.get(&obj).map(|o| o.generation) != Some(generation)
    }

    /// Apply a resolved instruction's effects (9.9.2a: expected effects
    /// correspond to what happens, except 9.9.7d dead values).
    fn apply_imminent(
        &mut self,
        imm: ImminentWrap,
        controller: Side,
        source: AbilityRef,
        source_moved: bool,
    ) {
        cite!("rule_expected_effects_resolve");
        let instr = imm.instr.clone();
        // 1.14.5: peel the "<player> does this" wrapper — the named player is
        // the one carrying the effect out from here on.
        let (controller, instr) = match instr {
            Instruction::PerformedBy { side, instr } => {
                cite!("rule_controller_choices");
                (side, *instr)
            }
            other => (controller, other),
        };
        match &instr {
            Instruction::GainCredits(side, _) => {
                // 1.10.3a: credits enter the pool from the bank.
                cite!("rule_gain_credits");
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let n = a.value.max(0) as u32;
                    self.st.player_mut(*side).credits += n;
                    self.changes.record(GameChange::CreditsGained { side: *side, amount: n });
                }
            }
            Instruction::LoseCredits(side, _) => {
                // 1.10.3b: a forced loss moves credits from the pool to the
                // bank — as many as the pool holds and no more, and credits
                // on cards can never be lost this way (1.13.3 keeps the two
                // populations apart).
                cite!("rule_lose_credits");
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let have = self.st.player(*side).credits;
                    let n = (a.value.max(0) as u32).min(have);
                    self.st.player_mut(*side).credits -= n;
                    self.changes.record(GameChange::CreditsLost { side: *side, amount: n });
                }
            }
            // 1.11.3a: gaining clicks increases the number the player has.
            Instruction::GainClicks(side, _) => {
                cite!("rule_gain_spend_lose_clicks");
                cite!("rule_gain_clicks");
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let n = a.value.max(0) as u32;
                    self.st.player_mut(*side).clicks += n;
                    self.changes.record(GameChange::ClicksGained { side: *side, amount: n });
                }
            }
            // 1.11.3b: losing clicks reduces the number the player has by that
            // amount — a player holding fewer simply reaches zero, exactly as
            // 1.10.3b's forced credit loss takes what the pool holds.
            Instruction::LoseClicks(side, _) => {
                cite!("rule_lose_spend_clicks");
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let have = self.st.player(*side).clicks;
                    let n = (a.value.max(0) as u32).min(have);
                    self.st.player_mut(*side).clicks -= n;
                    self.changes.record(GameChange::ClicksLost { side: *side, amount: n });
                }
            }
            // A `Draw` that reached resolution unexpanded (no ability frame to
            // splice into) keeps the one-shot behaviour.
            Instruction::Draw(side, _) => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    self.draw_cards(*side, a.value.max(0) as u32, false);
                }
            }
            Instruction::DrawStepSetAside { side, group, .. } => {
                cite!("step_draw_set_aside");
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    self.set_aside_drawn(*side, a.value.max(0) as u32, *group);
                }
            }
            Instruction::DrawStepAddToHand { side, group } => {
                cite!("step_draw_add_to_hand");
                self.add_drawn_set_to_hand(*side, *group);
            }
            Instruction::Damage { responsible, .. } => {
                // 10.4.3a: a declaration may make the trashed cards a CHOICE
                // rather than random. The choice is a Decision, so this
                // instruction may suspend and finish in `answer` — the same
                // shape `Instruction::Sabotage` uses.
                let hit = imm
                    .atoms
                    .iter()
                    .filter(|a| a.occurs_at_resolution())
                    .find_map(|a| match a.class {
                        EffectClass::Damage(kind) if a.value > 0 => Some((kind, a.value as u32)),
                        _ => None,
                    });
                if let Some((kind, amount)) = hit {
                    if let Some((by, n)) = self.damage_trash_selector() {
                        let hand = self.st.hand[&Side::Runner].clone();
                        let want = n.min(amount).min(hand.len() as u32);
                        if want > 0 {
                            cite!("rule_multiple_damage_selected_sequentially");
                            self.ask(
                                by,
                                DecisionSpec::ChooseTargets {
                                    candidates: hand,
                                    count: want,
                                    up_to: false,
                                    min: want,
                                },
                                DecisionCtx::DamageSelection { kind, amount },
                            );
                            return;
                        }
                    }
                }
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    if let EffectClass::Damage(kind) = a.class {
                        self.do_damage(kind, a.value as u32, *responsible);
                    }
                }
            }
            Instruction::GainTags(_) => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let n = a.value as u32;
                    self.st.runner.tags += n;
                    self.changes.record(GameChange::TagsTaken { amount: n });
                }
            }
            Instruction::TrashCards(_) | Instruction::TrashSelf => {
                let self_trash = matches!(instr, Instruction::TrashSelf);
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let targets: Vec<ObjectId> = if self_trash {
                        if source_moved {
                            // 9.1.4: stranded self-reference does nothing
                            // (Compile/Mayfly).
                            Vec::new()
                        } else {
                            vec![source.obj]
                        }
                    } else {
                        a.targets.clone()
                    };
                    for t in targets {
                        self.trash_card(t, controller);
                    }
                }
            }
            Instruction::EndTheRun => {
                if imm.atoms.iter().any(|a| a.occurs_at_resolution()) {
                    self.end_the_run();
                    return; // frame already unwound; no phase advance
                }
            }
            Instruction::Combined(list) => {
                // CR 9.11.4a: a sentence describing several effects is
                // normally several INSTRUCTIONS. `Combined` is the exception
                // the CR's own examples force (Snare!'s "do 3 net damage and
                // give the Runner 1 tag" is one instruction, so one interrupt
                // window sees both), and it works by merging the sub-
                // instructions' expected atoms into one imminent set.
                //
                // That merge can only carry a sub-instruction whose effect IS
                // a value: a STRUCTURAL atom carries none, so the merged set
                // has nothing to resolve from and the sub-instruction used to
                // be silently dropped (Earthrise Hotel's "remove 1 hosted
                // power counter and draw 2 cards" removed nothing). Those
                // sub-instructions are what 9.11.4a calls separate
                // instructions (9.11.3: "usually, each sentence in the text
                // of an ability forms a single instruction"), and they are
                // spliced in after this one.
                //
                // DEVIATION: a spliced sub-instruction resolves AFTER every
                // merged one, so printed order is not preserved between the
                // two kinds. Nothing in the corpus distinguishes them; a card
                // that did would want its sentence written as two
                // instructions, which 9.11.4a already permits.
                cite!("rule_instructions_in_ability_text");
                cite!("rule_instruction_sentence_exceptions");
                let deferred: Vec<Instruction> = list
                    .iter()
                    .filter(|i| {
                        let atoms =
                            self.expected_atoms(i, controller, &imm.targets, Some(source.obj));
                        !atoms.is_empty()
                            && atoms.iter().all(|a| a.class == EffectClass::Structural)
                    })
                    .cloned()
                    .collect();
                if !deferred.is_empty() {
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        let at = af.idx + 1;
                        for (k, ins) in deferred.into_iter().enumerate() {
                            af.instructions.insert(at + k, ins);
                        }
                    }
                }
                // Combined instructions carry heterogeneous atoms; apply each.
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    match a.class {
                        EffectClass::Damage(kind) => {
                            self.do_damage(kind, a.value as u32, controller)
                        }
                        EffectClass::TakeTags => {
                            let n = a.value as u32;
                            self.st.runner.tags += n;
                            self.changes.record(GameChange::TagsTaken { amount: n });
                        }
                        EffectClass::GainCredits => {
                            let n = a.value.max(0) as u32;
                            self.st.player_mut(a.side).credits += n;
                            self.changes
                                .record(GameChange::CreditsGained { side: a.side, amount: n });
                        }
                        EffectClass::LoseCredits => {
                            let have = self.st.player(a.side).credits;
                            let n = (a.value.max(0) as u32).min(have);
                            self.st.player_mut(a.side).credits -= n;
                            self.changes
                                .record(GameChange::CreditsLost { side: a.side, amount: n });
                        }
                        EffectClass::Draw => self.draw_cards(a.side, a.value.max(0) as u32, false),
                        EffectClass::TrashCards => {
                            for t in a.targets.clone() {
                                self.trash_card(t, controller);
                            }
                        }
                        EffectClass::EndTheRun => {
                            self.end_the_run();
                            return;
                        }
                        _ => {}
                    }
                }
            }
            Instruction::DeclineableChoice(inner) => {
                let declined = {
                    let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                    af.declined
                };
                if !declined {
                    // 9.6.9d: an optional component was carried out — used;
                    // this expends once-per-turn restrictions (9.3.6g).
                    cite!("rule_optional_conditional_ability_use");
                    cite!("rule_once_per_turn_flag");
                    self.changes.record(GameChange::AbilityUsed { source: source.obj });
                    self.once_per_turn_used.insert((source, self.generation(source.obj)));
                    // An inner instruction that EXPANDS into a step sequence
                    // (installing, playing, a trace — 9.2.2e procedures) has to
                    // go back through imminence to be expanded and to announce
                    // its own targets, so it is spliced in as the next
                    // instruction, exactly as a nested cost's paid-for branch
                    // is (9.11.4f).
                    if matches!(
                        **inner,
                        Instruction::InstallCard { .. }
                            | Instruction::InstallCards { .. }
                            | Instruction::PlayCard { .. }
                            | Instruction::PlayCards { .. }
                            | Instruction::Trace { .. }
                    ) {
                        cite!("rule_nested_cost_instruction");
                        let next = (**inner).clone();
                        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                            let at = af.idx + 1;
                            af.instructions.insert(at, next);
                        }
                        return;
                    }
                    let inner_imm = ImminentWrap {
                        counter_targets: Vec::new(),
                        instr: (**inner).clone(),
                        atoms: imm.atoms.clone(),
                        controller,
                        targets: imm.targets.clone(),
                        sub_targets: imm.sub_targets.clone(),
                        run_ordinal: imm.run_ordinal.clone(),
                        turn_ordinal: imm.turn_ordinal.clone(),
                        seq: imm.seq,
                    };
                    self.apply_imminent(inner_imm, controller, source, source_moved);
                }
            }
            Instruction::NestedCostThen { .. } | Instruction::NestedCostUnless { .. } => {
                // Handled at answer time (rule_nested_cost_instruction): the
                // choice ended this instruction; the appropriate branch was
                // injected as the next instruction.
            }
            Instruction::MoveSetAsideCounters { kind, target } => {
                // CR 9.5.5 (Reconstruction Contract): move the set-aside
                // counters to the chosen target.
                cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                let moved: u32 = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { unreachable!() };
                    let mut total = 0;
                    af.set_aside_counters.retain(|(k, n)| {
                        if k == kind {
                            total += *n;
                            false
                        } else {
                            true
                        }
                    });
                    total
                };
                if let Some(t) = targets.first() {
                    if moved > 0 {
                        let obj = self.st.objects.get_mut(t).unwrap();
                        *obj.counters.entry(*kind).or_insert(0) += moved;
                        self.changes.record(GameChange::CounterPlaced {
                            obj: *t,
                            kind: *kind,
                            amount: moved,
                        });
                    }
                }
            }
            Instruction::PreventDamage { kind, amount } => {
                // 9.9.7f: the condition is met only if the value was ABOVE 0
                // before this interrupt decreased it.
                let before = self.imminent_damage_value(*kind);
                self.modify_parent_imminent(|atom| {
                    if atom.class == EffectClass::Damage(*kind) {
                        atom.prevent(*amount as i64);
                        true
                    } else {
                        false
                    }
                });
                if before > 0 {
                    cite!("rule_prevent_as_trigger_condition");
                    let after = self.imminent_damage_value(*kind);
                    let prevented = (before - after).max(0) as u32;
                    self.changes.record(GameChange::DamagePrevented {
                        by: source.obj,
                        kind: *kind,
                        amount: prevented,
                    });
                }
            }
            Instruction::PreventAllDamage { kind } => {
                let before = self.imminent_damage_value(*kind);
                self.modify_parent_imminent(|atom| {
                    if atom.class == EffectClass::Damage(*kind) {
                        atom.prevent_all();
                        true
                    } else {
                        false
                    }
                });
                if before > 0 {
                    cite!("rule_prevent_as_trigger_condition");
                    self.changes.record(GameChange::DamagePrevented {
                        by: source.obj,
                        kind: *kind,
                        amount: before as u32,
                    });
                }
            }
            Instruction::AvoidTags(n) => {
                self.modify_parent_imminent(|atom| {
                    if atom.class == EffectClass::TakeTags {
                        atom.prevent(*n as i64);
                        true
                    } else {
                        false
                    }
                });
                // Thunder-Art-Gallery-class conditions meet on tag
                // avoidance; the chain reaction resolves while the interrupt
                // window is still open (9.9.4c/d examples).
                self.changes.record(GameChange::TagsAvoided { amount: *n });
            }
            Instruction::ReduceImminentCost { amount } => {
                // 9.9.6c: decrease the cost value of the imminent instruction.
                cite!("rule_modifiable_value_cost");
                let n = self.eval_quantity(amount, Some(source.obj)).max(0);
                self.modify_parent_imminent(move |atom| {
                    if atom.class == EffectClass::PayCost {
                        atom.value -= n;
                        true
                    } else {
                        false
                    }
                });
            }
            Instruction::IncreaseImminentDamage { kind, amount } => {
                self.modify_parent_imminent(|atom| {
                    if atom.class == EffectClass::Damage(*kind) {
                        atom.value += *amount as i64;
                        true
                    } else {
                        false
                    }
                });
            }
            Instruction::DamageUnpreventable { responsible, .. } => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    if let EffectClass::Damage(kind) = a.class {
                        self.do_damage(kind, a.value as u32, *responsible);
                    }
                }
            }
            Instruction::ReplaceImminentDamageKind { to } => {
                // CR 9.9.10: the replacement applies immediately when the
                // interrupt resolves; relevance is re-evaluated against the
                // NEW expected effects afterwards.
                cite!("rule_replace_imminent_effects");
                let to = *to;
                self.modify_parent_imminent(move |atom| {
                    if matches!(atom.class, EffectClass::Damage(_)) {
                        atom.class = EffectClass::Damage(to);
                        true
                    } else {
                        false
                    }
                });
            }
            Instruction::AccessCards { cards } => {
                // CR 7.2: each announced card is accessed in its own access
                // timing structure. Pushed innermost-last so the first
                // announced card is accessed first (9.2.4d LIFO).
                cite!("rule_accessing");
                let targets = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                for c in targets.into_iter().rev() {
                    self.push_access(c);
                }
            }
            Instruction::BreachServer(server) => {
                // 7.3.8: a breach that would begin while one is in progress
                // takes place when the current breach ends instead. The rule
                // says how: "the effect creating the delayed breach is treated
                // as a conditional ability controlled by the Runner", so the
                // kernel makes exactly that — a one-shot delayed conditional
                // whose condition is the current breach ending and whose
                // instruction is the breach that was postponed.
                if self.breach_in_progress() {
                    cite!("rule_consecutive_breaches");
                    let def = AbilityDef::conditional(
                        TriggerCond::BreachEnds,
                        vec![Instruction::BreachServer(*server)],
                        false,
                    )
                    .labeled("delayed breach");
                    let id = self.next_lingering;
                    self.next_lingering += 1;
                    self.lingering.push(LingeringEffect::new(
                        id,
                        source.obj,
                        Payload::DelayedConditional { def },
                        // 9.6.13c: no stated duration — until it resolves.
                        crate::lingering::Duration::UntilResolved,
                    ));
                } else {
                    self.push_breach(*server);
                }
            }
            Instruction::InitiateRun { server, allowed, if_successful } => {
                cite!("rule_run_timing_structure");
                // 6.9.1a: the attacked server has been announced by now — an
                // effect that named one carried it, an effect that did not
                // asked. With no legal announcement available there is no run
                // to initiate and the instruction does nothing.
                let Some(server) = *server else { return };
                self.initiate_run(server);
                // CR 6.7.4/6.7.4a: the "If successful" ability belongs to the
                // effect that initiated the run, and is tied to the servers
                // that effect allowed. Both travel with the run.
                if !if_successful.is_empty() {
                    cite!("rule_if_successful");
                    cite!("rule_if_successful_tied_to_server");
                    let clause = IfSuccessful {
                        source: source,
                        controller,
                        allowed: allowed.clone(),
                        effects: if_successful.clone(),
                    };
                    if let Some(r) = self.run_ctx_mut() {
                        r.if_successful = Some(clause);
                    }
                }
                // The nested run frame is now on top; this ability resumes
                // after the run completes (9.2.4d LIFO nesting).
            }
            Instruction::TraceInitiate { .. } => {
                // 10.8.6a: the trace initiates; "when initiated" conditions
                // meet. The (possibly modified) base is the atom's value.
                cite!("step_trace_initiated");
                cite!("rule_trace_attempt_and_base_trace_strength");
                let base = imm.atoms.first().map(|a| a.value).unwrap_or(0);
                self.trace = Some(TraceState { trace_strength: base, link_strength: 0 });
                self.changes.record(GameChange::TraceInitiated { base });
                // 10.8.6b: a checkpoint occurs — this is the post-instruction
                // checkpoint of this expanded step (9.11.1e).
                cite!("step_trace_checkpoint");
                cite!("rule_trace_checkpoint");
            }
            Instruction::TraceCorpSpend => {
                cite!("step_trace_corp_spend_credits");
                cite!("rule_trace_strength");
                let max = self.spendable_credits(Side::Corp);
                let strength = self.trace.as_ref().map(|t| t.trace_strength).unwrap_or(0);
                self.ask(
                    Side::Corp,
                    DecisionSpec::TraceSpend { max, strength_so_far: strength, corp_side: true },
                    DecisionCtx::TraceSpend(Side::Corp),
                );
            }
            Instruction::TraceRunnerSpend => {
                cite!("step_trace_runner_spend_credits");
                cite!("rule_link_strength");
                let max = self.spendable_credits(Side::Runner);
                let link = self.trace.as_ref().map(|t| t.link_strength).unwrap_or(0);
                self.ask(
                    Side::Runner,
                    DecisionSpec::TraceSpend { max, strength_so_far: link, corp_side: false },
                    DecisionCtx::TraceSpend(Side::Runner),
                );
            }
            Instruction::TraceDetermine { if_successful, if_unsuccessful, determined_min } => {
                cite!("step_trace_determine_success");
                cite!("rule_compare_trace_and_link_strength");
                let t = self.trace.take().unwrap_or(TraceState {
                    trace_strength: 0,
                    link_strength: 0,
                });
                let success = t.trace_strength > t.link_strength;
                self.changes.record(GameChange::TraceDetermined {
                    success,
                    trace_strength: t.trace_strength,
                    link_strength: t.link_strength,
                });
                // 10.8.5: the associated conditionals pend after (e); they
                // resolve as the following instructions of this ability.
                cite!("rule_trace_conditional_abilities");
                let mut inject: Vec<Instruction> =
                    if success { if_successful.clone() } else { if_unsuccessful.clone() };
                if let Some((min, extra)) = determined_min {
                    if t.trace_strength >= *min {
                        inject.extend(extra.iter().cloned());
                    }
                }
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    for (k, ins) in inject.into_iter().enumerate() {
                        af.instructions.insert(af.idx + 1 + k, ins);
                    }
                }
                cite!("step_trace_complete");
            }
            Instruction::PsiGame { on_match, on_differ } => {
                // 10.14.6c: one instruction — sealed bids, reveal, immediate
                // spend, then the outcome branch; no checkpoints inside.
                cite!("rule_psi_game");
                cite!("rule_psi_bid_reveal");
                let _ = (on_match, on_differ);
                let legal = self.psi_legal_bids(Side::Corp);
                self.ask(
                    Side::Corp,
                    DecisionSpec::PsiBid { legal },
                    DecisionCtx::PsiBid(Side::Corp),
                );
            }
            Instruction::GrantSubroutines { to, grant, before, any_order, duration } => {
                // 9.8.3a/e: externally-granted subroutines, ordered by grant
                // time within their category; they arrive unbroken (9.8.4b).
                cite!("rule_subroutine_origins");
                let dur = crate::lingering::bind_duration(
                    *duration,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                let ice = match to {
                    TargetSpec::SelfSource => vec![source.obj],
                    other => self.resolve_targets(other, Some(source.obj), &imm.targets),
                };
                // WHICH subroutines: a stated one repeated, or the effective
                // subroutines of another card (Loki class), in ITS order.
                let subs: Vec<AbilityDef> = match grant {
                    crate::instr::SubroutineGrant::Stated { count, sub } => {
                        (0..*count).map(|_| (**sub).clone()).collect()
                    }
                    crate::instr::SubroutineGrant::CopiedFrom(spec) => {
                        cite!("rule_subroutine_origin_external_before");
                        self.resolve_targets(spec, Some(source.obj), &imm.targets)
                            .into_iter()
                            .flat_map(|from| {
                                self.current_subs(from).into_iter().map(|(_, d)| d)
                            })
                            .collect()
                    }
                };
                for obj in ice {
                    // 9.8.3a/e order by "when the effect granting them began
                    // to apply"; `active_seq` is the kernel's clock for that,
                    // shared with static grants (`static_subroutine_grants`)
                    // so the two compare. ONE effect granting several
                    // subroutines is ONE moment, so they share the stamp and
                    // are ordered among themselves by `ord`.
                    self.st.active_seq += 1;
                    let seq = self.st.active_seq;
                    for (ord, sub) in subs.iter().enumerate() {
                        let id = self.next_lingering;
                        self.next_lingering += 1;
                        self.lingering.push(LingeringEffect::new(
                            id,
                            source.obj,
                            Payload::GrantedSubroutine {
                                to: obj,
                                sub: sub.clone(),
                                before: *before,
                                seq,
                                ord: ord as u32,
                                placement: None,
                            },
                            dur,
                        ));
                    }
                    // 9.8.2c: with the order declared, the Corp says where
                    // each granted subroutine goes relative to the ones the
                    // ice has at this moment.
                    if *any_order {
                        self.ask_subroutine_placement(obj);
                    }
                }
            }
            Instruction::CorpDiscards { count } => {
                let n = (*count as usize).min(self.st.hand[&Side::Corp].len());
                let cards: Vec<ObjectId> =
                    self.st.hand[&Side::Corp].iter().take(n).copied().collect();
                for c in cards {
                    self.move_card(c, Zone::Discard(Side::Corp));
                    self.changes.record(GameChange::CardDiscarded { obj: c, side: Side::Corp });
                }
            }
            Instruction::RestrictAccessToSelf => {
                // 7.4.2: prohibit access to everything except the source for
                // the remainder of the run.
                cite!("rule_prohibiting_access");
                let dur = crate::lingering::bind_duration(
                    crate::lingering::WantedDuration::ThisRun,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                let id = self.next_lingering;
                self.next_lingering += 1;
                self.lingering.push(LingeringEffect::new(id, source.obj, Payload::RestrictCandidatesTo(source.obj), dur));
            }
            Instruction::CreateDelayedConditional { def, duration } => {
                cite!("rule_delayed_conditional_ability");
                // CR 1.15.4: an ability created by this instruction can
                // refer to a target the SAME ability already announced —
                // Howler's delayed conditional acts on the card its install
                // instruction chose. The reference is bound HERE, when the
                // delayed ability is created, so the later ability "can find
                // and act on the card" without re-announcing it.
                let def = {
                    let announced = self.ability_targets();
                    let mut d = (**def).clone();
                    d.instructions =
                        d.instructions.into_iter().map(|i| bind_targets(i, &announced)).collect();
                    Box::new(d)
                };
                let def = &def;
                // 9.6.13d: "when this run ends" with no run in progress —
                // the lingering effect is not created.
                if matches!(
                    def.condition,
                    Some(crate::ability::Condition::Trigger(TriggerCond::RunEnds { .. }))
                ) && self.current_run.is_none()
                {
                    cite!("rule_delayed_run_ends_condition_outside_run");
                } else {
                    // 9.6.13b: an explicit duration allows repeated
                    // triggering; 9.6.13c: otherwise until first resolution.
                    cite!("rule_delayed_conditional_ability_specified_duration");
                    cite!("rule_delayed_conditional_ability_relevant_once");
                    let dur = crate::lingering::bind_duration(
                        *duration,
                        self.st.encounter.as_ref().map(|e| e.id),
                        self.current_run.map(|(r, _, _)| r),
                        self.st.turn_seq,
                    );
                    let id = self.next_lingering;
                    self.next_lingering += 1;
                    self.lingering.push(LingeringEffect::new(id, source.obj, Payload::DelayedConditional { def: (**def).clone() }, dur));
                }
            }
            Instruction::CreateLingeringEffect { payload, duration } => {
                // 9.10.1: the effect is created with its source and duration
                // and then exists independently of that source; 9.10.4 binds
                // the requested duration to the structure instance in
                // progress (none in progress → expires at the next
                // checkpoint).
                cite!("rule_instruction_lingering_effect");
                cite!("rule_lingering_effect");
                let dur = crate::lingering::bind_duration(
                    *duration,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                // 9.5.3a: "cannot use <card>'s abilities" is a per-target
                // prohibition, so it makes one lingering effect per target.
                if let crate::instr::LingeringSpec::CannotUseAbilitiesOf(spec) = payload {
                    cite!("rule_forced_mid_access_ability_optional");
                    for t in self.resolve_targets(spec, Some(source.obj), &imm.targets) {
                        let id = self.next_lingering;
                        self.next_lingering += 1;
                        self.lingering.push(LingeringEffect::new(
                            id,
                            source.obj,
                            Payload::CannotUseAbilitiesOf(t),
                            dur,
                        ));
                    }
                    return;
                }
                let payload = match payload {
                    crate::instr::LingeringSpec::CannotUseAbilitiesOf(_) => unreachable!(),
                    crate::instr::LingeringSpec::PreventAllDamage => Payload::DamagePreventionAll,
                    crate::instr::LingeringSpec::Replacement { applies_to, with, optional } => {
                        // 9.9.8c: a replacement effect can be created ahead
                        // of the effect it replaces.
                        cite!("rule_replacement_effect_from_lingering_effect");
                        Payload::ReplacementEffect {
                            applies_to: *applies_to,
                            replace_with: with.clone(),
                            optional: *optional,
                        }
                    }
                    crate::instr::LingeringSpec::AccessLimit { limit } => {
                        cite!("rule_prohibiting_access_to_1");
                        Payload::AccessLimitThisRun {
                            limit: self.eval_quantity(limit, Some(source.obj)).max(0) as u32,
                        }
                    }
                    crate::instr::LingeringSpec::AdditionalAccess { server, extra } => {
                        Payload::AdditionalAccess { server: *server, extra: *extra }
                    }
                };
                let id = self.next_lingering;
                self.next_lingering += 1;
                self.lingering.push(LingeringEffect::new(id, source.obj, payload, dur));
            }
            Instruction::ReduceRunnerMemoryThisTurn(n) => {
                cite!("rule_memory_limit");
                let id = self.next_lingering;
                self.next_lingering += 1;
                self.lingering.push(LingeringEffect::new(id, source.obj, Payload::MemoryLimitMod { delta: -(*n as i32) }, Duration::Turn(self.st.turn_seq)));
            }
            Instruction::MaintainChoice { key, of, duration } => {
                // 9.10.3: the choice is remembered by a lingering effect, so
                // later abilities of the same source can refer to it. Which
                // of 9.10.3's three durations applies is stated by the card
                // layer; 9.10.4 binds it to the structure in progress.
                cite!("rule_lingering_effect_maintaining_choice_default_duration");
                cite!("rule_lingering_effect_maintaining_choice_turn_begins_duration");
                cite!("rule_lingering_effect_maintaining_choice_duration_other_cases");
                let value = match of {
                    crate::instr::ChoiceSpec::Server(s) => {
                        Some(crate::lingering::ChoiceValue::Server(*s))
                    }
                    crate::instr::ChoiceSpec::Subtype(t) => {
                        Some(crate::lingering::ChoiceValue::Subtype(t))
                    }
                    crate::instr::ChoiceSpec::Object(spec) => self
                        .resolve_targets(spec, Some(source.obj), &imm.targets)
                        .first()
                        .copied()
                        .map(crate::lingering::ChoiceValue::Object),
                };
                let Some(choice) = value else { return };
                let dur = crate::lingering::bind_duration(
                    *duration,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                // A source maintains ONE choice per key: a new one replaces
                // the old (9.10.3b's "always look for the server chosen this
                // turn").
                self.lingering.retain(|l| {
                    !matches!(&l.payload, Payload::MaintainedChoice { key: k, .. }
                        if *k == *key && l.source == source.obj)
                });
                let id = self.next_lingering;
                self.next_lingering += 1;
                self.lingering.push(LingeringEffect::new(
                    id,
                    source.obj,
                    Payload::MaintainedChoice { key, choice },
                    dur,
                ));
            }
            Instruction::MustTrashAccessedCard { means } => {
                // 9.12.3a/b: a requirement, not an effect — it is recorded
                // against the access in progress and read when the mid-access
                // window (9.2.10) asks whether the Runner may pass.
                cite!("rule_must_with_choice");
                cite!("rule_must_without_choice");
                for f in self.frames.iter_mut().rev() {
                    if let Frame::Structure(StructureFrame {
                        ctx: StructCtx::Access(a), ..
                    }) = f
                    {
                        a.must_trash = Some(*means);
                        break;
                    }
                }
            }
            Instruction::ChooseOne { .. } => {
                // Handled at answer time (the choice ends the instruction;
                // the chosen effect is injected as the next instruction).
            }
            Instruction::PreventTrashOf(protected) => {
                // Sacrificial-Construct class: remove the object from the
                // imminent trash effect; the effect may become empty (9.9.7b
                // analogue for target sets).
                let protected = *protected;
                self.modify_parent_imminent(move |atom| {
                    if atom.class == EffectClass::TrashCards
                        && atom.targets.contains(&protected)
                    {
                        atom.targets.retain(|&t| t != protected);
                        atom.value = atom.targets.len() as i64;
                        if atom.targets.is_empty() {
                            atom.removed = true;
                        }
                        true
                    } else {
                        false
                    }
                });
            }
            Instruction::BypassEncounteredIce => {
                // 6.5.8a: the Encounter Ice Phase is ABORTED and the Runner
                // immediately proceeds to pass that ice — which is where the
                // run was already parked when it opened the phase, so nothing
                // redirects the run here. 6.5.8b/c: steps 6.9.3b and 6.9.3c
                // simply never occur, so subroutines are neither broken nor
                // resolved (and for zero-sub ice, 9.12.2d's vacuous "all
                // broken" is never noted, because it is noted when step
                // 6.9.3b BEGINS).
                cite!("rule_bypass");
                cite!("rule_bypass_start_of_encounter");
                cite!("rule_bypass_during_encounter");
                self.abort_encounter_phase();
            }
            Instruction::ForceEncounter { ice } => {
                // 6.5.9a: resolve an Encounter Ice Phase WITHOUT changing the
                // Runner's position, then return to this effect and proceed —
                // which is what pushing the phase's frame above this ability's
                // frame means (6.5.9c). The ice need not be installed: 9.1.8h
                // keeps its subroutines active for this encounter.
                cite!("rule_forced_encounter");
                cite!("rule_active_exception_encounter_not_installed");
                let targets = self.resolve_targets(ice, Some(source.obj), &imm.targets);
                if let Some(&t) = targets.first() {
                    self.open_encounter_phase(t, true);
                }
            }
            Instruction::ModifyStrength { target, amount, duration } => {
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                // §12 rule 6: the amount is a selector, evaluated where the
                // instruction resolves — 1.16.2c's announced X, a count of
                // installed icebreakers, or a printed constant.
                let delta = self.eval_quantity(amount, Some(source.obj)) as i32;
                let enc = self.st.encounter.as_ref().map(|e| e.id);
                let run = self.current_run.map(|(r, _, _)| r);
                let turn = self.st.turn_seq;
                for t in targets {
                    // 3.9.5b / 9.10.4a: an ability ON AN ICEBREAKER that
                    // modifies ITS OWN strength implicitly lasts for the
                    // remainder of the current encounter — and 3.9.5d makes
                    // that "until the next checkpoint" when there is no
                    // encounter, which is what binding an inapplicable
                    // structure already does (9.10.4).
                    let self_icebreaker = t == source.obj
                        && self.has_subtype(t, "icebreaker");
                    let (stated, implicit) = match duration {
                        Some(w) => (Some(*w), self_icebreaker),
                        None => (None, true),
                    };
                    let base = match stated {
                        Some(w) => crate::lingering::bind_duration(w, enc, run, turn),
                        None => {
                            cite!("rule_icebreaker_strength_increase_implicit");
                            cite!("rule_icebreaker_strength_increase_implicit_link");
                            cite!("rule_icebreaker_strength_increase_outside_of_encounter");
                            crate::lingering::bind_duration(
                                crate::lingering::WantedDuration::ThisEncounter,
                                enc,
                                run,
                                turn,
                            )
                        }
                    };
                    let id = self.next_lingering;
                    self.next_lingering += 1;
                    let mut l = LingeringEffect::new(
                        id,
                        source.obj,
                        Payload::StrengthMod { target: t, delta },
                        base,
                    );
                    // 3.9.5c / 3.4.4a: a STATED duration runs alongside the
                    // implicit encounter one; the effect ends when both have.
                    if stated.is_some() && (implicit || !self.st.objects[&t].zone.is_installed()) {
                        cite!("rule_icebreaker_strength_increase_specified");
                    }
                    if stated.is_some() {
                        cite!("rule_ice_strength_modification_duration");
                        l.also = enc.map(crate::lingering::Duration::Encounter);
                    }
                    self.lingering.push(l);
                }
            }
            Instruction::ModifySubtypes { target, add, remove, duration } => {
                // CR 9.11.4c: the "choose" sentence and the modifying
                // sentence are ONE instruction — the targets were announced
                // when it became imminent, the subtypes change now.
                cite!("rule_choose_instruction");
                cite!("rule_add_remove_subtypes");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                let dur = crate::lingering::bind_duration(
                    *duration,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                for t in targets {
                    let id = self.next_lingering;
                    self.next_lingering += 1;
                    self.lingering.push(LingeringEffect::new(
                        id,
                        source.obj,
                        Payload::SubtypeMod {
                            target: t,
                            add: add.clone(),
                            remove: remove.clone(),
                        },
                        dur,
                    ));
                }
            }
            Instruction::RezCard { target, ignore_costs } => {
                // CR 8.1.2b: an ability directs the Corp to rez a card. The
                // rez cost is paid first (8.1.2d) unless the ability states
                // that it is ignored (1.16.5c).
                cite!("rule_rez_by_ability");
                cite!("rule_inherent_rez_cost");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                for t in targets {
                    let Some(o) = self.st.objects.get(&t) else { continue };
                    // 8.1.1: only an installed, facedown, non-agenda Corp card
                    // is unrezzed (8.1.2c: agendas cannot be rezzed).
                    cite!("rule_rezzed_unrezzed");
                    cite!("rule_cannot_rez_agendas");
                    if o.faceup
                        || o.printed.side != Side::Corp
                        || o.printed.card_type == CardType::Agenda
                        || !self.is_installed(o)
                    {
                        continue;
                    }
                    if *ignore_costs {
                        cite!("rule_ignoring_costs");
                        self.rez_card_free(t);
                    } else {
                        self.rez_card(t);
                    }
                }
            }
            Instruction::ResolveAbilityOf { source: spec, which } => {
                // CR 9.6.14d: mark the named class of ability pending as
                // though its stipulation had occurred; the ordinary reaction
                // window then offers it. A subroutine is not a conditional
                // ability, so the subroutine class resolves where it is named
                // instead (9.8.10), as a nested ability frame — which is what
                // makes "this server" in it read from the ICE (4.6.6i).
                cite!("rule_instructed_to_resolve_conditional_ability");
                let targets = self.resolve_targets(spec, Some(source.obj), &imm.targets);
                for t in targets {
                    match which {
                        crate::ability::AbilityClass::Subroutine(n) => {
                            cite!("rule_subroutines_ordered");
                            let subs = self.current_subs(t);
                            let Some((key, def)) = subs.get(*n).cloned() else { continue };
                            let index =
                                if key.category == 3 { key.ord as usize } else { usize::MAX };
                            // 9.8.10: the subroutine resolves, wherever it was
                            // named — so it is recorded like any other.
                            self.changes
                                .record(GameChange::SubroutineResolved { ice: t, index: *n });
                            self.push_ability_frame(
                                ResolutionKind::Subroutine,
                                AbilityRef { obj: t, index },
                                Side::Corp,
                                def.instructions,
                                None,
                                Some(*n),
                            );
                        }
                        class => {
                            let ids = self.pend_abilities_by_class(t, *class);
                            self.pending_from_effect.extend(ids);
                        }
                    }
                }
            }
            Instruction::EndActionPhase(side) => {
                // 5.6.2: the action phase's steps are (a) a paid window, (b)
                // an action, (c) return to (a) — so ENDING the phase is a jump
                // to step (d), not merely running the player out of clicks.
                // The difference is observable: an ended phase does not open
                // another (a), so a terminal operation leaves no window in
                // which the Corp could still score (S).
                //
                // 5.2.2a keeps the action itself intact — this instruction
                // resolves inside it, and the jump takes effect when the turn
                // structure next advances, which is after the action
                // completes.
                cite!("step_corp_turn_action_phase_end");
                cite!("step_runner_turn_action_phase_end");
                cite!("rule_action_completion");
                let n = self.st.player(*side).clicks;
                self.st.player_mut(*side).clicks = 0;
                if n > 0 {
                    self.changes.record(GameChange::ClicksLost { side: *side, amount: n });
                }
                let (kind, step) = match side {
                    Side::Corp => {
                        (crate::timing::StructKind::CorpTurn, "step_corp_turn_action_phase_end")
                    }
                    Side::Runner => (
                        crate::timing::StructKind::RunnerTurn,
                        "step_runner_turn_action_phase_end",
                    ),
                };
                let idx = self.table(kind).index_of(step);
                for f in self.frames.iter_mut().rev() {
                    if let Frame::Structure(sf) = f {
                        if sf.kind == kind {
                            sf.cursor = idx;
                            sf.phase = StepPhase::Enter;
                            sf.pending_jump = None;
                            break;
                        }
                    }
                }
            }
            Instruction::LookAtCards { cards, by } => {
                // 1.21.2: the looking player sees the front faces. 9.11.4e:
                // this ENDS an instruction — the post-instruction checkpoint
                // is the one the rule calls for, and the next instruction
                // announces its targets with the cards already visible.
                cite!("rule_look");
                cite!("rule_look_reveal_instruction");
                cite!("rule_look_reveal_expose_access_distinct");
                let targets = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                // 1.12.3: remember WHICH objects are being looked at. A card
                // that then moves to an unknown location becomes a new object
                // and this ability cannot act on it any more.
                cite!("rule_object_move_location");
                let stamped: Vec<(ObjectId, u32)> =
                    targets.iter().map(|t| (*t, self.generation(*t))).collect();
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    af.looked_at = stamped;
                }
                for t in targets {
                    // 1.21.2 / 10.2.2b: looking lets THAT player see the front
                    // face without showing it to the other one.
                    self.st.seen.show(*by, t);
                    self.changes.record(GameChange::CardLookedAt { obj: t, by: *by });
                }
            }
            Instruction::ExposeCards { cards } => {
                // CR 1.21.4: exposing is revealing, restricted to installed
                // UNREZZED cards. 9.12.2 does not aggregate exposing, so each
                // card exposed is its own occurrence (9.6.4b, Blackguard).
                cite!("rule_expose");
                cite!("rule_look_reveal_expose_access_distinct");
                let targets = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                for t in targets {
                    let Some(o) = self.st.objects.get(&t) else { continue };
                    if o.faceup || !self.is_installed(o) {
                        continue;
                    }
                    // 1.21.4: exposing IS revealing, so both players see it.
                    self.st.seen.show_all(t);
                    self.changes.record(GameChange::CardExposed { obj: t });
                }
            }
            // 1.21.3: reveal — show the front faces, then return the cards to
            // their previous state. 1.21.3a keeps a facedown card facedown;
            // the whole effect is on what each player has SEEN (10.2.2b).
            Instruction::RevealCards { cards } => {
                cite!("rule_reveal");
                cite!("rule_reveal_not_turn_faceup");
                cite!("rule_look_reveal_expose_access_distinct");
                let targets = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                for t in targets {
                    if !self.st.objects.contains_key(&t) {
                        continue;
                    }
                    self.st.seen.show_all(t);
                    self.changes.record(GameChange::CardRevealed { obj: t });
                }
            }
            // 1.10.3a: hosted credits entering a credit pool are GAINED.
            Instruction::TakeHostedCredits { from, amount, to } => {
                cite!("rule_gain_credits");
                cite!("rule_hosted_counters_not_on_player");
                let want = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                let targets = self.resolve_targets(from, Some(source.obj), &imm.targets);
                for t in targets {
                    let have = self
                        .st
                        .objects
                        .get(&t)
                        .and_then(|o| o.counters.get(&CounterKind::Credit).copied())
                        .unwrap_or(0);
                    let n = want.min(have);
                    if n == 0 {
                        continue;
                    }
                    if let Some(o) = self.st.objects.get_mut(&t) {
                        let c = o.counters.entry(CounterKind::Credit).or_insert(0);
                        *c -= n;
                    }
                    self.changes.record(GameChange::CounterRemoved {
                        obj: Some(t),
                        kind: CounterKind::Credit,
                        amount: n,
                    });
                    self.st.player_mut(*to).credits += n;
                    self.changes.record(GameChange::CreditsGained { side: *to, amount: n });
                }
            }
            // 1.9.2: counters removed from a card return to the bank. Not a
            // cost — costs are SPENT (1.16.1) — so nothing is announced as
            // being paid and no payment window opens.
            Instruction::RemoveCounters { target, kind, amount, up_to } => {
                cite!("rule_bank");
                let want = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                for t in targets {
                    let have = self
                        .st
                        .objects
                        .get(&t)
                        .and_then(|o| o.counters.get(kind).copied())
                        .unwrap_or(0);
                    let n = if *up_to { want.min(have) } else { want.min(have) };
                    if n == 0 {
                        continue;
                    }
                    if let Some(o) = self.st.objects.get_mut(&t) {
                        let c = o.counters.entry(*kind).or_insert(0);
                        *c -= n;
                    }
                    self.changes.record(GameChange::CounterRemoved {
                        obj: Some(t),
                        kind: *kind,
                        amount: n,
                    });
                }
            }
            Instruction::Derez { target } => {
                // 8.1.3/8.1.3a-c: derezzing turns a rezzed card facedown; it
                // happens only through a card effect, has no inherent cost,
                // and is instantaneous — no component steps.
                cite!("sec_derez");
                cite!("rule_derez_by_ability");
                cite!("rule_derez_cost");
                cite!("rule_derez_procedure");
                // CR 8.1.2 / 1.12.5: the card is turned facedown. It stays
                // the same object — it never changed zones — so anything
                // keyed to the object (a once-per-turn use, a maintained
                // choice) survives.
                cite!("rule_object_turn_faceup_facedown");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                for t in targets {
                    let Some(o) = self.st.objects.get_mut(&t) else { continue };
                    if !o.faceup || o.printed.side != Side::Corp {
                        continue;
                    }
                    o.faceup = false;
                    self.changes.record(GameChange::CardDerezzed { obj: t });
                }
            }
            Instruction::BreakSubroutines { subs } => {
                cite!("rule_break_subroutine");
                cite!("rule_unbroken_subroutines_target_for_break_abilities");
                if let Some(ice) = self.st.encounter.as_ref().map(|e| e.ice) {
                    let all = self.current_subs(ice);
                    let announced = &imm.sub_targets;
                    let broken_now: Vec<SubKey> = {
                        let e = self.st.encounter.as_ref().unwrap();
                        let unbroken =
                            all.iter().filter(|(k, _)| !e.broken.contains(k)).map(|(k, _)| *k);
                        match subs {
                            // 9.8.6a: no targets — every unbroken subroutine.
                            crate::instr::SubroutineSpec::All => {
                                cite!("rule_break_all_subroutines_no_targets");
                                unbroken.collect()
                            }
                            // 1.15.3: an announced target that is no longer
                            // unbroken is simply not acted on.
                            crate::instr::SubroutineSpec::Chosen { .. } => {
                                cite!("rule_targets_gone");
                                unbroken.filter(|k| announced.contains(k)).collect()
                            }
                            // 9.8.6b: everything unbroken EXCEPT the target.
                            crate::instr::SubroutineSpec::AllBut { .. } => {
                                cite!("rule_break_all_but_x_subroutines_targets");
                                unbroken.filter(|k| !announced.contains(k)).collect()
                            }
                        }
                    };
                    if let Some(e) = self.st.encounter.as_mut() {
                        for k in broken_now {
                            e.broken.insert(k);
                        }
                    }
                    // 9.12.2d: breaking the last subroutine satisfies
                    // "all subroutines broken".
                    self.check_all_subs_broken();
                }
            }
            Instruction::HostCards { cards, host } => {
                cite!("rule_placed_loaded");
                let h = self
                    .resolve_targets(host, Some(source.obj), &imm.targets)
                    .first()
                    .copied();
                let Some(h) = h else { return };
                let guests = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                if matches!(cards, TargetSpec::FoundBySearch) {
                    self.take_found_cards();
                }
                for g in guests {
                    if g == h {
                        continue;
                    }
                    // 1.13.2a: this instruction hosts without installing.
                    self.create_host_relationship(g, h, false);
                }
            }
            Instruction::SwapCards { a, b } => {
                cite!("rule_swap");
                cite!("rule_swap_simultaneous");
                // 1.15.2: each target POSITION got its own announcement, in
                // order, so the announced list is read positionally here
                // rather than as one union.
                let a_chosen = matches!(a, TargetSpec::Choose { .. });
                let b_chosen = matches!(b, TargetSpec::Choose { .. });
                let x = if a_chosen {
                    imm.targets.first().copied()
                } else {
                    self.resolve_targets(a, Some(source.obj), &imm.targets).first().copied()
                };
                let y = if b_chosen {
                    imm.targets.get(usize::from(a_chosen)).copied()
                } else {
                    self.resolve_targets(b, Some(source.obj), &imm.targets).first().copied()
                };
                if let (Some(x), Some(y)) = (x, y) {
                    self.swap_cards(x, y);
                }
            }
            Instruction::MoveIce { ice, dest } => {
                cite!("rule_create_position");
                let targets = self.resolve_targets(ice, Some(source.obj), &imm.targets);
                for t in targets {
                    // 6.2.2: for installed ice being moved, the position is
                    // created when the movement happens and the ice occupies
                    // it immediately.
                    let placed = match dest {
                        crate::instr::InstallDest::Protecting(s) => {
                            cite!("rule_create_position_outermost");
                            let at = self.positions_at(*s).len();
                            Some((*s, self.create_position(*s, at)))
                        }
                        crate::instr::InstallDest::InwardFromSource => {
                            cite!("rule_create_position_innermost");
                            self.position_of_ice(source.obj).and_then(|(s, p)| {
                                self.positions_inward_of(s, p)
                                    .map(|i| (s, self.create_position(s, i)))
                            })
                        }
                        // Nothing else names a position protecting a server.
                        _ => None,
                    };
                    let Some((s, pos)) = placed else { continue };
                    self.pending_ice_position = Some((s, pos));
                    self.move_card(t, Zone::Ice(s));
                    self.pending_ice_position = None;
                }
            }
            Instruction::MoveRunnerToIce { ice, encounter } => {
                cite!("rule_move_runner_to_position");
                cite!("rule_move_to_piece_of_ice");
                let targets = self.resolve_targets(ice, Some(source.obj), &imm.targets);
                let Some(&t) = targets.first() else { return };
                let Some((server, pos)) = self.position_of_ice(t) else { return };
                // 6.2.8c: no position can be entered during the Success Phase,
                // the Run Ends Phase, or outside a run — the Runner does
                // nothing instead (6.2.5d).
                let movable = self
                    .run_ctx()
                    .map(|r| !r.reached_success && !r.jump_to_run_ends)
                    .unwrap_or(false);
                if !movable {
                    cite!("rule_ineffective_move");
                    cite!("rule_no_position_after_approach_server");
                    return;
                }
                // 6.2.8d: already approaching that very position — nothing.
                if self.run_ctx().map(|r| (r.server, r.position) == (server, Some(pos)))
                    == Some(true)
                    && !*encounter
                {
                    cite!("rule_move_to_current_ice");
                    return;
                }
                if let Some(r) = self.run_ctx_mut() {
                    r.server = server;
                    r.position = Some(pos);
                    r.came_from_ice = false;
                }
                if let Some((_, s, _)) = self.current_run.as_mut() {
                    *s = server;
                }
                self.jump_run_to(if *encounter {
                    "step_encounter_begins"
                } else {
                    "step_approach_begins"
                });
            }
            Instruction::RemoveTags(amount) => {
                // 10.5.5: removing a tag returns the tag counter to the bank;
                // "as much as possible" if fewer are there (1.10.3b's shape).
                cite!("rule_tag");
                let n = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                let take = self.st.runner.tags.min(n);
                self.st.runner.tags -= take;
                for _ in 0..take {
                    self.changes.record(GameChange::TagRemoved);
                }
            }
            Instruction::RemoveCountersFromPlayer { side, kind, amount } => {
                // 1.13.3: counters hosted on cards are not on a player, so
                // nothing here can reach them — the pools this touches are
                // the player's own.
                cite!("rule_hosted_counters_not_on_player");
                let n = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                self.remove_player_counters(*side, *kind, n);
            }
            Instruction::TakeBadPublicity { side, amount } => {
                // 10.6.1: bad publicity counters are placed on the PLAYER.
                // 10.6.3c: the bad publicity fund of a run already in progress
                // was filled at 6.9.1b and does not change here.
                cite!("rule_bad_publicity");
                cite!("rule_bad_publicity_during_run");
                let n = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                if n > 0 {
                    self.st.player_mut(*side).bad_publicity += n;
                    self.changes.record(GameChange::BadPublicityTaken { side: *side, amount: n });
                }
            }
            Instruction::PlaceCounters { target, kind, amount } => {
                // 1.18.2: placing counters directly is NOT advancing, whatever
                // kind they are — only `CounterPlaced` is recorded.
                cite!("rule_placing_advancement_counter");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                let n = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                if n == 0 {
                    return;
                }
                for t in targets {
                    let obj = self.st.objects.get_mut(&t).unwrap();
                    *obj.counters.entry(*kind).or_insert(0) += n;
                    self.changes.record(GameChange::CounterPlaced { obj: t, kind: *kind, amount: n });
                }
            }
            Instruction::ForEach { count, effects } => {
                // 9.12.2b: the whole rule in one place. Whether the effects
                // tied to the quantity aggregate is a property of the SET of
                // effects, not of any one of them — "if ANY part of those
                // effects is not listed in rule 9.12.2c, then the effects are
                // not aggregated".
                cite!("rule_calculated_quantity");
                cite!("rule_aggregated_instructions");
                let x = self.eval_quantity(count, Some(source.obj)).max(0);
                let all_aggregated = effects.iter().all(instruction_aggregates);
                let expanded: Vec<Instruction> = if all_aggregated {
                    // Performed once, with the values multiplied by the
                    // quantity. 9.12.2b: an aggregated value of 0 or less
                    // means that part of the effect does not take place —
                    // which the scaled selector says by itself.
                    effects.iter().map(|i| scale_instruction(i, x)).collect()
                } else {
                    // Performed once per unit, each its own occurrence.
                    (0..x).flat_map(|_| effects.iter().cloned()).collect()
                };
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    for (k, ins) in expanded.into_iter().enumerate() {
                        af.instructions.insert(af.idx + 1 + k, ins);
                    }
                }
            }
            Instruction::IdentifyMark => {
                // 10.11.3: if a server is already the mark, this does nothing
                // — the mark is immutable for the remainder of the turn.
                cite!("rule_mark_identification");
                cite!("rule_mark_already_identified");
                if self.mark().is_none() {
                    // 10.11.2a: an equal chance of each of the 3 centrals.
                    cite!("rule_mark_identification_method");
                    let centrals = [ServerId::Hq, ServerId::Rnd, ServerId::Archives];
                    let server = centrals[self.rng.random_range(0..centrals.len())];
                    // 10.11.4: the designation IS a lingering effect, and it
                    // expires at the end of the turn.
                    cite!("rule_mark_designation_lingering_effect");
                    cite!("rule_mark");
                    cite!("rule_only_one_mark");
                    let id = self.next_lingering;
                    self.next_lingering += 1;
                    let since = self.changes.log.len();
                    self.lingering.push(LingeringEffect::new(
                        id,
                        source.obj,
                        Payload::MarkDesignation { server, since },
                        Duration::Turn(self.st.turn_seq),
                    ));
                }
            }
            Instruction::LoadCounters { target, kind, amount } => {
                // 10.9.1: loading IS placing — the difference is that the
                // kind is remembered, so an "empty" ability on the same card
                // has something to be linked to (10.9.2). 10.9.4: loading
                // imposes no further restrictions on those counters.
                cite!("rule_load_and_empty");
                cite!("rule_loading_does_not_restrict_counters");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                let n = self.eval_quantity(amount, Some(source.obj)).max(0) as u32;
                for t in targets {
                    let obj = self.st.objects.get_mut(&t).unwrap();
                    obj.loaded_kinds.insert(*kind);
                    if n > 0 {
                        *obj.counters.entry(*kind).or_insert(0) += n;
                        self.changes
                            .record(GameChange::CounterPlaced { obj: t, kind: *kind, amount: n });
                    }
                }
            }
            Instruction::ChangeAttackedServer { server } => {
                // 6.1.2d: the attacked server changes DIRECTLY, without
                // reference to the Runner's position — so the run's current
                // timing step does not change and the Runner does not approach
                // or encounter the ice protecting the new server. (Their
                // position is left exactly as it was; 6.2.5 keeps it a
                // position of whatever server it belonged to, and the run
                // proceeds from the step it was already at.)
                cite!("rule_change_attacked_server_directly");
                cite!("rule_attacked_server");
                if let Some(r) = self.run_ctx_mut() {
                    r.server = *server;
                }
                if let Some((_, s, _)) = self.current_run.as_mut() {
                    *s = *server;
                }
            }
            Instruction::AdvanceCard { target } => {
                // 1.18.1: to advance a card is to place an advancement counter
                // from the bank on it — and it IS an advance, so 1.18.2's
                // distinction is carried by the extra change record.
                cite!("rule_advance");
                cite!("rule_advanced_card");
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                for t in targets {
                    let Some(obj) = self.st.objects.get_mut(&t) else { continue };
                    *obj.counters.entry(CounterKind::Advancement).or_insert(0) += 1;
                    self.changes.record(GameChange::CounterPlaced {
                        obj: t,
                        kind: CounterKind::Advancement,
                        amount: 1,
                    });
                    self.changes.record(GameChange::CardAdvanced { obj: t });
                }
            }
            Instruction::FlipIdentity(side) => {
                // CR rule_identity_double_sided: turning the identity over
                // changes which face's printed characteristics apply. The
                // 10.3.1a checkpoint after this instruction re-derives
                // abilities from the new face, so pendings/statics follow.
                cite!("rule_identity_double_sided");
                cite!("rule_double_sided_identity");
                let id = self
                    .st
                    .objects
                    .values()
                    .find(|o| {
                        o.printed.card_type == CardType::Identity && o.printed.side == *side
                    })
                    .map(|o| o.id);
                if let Some(id) = id {
                    if let Some(o) = self.st.objects.get_mut(&id) {
                        if o.printed.flip_face.is_some() {
                            o.flipped = !o.flipped;
                            self.changes.record(GameChange::IdentityFlipped { side: *side });
                        }
                    }
                }
            }
            Instruction::ShuffleCardsIntoDeck { targets, to } => {
                // Jackson class: the announced cards enter the deck (1.12.3
                // makes them new objects on entering a hidden zone) and the
                // deck is shuffled (8.1.4).
                cite!("rule_shuffle_deck_after_search");
                let to = *to;
                let ts = self.resolve_targets(targets, Some(source.obj), &imm.targets);
                for t in ts {
                    self.move_card(t, Zone::Deck(to));
                }
                self.shuffle_deck(to);
            }
            Instruction::RemoveCardsFromGame { targets } => {
                // §4.9: removed from the game.
                cite!("sec_removed_from_game");
                let ts = self.resolve_targets(targets, Some(source.obj), &imm.targets);
                for t in ts {
                    self.move_card(t, Zone::RemovedFromGame);
                }
            }
            Instruction::PurgeVirusCounters => {
                // CR 10.1.2: remove ALL virus counters hosted on cards and
                // return them to the bank. One occurrence, however many cards
                // it touches — "purge virus counters" names the board.
                cite!("rule_purge");
                let hosts: Vec<(ObjectId, u32)> = self
                    .st
                    .objects
                    .values()
                    .filter_map(|o| match o.counters.get(&CounterKind::Virus) {
                        Some(n) if *n > 0 => Some((o.id, *n)),
                        _ => None,
                    })
                    .collect();
                for (id, n) in hosts {
                    if let Some(o) = self.st.objects.get_mut(&id) {
                        o.counters.remove(&CounterKind::Virus);
                    }
                    self.changes.record(GameChange::CounterRemoved {
                        obj: Some(id),
                        kind: CounterKind::Virus,
                        amount: n,
                    });
                }
                self.changes.record(GameChange::VirusCountersPurged);
            }
            Instruction::StealSelfAgenda => {
                if !source_moved {
                    self.steal_agenda(source.obj);
                }
            }
            Instruction::ScoreSelfAgenda => {
                // 1.16.10c: the additional cost was paid in this frame's
                // PayCost phase and its checkpoint has already resolved, so
                // everything that reacted to the payment resolved BEFORE the
                // agenda is added to the score area.
                cite!("rule_additional_cost_checkpoint");
                if !source_moved {
                    self.score_agenda(source.obj);
                }
            }
            Instruction::InstallCard { .. } | Instruction::InstallCards { .. } => {
                // Expanded at imminence time (begin_imminence); unreachable
                // here, but harmless.
            }
            Instruction::InstallStepPlace => {
                cite!("rule_steps_installing_place");
                cite!("rule_steps_installing_destination");
                cite!("rule_steps_installing_trash_like_cards");
                let Some(p) = self.installs.last().cloned() else { return };
                if p.aborted {
                    return;
                }
                let c = p.card;
                // (b)-precondition: identify the destination. If it is
                // invalid or cannot be identified, no installation can take
                // place (8.5.14) — the card never even moves.
                let resolved: Option<(Zone, Option<usize>)> = match p.dest {
                    crate::instr::InstallDest::Root(s) => Some((Zone::Root(s), None)),
                    crate::instr::InstallDest::NewRemoteRoot => {
                        cite!("rule_corp_install_choose_destination_server");
                        // 4.6.8f: a limit on remote servers makes "a new
                        // remote server" an unavailable destination, so the
                        // destination cannot be identified (8.5.14).
                        if !self.can_create_new_remote() {
                            None
                        } else {
                            let s = ServerId::Remote(self.next_remote);
                            self.next_remote += 1;
                            Some((Zone::Root(s), None))
                        }
                    }
                    crate::instr::InstallDest::Protecting(s) => {
                        cite!("rule_ice_outermost_position");
                        Some((Zone::Ice(s), None))
                    }
                    crate::instr::InstallDest::NewRemoteProtecting => {
                        // 8.5.2a: the new remote server is created at step
                        // 8.5.16e, with this ice protecting it.
                        cite!("rule_corp_install_choose_destination_server");
                        if !self.can_create_new_remote() {
                            None
                        } else {
                            let s = ServerId::Remote(self.next_remote);
                            self.next_remote += 1;
                            Some((Zone::Ice(s), None))
                        }
                    }
                    crate::instr::InstallDest::DeclaredByInstaller => {
                        // 8.5.16b replaced this with the declared destination
                        // before the instruction became imminent; reaching it
                        // here means no destination could be identified.
                        cite!("rule_install_to_invalid_destination");
                        None
                    }
                    crate::instr::InstallDest::InwardFromSource => {
                        // 6.2.2c: the new position goes inward from the
                        // source's own position. The source not protecting a
                        // server has no position from which "directly
                        // inward" can be evaluated (8.5.14).
                        cite!("rule_create_position_directly_inward");
                        self.position_of_ice(source.obj).and_then(|(s, pos)| {
                            self.positions_inward_of(s, pos).map(|i| (Zone::Ice(s), Some(i)))
                        })
                    }
                    crate::instr::InstallDest::Rig
                    | crate::instr::InstallDest::RunnerChoiceHostOrRig => {
                        Some((Zone::Rig, None))
                    }
                    // 1.13.12: the hosted object moves to the host's zone.
                    crate::instr::InstallDest::HostedOn(h) => {
                        cite!("rule_hosted_object_same_zone_as_host");
                        self.st.objects.get(&h).map(|host| (host.zone, None))
                    }
                    crate::instr::InstallDest::BreachedServerRoot => {
                        self.breach_server().map(|s| (Zone::Root(s), None))
                    }
                };
                let Some((zone, ice_at)) = resolved else {
                    cite!("rule_install_to_invalid_destination");
                    if let Some(p) = self.installs.last_mut() {
                        p.aborted = true;
                    }
                    return;
                };
                // (a) place into the play area with its final faceup status;
                // not yet installed or active.
                let side = self.st.objects[&c].printed.side;
                self.move_card(c, Zone::PlayArea(side));
                {
                    let o = self.st.objects.get_mut(&c).unwrap();
                    o.staged = true;
                    o.faceup = side == Side::Runner;
                }
                // 6.2.2: for ice being installed, the position is created
                // HERE — when the destination is declared — and the ice
                // occupies it at step 8.5.16e.
                let ice_position = match zone {
                    Zone::Ice(s) => {
                        let at = ice_at.unwrap_or_else(|| self.positions_at(s).len());
                        if ice_at.is_none() {
                            cite!("rule_create_position_outermost");
                        }
                        Some(self.create_position(s, at))
                    }
                    _ => None,
                };
                if let Some(p) = self.installs.last_mut() {
                    p.resolved_zone = Some(zone);
                    p.ice_position = ice_position;
                }
                // (c) trash like cards — the MUST component of 8.5.6a.
                if let Zone::Root(s) = zone {
                    let must_trash: Vec<ObjectId> = self
                        .st
                        .root
                        .get(&s)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|&other| self.like_cards(c, other))
                        .collect();
                    for t in must_trash {
                        cite!("rule_must_trash_cases_in_root_of_server");
                        // 8.5.7: trashed with the same faceup/facedown
                        // status they had while installed (trash_card keeps
                        // the flag).
                        cite!("rule_install_corp_cards_trashed_facedown_archives");
                        self.trash_card(t, Side::Corp);
                    }
                }
            }
            Instruction::InstallStepPayCost => {
                cite!("rule_steps_installing_pay_install_cost");
                cite!("rule_install_cost_checkpoint");
                let Some(p) = self.installs.last().cloned() else { return };
                if p.aborted {
                    return;
                }
                // 1.16.5c: "ignoring all costs" reduces the cost to 0, but
                // the step still happens and is still followed by a
                // checkpoint (1.16.3a — the 9.6.5b THG example).
                cite!("rule_ignore_all_costs");
                cite!("rule_cost_checkpoint_cost_zero");
                // 1.16.2f: an install-and-rez effect reducing the TOTAL cost
                // is divided by the Corp HERE, "at the beginning of step
                // 8.5.16d, before calculating the value of the install cost".
                if p.reduce_total > 0 {
                    cite!("rule_install_and_rez_reducing_total");
                    self.ask(
                        Side::Corp,
                        DecisionSpec::DivideCostReduction { total: p.reduce_total },
                        DecisionCtx::CostDivision,
                    );
                    return;
                }
                // 9.9.6c: an interrupt may have decreased the cost value while
                // this instruction was imminent.
                let modified = imm
                    .atoms
                    .iter()
                    .find(|a| a.class == EffectClass::PayCost)
                    .map(|a| a.value.max(0) as u32);
                self.pay_install_cost(modified);
            }
            Instruction::InstallStepComplete => {
                cite!("rule_steps_installing_become_installed");
                cite!("rule_steps_installing_installed_condition");
                let Some(p) = self.installs.last().cloned() else { return };
                if p.aborted {
                    if !p.and_rez {
                        self.installs.pop();
                    }
                    return;
                }
                let c = p.card;
                let zone = p.resolved_zone.expect("destination resolved at step (b)");
                // (e) create the server if new, move the card into place; it
                // becomes installed; if faceup, it becomes active.
                // 6.2.2: the ice occupies the position created at (b).
                if let (Zone::Ice(s), Some(pos)) = (zone, p.ice_position) {
                    self.pending_ice_position = Some((s, pos));
                }
                self.move_card(c, zone);
                if let crate::instr::InstallDest::HostedOn(h) = p.dest {
                    cite!("rule_host_via_install");
                    // 1.13.7a: hosted while being installed, so it keeps the
                    // faceup status the installation gave it, and 1.13.2 has
                    // it both installed AND hosted.
                    cite!("rule_hosted_when_installed");
                    self.create_host_relationship(c, h, true);
                }
                {
                    self.st.active_seq += 1;
                    let seq = self.st.active_seq;
                    let o = self.st.objects.get_mut(&c).unwrap();
                    o.staged = false;
                    if o.faceup {
                        o.active_since = seq;
                    }
                }
                // 1.10.5b: recurring credits arrive at step 8.5.16e.
                self.place_recurring_credits(c);
                // (f) "when installed" conditions meet their trigger
                // conditions; the install effect is complete.
                let side = self.st.objects[&c].printed.side;
                self.changes.record(GameChange::CardInstalled { obj: c, side, from: p.from_zone });
                if !p.and_rez {
                    let done = self.installs.pop().unwrap();
                    self.install_terminal_reveal(&done);
                }
            }
            Instruction::InstallRezPayCost => {
                cite!("rule_install_and_rez");
                cite!("rule_inherent_rez_cost");
                let Some(p) = self.installs.last().cloned() else { return };
                if p.aborted {
                    return;
                }
                let c = p.card;
                let rezzable = matches!(
                    self.st.objects[&c].printed.card_type,
                    CardType::Asset | CardType::Ice | CardType::Upgrade
                );
                if !rezzable {
                    // 8.5.13d: e.g. an agenda — it cannot be rezzed, so the
                    // card must be revealed (Trust Operation example).
                    cite!("rule_cannot_rez_agendas");
                    cite!("rule_reveal_for_install_and_rez");
                    self.install_reveal(c);
                    if let Some(p) = self.installs.last_mut() {
                        p.rez_skipped = true;
                    }
                    return;
                }
                // 8.1.2d + 1.16.1b: a rez cost the Corp cannot pay makes the
                // card one they "are unable to rez", so the rez does not
                // happen and 8.5.13d forces the reveal — this is what lets
                // 8.7.2b permit finding a card that can be installed but not
                // rezzed (the Tucana example).
                // 1.16.2f: the share of the "total" modifier the Corp did NOT
                // put on the install cost comes off the rez cost here
                // (1.16.2a: lower, then floor at 0).
                let rez_reduction = p.reduce_total.saturating_sub(p.reduce_install);
                let base_rez = if p.ignore_costs {
                    Cost::free()
                } else {
                    cite!("rule_cost_calculation");
                    Cost::credits(
                        self.st.objects[&c]
                            .printed
                            .cost
                            .unwrap_or(0)
                            .saturating_sub(rez_reduction),
                    )
                };
                let full_rez = match &self.st.objects[&c].printed.additional_rez_cost {
                    Some(add) => base_rez.plus(add),
                    None => base_rez.clone(),
                };
                if !self.cost_payable(Side::Corp, c, &full_rez) {
                    cite!("rule_cost");
                    cite!("rule_reveal_for_install_and_rez");
                    self.install_reveal(c);
                    if let Some(p) = self.installs.last_mut() {
                        p.rez_skipped = true;
                    }
                    return;
                }
                // 1.16.4c: additional costs are NOT covered by "ignoring all
                // costs" of the inherent kind and may be declined; declining
                // means the card is not rezzed (8.5.13d, the Ob/Archer
                // example).
                let additional = self.st.objects[&c].printed.additional_rez_cost.clone();
                if let Some(add) = additional {
                    cite!("rule_inherent_and_additional_cost");
                    let base = base_rez.clone();
                    let total = base.plus(&add);
                    self.ask(
                        Side::Corp,
                        DecisionSpec::NestedCost { cost: total },
                        DecisionCtx::RezAdditionalCost,
                    );
                    return;
                }
                // The cost-paid checkpoint that follows is the checkpoint
                // that processes the CardInstalled change, while the card is
                // still facedown (the 9.6.5b THG example).
                cite!("rule_cost_checkpoint_cost_zero");
                self.pay_cost(Side::Corp, c, &base_rez);
            }
            Instruction::InstallRezFinish => {
                let Some(p) = self.installs.pop() else { return };
                if p.aborted {
                    return;
                }
                let c = p.card;
                if !p.rez_skipped {
                    cite!("rule_install_and_rez");
                    self.st.active_seq += 1;
                    let seq = self.st.active_seq;
                    let o = self.st.objects.get_mut(&c).unwrap();
                    o.faceup = true;
                    o.active_since = seq;
                    let ct = self.st.objects[&c].printed.card_type;
                    self.changes.record(GameChange::CardRezzed { obj: c, card_type: ct });
                    self.place_recurring_credits(c);
                }
                self.install_terminal_reveal(&p);
            }
            Instruction::PlayCard { .. } | Instruction::PlayCards { .. } => {
                // Expanded at imminence time; unreachable here.
            }
            Instruction::PlayStepPlace => {
                cite!("rule_steps_playing_place");
                let Some(p) = self.plays.last().cloned() else { return };
                let c = p.card;
                let side = self.st.objects[&c].printed.side;
                // (a) place faceup into the play area; not installed, not
                // yet active.
                self.move_card(c, Zone::PlayArea(side));
                let o = self.st.objects.get_mut(&c).unwrap();
                o.faceup = true;
                o.staged = true;
            }
            Instruction::PlayStepPayCost => {
                cite!("rule_steps_playing_play_cost");
                cite!("rule_playing_play_cost");
                cite!("rule_play_cost_checkpoint");
                let Some(p) = self.plays.last().cloned() else { return };
                let c = p.card;
                let side = self.st.objects[&c].printed.side;
                // 9.9.6c: the cost is a VALUE, and the interrupt window that
                // just closed may have modified it. 1.16.2a applies to the
                // final value at the time the cost is paid, so it is floored
                // at 0 here.
                cite!("rule_modifiable_value_cost");
                cite!("rule_cost_calculation");
                let amount = if p.ignore_costs {
                    0
                } else {
                    imm.atoms
                        .iter()
                        .find(|a| a.class == EffectClass::PayCost)
                        .map(|a| a.value.max(0) as u32)
                        .unwrap_or_else(|| self.st.objects[&c].printed.cost.unwrap_or(0))
                };
                // 1.16.2c: a play cost of X is announced before it is paid,
                // and the announced value IS the cost's credit amount.
                let mut cost = match self.st.objects[&c].printed.cost_x.clone() {
                    Some(restriction) => {
                        cite!("rule_cost_x");
                        Cost::x(restriction)
                    }
                    None => Cost::credits(amount),
                };
                // 1.16.10b: an additional cost to play the card combines with
                // the play cost into ONE payment.
                if let Some(extra) = self.st.objects[&c].printed.additional_play_cost.clone() {
                    cite!("rule_additional_cost");
                    cost = cost.plus(&extra);
                }
                self.pay_cost(side, c, &cost);
            }
            Instruction::PlayStepActivate => {
                cite!("rule_steps_playing_active");
                cite!("rule_steps_playing_played_condition");
                cite!("rule_steps_playing_played_checkpoint");
                let Some(p) = self.plays.last().cloned() else { return };
                let c = p.card;
                let side = self.st.objects[&c].printed.side;
                {
                    self.st.active_seq += 1;
                    let seq = self.st.active_seq;
                    let o = self.st.objects.get_mut(&c).unwrap();
                    o.staged = false;
                    o.active_since = seq;
                }
                // 1.10.5b: recurring credits arrive at step 8.6.7c.
                self.place_recurring_credits(c);
                // (d) conditions related to playing the card are met. The
                // post-instruction checkpoint IS the 8.6.7e checkpoint.
                self.changes.record(GameChange::CardPlayed { obj: c, side });
            }
            Instruction::PlayStepResolve => {
                cite!("rule_steps_playing_resolve_play_abilities");
                let Some(p) = self.plays.last().cloned() else { return };
                let c = p.card;
                // (f) resolve the play abilities — a nested frame; the play
                // steps resume after it completes (9.2.4d).
                let instrs: Vec<Instruction> = self.st.objects[&c]
                    .printed
                    .abilities
                    .iter()
                    .filter(|a| a.kind == AbilityKind::Play)
                    .flat_map(|a| a.instructions.iter().cloned())
                    .collect();
                if !instrs.is_empty() {
                    let side = self.st.objects[&c].printed.side;
                    self.push_ability_frame(
                        ResolutionKind::Play,
                        AbilityRef { obj: c, index: 0 },
                        side,
                        instrs,
                        None,
                        None,
                    );
                }
            }
            Instruction::PlayStepFinish => {
                cite!("rule_steps_playing_trash_played_card");
                cite!("rule_steps_playing_after_resolve_condition");
                cite!("rule_steps_playing_complete");
                let Some(p) = self.plays.pop() else { return };
                let c = p.card;
                let in_play_area = matches!(self.st.objects[&c].zone, Zone::PlayArea(_));
                if in_play_area {
                    // 8.6.6c: a "not trashed until <effect>" ability keeps
                    // the card in the play area via a lingering effect.
                    let shielded = self.st.objects[&c].face().abilities.iter().any(|a| {
                        a.statics.iter().any(|d| {
                            matches!(d, StaticDecl::PlayedNotTrashedUntilAgendaSteal)
                        })
                    });
                    if shielded {
                        cite!("rule_play_not_trashed_until");
                        let id = self.next_lingering;
                        self.next_lingering += 1;
                        self.lingering.push(LingeringEffect::new(id, c, Payload::PlayedTrashShield { card: c }, Duration::UntilResolved));
                    } else {
                        // (g) trash the card.
                        let owner = self.st.objects[&c].owner;
                        self.trash_card(c, owner);
                    }
                } else {
                    // 8.6.6a: no longer in the play area — not trashed
                    // (Ashen Epilogue).
                    cite!("rule_play_no_trash_left_play_area");
                }
                // (h) conditions related to finishing resolution are met.
                self.changes.record(GameChange::CardPlayResolved { obj: c });
            }
            Instruction::RemoveSelfFromGame => {
                cite!("rule_play_no_trash_left_play_area");
                if !source_moved {
                    self.move_card(source.obj, Zone::RemovedFromGame);
                }
            }
            Instruction::SetAsideTopOfDeck { deck_of, count } => {
                // 8.3.3 / 4.8.2: "that player sets aside the appropriate
                // number of cards facedown". 4.8.7 keeps them as one distinct
                // group; 8.3.3a is why only the arranging player may look.
                cite!("rule_arrange_secretly");
                cite!("rule_set_aside");
                cite!("rule_facedown_set_aside_distinct_groups");
                let n = self.eval_quantity(count, Some(source.obj)).max(0) as usize;
                let group = self.st.next_set_aside_group;
                self.st.next_set_aside_group += 1;
                let take: Vec<ObjectId> =
                    self.st.deck[deck_of].iter().copied().take(n).collect();
                for c in &take {
                    self.st.deck.get_mut(deck_of).unwrap().retain(|x| x != c);
                    let o = self.st.objects.get_mut(c).unwrap();
                    // 4.8.3: an ability that does not refer to the set-aside
                    // zone sees a move straight from the deck.
                    o.set_aside_from = Some(Zone::Deck(*deck_of));
                    o.zone = Zone::SetAside;
                    o.faceup = false;
                    o.set_aside_group = Some(crate::view::SetAsideGroup {
                        id: group,
                        by: controller,
                        drawn: false,
                    });
                }
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    af.set_aside_group = Some(group);
                }
            }
            Instruction::ArrangeSetAside { to_top_of } => {
                cite!("rule_arrange_rearrange");
                cite!("rule_arrange_secretly");
                let cards = self.ability_set_aside_group_cards();
                // 8.3.1a: "if a player is instructed to arrange 1 or fewer
                // cards, instead that player does nothing" — but the cards
                // still have to go back.
                if cards.len() < 2 {
                    cite!("rule_arrange_1_or_fewer");
                    self.finish_arrangement(*to_top_of, cards);
                } else {
                    self.ask(
                        controller,
                        DecisionSpec::ArrangeCards { cards },
                        DecisionCtx::Arrange { to_top_of: *to_top_of },
                    );
                }
            }
            Instruction::MoveCounters { kind, to, .. } => {
                // 1.18.2: moving an advancement counter is NOT advancing, so
                // nothing here records `CardAdvanced` and no "whenever you
                // advance" condition is met.
                cite!("rule_placing_advancement_counter");
                cite!("rule_object");
                let dest = self
                    .resolve_targets(to, Some(source.obj), &imm.targets)
                    .first()
                    .copied();
                let Some(dest) = dest else { return };
                let moved = imm.counter_targets.clone();
                let mut per_host: BTreeMap<ObjectId, u32> = BTreeMap::new();
                for c in &moved {
                    *per_host.entry(c.host).or_insert(0) += 1;
                }
                let mut total = 0;
                for (host, n) in per_host {
                    let have = self.st.objects.get(&host).map(|o| o.counter(*kind)).unwrap_or(0);
                    let take = n.min(have);
                    if take == 0 {
                        continue;
                    }
                    if let Some(o) = self.st.objects.get_mut(&host) {
                        let left = o.counter(*kind) - take;
                        if left == 0 {
                            o.counters.remove(kind);
                        } else {
                            o.counters.insert(*kind, left);
                        }
                    }
                    self.changes.record(GameChange::CounterRemoved {
                        obj: Some(host),
                        kind: *kind,
                        amount: take,
                    });
                    total += take;
                }
                if total > 0 {
                    if let Some(o) = self.st.objects.get_mut(&dest) {
                        *o.counters.entry(*kind).or_insert(0) += total;
                    }
                    self.changes.record(GameChange::CounterPlaced {
                        obj: dest,
                        kind: *kind,
                        amount: total,
                    });
                }
            }
            Instruction::CorpRearrangesRnd => {
                // 1.12.3 / 7.4.7a example 1: cards returned to R&D are NEW
                // OBJECTS — the breach's already-chosen bookkeeping forgets
                // them, and the topmost eligible card is recomputed.
                cite!("rule_object_move_location");
                cite!("rule_rnd_topmost_eligibile_candidate");
                let deck: Vec<ObjectId> = self.st.deck[&Side::Corp].clone();
                self.new_objects_for_unknown_location(&deck);
                if let Some(b) = self.breach_ctx_mut() {
                    b.chosen_ever.retain(|(c, _)| !deck.contains(c));
                    b.accessed.retain(|c| !deck.contains(c));
                }
                self.refresh_candidates_after_access();
            }
            Instruction::MoveToDeck { card, top } => {
                cite!("rule_rnd_topmost_eligibile_candidate");
                cite!("rule_deck_ordered");
                let targets = self.resolve_targets(card, Some(source.obj), &imm.targets);
                for t in targets {
                    // 4.2.1: a card goes to ITS OWNER's deck. 1.12.3: the move
                    // between zones makes the card a new object, which is what
                    // strands a self-referencing ability of the card that moved
                    // (9.1.4, the Compile/Mayfly example).
                    let owner = self.st.objects[&t].printed.side;
                    self.move_card(t, Zone::Deck(owner));
                    let deck = self.st.deck.get_mut(&owner).unwrap();
                    deck.retain(|&x| x != t);
                    if *top {
                        deck.insert(0, t);
                    } else {
                        deck.push(t);
                    }
                    // Cards entered/reordered: recompute the topmost
                    // eligible candidate (7.4.7a).
                    self.refresh_candidates_after_access();
                }
            }
            Instruction::Search { zone, criteria, count, may_fail } => {
                // 8.7.1: the searching player looks at the whole zone. The
                // find is NOT a target announcement (2.5.2a), so the choice
                // is put to them HERE, while the instruction resolves.
                cite!("rule_searching_does_not_target");
                let searcher = controller;
                let candidates = self.valid_find_targets(*zone, criteria);
                let n = self.eval_quantity(count, Some(source.obj)).max(0) as u32;
                if candidates.is_empty() || n == 0 {
                    // 8.7.3: the deck is reshuffled whether or not anything
                    // was found; 8.7.4 then resumes with no found cards.
                    self.complete_search(*zone, &[], searcher);
                    return;
                }
                // 8.7.2d: with a set number and no criteria the player must
                // find that many, or all there are; 8.7.2e lets a criteria
                // search of a deck fail to find.
                cite!("rule_search_multiple_cards");
                cite!("rule_fail_to_find");
                let want = n.min(candidates.len() as u32);
                self.ask(
                    searcher,
                    DecisionSpec::ChooseTargets {
                        candidates,
                        count: want,
                        up_to: *may_fail,
                        min: 0,
                    },
                    DecisionCtx::SearchFind { zone: *zone },
                );
            }
            Instruction::Sabotage { count } => {
                // 10.12.2: the Corp chooses which cards to trash from HQ and
                // the rest come off the top of R&D. The choice is made when
                // the instruction resolves — a sabotage names no targets in
                // advance, so this is not an announcement (1.15.1b).
                cite!("rule_sabotage");
                cite!("rule_sabotage_resolution");
                let n = self.eval_quantity(count, Some(source.obj)).max(0) as u32;
                let hq = self.st.hand[&Side::Corp].clone();
                let rnd = self.st.deck[&Side::Corp].len() as u32;
                // 10.12.3a: enough must come from HQ that the remainder fits
                // in R&D; 10.12.3b: if both together are short, everything
                // goes, so the floor and the ceiling meet at |HQ|.
                cite!("rule_sabotage_with_not_enough_cards");
                cite!("rule_sabotage_hq_first");
                cite!("rule_sabotage_all_remaining_cards");
                let max_from_hq = n.min(hq.len() as u32);
                let min_from_hq = n.saturating_sub(rnd).min(hq.len() as u32);
                self.ask(
                    Side::Corp,
                    DecisionSpec::ChooseTargets {
                        candidates: hq,
                        count: max_from_hq,
                        up_to: min_from_hq == 0,
                        min: min_from_hq,
                    },
                    DecisionCtx::Sabotage { count: n },
                );
            }
            Instruction::AddCardsToHand { cards } => {
                cite!("rule_continue_after_search");
                let targets = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                if matches!(cards, TargetSpec::FoundBySearch) {
                    self.take_found_cards();
                }
                // CR 4.1.2a: a card moving out of a hidden or secret zone must
                // be REVEALED when it has to be demonstrated that it meets the
                // ability's stipulations. The criteria are the kernel's only
                // representation of what the ability stipulated (deviation
                // 21's reading), so a characteristic criterion plus a card
                // that is not otherwise visible is exactly that case.
                let stipulated = match cards {
                    TargetSpec::Choose { criteria, .. } => {
                        criteria.iter().any(|f| f.stipulates_characteristic())
                    }
                    _ => false,
                };
                for t in targets {
                    if stipulated && !self.st.objects[&t].faceup {
                        cite!("rule_reveal_from_hidden");
                        cite!("rule_reveal");
                        self.st.seen.show_all(t);
                        self.changes.record(GameChange::CardRevealed { obj: t });
                    }
                    let owner = self.st.objects[&t].owner;
                    self.move_card(t, Zone::Hand(owner));
                }
            }
            Instruction::AddToScoreArea { cards, to, as_agenda } => {
                // CR 1.17.3e/f: an effect that DIRECTLY adds a card to a
                // score area — "that agenda is not considered scored or
                // stolen", and neither is a card converted "as an agenda". So
                // this is an ordinary move (8.2.1a) and records no
                // AgendaScored/AgendaStolen: nothing a scored/stolen trigger
                // condition can meet.
                cite!("rule_add_agenda_to_score_area");
                cite!("rule_added_agendas_not_scored_or_stolen");
                let targets = self.resolve_targets(cards, Some(source.obj), &imm.targets);
                for t in targets {
                    if let Some(points) = as_agenda {
                        // 10.1.3: the conversion, recorded on the object; the
                        // card "loses all its previous properties and gains
                        // only those properties specified" until it leaves.
                        cite!("rule_add_card_to_score_area");
                        self.st.objects.get_mut(&t).unwrap().converted_agenda = Some(*points);
                    }
                    self.move_card(t, Zone::ScoreArea(*to));
                    // 1.17.4: agendas are always added to a score area faceup.
                    cite!("rule_score_area_faceup");
                    self.st.objects.get_mut(&t).unwrap().faceup = true;
                }
            }
            Instruction::TrashRandomFromHand { side, .. } => {
                // The value rides the atom so interrupts can modify it
                // (9.9.6); 9.9.7d drops it entirely if it falls to 0.
                let n = imm
                    .atoms
                    .iter()
                    .find(|a| a.expected() && a.class == EffectClass::TrashCards)
                    .map(|a| a.value.max(0) as u32)
                    .unwrap_or(0);
                for _ in 0..n {
                    let hand = &self.st.hand[side];
                    if hand.is_empty() {
                        break;
                    }
                    let i = self.rng.random_range(0..hand.len());
                    let card = self.st.hand[side][i];
                    self.trash_card(card, *side);
                }
            }
            Instruction::IfRunnerLinkAtLeast { n, then } => {
                // 9.6.5d: requirements in the INSTRUCTIONS are checked when
                // the relevant instructions resolve — not at trigger time.
                cite!("rule_condition_requirements_part_of_effect");
                if self.runner_link() >= *n as i32 {
                    let atoms =
                        self.expected_atoms(then, controller, &imm.targets, Some(source.obj));
                    let inner = ImminentWrap {
                        counter_targets: Vec::new(),
                        instr: (**then).clone(),
                        atoms,
                        controller,
                        targets: imm.targets.clone(),
                        sub_targets: imm.sub_targets.clone(),
                        run_ordinal: imm.run_ordinal.clone(),
                        turn_ordinal: imm.turn_ordinal.clone(),
                        seq: imm.seq,
                    };
                    self.apply_imminent(inner, controller, source, source_moved);
                }
            }
            other => {
                debug_assert!(
                    false,
                    "instruction {other:?} should be executed as a structure step"
                );
            }
        }
    }

    /// Apply a modification to the top-most OTHER imminence (the instruction
    /// this interrupt is modifying).
    /// CR 9.9.7f: the value the innermost imminent instruction currently
    /// expects for damage of this kind (0 when there is no such effect —
    /// which 9.9.7b tombstoning is exactly the case of).
    fn imminent_damage_value(&self, kind: DamageKind) -> i64 {
        self.imminents
            .last()
            .map(|imm| {
                imm.atoms
                    .iter()
                    .filter(|a| a.expected() && a.class == EffectClass::Damage(kind))
                    .map(|a| a.value)
                    .sum()
            })
            .unwrap_or(0)
    }

    fn modify_parent_imminent(&mut self, mut f: impl FnMut(&mut EffectAtom) -> bool) {
        cite!("rule_negative_values_imminent");
        if let Some(imm) = self.imminents.last_mut() {
            for atom in imm.atoms.iter_mut() {
                if atom.expected() && f(atom) {
                    return;
                }
            }
            // 9.9.7b tombstones still accept nothing; fall through silently.
        }
    }

    fn resolve_targets(
        &self,
        spec: &TargetSpec,
        source: Option<ObjectId>,
        announced: &[ObjectId],
    ) -> Vec<ObjectId> {
        match spec {
            TargetSpec::Objects(v) => v.clone(),
            TargetSpec::SelfSource => source.into_iter().collect(),
            TargetSpec::HostOfSource => source
                .and_then(|s| self.st.objects.get(&s))
                .and_then(|o| o.host)
                .into_iter()
                .collect(),
            TargetSpec::AccessedCard => self.st.accessed.into_iter().collect(),
            // 9.10.3: "that ice" — the object the source is remembering.
            TargetSpec::MaintainedChoice(key) => source
                .and_then(|src| self.maintained_choice(src, key))
                .and_then(|c| match c {
                    crate::lingering::ChoiceValue::Object(o) => Some(o),
                    _ => None,
                })
                .into_iter()
                .collect(),
            TargetSpec::EncounteredIce => {
                self.st.encounter.as_ref().map(|e| e.ice).into_iter().collect()
            }
            // 1.15.2d/9.12.2a: one announcement, one set, one effect — and
            // for a several-announcement instruction (1.15.2), the union of
            // its announcements, which is what `announced` accumulated.
            TargetSpec::Choose { .. } | TargetSpec::Each(_) => announced.to_vec(),
            // CR 1.15.4: a target announced earlier by the SAME ability,
            // acted on again without being re-selected.
            TargetSpec::EarlierTarget { nth } => {
                cite!("rule_target_beyond_move");
                self.frames
                    .iter()
                    .rev()
                    .find_map(|f| match f {
                        Frame::Ability(af) => Some(af.ability_targets.get(*nth).copied()),
                        _ => None,
                    })
                    .flatten()
                    .into_iter()
                    .collect()
            }
            TargetSpec::TopOfDeck(side, n) => self.st.deck[side]
                .iter()
                .take(*n as usize)
                .copied()
                .collect(),
            // CR 8.7.4: the cards this ability's search found, still set
            // aside facedown.
            TargetSpec::FoundBySearch => {
                cite!("rule_continue_after_search");
                self.found_cards()
            }
        }
    }

    /// The cards found by the innermost resolving ability's search (4.8.4).
    fn found_cards(&self) -> Vec<ObjectId> {
        self.frames
            .iter()
            .rev()
            .find_map(|f| match f {
                Frame::Ability(af) => Some(af.found_cards.iter().map(|(c, _)| *c).collect()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Consume the found-card list of the innermost ability frame: the
    /// instruction referring to them has now acted on them.
    fn take_found_cards(&mut self) -> Vec<ObjectId> {
        for f in self.frames.iter_mut().rev() {
            if let Frame::Ability(af) = f {
                return af.found_cards.drain(..).map(|(c, _)| c).collect();
            }
        }
        Vec::new()
    }

    fn finish_ability_frame(&mut self) {
        cite!("step_paid_ability_complete");
        cite!("step_conditional_ability_complete");
        cite!("step_play_ability_complete");
        cite!("step_subroutine_complete");
        let Some(Frame::Ability(af)) = self.frames.pop() else { unreachable!() };
        // CR 9.5.5: anything still set aside when the ability finishes is
        // trashed/banked during step 10.3.1f/g of the next checkpoint.
        if !af.set_aside_counters.is_empty() {
            self.orphan_set_aside_counters.extend(af.set_aside_counters.iter().copied());
        }
        if !af.set_aside_cards.is_empty() {
            self.set_aside_card_cleanup.extend(af.set_aside_cards.iter().copied());
        }
        // CR 8.7.4: nothing in §8.7 disposes of a found card the ability
        // never acted on, and 4.8.4 sets it aside only "while the search
        // completes" — so it goes back to the zone it was found in.
        for (card, from) in af.found_cards.iter().copied() {
            cite!("rule_searched_cards_set_aside");
            self.move_card(card, from);
            if let Zone::Deck(side) = from {
                self.shuffle_deck(side);
            }
        }
        if !af.instructions.is_empty() {
            let label = self.st.objects.get(&af.source.obj)
                .map(|o| o.printed.name)
                .unwrap_or("?");
            self.resolution_log.push(format!("{label}#{}", af.source.index));
        }
        // 9.3.6g: completing a conditional ability's resolution is USING it
        // — which is what spends the once-per-turn flag (the optional path
        // in DeclineableChoice marks the same way; a declined optional is
        // NOT a use and pends again).
        if matches!(af.kind, ResolutionKind::Conditional) && !af.declined {
            let used_def = self
                .st
                .objects
                .get(&af.source.obj)
                .and_then(|o| o.face().abilities.get(af.source.index))
                .is_some_and(|d| d.has_flag(crate::ability::AbilityFlag::OncePerTurn));
            if used_def {
                cite!("rule_once_per_turn_flag");
                self.once_per_turn_used.insert((af.source, af.source_generation));
            }
        }
        // CR 9.6.7d: a static-condition conditional that resolved with no
        // expected effects at any interrupt-window open is throttled until a
        // timing structure step completes.
        if let Some(iid) = af.instance {
            if let Some(inst) = self.instances.remove(&iid) {
                if matches!(inst.def.condition, Some(Condition::Static(_)))
                    && !af.any_expected_effects
                {
                    cite!("rule_conditional_ability_static_condition_no_effect");
                    self.throttled.insert(af.source);
                }
                // 9.6.13c: a delayed conditional with no stated duration
                // exists until the next time it resolves.
                if let Some(lid) = inst.from_lingering {
                    cite!("rule_delayed_conditional_ability_relevant_once");
                    self.lingering
                        .retain(|l| !(l.id == lid && l.duration == Duration::UntilResolved));
                }
            }
        }
        // Whoever had priority in the window below continues (9.2.4c) — the
        // window frame is now on top again and re-offers options.
        if let Some(Frame::Window(w)) = self.frames.last_mut() {
            w.option_resolved();
        }
    }

    // ------------------------------------------------------------------
    // Windows
    // ------------------------------------------------------------------

    fn open_paid_window(&mut self, classes: PawClasses) {
        cite!("rule_paid_ability_window");
        let wid = self.next_window;
        self.next_window += 1;
        let w = WindowFrame::new(wid, WindowKind::Paid(classes), self.st.turn_side);
        self.frames.push(Frame::Window(w));
    }

    fn open_action_window(&mut self) {
        cite!("rule_action_window");
        let wid = self.next_window;
        self.next_window += 1;
        let w = WindowFrame::new(wid, WindowKind::Action, self.st.turn_side);
        self.frames.push(Frame::Window(w));
    }

    fn open_mid_access_window(&mut self) {
        cite!("rule_mid_access_window");
        let wid = self.next_window;
        self.next_window += 1;
        let w = WindowFrame::new(wid, WindowKind::MidAccess, self.st.turn_side);
        self.frames.push(Frame::Window(w));
    }

    /// CR 10.3.2: open a reaction window for freshly-pended instances.
    fn open_reaction_window(&mut self, pending: Vec<u64>, originating_structure: Option<u64>) {
        cite!("rule_after_checkpoint_reaction_window");
        cite!("rule_reaction_window_linked_to_pending_conditional_abilities");
        let wid = self.next_window;
        self.next_window += 1;
        let mut w = WindowFrame::new(wid, WindowKind::Reaction, self.st.turn_side);
        w.pending = pending.clone();
        w.originating_structure = originating_structure;
        for id in &pending {
            if let Some(inst) = self.instances.get_mut(id) {
                inst.window = Some(wid);
            }
        }
        self.frames.push(Frame::Window(w));
    }

    fn tick_window(&mut self) {
        // 9.2.8f: if the window's originating structure has ended, close it
        // immediately, dropping even mandatory pendings.
        let close_now = {
            let Some(Frame::Window(w)) = self.frames.last() else { unreachable!() };
            if let (WindowKind::Reaction, Some(enc)) = (&w.kind, w.originating_structure) {
                self.st.encounter.as_ref().map(|e| e.id) != Some(enc)
                    && self.encounter_structure_bound(enc)
            } else {
                false
            }
        };
        if close_now {
            cite!("rule_reaction_window_closing_timing_structure");
            let Some(Frame::Window(w)) = self.frames.pop() else { unreachable!() };
            self.drop_window_pendings(&w);
            self.after_window_closed();
            return;
        }

        // CR 9.2.4e/10.3.3: checkpoint before the holder may act.
        let needs_checkpoint = {
            let Some(Frame::Window(w)) = self.frames.last() else { unreachable!() };
            !w.checkpoint_done_for_priority
        };
        if needs_checkpoint {
            cite!("rule_checkpoint_before_receiving_priority");
            if let Some(Frame::Window(w)) = self.frames.last_mut() {
                w.checkpoint_done_for_priority = true;
            }
            self.checkpoint_and_react(None);
            return; // nested reaction window may now be on top
        }

        let (wid, kind, priority) = {
            let Some(Frame::Window(w)) = self.frames.last() else { unreachable!() };
            (w.id, w.kind.clone(), w.priority)
        };

        match kind {
            WindowKind::Action => {
                let options = self.action_options(priority);
                self.ask(priority, DecisionSpec::TakeAction { options }, DecisionCtx::Window(wid));
            }
            WindowKind::Paid(classes) => {
                let options = self.paid_window_options(priority, classes);
                if options.is_empty() {
                    self.window_pass();
                } else {
                    self.ask(
                        priority,
                        DecisionSpec::PaidWindow { classes, options },
                        DecisionCtx::Window(wid),
                    );
                }
            }
            WindowKind::Reaction => {
                let (options, can_pass) = self.reaction_options(wid, priority);
                if options.is_empty() && can_pass {
                    self.window_pass();
                } else {
                    self.ask(
                        priority,
                        DecisionSpec::ReactionWindow { options, can_pass },
                        DecisionCtx::Window(wid),
                    );
                }
            }
            WindowKind::Interrupt => {
                let (options, can_pass) = self.interrupt_options(wid, priority);
                if options.is_empty() && can_pass {
                    self.window_pass();
                } else {
                    self.ask(
                        priority,
                        DecisionSpec::InterruptWindow { options, can_pass },
                        DecisionCtx::Window(wid),
                    );
                }
            }
            WindowKind::MidAccess => {
                let options = self.mid_access_options();
                if options.is_empty() {
                    // 9.12.3a/b: a "must trash" with no permitted means
                    // available compels nothing — the window simply closes.
                    self.window_pass();
                } else {
                    let can_pass = self.mid_access_can_pass(&options);
                    self.ask(
                        Side::Runner,
                        DecisionSpec::MidAccessWindow { options, can_pass },
                        DecisionCtx::Window(wid),
                    );
                }
            }
        }
    }

    fn encounter_structure_bound(&self, _enc: u64) -> bool {
        true
    }

    fn window_pass(&mut self) {
        let closed = {
            let Some(Frame::Window(w)) = self.frames.last_mut() else { unreachable!() };
            w.pass()
        };
        if closed {
            let Some(Frame::Window(w)) = self.frames.pop() else { unreachable!() };
            let was_interrupt = w.kind == WindowKind::Interrupt;
            self.drop_window_pendings(&w);
            if was_interrupt {
                // CR 9.11.2: a step is one instruction, preceded by an
                // interrupt window — the instruction still has to RESOLVE
                // when the window closes. Only a window opened AFTER the
                // step (a checkpoint's reaction window) leaves nothing to do.
                cite!("rule_step_in_timing_structure_is_instruction");
                // CR 9.9.10: a replacement effect an interrupt CREATED is
                // applied to the instruction that was imminent, which is the
                // step's imminence now that the window has closed.
                cite!("rule_replace_imminent_effects");
                if self.resolve_replacements_or_ask() {
                    return;
                }
                if matches!(
                    self.frames.last(),
                    Some(Frame::Structure(StructureFrame { phase: StepPhase::Exec, .. }))
                ) {
                    return;
                }
            }
            self.after_window_closed();
        }
    }

    /// CR 9.6.11: instances lose pending when their window closes.
    fn drop_window_pendings(&mut self, w: &WindowFrame) {
        cite!("rule_conditional_ability_lose_pending_when_priority_window_closes");
        for id in &w.pending {
            self.instances.remove(id);
        }
    }

    /// Route control after a window closes: the frame below decides.
    fn after_window_closed(&mut self) {
        match self.frames.last_mut() {
            Some(Frame::Structure(sf)) => match sf.phase {
                StepPhase::Enter | StepPhase::Exec => sf.phase = StepPhase::Checkpoint,
                StepPhase::Checkpoint => {}
            },
            Some(Frame::Ability(af)) => {
                if af.phase == AbilityPhase::Imminent {
                    af.phase = AbilityPhase::Resolve;
                } else if af.phase == AbilityPhase::SubInterrupt {
                    af.phase = AbilityPhase::Targets;
                }
            }
            _ => {}
        }
    }

    fn action_options(&self, side: Side) -> Vec<ActionOption> {
        cite!("rule_action_window_options");
        cite!("rule_corp_basic_actions");
        cite!("rule_runner_basic_actions");
        let mut out = vec![ActionOption::BasicCredit, ActionOption::BasicDraw];
        if side == Side::Runner {
            cite!("runner_basic_action_run");
            for server in self.all_servers() {
                // 6.3.2a: "the Runner cannot initiate a run on this server"
                // refers to the ANNOUNCEMENT of the attacked server in the
                // Initiation Phase — so it removes the action, and nothing
                // more (an ability that changes the attacked server later is
                // untouched by it).
                if self.run_initiation_prohibited(server) {
                    cite!("rule_cannot_run_abilities");
                    continue;
                }
                out.push(ActionOption::BasicRun { server });
            }
            if self.st.runner.tags > 0 && self.st.runner.credits >= 2 {
                cite!("runner_basic_action_remove_tag");
                out.push(ActionOption::BasicRemoveTag);
            }
        }
        // 5.2.6d / 5.2.7d: "[click]: Install 1 agenda, asset, upgrade, or
        // piece of ice from HQ" / "1 program, resource, or piece of hardware
        // from the grip". The install cost is paid at step 8.5.16d, so it is
        // not a condition of the action being available (8.5.11 lets the
        // procedure fail there); what IS a condition is that a destination
        // exists at all (8.5.14 — an install with no identifiable
        // destination does not take place).
        cite!("rule_corp_basic_action_install");
        cite!("runner_basic_action_install");
        let installable = |t: CardType| match side {
            Side::Corp => matches!(
                t,
                CardType::Agenda | CardType::Asset | CardType::Upgrade | CardType::Ice
            ),
            Side::Runner => {
                matches!(t, CardType::Program | CardType::Resource | CardType::Hardware)
            }
        };
        for c in self.st.hand[&side].clone() {
            if !installable(self.st.objects[&c].printed.card_type) {
                continue;
            }
            if self.install_destinations_for(c, side).is_empty() {
                continue;
            }
            out.push(ActionOption::BasicInstall { card: c });
        }
        if side == Side::Corp {
            // 5.2.6f: "[click], 1[credit]: Advance 1 installed card." 1.18.3
            // says which installed cards those are; a card that cannot be
            // advanced is not an option, and with no credit there is no
            // action at all (1.16.1b).
            cite!("corp_basic_action_advance");
            if self.st.corp.credits >= 1 {
                for id in self.advanceable_cards() {
                    out.push(ActionOption::BasicAdvance { card: id });
                }
            }
            // 5.2.6g / 10.5.3: "Trash 1 resource. Take this action only if
            // the Runner is tagged." 10.5.2: tagged is one or more tags.
            cite!("corp_basic_action_trash_resource");
            cite!("rule_tagged_trash_resource");
            cite!("rule_tagged");
            if self.st.runner.tags > 0
                && self.st.corp.credits >= 2
                && self.st.objects.values().any(|o| {
                    o.zone == Zone::Rig
                        && o.printed.card_type == CardType::Resource
                        && !o.hosted_not_installed
                })
            {
                out.push(ActionOption::BasicTrashResource);
            }
            // 5.2.6h: "[click][click][click]: Purge virus counters." The
            // action costs three clicks, so it is only available with three
            // left to spend.
            cite!("corp_basic_action_purge_virus_counters");
            if self.st.corp.clicks >= 3 {
                out.push(ActionOption::BasicPurge);
            }
        }
        // 5.2.6e / 5.2.7e: "[click]: Play 1 operation from HQ." (the Runner's
        // is the same action for events from the grip). 1.16.4b: the play
        // cost must be payable, and 1.16.10a's additional cost is combined
        // with it at step 8.6.7b.
        cite!("rule_corp_basic_action_operation");
        // 5.2.7e: the Runner's counterpart is "[click]: Play 1 event from the
        // grip"; the card type is the only difference.
        cite!("runner_basic_action_event");
        let want_type = if side == Side::Corp { CardType::Operation } else { CardType::Event };
        for c in self.st.hand[&side].clone() {
            let o = &self.st.objects[&c];
            if o.printed.card_type != want_type {
                continue;
            }
            let mut cost = match o.printed.cost_x.clone() {
                Some(r) => Cost::x(r),
                None => Cost::credits(o.printed.cost.unwrap_or(0)),
            };
            if let Some(extra) = o.printed.additional_play_cost.clone() {
                cost = cost.plus(&extra);
            }
            // The action's own [click] is spent before the play cost, so it
            // is not part of what has to be affordable here.
            cost.clicks = cost.clicks.saturating_sub(0);
            // 9.1.8c: a "Play only if <state>" declaration on the card is
            // active while the card sits in hand, and an illegal play is not
            // an option at all.
            if !self.play_permitted(c) {
                continue;
            }
            if self.cost_payable(side, c, &cost) && self.st.player(side).clicks > cost.clicks {
                out.push(ActionOption::BasicPlayOperation { card: c });
            }
        }
        // CR 9.12.3a: a "must make a run with your first [click]" requirement
        // leaves the player no other action while it holds — the requirement
        // is stated over the DECISION, so it removes the options that would
        // not satisfy it rather than resolving anything.
        if self.must_run_with_first_click(side) {
            cite!("rule_must_with_choice");
            out.retain(|o| matches!(o, ActionOption::BasicRun { .. }));
            return out;
        }
        // Card actions ([click]-cost paid abilities, 5.2.1).
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.face().abilities.iter().enumerate() {
                if !a.is_action() {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                {
                    continue;
                }
                if self.cost_payable(side, o.id, a.cost.as_ref().unwrap()) {
                    out.push(ActionOption::CardAction {
                        ability: AbilityRef { obj: o.id, index: i },
                        label: a.label,
                    });
                }
            }
        }
        out
    }

    fn all_servers(&self) -> Vec<ServerId> {
        let mut v = vec![ServerId::Hq, ServerId::Rnd, ServerId::Archives];
        let mut remotes: BTreeSet<ServerId> = BTreeSet::new();
        for s in self.st.ice.keys().chain(self.st.root.keys()) {
            if let ServerId::Remote(_) = s {
                if !self.ice_at(*s).is_empty()
                    || !self.st.root.get(s).map(|r| r.is_empty()).unwrap_or(true)
                {
                    remotes.insert(*s);
                }
            }
        }
        v.extend(remotes);
        v
    }

    /// CR 9.5.6a: "A paid ability that contains an instruction that could
    /// break 1 or more subroutines can only be used during an encounter."
    fn break_ability_timing_ok(&self, a: &AbilityDef) -> bool {
        cite!("rule_paid_ability_breaks_subroutines");
        cite!("rule_paid_ability_effect_based_timing_restrictions");
        !crate::instr::could_break_subroutines(&a.instructions) || self.st.encounter.is_some()
    }

    fn paid_window_options(&self, side: Side, classes: PawClasses) -> Vec<WindowOption> {
        cite!("rule_paid_ability_window_options");
        let mut out = Vec::new();
        let threat = self.threat_level();
        // (P): regular paid abilities (not actions/interrupts/mid-access).
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Paid
                    || a.is_action()
                    || a.is_interrupt()
                    || a.has_flag(AbilityFlag::Access)
                {
                    continue;
                }
                cite!("rule_other_paid_abilities");
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                {
                    continue;
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                // 9.5.6a: an ability that could break a subroutine can only be
                // used during an encounter — a property of the INSTRUCTIONS,
                // so it holds for a card that names no ice at all (Botulus's
                // "break 1 subroutine on host ice").
                if !self.break_ability_timing_ok(a) {
                    continue;
                }
                // 9.5.6: effect-based timing restrictions.
                match a.timing {
                    Some(crate::ability::TimingRestriction::EncounterOnly {
                        required_subtype,
                    }) => {
                        cite!("rule_paid_ability_refers_to_encountered_ice");
                        let Some(e) = self.st.encounter.as_ref().map(|e| e.ice) else { continue };
                        if let Some(sub) = required_subtype {
                            if !self.has_subtype(e, sub) {
                                continue;
                            }
                        }
                    }
                    Some(crate::ability::TimingRestriction::ApproachOnly {
                        required_subtype,
                        rezzed,
                    }) => {
                        cite!("rule_paid_ability_refers_to_approached_ice");
                        // Only during the Approach Ice Phase (this window has
                        // the approach-ice rez class), with matching ice.
                        if !classes.rez_approached_ice {
                            continue;
                        }
                        let Some(r) = self.run_ctx() else { continue };
                        let Some(ice) = self.approached_ice(r) else { continue };
                        if rezzed && !self.st.objects[&ice].faceup {
                            continue;
                        }
                        if let Some(sub) = required_subtype {
                            let effects = self.char_effects();
                            let eff = crate::object::compute_effective(
                                &self.st.objects,
                                &effects,
                                ice,
                            );
                            if !eff.subtypes.contains(sub) {
                                continue;
                            }
                        }
                    }
                    None => {}
                }
                // 9.5.6c: encountered-ice references only during an encounter;
                // interface abilities also gated by strength (9.3.6c).
                if a.has_flag(AbilityFlag::Interface) {
                    cite!("rule_interface_ability");
                    let Some(e) = &self.st.encounter else { continue };
                    let bs = self.effective_strength(o.id).unwrap_or(0);
                    let is = self.effective_strength(e.ice).unwrap_or(0);
                    if bs < is {
                        continue;
                    }
                }
                if a.has_flag(AbilityFlag::OncePerTurn)
                    && self.once_per_turn_used.contains(&(AbilityRef { obj: o.id, index: i }, o.generation))
                {
                    cite!("rule_once_per_turn_flag");
                    continue;
                }
                if self.cost_payable(side, o.id, a.cost.as_ref().unwrap_or(&Cost::default())) {
                    out.push(WindowOption::TriggerPaid {
                        ability: AbilityRef { obj: o.id, index: i },
                        label: a.label,
                    });
                }
            }
        }
        if side == Side::Corp && classes.rez {
            cite!("rule_paid_ability_window_corp_rez");
            for o in self.st.objects.values() {
                if o.printed.side == Side::Corp
                    && !o.faceup
                    && matches!(o.zone, Zone::Root(_))
                    && matches!(o.printed.card_type, CardType::Asset | CardType::Upgrade)
                    && self.rez_affordable(o.id)
                {
                    out.push(WindowOption::Rez { card: o.id });
                }
            }
        }
        if side == Side::Corp && classes.rez_approached_ice {
            cite!("rule_paid_ability_window_corp_rez_ice");
            if let Some(r) = self.run_ctx() {
                if let Some(ice) = self.approached_ice(r) {
                    if !self.st.objects[&ice].faceup && self.rez_affordable(ice) {
                        out.push(WindowOption::RezApproachedIce { card: ice });
                    }
                }
            }
        }
        if side == Side::Corp && classes.score {
            cite!("rule_paid_ability_window_corp_score");
            for o in self.st.objects.values() {
                if o.printed.card_type == CardType::Agenda
                    && matches!(o.zone, Zone::Root(_))
                    && o.counter(CounterKind::Advancement) >= self.advancement_requirement(o.id)
                    && self.score_cost_payable(o.id)
                    && !self.score_prohibited(o.id)
                {
                    out.push(WindowOption::Score { card: o.id });
                }
            }
        }
        out
    }

    fn reaction_options(&self, wid: u64, side: Side) -> (Vec<WindowOption>, bool) {
        cite!("rule_reaction_window_options");
        let mut out = Vec::new();
        let mut has_mandatory = false;
        for (id, inst) in &self.instances {
            if inst.window == Some(wid) && inst.controller == side {
                if inst.mandatory {
                    has_mandatory = true;
                }
                out.push(WindowOption::TriggerInstance {
                    instance: *id,
                    label: inst.def.label,
                    mandatory: inst.mandatory,
                });
            }
        }
        // 9.2.8e: cannot pass while controlling pending mandatory abilities.
        cite!("rule_reaction_window_must_resolve_mandatory_abilities");
        (out, !has_mandatory)
    }

    fn interrupt_options(&self, wid: u64, side: Side) -> (Vec<WindowOption>, bool) {
        cite!("rule_interrupt_window_options");
        let atoms: Vec<EffectAtom> =
            self.imminents.last().map(|i| i.atoms.clone()).unwrap_or_default();
        let ordinals = self
            .imminents
            .last()
            .map(|i| i.run_ordinal.clone())
            .unwrap_or_default();
        let mut out = Vec::new();
        let mut has_mandatory_relevant = false;
        // Pending conditional interrupts: must still be relevant (9.9.4c).
        cite!("rule_trigger_conditional_ability_interrupt");
        for (id, inst) in &self.instances {
            if inst.window == Some(wid) && inst.controller == side {
                if self.interrupt_relevant(&inst.def, &atoms, &ordinals, inst.ability.obj) {
                    if inst.mandatory {
                        has_mandatory_relevant = true;
                    }
                    out.push(WindowOption::TriggerInstance {
                        instance: *id,
                        label: inst.def.label,
                        mandatory: inst.mandatory,
                    });
                }
            }
        }
        // Paid interrupts join freely (9.9.4d).
        out.extend(self.paid_interrupt_options(side, &atoms, &ordinals));
        cite!("rule_interrupt_window_must_resolve_mandatory_abilities");
        (out, !has_mandatory_relevant)
    }

    /// CR 7.1.5a: "all assets and upgrades have trash costs, as do some ice
    /// and operations. If a card does not have a trash cost, the Runner cannot
    /// pay its trash cost, and therefore cannot use the basic trash ability
    /// during that access." 7.1.5b: nor can they trash a card in Archives.
    /// 7.1.4: the Runner has ONE mid-access opportunity, after the reaction
    /// window at the beginning of the access and before stealing an agenda —
    /// which is where the 9.2.10 window sits in the §7.2 step table.
    fn mid_access_options(&self) -> Vec<WindowOption> {
        cite!("rule_mid_access_window_options");
        cite!("rule_paying_trash_costs");
        cite!("rule_trash_in_archives");
        cite!("rule_mid_access_ability_opportunity");
        let mut out = Vec::new();
        let Some(card) = self.st.accessed else { return out };
        let o = &self.st.objects[&card];
        // 7.1.5b: a card in the Corp's discard pile cannot be trashed, and its
        // trash cost cannot be paid — by the basic trash ability OR by any
        // other mid-access ability. A card accessed in Archives is already
        // there; so is one this access has just trashed.
        let in_archives = o.zone == Zone::Discard(Side::Corp);
        // 7.1.5: the basic trash ability — pay the trash cost, trash it.
        // 1.10.3c: what the Runner can pay it WITH includes hosted credits
        // their own cards let them spend (Scrubber class), not just the pool.
        if let (false, Some(tc)) = (in_archives, o.printed.trash_cost) {
            cite!("rule_basic_trash_ability");
            cite!("rule_spend_credits");
            let avail = self.st.runner.credits
                + self.st.bp_fund
                + self.spendable_hosted_credits(Side::Runner);
            if avail >= tc {
                out.push(WindowOption::BasicTrash { card, cost: tc });
            }
        }
        // Access-flagged paid abilities (9.3.6b).
        let threat = self.threat_level();
        for src in self.st.objects.values() {
            if src.controller != Side::Runner || self.ability_use_prohibited(src.id) {
                continue;
            }
            for (i, a) in src.face().abilities.iter().enumerate() {
                if a.kind == AbilityKind::Paid
                    && a.has_flag(AbilityFlag::Access)
                    && ability_active(src, a, None, self.st.accessed, threat)
                    && self.break_ability_timing_ok(a)
                    && !(in_archives
                        && crate::instr::could_trash_accessed_card(&a.instructions))
                    && self.cost_payable(
                        Side::Runner,
                        src.id,
                        a.cost.as_ref().unwrap_or(&Cost::default()),
                    )
                {
                    out.push(WindowOption::TriggerPaid {
                        ability: AbilityRef { obj: src.id, index: i },
                        label: a.label,
                    });
                }
            }
        }
        out
    }

    /// CR 9.5.3a (Wendigo class): a lingering effect can forbid the use of a
    /// card's abilities. The prohibition is on USE, so it removes the
    /// ability from every window it would be offered in — and, because the
    /// ability is still optional (9.5.3), a 9.12.3a "must" cannot reach past
    /// it either.
    pub fn ability_use_prohibited(&self, obj: ObjectId) -> bool {
        self.lingering
            .iter()
            .any(|l| matches!(l.payload, Payload::CannotUseAbilitiesOf(o) if o == obj))
    }

    /// CR 9.12.3a/b: may the Runner pass the mid-access window? Only if no
    /// "must trash this card" requirement is in force with a permitted means
    /// available to them among `options`.
    fn mid_access_can_pass(&self, options: &[WindowOption]) -> bool {
        let Some(means) = self.access_must_trash() else { return true };
        cite!("rule_must_with_choice");
        cite!("rule_must_without_choice");
        let card = self.st.accessed;
        !options.iter().any(|opt| match opt {
            // 7.1.5: paying the trash cost is a permitted means under BOTH
            // readings — it is the means 9.12.3b stipulates.
            WindowOption::BasicTrash { .. } => true,
            // 9.12.3a: with no means stipulated, any ability whose resolution
            // trashes the accessed card also satisfies the requirement;
            // 9.12.3b: it does not, and cannot be forced.
            WindowOption::TriggerPaid { ability, .. } => {
                means == crate::instr::TrashMeans::AnyAbility
                    && card.is_some()
                    && self.ability_trashes_accessed_card(*ability)
            }
            _ => false,
        })
    }

    /// The "must trash" requirement in force for the access in progress.
    fn access_must_trash(&self) -> Option<crate::instr::TrashMeans> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Structure(StructureFrame { ctx: StructCtx::Access(a), .. }) => Some(a.must_trash),
            _ => None,
        })?
    }

    /// Would using this ability trash the card being accessed? A shallow scan
    /// of its instructions for a trash naming the accessed card — the same
    /// shape 1.13.6b's scan uses (deviation 16).
    fn ability_trashes_accessed_card(&self, ability: AbilityRef) -> bool {
        let Some(o) = self.st.objects.get(&ability.obj) else { return false };
        let Some(a) = o.face().abilities.get(ability.index) else { return false };
        a.instructions
            .iter()
            .any(|i| matches!(i, Instruction::TrashCards(TargetSpec::AccessedCard)))
    }

    // ------------------------------------------------------------------
    // Costs (§1.16)
    // ------------------------------------------------------------------

    /// CR 1.16.2b: the credit component of a cost, calculated now.
    pub fn cost_credits(&self, cost: &Cost, source: ObjectId) -> u32 {
        self.eval_quantity(&cost.credits, Some(source)).max(0) as u32
    }

    /// CR 1.16.10 / 6.3.4: the additional cost to make a run, aggregated from
    /// every active declaration into ONE all-at-once payment (1.16.10b).
    pub fn run_action_cost(&self) -> Cost {
        cite!("rule_additional_cost");
        let mut total = Cost::free();
        for (_, d) in self.active_statics() {
            if let StaticDecl::AdditionalRunActionCost(c) = d {
                total = total.plus(&c);
            }
        }
        total
    }

    /// CR 9.12.3a/e: is this player required to spend their first [click] of
    /// the turn making a run? The requirement is discharged by making the run
    /// — or by being offered its additional cost and declining it (9.12.3e).
    pub fn must_run_with_first_click(&self, side: Side) -> bool {
        cite!("rule_must_with_choice");
        if self.st.run_requirement_discharged || side != self.st.turn_side {
            return false;
        }
        let p = self.st.player(side);
        if p.clicks < p.allotted_clicks {
            return false;
        }
        self.active_statics()
            .iter()
            .any(|(_, d)| matches!(d, StaticDecl::MustRunWithFirstClick(s) if *s == side))
    }

    /// CR 8.1.2d: can the Corp pay to rez this card? The rez cost is payable
    /// from every location 1.10.3c allows — not the credit pool alone — and a
    /// 1.16.2e alternate payment may cover part of it, which is exactly the
    /// case where reading the pool would refuse the (R) option outright.
    pub fn rez_affordable(&self, card: ObjectId) -> bool {
        cite!("rule_inherent_rez_cost");
        let printed = self.st.objects[&card].printed.cost.unwrap_or(0);
        if self.cost_payable(Side::Corp, card, &Cost::credits(printed)) {
            return true;
        }
        self.alternate_payments_for(card).into_iter().any(|(_, covers, instead)| {
            cite!("rule_alternate_payment");
            let reduced = Cost::credits(printed.saturating_sub(covers));
            self.cost_payable(Side::Corp, card, &reduced.plus(&instead))
        })
    }

    /// CR 1.16.1: a cost must be payable all at once.
    pub fn cost_payable(&self, side: Side, source: ObjectId, cost: &Cost) -> bool {
        self.cost_payable_under(side, source, cost, None)
    }

    /// CR 1.16.1c: the same question where the effect being paid for is
    /// subject to a restriction the payment must not break.
    pub fn cost_payable_under(
        &self,
        side: Side,
        source: ObjectId,
        cost: &Cost,
        restriction: Option<&PaymentRestriction>,
    ) -> bool {
        cite!("rule_cost");
        let p = self.st.player(side);
        // 1.10.3c: hosted credits their card lets them spend are part of what
        // this player can pay with (Cyberfeeder class, 9.1.6c) — and a
        // prohibition on spending (RSVP / Attini classes) removes all of it,
        // so any credit cost becomes unpayable (1.16.1b).
        let credits_avail = if self.credits_prohibited(side) {
            0
        } else {
            p.credits
                + self.spendable_hosted_credits(side)
                + if side == Side::Runner && self.current_run.is_some() {
                    self.st.bp_fund
                } else {
                    0
                }
        };
        // 1.16.2b: the calculation is performed when the cost is to be paid.
        let want_credits = self.cost_credits(cost, source);
        if credits_avail < want_credits || p.clicks < cost.clicks + cost.lose_clicks {
            return false;
        }
        if cost.trash_self && !self.st.objects[&source].zone.is_installed() {
            return false;
        }
        // 1.16.1b: a "trash N cards from your grip" component cannot be paid
        // with fewer than N cards there (the Patchwork branch of 8.7.2b).
        if (self.st.hand[&side].len() as u32) < cost.trash_from_hand {
            return false;
        }
        // 1.9.2: a counter component is spent from the source, so a card
        // without enough hosted counters cannot pay it.
        if let Some((kind, n)) = cost.spend_counters {
            cite!("rule_counters_default_from_bank");
            if self.st.objects[&source].counter(kind) < n {
                return false;
            }
        }
        // 8.2.5: a forfeit component needs that many agendas in the payer's
        // score area — 1.16.1b makes the whole cost unpayable otherwise.
        if cost.forfeit_agenda > 0
            && (self.st.score_area[&side].len() as u32) < cost.forfeit_agenda
        {
            cite!("rule_forfeit_rfg");
            return false;
        }
        // 1.16.1c: a "trash 1 of your installed cards" component needs that
        // many cards that can be spent WITHOUT breaking a restriction on the
        // effect being paid for — otherwise the cost cannot be paid at all.
        if let Some((n, _)) = &cost.trash_matching {
            cite!("rule_cost_restrictions");
            if (self.trash_matching_candidates(side, source, cost, restriction).len() as u32) < *n {
                return false;
            }
        }
        // CR 1.16.1b: if a static ability or a MANDATORY conditional
        // interrupt would prevent the steps of payment, the cost cannot be
        // paid (Jesminder vs Funhouse's take-a-tag nested cost).
        if cost.tags > 0 && self.tag_cost_blocked() {
            cite!("rule_cost_interrupt_static_mandatory");
            return false;
        }
        // The same rule for a damage component: a Guru-Davinder-class
        // mandatory interrupt that would prevent the damage makes "suffer 4
        // net damage" a cost that cannot be paid (the Obokata example).
        if cost.net_damage > 0 && self.damage_cost_blocked(DamageKind::Net) {
            cite!("rule_cost_interrupt_static_mandatory");
            return false;
        }
        true
    }

    /// CR 1.16.1b for a damage component: is there an ACTIVE, MANDATORY
    /// conditional interrupt that would prevent damage of this kind if the
    /// payment's steps were performed as an effect?
    fn damage_cost_blocked(&self, kind: DamageKind) -> bool {
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Conditional || a.optional || !a.is_interrupt() {
                    continue;
                }
                let Some(Condition::Trigger(TriggerCond::WouldDamage {
                    kind: k,
                    first_each_run,
                })) = &a.condition
                else {
                    continue;
                };
                if k.is_some_and(|k| k != kind) {
                    continue;
                }
                if *first_each_run && self.current_run.is_none() {
                    continue;
                }
                let prevents = a.instructions.iter().any(|x| {
                    matches!(x, Instruction::PreventAllDamage { kind: pk } if *pk == kind)
                        || matches!(x, Instruction::PreventDamage { kind: pk, .. } if *pk == kind)
                });
                if !prevents {
                    continue;
                }
                if !ability_active(
                    o,
                    a,
                    self.st.encounter.as_ref().map(|e| e.ice),
                    self.st.accessed,
                    threat,
                ) || !self.ability_present(o.id, i)
                {
                    continue;
                }
                return true;
            }
        }
        false
    }

    /// CR 1.10.3c-adjacent: credits a player can actually spend — pool plus
    /// hosted credits on cards that allow spending them, minus prohibitions
    /// (RSVP class → 0).
    pub fn spendable_credits(&self, side: Side) -> u32 {
        if self.credits_prohibited(side) {
            cite!("rule_bid_possible");
            return 0;
        }
        self.st.player(side).credits + self.spendable_hosted_credits(side)
    }

    /// CR 9.3.4: an active declaration forbidding this player from spending
    /// credits (RSVP class; the Attini class scopes the same declaration to
    /// its own resolution through 9.1.2b).
    pub fn credits_prohibited(&self, side: Side) -> bool {
        self.active_statics()
            .iter()
            .any(|(_, d)| matches!(d, StaticDecl::CannotSpendCredits(s) if *s == side))
    }

    /// CR 1.10.3c: credits hosted on this player's cards that the card's own
    /// ability allows them to spend. 1.13.3 keeps them out of the credit
    /// pool: they are never "on" the player.
    fn spendable_hosted_credits(&self, side: Side) -> u32 {
        cite!("rule_hosted_counters_not_on_player");
        self.st
            .objects
            .values()
            .filter(|o| o.controller == side && card_active(o) && o.printed.hosted_credits_spendable)
            .map(|o| o.counter(CounterKind::Credit))
            .sum()
    }

    /// 10.14.6b + 10.14.3: legal Psi bids — 0, 1, or 2, capped by what the
    /// player can actually spend; 0 is always legal.
    pub fn psi_legal_bids(&self, side: Side) -> Vec<u32> {
        cite!("rule_psi_bid_options");
        cite!("rule_bid_possible");
        let max = self.spendable_credits(side).min(2);
        (0..=max).collect()
    }

    /// CR 1.10.3c: spend credits exactly as the payer divided them among the
    /// allowed locations. `v` is one number per location, in the order
    /// `credit_locations` returned them; anything the answer leaves unpaid is
    /// completed greedily, the way every other Decision is clamped.
    fn spend_divided(&mut self, side: Side, total: u32, v: &[u32]) {
        cite!("rule_spend_credits");
        let locations = self.credit_locations(side);
        let mut left = total;
        for (i, (loc, have)) in locations.iter().enumerate() {
            if left == 0 {
                break;
            }
            let take = v.get(i).copied().unwrap_or(0).min(*have).min(left);
            self.spend_at(side, *loc, take);
            left -= take;
        }
        // Complete a short or illegal division from the front.
        for (loc, have) in locations {
            if left == 0 {
                break;
            }
            let already = self.spent_here(side, loc, have);
            let take = already.min(left);
            self.spend_at(side, loc, take);
            left -= take;
        }
    }

    /// How many credits are still in this location after the division's
    /// first pass.
    fn spent_here(&self, side: Side, loc: Option<ObjectId>, _had: u32) -> u32 {
        match loc {
            None => self.st.player(side).credits,
            Some(id) => self.st.objects.get(&id).map(|o| o.counter(CounterKind::Credit)).unwrap_or(0),
        }
    }

    /// Take `n` credits out of one allowed location (1.10.3c).
    fn spend_at(&mut self, side: Side, loc: Option<ObjectId>, n: u32) {
        if n == 0 {
            return;
        }
        match loc {
            None => {
                self.st.player_mut(side).credits -= n.min(self.st.player(side).credits);
            }
            Some(id) => {
                let have = self.st.objects[&id].counter(CounterKind::Credit);
                let take = have.min(n);
                // 1.13.11: hosted objects can be spent from their host
                // without affecting the host.
                cite!("rule_remove_spend_hosted_objects");
                self.st
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .counters
                    .insert(CounterKind::Credit, have - take);
                self.changes.record(GameChange::CounterRemoved {
                    obj: Some(id),
                    kind: CounterKind::Credit,
                    amount: take,
                });
                // 9.1.6c: the card whose ability allowed the counters to be
                // spent has been used.
                cite!("rule_hosted_counter_used_condition");
                self.changes.record(GameChange::AbilityUsed { source: id });
            }
        }
    }

    /// Spend credits from the pool first, then from spendable hosted pools.
    fn spend_flexible(&mut self, side: Side, mut n: u32) {
        let from_pool = n.min(self.st.player(side).credits);
        self.st.player_mut(side).credits -= from_pool;
        n -= from_pool;
        if n > 0 {
            let ids: Vec<ObjectId> = self
                .st
                .objects
                .values()
                .filter(|o| {
                    o.controller == side && card_active(o) && o.printed.hosted_credits_spendable
                })
                .map(|o| o.id)
                .collect();
            for id in ids {
                if n == 0 {
                    break;
                }
                let have = self.st.objects[&id].counter(CounterKind::Credit);
                let take = have.min(n);
                if take > 0 {
                    // 1.13.11: hosted objects can be spent from their host
                    // without affecting the host.
                    cite!("rule_remove_spend_hosted_objects");
                    self.st
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .counters
                        .insert(CounterKind::Credit, have - take);
                    self.changes.record(GameChange::CounterRemoved {
                        obj: Some(id),
                        kind: CounterKind::Credit,
                        amount: take,
                    });
                    // 9.1.6c: the card whose ability allowed the counters to
                    // be spent has been used, even though the ability being
                    // paid for lives on another card.
                    cite!("rule_hosted_counter_used_condition");
                    self.changes.record(GameChange::AbilityUsed { source: id });
                    n -= take;
                }
            }
        }
    }

    /// Would an active MANDATORY interrupt avoid a tag the Runner takes now?
    fn tag_cost_blocked(&self) -> bool {
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            for (i, a) in o.face().abilities.iter().enumerate() {
                if a.kind != AbilityKind::Conditional || a.optional || !a.is_interrupt() {
                    continue;
                }
                let Some(Condition::Trigger(TriggerCond::WouldTakeTags { during_run })) =
                    &a.condition
                else {
                    continue;
                };
                if *during_run && self.current_run.is_none() {
                    continue;
                }
                if !a.instructions.iter().any(|x| matches!(x, Instruction::AvoidTags(_))) {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                    || !self.ability_present(o.id, i)
                {
                    continue;
                }
                return true;
            }
        }
        false
    }

    /// CR 1.16: begin paying a cost. The payer's choices (1.16.2c's X,
    /// 1.16.2e's alternate payments, 1.10.3c's division, which cards and
    /// agendas are spent) are gathered first as Decisions; the whole cost is
    /// then paid at once (1.16.1) and `cont` is resumed. Callers with nothing
    /// left to do use [`Vm::pay_cost`].
    pub fn begin_payment(
        &mut self,
        side: Side,
        source: ObjectId,
        cost: &Cost,
        cont: PaymentCont,
        restriction: Option<PaymentRestriction>,
    ) {
        cite!("rule_cost");
        self.payment = Some(Payment {
            side,
            source,
            cost: cost.clone(),
            announced_x: None,
            alternates_offered: 0,
            alternate_covers: 0,
            alternate_cost: Cost::free(),
            division: None,
            from_hand: None,
            forfeited: None,
            trashed: None,
            restriction,
            cont,
        });
        self.advance_payment();
    }

    /// CR 1.16.2e: the alternate payments active for the cost being paid FOR
    /// this source — an ability of the source itself ("as you rez this ice").
    fn alternate_payments_for(&self, source: ObjectId) -> Vec<(&'static str, u32, Cost)> {
        cite!("rule_alternate_payment");
        self.active_statics()
            .into_iter()
            .filter_map(|(src, d)| match d {
                StaticDecl::AlternatePaymentForSelf { label, covers, instead } if src == source => {
                    Some((label, covers, instead))
                }
                _ => None,
            })
            .collect()
    }

    /// CR 1.10.3c: the locations this player may spend credits from, and how
    /// many are in each — the credit pool first, then each card whose ability
    /// allows its hosted credits to be spent.
    fn credit_locations(&self, side: Side) -> Vec<(Option<ObjectId>, u32)> {
        cite!("rule_spend_credits");
        let mut out = vec![(None, self.st.player(side).credits)];
        for o in self.st.objects.values() {
            if o.controller == side && card_active(o) && o.printed.hosted_credits_spendable {
                let n = o.counter(CounterKind::Credit);
                if n > 0 {
                    out.push((Some(o.id), n));
                }
            }
        }
        out
    }

    /// CR 1.16.1c: the installed cards this payer could spend for a
    /// `trash_matching` component — filtered so that no offered card, once
    /// trashed, would leave the restriction on the effect being paid for
    /// unmet.
    pub fn trash_matching_candidates(
        &self,
        side: Side,
        source: ObjectId,
        cost: &Cost,
        restriction: Option<&PaymentRestriction>,
    ) -> Vec<ObjectId> {
        let Some((_, criteria)) = &cost.trash_matching else { return Vec::new() };
        let mut out: Vec<ObjectId> = self
            .st
            .objects
            .values()
            .filter(|o| o.controller == side && o.zone.is_installed())
            .filter(|o| criteria.iter().all(|f| self.filter_matches(o, *f, Some(source))))
            .map(|o| o.id)
            .collect();
        out.sort();
        if let Some(r) = restriction {
            cite!("rule_cost_restrictions");
            out.retain(|c| !self.payment_breaks_restriction(r, &[*c]));
        }
        out
    }

    /// CR 1.16.1c: would spending these cards leave the restriction unmet?
    fn payment_breaks_restriction(&self, r: &PaymentRestriction, spent: &[ObjectId]) -> bool {
        match r {
            PaymentRestriction::ScoreRequirement(agenda) => {
                cite!("rule_advancement_requirement");
                let have = self
                    .st
                    .objects
                    .get(agenda)
                    .map(|o| o.counter(CounterKind::Advancement))
                    .unwrap_or(0);
                self.advancement_requirement_without(*agenda, spent) > have
            }
        }
    }

    /// CR 1.16.2c: the greatest value the payer may announce for X.
    ///
    /// "The chosen value must follow any applicable restrictions" — a stated
    /// restriction (Misdirection's "X must be no greater than the number of
    /// tags") is one; and where none is stated the bound is what 1.16.1a
    /// leaves: a cost must be paid all at once, so a value the payer could
    /// not pay for would make the ability unusable (1.16.1b) rather than
    /// legal. Both bounds apply when both exist.
    fn x_bound(&self, p: &Payment) -> u32 {
        cite!("rule_cost_x");
        cite!("rule_cost_restrictions");
        let mut max = self.spendable_credits(p.side);
        if let Some(q) = &p.cost.x_restriction {
            max = max.min(self.eval_quantity(q, Some(p.source)).max(0) as u32);
        }
        max
    }

    /// CR 1.16: ask for the next choice this payment needs, or commit it.
    fn advance_payment(&mut self) {
        let Some(p) = self.payment.clone() else { return };
        // 1.16.2c: X is announced BEFORE the cost is paid.
        if p.announced_x.is_none() {
            // 1.16.2c: the announcement is owed whenever the cost CONTAINS X
            // — a stated restriction is an extra bound on the value, not what
            // creates the choice (Corporate Troubleshooter states none).
            if p.cost.credits.mentions_announced_x() || p.cost.x_restriction.is_some() {
                cite!("rule_cost_x");
                let max = self.x_bound(&p);
                self.ask(p.side, DecisionSpec::DeclareX { max }, DecisionCtx::Payment);
                return;
            }
            if let Some(pm) = self.payment.as_mut() {
                pm.announced_x = Some(0);
            }
        }
        // 1.16.2e: each alternate payment is an option offered to the payer.
        let alternates = self.alternate_payments_for(p.source);
        if p.alternates_offered < alternates.len() {
            let (label, covers, instead) = alternates[p.alternates_offered].clone();
            self.ask(
                p.side,
                DecisionSpec::AlternatePayment { label, covers, instead },
                DecisionCtx::Payment,
            );
            return;
        }
        // "Trash 1 of your other installed cards" — the payer chooses which,
        // subject to 1.16.1c.
        if let Some((n, _)) = &p.cost.trash_matching {
            if p.trashed.is_none() {
                let cands =
                    self.trash_matching_candidates(p.side, p.source, &p.cost, p.restriction.as_ref());
                if cands.len() as u32 > *n {
                    cite!("rule_target");
                    self.ask(
                        p.side,
                        DecisionSpec::PaymentCards {
                            candidates: cands,
                            count: *n,
                            label: "trash",
                        },
                        DecisionCtx::Payment,
                    );
                    return;
                }
                if let Some(pm) = self.payment.as_mut() {
                    pm.trashed = Some(cands);
                }
            }
        }
        // 8.2.5: which agenda is forfeited is the payer's choice.
        if p.cost.forfeit_agenda > 0 && p.forfeited.is_none() {
            let area = self.st.score_area[&p.side].clone();
            if area.len() as u32 > p.cost.forfeit_agenda {
                cite!("rule_forfeit_rfg");
                cite!("rule_target");
                self.ask(
                    p.side,
                    DecisionSpec::PaymentCards {
                        candidates: area,
                        count: p.cost.forfeit_agenda,
                        label: "forfeit",
                    },
                    DecisionCtx::Payment,
                );
                return;
            }
            if let Some(pm) = self.payment.as_mut() {
                pm.forfeited = Some(area);
            }
        }
        // "Trash N cards from your grip" — likewise the payer's choice.
        if p.cost.trash_from_hand > 0 && p.from_hand.is_none() {
            let hand = self.st.hand[&p.side].clone();
            if hand.len() as u32 > p.cost.trash_from_hand {
                cite!("rule_target");
                self.ask(
                    p.side,
                    DecisionSpec::PaymentCards {
                        candidates: hand,
                        count: p.cost.trash_from_hand,
                        label: "trash from hand",
                    },
                    DecisionCtx::Payment,
                );
                return;
            }
            if let Some(pm) = self.payment.as_mut() {
                pm.from_hand = Some(hand);
            }
        }
        // 1.10.3c: the division of the credits among the allowed locations.
        if p.division.is_none() {
            let total = self.payment_credits_from_locations(&p);
            let locations = self.credit_locations(p.side);
            let available: u32 = locations.iter().map(|(_, n)| *n).sum();
            // The choice is real only when there is more than one location and
            // the payer is not spending everything they have.
            if locations.len() > 1 && total > 0 && total < available {
                self.ask(
                    p.side,
                    DecisionSpec::DivideCreditPayment { total, locations },
                    DecisionCtx::Payment,
                );
                return;
            }
        }
        self.commit_payment();
    }

    /// The credits this payment still takes from the payer's own locations:
    /// the cost's calculated credit amount (1.16.2b), less what a 1.16.2e
    /// alternate payment covers, less what the bad-publicity fund pays.
    fn payment_credits_from_locations(&self, p: &Payment) -> u32 {
        let want = self.cost_credits(&p.cost, p.source).saturating_sub(p.alternate_covers);
        if p.side == Side::Runner && self.current_run.is_some() {
            want.saturating_sub(self.st.bp_fund)
        } else {
            want
        }
    }

    /// CR 1.16: the answer to one of the payment's choices.
    pub(crate) fn answer_payment(&mut self, a: DecisionAnswer) {
        let Some(p) = self.payment.clone() else { return };
        match a {
            DecisionAnswer::DeclaredX(n) => {
                // 1.16.2c: "the chosen value must follow any applicable
                // restrictions" — the announcement is clamped to them.
                cite!("rule_cost_x");
                let max = self.x_bound(&p);
                if let Some(pm) = self.payment.as_mut() {
                    pm.announced_x = Some(n.min(max));
                }
            }
            DecisionAnswer::ResolveOptional(use_it) => {
                let alternates = self.alternate_payments_for(p.source);
                if let Some((_, covers, instead)) = alternates.get(p.alternates_offered).cloned() {
                    if let Some(pm) = self.payment.as_mut() {
                        pm.alternates_offered += 1;
                        if use_it {
                            // 1.16.2e: the value of the cost does not change;
                            // part of it is simply paid another way, and what
                            // is paid instead joins the same all-at-once
                            // payment (1.16.1).
                            cite!("rule_alternate_payment");
                            pm.alternate_covers += covers;
                            pm.alternate_cost = pm.alternate_cost.plus(&instead);
                            pm.cost = pm.cost.plus(&instead);
                        }
                    }
                }
            }
            DecisionAnswer::Targets(chosen) => {
                if let Some(pm) = self.payment.as_mut() {
                    if pm.cost.trash_matching.is_some() && pm.trashed.is_none() {
                        pm.trashed = Some(chosen);
                    } else if pm.cost.forfeit_agenda > 0 && pm.forfeited.is_none() {
                        pm.forfeited = Some(chosen);
                    } else {
                        pm.from_hand = Some(chosen);
                    }
                }
            }
            DecisionAnswer::Division(v) => {
                if let Some(pm) = self.payment.as_mut() {
                    pm.division = Some(v);
                }
            }
            _ => {}
        }
        self.advance_payment();
    }

    /// Pay a cost with nothing to do afterwards; CR 1.16.3/10.3.4: a
    /// checkpoint occurs immediately after — zero costs included (1.16.1d).
    pub fn pay_cost(&mut self, side: Side, source: ObjectId, cost: &Cost) {
        self.begin_payment(side, source, cost, PaymentCont::None, None);
    }

    /// CR 1.16.4d: count a [click] against the action in progress. "When a
    /// player takes an action that plays or installs one or more cards and has
    /// no other effects, the play cost or install cost of each of those cards
    /// is considered to have been spent to take that action" — so the clicks
    /// of an additional play cost are clicks spent to take the action, even
    /// though the payment happens several steps later.
    fn note_click_on_action(&mut self, n: u32) {
        if let Some((_, spent)) = self.st.current_action.as_mut() {
            *spent += n;
        }
    }

    /// CR 1.16.1: spend everything the payment decided on, all at once.
    fn commit_payment(&mut self) {
        let Some(p) = self.payment.clone() else { return };
        // 1.16.2b/c: the calculation — including any announced X — is
        // performed while the payment is still in progress.
        let want_credits = self.cost_credits(&p.cost, p.source);
        self.payment = None;
        // 1.16.2c: the announced X belongs to this USE of the ability, so it
        // outlives the payment record that collected it.
        if matches!(p.cont, PaymentCont::TriggerCost) {
            cite!("rule_cost_x");
            if let Some(Frame::Ability(af)) =
                self.frames.iter_mut().rev().find(|f| matches!(f, Frame::Ability(_)))
            {
                af.announced_x = p.announced_x;
            }
        }
        self.pay_cost_committed(&p, want_credits);
        self.resume_payment(p.cont);
    }

    /// Continue whatever the payment was for.
    fn resume_payment(&mut self, cont: PaymentCont) {
        match cont {
            PaymentCont::None | PaymentCont::TriggerCost => {}
            PaymentCont::Rez(id) => self.rez_card_finish(id),
            PaymentCont::Access(card) => self.push_access(card),
            PaymentCont::BasicTrash { card, window } => {
                self.trash_card(card, Side::Runner);
                if let Some(Frame::Window(w)) = self.frames.last_mut() {
                    if w.id == window {
                        w.option_resolved();
                    }
                }
                self.changes
                    .record(GameChange::TrashAbilityUsed { source: card, side: Side::Runner });
            }
        }
    }

    fn pay_cost_committed(&mut self, p: &Payment, want_credits: u32) {
        let (side, source, cost) = (p.side, p.source, &p.cost);
        cite!("rule_cost_zero");
        cite!("rule_checkpoint_after_paying_cost");
        // 1.16.2b: "the result of that calculation is determined at the time
        // the cost is to be paid. The result is taken as an aggregate, so that
        // paying the cost is a single instance of whatever was paid."
        cite!("rule_cost_quantities");
        // 1.16.2e: an alternate payment covers part of the cost's value
        // without changing that value — what it covers is simply not spent
        // from the payer's credit locations.
        let mut credits_to_pay = want_credits.saturating_sub(p.alternate_covers);
        // Bad publicity fund credits spend first during runs (6.4.3-ish).
        if side == Side::Runner && self.current_run.is_some() && self.st.bp_fund > 0 {
            cite!("rule_bad_publicity_fund");
            let from_fund = credits_to_pay.min(self.st.bp_fund);
            self.st.bp_fund -= from_fund;
            credits_to_pay -= from_fund;
        }
        // 1.10.3c: "spend" and "pay" are the same thing — the credits go
        // back to the bank, from the locations the payer divided them among.
        // 9.1.6c then makes each card whose hosted credits were spent used
        // alongside the card whose ability is being paid for.
        cite!("rule_spend_credits");
        match &p.division {
            Some(v) => self.spend_divided(side, credits_to_pay, v),
            None => self.spend_flexible(side, credits_to_pay),
        }
        // 5.2.1a: a "Lose [click]" component spends clicks exactly like a
        // [click] cost — the difference is only that it is not an action.
        cite!("rule_costs_with_click");
        self.st.player_mut(side).clicks -= cost.clicks + cost.lose_clicks;
        // 1.9.2: counters spent as a cost come off the source and go back to
        // the bank.
        if let Some((kind, n)) = cost.spend_counters {
            cite!("rule_counters_default_from_bank");
            let o = self.st.objects.get_mut(&source).unwrap();
            let have = *o.counters.get(&kind).unwrap_or(&0);
            let spent = n.min(have);
            o.counters.insert(kind, have - spent);
            self.changes
                .record(GameChange::CounterRemoved { obj: Some(source), kind, amount: spent });
        }
        let mut trashed = Vec::new();
        if cost.trash_self {
            // 1.19.4: [trash] on a card means "trash this object", used as a
            // trigger cost.
            cite!("rule_trash_symbol");
            self.trash_card(source, side);
            trashed.push(source);
            self.changes.record(GameChange::TrashAbilityUsed { source, side });
        }
        if cost.remove_self_from_game {
            // "Remove <this card> from the game:" as a trigger cost (Jackson
            // class) — §4.9, paid by moving the source out of the game.
            cite!("sec_removed_from_game");
            self.move_card(source, Zone::RemovedFromGame);
        }
        // 1.16.10: "trash 1 of your other installed cards" — the cards the
        // payer chose while the payment gathered its choices.
        for c in p.trashed.clone().unwrap_or_default() {
            self.trash_card(c, side);
            trashed.push(c);
        }
        // "Trash N cards from your grip" as a cost (Patchwork class).
        for c in p
            .from_hand
            .clone()
            .unwrap_or_else(|| self.st.hand[&side].iter().take(cost.trash_from_hand as usize).copied().collect())
        {
            self.trash_card(c, side);
            trashed.push(c);
        }
        // "…trash all cards from your grip:" (Citadel Sanctuary class) — the
        // whole grip, whatever it holds; no choice to gather.
        if cost.trash_all_from_hand {
            cite!("rule_calculated_quantity");
            for c in self.st.hand[&side].clone() {
                self.trash_card(c, side);
                trashed.push(c);
            }
        }
        // 8.2.5 / 4.9.3: "forfeit an agenda" moves it from the score area to
        // the removed-from-game zone; its agenda points stop counting because
        // `Vm::score` sums the score area.
        for a in p.forfeited.clone().unwrap_or_else(|| {
            self.st.score_area[&side].iter().take(cost.forfeit_agenda as usize).copied().collect()
        }) {
            cite!("movement_forfeit");
            cite!("rule_forfeit_rfg");
            self.move_card(a, Zone::RemovedFromGame);
            self.changes.record(GameChange::AgendaForfeited { obj: a, by: side });
        }
        // CR 1.16.1a: paying a cost cannot be modified or interrupted — tag
        // and damage components apply directly, with their changes recorded
        // so conditions can meet AFTER payment (1.16.10b).
        cite!("rule_cost_no_interrupt");
        if cost.tags > 0 {
            self.st.runner.tags += cost.tags;
            self.changes.record(GameChange::TagsTaken { amount: cost.tags });
        }
        if cost.net_damage > 0 {
            self.do_damage(DamageKind::Net, cost.net_damage, side);
        }
        for _ in 0..cost.clicks {
            cite!("rule_inherent_cost_aggregates");
            self.note_click_on_action(1);
            self.changes.record(GameChange::ClickSpent { side });
        }
        self.changes.record(GameChange::CostPaid {
            side,
            credits: want_credits,
            clicks: cost.clicks,
            trashed,
        });
        self.checkpoint_and_react(None);
    }

    // ------------------------------------------------------------------
    // Elementary mutations
    // ------------------------------------------------------------------

    fn draw_card_silent(&mut self, side: Side) -> Option<ObjectId> {
        let deck = self.st.deck.get_mut(&side).unwrap();
        if deck.is_empty() {
            return None;
        }
        let id = deck.remove(0);
        self.st.objects.get_mut(&id).unwrap().zone = Zone::Hand(side);
        self.st.hand.get_mut(&side).unwrap().push(id);
        Some(id)
    }

    /// Draw cards; `mandatory` marks the Corp's required draws (1.7.2c).
    pub fn draw_cards(&mut self, side: Side, n: u32, mandatory: bool) {
        if self.draw_prohibited(side) {
            return;
        }
        for _ in 0..n {
            if self.st.deck[&side].is_empty() {
                if side == Side::Corp && mandatory {
                    cite!("rule_empty_rnd");
                    self.game_over = Some(GameResult::RndEmpty);
                }
                return;
            }
            let id = self.draw_card_silent(side).unwrap();
            self.changes.record(GameChange::CardDrawn { side, obj: id });
        }
    }

    /// CR 10.4.2: meat/net trash 1 random grip card per point; core also
    /// takes a core damage counter. Flatline if damage > grip (1.7.2b).
    /// CR 10.4.3a / 9.12.1c: who, if anyone, selects the cards trashed by
    /// damage, and how many of them. With declarations from BOTH players the
    /// choice can only be made once, so the active player makes it (9.12.1c)
    /// — and the rest of each ability still resolves, which is why this
    /// function decides nothing except the choice itself.
    pub fn damage_trash_selector(&self) -> Option<(Side, u32)> {
        let mut found: Vec<(Side, u32)> = Vec::new();
        for (obj, d) in self.active_statics() {
            if let StaticDecl::SelectsDamageTrashes { by, count } = d {
                let n = self.eval_quantity(&count, Some(obj)).max(0) as u32;
                found.push((by, n));
            }
        }
        if found.is_empty() {
            return None;
        }
        cite!("rule_multiple_damage_selected_sequentially");
        if found.iter().any(|(s, _)| *s == Side::Corp)
            && found.iter().any(|(s, _)| *s == Side::Runner)
        {
            cite!("rule_modify_ability_with_choice");
            let active = self.st.turn_side;
            return found.into_iter().find(|(s, _)| *s == active);
        }
        found.into_iter().max_by_key(|(_, n)| *n)
    }

    pub fn do_damage(&mut self, kind: DamageKind, amount: u32, responsible: Side) {
        self.do_damage_selecting(kind, amount, &[], responsible)
    }

    /// The damage procedure with `chosen` cards selected up front (10.4.3a);
    /// the remainder is random, and all of them are trashed simultaneously.
    pub fn do_damage_selecting(
        &mut self,
        kind: DamageKind,
        amount: u32,
        chosen: &[ObjectId],
        responsible: Side,
    ) {
        cite!("rule_meat_net_damage");
        // 10.4.3: more than 1 damage of a type trashes the cards randomly and
        // SIMULTANEOUSLY — one occurrence, recorded as one change below, so a
        // conditional watching for the trashes sees a single event. 10.4.3a's
        // sequential-selection case (Chronos Protocol class) still trashes
        // simultaneously; only the selection order differs.
        cite!("rule_multiple_damage_taken_simultaneously");
        cite!("rule_multiple_damage_selected_sequentially");
        if amount == 0 {
            return;
        }
        let grip_len = self.st.hand[&Side::Runner].len() as u32;
        if amount > grip_len {
            cite!("rule_flatline");
            self.game_over = Some(GameResult::Flatline);
        }
        let mut trashed = Vec::new();
        // 10.4.3a: the selected cards first, in the order they were selected,
        // then the rest at random.
        for c in chosen.iter().copied() {
            if trashed.len() as u32 >= amount.min(grip_len) {
                break;
            }
            if self.st.hand[&Side::Runner].contains(&c) {
                // 10.4.2a: the responsible player TRASHES the card — it is a
                // trash movement (8.2.12), not a bare move, so it records
                // `CardTrashed` and 8.2.2's replaced destinations apply.
                self.trash_card(c, responsible);
                trashed.push(c);
            }
        }
        while (trashed.len() as u32) < amount.min(grip_len) {
            let hand = &self.st.hand[&Side::Runner];
            if hand.is_empty() {
                break;
            }
            let i = self.rng.random_range(0..hand.len());
            let card = hand[i];
            self.trash_card(card, responsible);
            trashed.push(card);
        }
        if kind == DamageKind::Core {
            cite!("rule_core_damage");
            self.st.runner.core_damage += amount;
            self.st.runner.max_hand_size_base -= amount as i32;
        }
        // One aggregated occurrence (9.12.2c: trash by damage aggregates).
        self.changes.record(GameChange::DamageSuffered { kind, amount, cards: trashed });
    }

    /// Move a card between zones, maintaining zone lists.
    pub fn move_card(&mut self, id: ObjectId, to: Zone) {
        let from = self.st.objects[&id].zone;
        // CR 4.4.6b: "If a Corp card is visible to the Runner when it is
        // trashed or discarded, it is put in Archives faceup. If a Corp card
        // is not visible to the Runner when it is trashed or discarded, then
        // it is put in Archives facedown." The state that decides it is the
        // one BEFORE the move, so it is read here.
        let archives_faceup = to == Zone::Discard(Side::Corp)
            && self.st.objects[&id].printed.side == Side::Corp
            && self.identity_visible_to(id, Side::Runner);
        // CR 4.8.3: a card moving out of the set-aside zone is treated as
        // having entered its destination directly from where it was before it
        // was set aside — for every ability except the one that set it aside,
        // which is the only one that can see the set-aside zone at all. The
        // kernel's representation of "what other abilities see" is the change
        // log, so the substitution happens exactly there.
        let reported_from = if from == Zone::SetAside {
            cite!("rule_set_aside_zone_passthrough");
            self.st.objects[&id].set_aside_from.unwrap_or(from)
        } else {
            from
        };
        // Remove from any zone list.
        for v in self.st.deck.values_mut() {
            v.retain(|&c| c != id);
        }
        for v in self.st.hand.values_mut() {
            v.retain(|&c| c != id);
        }
        for v in self.st.discard.values_mut() {
            v.retain(|&c| c != id);
        }
        for v in self.st.score_area.values_mut() {
            v.retain(|&c| c != id);
        }
        // 6.2.4: leaving a position vacates it; the position itself survives
        // until step 10.3.1i.
        self.vacate_ice(id);
        for v in self.st.root.values_mut() {
            v.retain(|&c| c != id);
        }
        let reserved = self.pending_ice_position.take();
        match to {
            Zone::Deck(s) => self.st.deck.get_mut(&s).unwrap().push(id),
            Zone::Hand(s) => self.st.hand.get_mut(&s).unwrap().push(id),
            Zone::Discard(s) => self.st.discard.get_mut(&s).unwrap().push(id),
            Zone::ScoreArea(s) => self.st.score_area.get_mut(&s).unwrap().push(id),
            Zone::Ice(s) => self.occupy_ice_position(id, s, reserved),
            Zone::Root(s) => self.st.root.entry(s).or_default().push(id),
            _ => {}
        }
        self.st.move_seq += 1;
        // CR 4.6.6i: leaving a server, its root, or a position protecting it
        // records the server so an ability of the card that moved still
        // refers to it as "this server".
        if let Zone::Root(s) | Zone::Ice(s) = from {
            if !matches!(to, Zone::Root(t) | Zone::Ice(t) if t == s) {
                cite!("rule_this_server");
                self.st.objects.get_mut(&id).unwrap().last_server = Some(s);
            }
        }
        let o = self.st.objects.get_mut(&id).unwrap();
        // CR 1.12.3: changing ZONES makes a NEW object out of the card —
        // and 1.12.4 says moving within a zone to a known location does not,
        // which is why the whole play area (root, ice, rig) is one class.
        if from.zone_class() != to.zone_class() {
            cite!("rule_object_move_location");
            o.generation += 1;
        }
        o.zone = to;
        // CR 4.8: leaving the set-aside zone ends the set-aside state, so a
        // 9.5.5 hosted card or an 8.7.2 found card becomes visible to every
        // ability again the moment it is put somewhere else.
        if to != Zone::SetAside {
            o.set_aside_for_ability = false;
            // 4.8.7: and it leaves its facedown group.
            o.set_aside_group = None;
        }
        if archives_faceup {
            cite!("rule_archives_faceup_facedown");
            o.faceup = true;
        }
        // CR 10.1.3: a card converted into an agenda by being added to a score
        // area keeps that conversion "until the card moves to a zone that is
        // not a score area, at which point it returns to being its original
        // printed card". A move between the two score areas (8.8.4c's swap)
        // is not such a move, so the conversion survives it.
        if !matches!(to, Zone::ScoreArea(_)) && o.converted_agenda.is_some() {
            cite!("rule_add_card_to_score_area");
            o.converted_agenda = None;
        }
        // CR 1.21.6: a card a resolving ability showed a player "remains
        // visible to the relevant player(s) until the entire ability is
        // finished resolving OR THE CARD MOVES TO A DIFFERENT LOCATION" — so
        // a disclosure does not survive the move. What a player is
        // CONTINUOUSLY entitled to (their own hand, their own facedown cards,
        // faceup cards anywhere) is derived from the new zone instead, and
        // 7.3.1a's access sighting is derived from the breach, so neither is
        // lost here.
        self.st.seen.forget(id);
        // CR 1.13.12: if a hosted object is moved to another zone, the
        // hosting relationship ends. (A 9.5.5 set-aside does not go through
        // here, so those relationships survive their host's trashing.)
        if from != to {
            if let Some(h) = o.host.take() {
                cite!("rule_hosted_object_same_zone_as_host");
                o.hosted_not_installed = false;
                if let Some(host) = self.st.objects.get_mut(&h) {
                    host.hosted.retain(|&x| x != id);
                }
            }
        }
        self.changes.record(GameChange::CardMoved { obj: id, from: reported_from, to });
        if reported_from.is_installed() && !to.is_installed() {
            self.changes
                .record(GameChange::CardUninstalled { obj: id, was_zone: reported_from });
        }
        // Mid-breach root entries (10.3.1j).
        if let Zone::Root(server) = to {
            self.changes.record(GameChange::CardEnteredRoot { obj: id, server });
        }
        // CR 1.13.12: a hosted object is in the same zone as its host, so
        // when a HOST moves the objects hosted on it move with it and the
        // hosting relationship is unaffected (8.8.3a's swap, 6.2.7d's move).
        // Only while the host is still installed: a host leaving the play
        // area is 1.13.13's business (its hosted objects are trashed at the
        // checkpoint, into their OWNERS' discard piles), and 8.8.4b's.
        if from != to && to.is_installed() {
            let guests: Vec<ObjectId> = self.st.objects[&id].hosted.clone();
            for g in guests {
                if self.st.objects[&g].zone != to
                    && self.st.objects[&g].zone != Zone::SetAside
                {
                    cite!("rule_hosted_object_same_zone_as_host");
                    self.move_hosted_with_host(g, to);
                }
            }
        }
    }

    /// A hosted object following its host to another zone (1.13.12) — the
    /// same move as [`Vm::move_card`] except that the hosting relationship
    /// survives it, because the guest did not move of its own accord.
    fn move_hosted_with_host(&mut self, guest: ObjectId, to: Zone) {
        let host = self.st.objects[&guest].host;
        self.move_card(guest, to);
        if let Some(h) = host {
            let g = self.st.objects.get_mut(&guest).unwrap();
            g.host = Some(h);
            if let Some(hh) = self.st.objects.get_mut(&h) {
                if !hh.hosted.contains(&guest) {
                    hh.hosted.push(guest);
                }
            }
        }
    }

    /// CR 1.19: trash = move to owner's discard pile.
    pub fn trash_card(&mut self, id: ObjectId, by: Side) {
        cite!("rule_trashing");
        let was = self.st.objects[&id].zone;
        let owner = self.st.objects[&id].owner;
        // CR 9.12.5a/b: when the Runner trashes a rezzed card they are
        // accessing, its persistent abilities begin to persist via a
        // lingering effect created simultaneously with the trash.
        if by == Side::Runner
            && self.st.accessed == Some(id)
            && self.st.objects[&id].faceup
        {
            if let Some((run_id, _, _)) = self.current_run {
                let defs: Vec<AbilityDef> = self.st.objects[&id]
                    .printed
                    .abilities
                    .iter()
                    .filter(|a| a.has_flag(AbilityFlag::Persistent))
                    .cloned()
                    .collect();
                for def in defs {
                    cite!("rule_persistent");
                    cite!("rule_persistent_continuous");
                    let lid = self.next_lingering;
                    self.next_lingering += 1;
                    self.lingering.push(LingeringEffect::new(lid, id, Payload::PersistedAbility { def, run_id }, Duration::PersistUntilAfterRun(run_id)));
                }
            }
        }
        // 8.2.2: the trash movement is recorded whether or not a replacement
        // changed where the card ends up — "the modified effect is still an
        // occurrence of that movement and can still meet trigger conditions
        // relating to that type of movement". Only 8.2.2a's fully replaced or
        // prevented trash records nothing, and that one never reaches here.
        self.changes.record(GameChange::CardTrashed { obj: id, by, was_zone: was });
        match self.replaced_trash_destination(id) {
            // 4.9: removed from the game instead of the discard pile.
            Some(crate::instr::TrashDestination::RemovedFromGame) => {
                cite!("sec_removed_from_game");
                self.move_card(id, Zone::RemovedFromGame);
            }
            // 8.1.4/8.1.4d: the installed Runner card is turned facedown and
            // stays in the play area — it is not uninstalled, so it never
            // moves at all.
            Some(crate::instr::TrashDestination::FacedownInPlay) => {
                cite!("sec_facedown_runner_cards");
                cite!("rule_flip_is_not_uninstall");
                if let Some(o) = self.st.objects.get_mut(&id) {
                    o.faceup = false;
                }
                self.changes.record(GameChange::CardDerezzed { obj: id });
            }
            None => self.move_card(id, Zone::Discard(owner)),
        }
    }

    /// CR 9.9.8b / 8.2.2: an active static ability may stipulate that a card
    /// being trashed goes somewhere other than its owner's discard pile. The
    /// declaration is read where the movement happens, so it applies to every
    /// trash — including the damage trashes of 10.4.2, which are not
    /// instructions and so have no imminence for an interrupt to modify.
    fn replaced_trash_destination(&self, id: ObjectId) -> Option<crate::instr::TrashDestination> {
        cite!("rule_replacement_effect_from_static_ability");
        cite!("sec_replacing_movements");
        let o = self.st.objects.get(&id)?;
        for (src, d) in self.active_statics() {
            if let StaticDecl::ReplaceTrashDestination { criteria, to } = d {
                if criteria.iter().all(|f| self.filter_matches(o, *f, Some(src))) {
                    return Some(to);
                }
            }
        }
        None
    }

    /// Rez: pay cost (checkpoint per 8.1.2e), turn faceup, active stamp.
    pub fn rez_card(&mut self, id: ObjectId) {
        self.rez_card_inner(id, false)
    }

    /// CR 1.16.5c / 8.1.2d: rez a card whose rez cost the rezzing ability
    /// states is ignored — the payment (and its 10.3.4 checkpoint) is simply
    /// not part of the procedure.
    pub fn rez_card_free(&mut self, id: ObjectId) {
        self.rez_card_inner(id, true)
    }

    fn rez_card_inner(&mut self, id: ObjectId, ignore_costs: bool) {
        // 8.1.2: to rez an unrezzed card is to turn it faceup. 8.1.2a: some
        // paid ability windows allow it (the (R) class, and ice at 6.9.2b);
        // 8.1.4f: Runner cards are never rezzed or unrezzed, which is why
        // nothing routes them here.
        cite!("sec_rez");
        cite!("rule_rez_in_paw");
        cite!("rule_runner_cards_neither_rezzed_nor_unrezzed");
        cite!("rule_rez_procedure");
        if !ignore_costs {
            let cost = Cost::credits(self.st.objects[&id].printed.cost.unwrap_or(0));
            // The rez cost may take Decisions to pay (1.16.2e/1.10.3c), so the
            // rest of the procedure is the payment's continuation.
            self.begin_payment(Side::Corp, id, &cost, PaymentCont::Rez(id), None);
            return;
        }
        self.rez_card_finish(id);
    }

    /// CR 8.1.2: the rest of the rez procedure, once the cost is paid.
    fn rez_card_finish(&mut self, id: ObjectId) {
        let seq = {
            self.st.active_seq += 1;
            self.st.active_seq
        };
        let o = self.st.objects.get_mut(&id).unwrap();
        o.faceup = true;
        o.active_since = seq;
        let ct = self.st.objects[&id].printed.card_type;
        self.changes.record(GameChange::CardRezzed { obj: id, card_type: ct });
        // 1.10.5b: recurring credits arrive as soon as the card is faceup.
        self.place_recurring_credits(id);
    }

    fn discard_step(&mut self, side: Side) {
        cite!("rule_discard_step");
        // 5.5.4a: the Runner checks for flatline first.
        if side == Side::Runner {
            cite!("rule_flatline");
            let max = self.max_hand_size(side);
            if max < 0 {
                self.game_over = Some(GameResult::Flatline);
                return;
            }
        }
        let max = self.max_hand_size(side).max(0) as usize;
        let hand = self.st.hand[&side].clone();
        if hand.len() > max {
            let count = (hand.len() - max) as u32;
            self.ask(
                side,
                DecisionSpec::DiscardCards { count, hand },
                DecisionCtx::Discard(side),
            );
        }
    }

    pub fn max_hand_size(&self, side: Side) -> i32 {
        cite!("rule_max_hand_size");
        self.st.player(side).max_hand_size_base
    }

    /// CR 1.10.5b: recurring credits are first placed on a card as soon as it
    /// becomes active — step 8.5.16e of installing it active, step 8.6.7c of
    /// playing it, or when it is turned faceup or scored. "N[recurring]" is
    /// shorthand for that placement plus the 1.10.5c turn-begins refill, so
    /// this is the same top-up rule, not an addition: a card that already
    /// holds N gains nothing (1.10.5d).
    pub fn place_recurring_credits(&mut self, id: ObjectId) {
        cite!("rule_placing_recurring_credits");
        let Some(o) = self.st.objects.get(&id) else { return };
        let Some(n) = o.printed.recurring_credits else { return };
        if !card_active(o) {
            return;
        }
        let have = o.counter(CounterKind::Credit);
        if have >= n {
            return;
        }
        cite!("rule_recurring_credits_do_not_accumulate");
        self.st.objects.get_mut(&id).unwrap().counters.insert(CounterKind::Credit, n);
        self.changes.record(GameChange::CounterPlaced {
            obj: id,
            kind: CounterKind::Credit,
            amount: n - have,
        });
    }

    /// CR 1.17.1: a player's score is the sum of the agenda points on
    /// agendas in their score area.
    pub fn score(&self, side: Side) -> i32 {
        cite!("rule_score");
        let effects = self.char_effects();
        self.st.score_area[&side]
            .iter()
            .filter(|id| self.st.objects.contains_key(id))
            .filter_map(|id| {
                crate::object::compute_effective(&self.st.objects, &effects, *id).agenda_points
            })
            .sum()
    }

    /// CR 2.5 / 9.12.1a: this card's agenda point value as modified — the
    /// printed number (or 10.1.3's converted one) after every active
    /// declaration has been applied.
    pub fn effective_agenda_points(&self, obj: ObjectId) -> Option<i32> {
        let effects = self.char_effects();
        crate::object::compute_effective(&self.st.objects, &effects, obj).agenda_points
    }

    /// CR 1.17.1a: the threat level is the greatest score of any player.
    /// It is what the "threat N" ability flag reads (9.3.6f).
    pub fn threat_level(&self) -> i32 {
        cite!("rule_threat_level");
        self.score(Side::Corp).max(self.score(Side::Runner))
    }

    /// The threat level as the characteristics pipeline's GATHER pass reads
    /// it: printed (and 10.1.3-converted) agenda point values, without the
    /// 2.5 modifications the pipeline is about to compute.
    ///
    /// Deviation 2b's reading, widened: gathering the pipeline's input cannot
    /// ask for a value the pipeline produces, so the one place the two meet —
    /// a `[threat N]` flag (9.3.6f) gating an ability on a board where a
    /// Merger-class declaration straddles the threshold — reads the
    /// unmodified score. Every other reader of the threat level goes through
    /// [`Vm::threat_level`], which is exact.
    fn gather_threat_level(&self) -> i32 {
        cite!("rule_threat_level");
        let of = |side: Side| -> i32 {
            self.st.score_area[&side]
                .iter()
                .filter_map(|id| self.st.objects.get(id))
                .filter_map(|o| o.converted_agenda.or(o.printed.agenda_points))
                .sum()
        };
        of(Side::Corp).max(of(Side::Runner))
    }

    pub fn memory_limit(&self) -> i32 {
        cite!("rule_memory_limit");
        let mut m = self.st.runner.memory_limit_base;
        for (_, d) in self.active_statics() {
            if let StaticDecl::MemoryLimitMod(delta) = d {
                m += delta;
            }
        }
        for l in &self.lingering {
            if let Payload::MemoryLimitMod { delta } = l.payload {
                m += delta;
            }
        }
        m
    }

    // ------------------------------------------------------------------
    // Checkpoints
    // ------------------------------------------------------------------

    /// Run a checkpoint; if instances were marked pending, immediately open
    /// a reaction window (10.3.2).
    pub fn checkpoint_and_react(&mut self, originating_structure: Option<u64>) {
        let newly = checkpoint::run_checkpoint(self);
        if let Some(result) = self.game_over {
            let _ = result;
            return;
        }
        if !newly.is_empty() {
            // Bind the window to a structure that just began, if the step
            // that triggered this was a structure-beginning (9.2.8f).
            // 9.2.8f: a reaction window is bound to the timing structure it
            // is opened DURING — the encounter in progress right now. A
            // window opened by the checkpoint that FOLLOWS an encounter's
            // end belongs to no encounter, so an ability triggered by that
            // end (Howler class, 1.15.4) is not dropped with it.
            let orig = originating_structure
                .or_else(|| self.st.encounter.as_ref().map(|e| e.id));
            self.open_reaction_window(newly, orig);
        }
    }

    // ------------------------------------------------------------------
    // Answers
    // ------------------------------------------------------------------

    fn apply_answer(&mut self) {
        let (side, spec, ctx) = self.pending_decision.take().unwrap();
        let _spec = &spec;
        let answer = self.answer.take().unwrap();
        match (ctx, answer) {
            (DecisionCtx::Mulligan(s), a) => {
                cite!("rule_mulligan");
                if a == DecisionAnswer::TakeMulligan {
                    let hand = self.st.hand[&s].clone();
                    for c in hand {
                        self.move_card(c, Zone::Deck(s));
                    }
                    let deck = self.st.deck.get_mut(&s).unwrap();
                    deck.shuffle(&mut self.rng);
                    for _ in 0..self.starting_hand_size(side) {
                        self.draw_card_silent(s);
                    }
                }
                self.setup = match s {
                    Side::Corp => SetupPhase::RunnerMulligan,
                    Side::Runner => SetupPhase::Done,
                };
            }
            // CR 1.16: every choice a payment needs routes here; the payment
            // itself decides what it was asking for and asks the next one.
            (DecisionCtx::Payment, a) => {
                self.answer_payment(a);
            }
            (DecisionCtx::Window(wid), DecisionAnswer::Pass) => {
                let _ = wid;
                self.window_pass();
            }
            (DecisionCtx::Window(_), DecisionAnswer::Action(opt)) => {
                self.take_action(side, opt);
            }
            (DecisionCtx::Window(wid), DecisionAnswer::Take(opt)) => {
                self.take_window_option(side, wid, opt);
            }
            (DecisionCtx::JackOut, DecisionAnswer::JackOut(yes)) => {
                cite!("step_jack_out_choice");
                if yes {
                    // Jacking out ends the run (6.6): jump to Run Ends.
                    self.end_the_run();
                } else {
                    self.set_structure_phase(StepPhase::Checkpoint);
                }
            }
            (DecisionCtx::Discard(s), DecisionAnswer::Discard(cards)) => {
                cite!("rule_discarding_process");
                self.changes.bump_group();
                for c in cards {
                    self.move_card(c, Zone::Discard(s));
                    self.changes.record(GameChange::CardDiscarded { obj: c, side: s });
                }
                self.set_structure_phase(StepPhase::Checkpoint);
            }
            (DecisionCtx::Candidate, DecisionAnswer::Candidate(c)) => {
                self.take_candidate(c);
                self.set_structure_phase(StepPhase::Checkpoint);
            }
            (DecisionCtx::Targets, DecisionAnswer::Option(i)) => {
                // 9.12.3d: the choice is its own instruction; the chosen
                // effect becomes the next instruction and can still be
                // interrupted as normal.
                cite!("rule_mandatory_choice_effects_can_be_modified");
                let (idx, instr) = {
                    let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                    (af.idx, af.instructions[af.idx].clone())
                };
                let (chooser_override, instr) = match instr {
                    Instruction::PerformedBy { side, instr } => (Some(side), *instr),
                    other => (None, other),
                };
                if let Instruction::ChooseOne { options } = instr {
                    // The answer indexes the RESOLVABLE label list; map back.
                    let resolvable: Vec<usize> = options
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, instrs))| self.option_resolvable(instrs))
                        .map(|(k, _)| k)
                        .collect();
                    let chosen = resolvable.get(i).copied().unwrap_or(0);
                    let inject = wrap_all(options[chosen].1.clone(), chooser_override);
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        for (k, ins) in inject.into_iter().enumerate() {
                            af.instructions.insert(idx + 1 + k, ins);
                        }
                        af.phase = AbilityPhase::Checkpoint;
                    }
                }
            }
            (DecisionCtx::Targets, DecisionAnswer::Targets(t)) => {
                // CR 1.15.2b/e: only valid targets, each chosen once, and as
                // many distinct ones as possible. The announcement carries
                // both the candidate list and the floor, so the answer is
                // filtered to the candidates, deduplicated, capped at the
                // count and completed from the candidates if short — the
                // same clamping every other Decision does (there is no
                // "your answer was illegal, choose again" path).
                cite!("rule_targets_must_be_valid");
                cite!("rule_distinct_targets");
                let t = clamp_announcement(&spec, t);
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                    af.targets.extend(t.iter().copied());
                    af.ability_targets.extend(t.iter().copied());
                    af.announce_slot += 1;
                    af.instructions[af.idx].clone()
                };
                // 1.15.2: an instruction requiring several announcements
                // asks again before becoming imminent.
                if let Some((side, next)) = self.targets_needed(&instr) {
                    self.ask(side, next, DecisionCtx::Targets);
                    return;
                }
                self.begin_imminence(instr);
            }
            (DecisionCtx::Targets, DecisionAnswer::AttackedServer(chosen)) => {
                // CR 6.9.1a: the announcement fills the instruction's server
                // position, exactly as 8.5.16b's declaration fills an
                // install's destination; the instruction then becomes
                // imminent carrying the announced server.
                cite!("step_initiation_announce");
                let declared = match &spec {
                    DecisionSpec::DeclareAttackedServer { options } => {
                        if options.contains(&chosen) { Some(chosen) } else { options.first().copied() }
                    }
                    _ => None,
                };
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                    let idx = af.idx;
                    if let (Some(s), Instruction::InitiateRun { server, .. }) =
                        (declared, &mut af.instructions[idx])
                    {
                        *server = Some(s);
                    }
                    af.instructions[idx].clone()
                };
                if let Some((side, next)) = self.targets_needed(&instr) {
                    self.ask(side, next, DecisionCtx::Targets);
                    return;
                }
                self.begin_imminence(instr);
            }
            (DecisionCtx::Targets, DecisionAnswer::InstallDestination(d)) => {
                // CR 8.5.16b: the declaration replaces the stated-nothing
                // destination with the declared one; the instruction then
                // becomes imminent like any other, so an interrupt sees the
                // destination the installer declared.
                cite!("rule_steps_installing_destination");
                // The answer is clamped to the offered list, like every other
                // Decision: there is no "your answer was illegal" path.
                let declared = match &spec {
                    DecisionSpec::DeclareInstallDestination { options } => {
                        if options.contains(&d) { Some(d) } else { options.first().copied() }
                    }
                    _ => None,
                };
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                    let idx = af.idx;
                    if let (Some(decl), Instruction::InstallCard { dest, .. }) =
                        (declared, &mut af.instructions[idx])
                    {
                        *dest = decl;
                    }
                    af.instructions[idx].clone()
                };
                if let Some((side, next)) = self.targets_needed(&instr) {
                    self.ask(side, next, DecisionCtx::Targets);
                    return;
                }
                self.begin_imminence(instr);
            }
            (DecisionCtx::Targets, DecisionAnswer::Counters(chosen)) => {
                // CR 1.15.1/1.15.2b: counters are announced exactly like any
                // other object — validated against the candidate list, chosen
                // at most once each, capped at the count. "If 2 tokens are
                // chosen, they must be hosted on the same card" is the
                // instruction's own requirement ("from 1 other card"), so the
                // announcement keeps only the counters sharing the host of the
                // first one named.
                cite!("rule_target");
                let keep: Vec<crate::object::CounterRef> = match &spec {
                    DecisionSpec::ChooseCounters { candidates, count, .. } => {
                        let mut out: Vec<crate::object::CounterRef> = Vec::new();
                        let mut host = None;
                        for c in chosen {
                            if !candidates.contains(&c) || out.contains(&c) {
                                continue;
                            }
                            if (out.len() as u32) >= *count {
                                break;
                            }
                            match host {
                                None => host = Some(c.host),
                                Some(h) if h == c.host => {}
                                Some(_) => continue,
                            }
                            out.push(c);
                        }
                        out
                    }
                    _ => chosen,
                };
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                    af.counter_targets.extend(keep);
                    af.announce_slot += 1;
                    af.instructions[af.idx].clone()
                };
                self.begin_imminence(instr);
            }
            (DecisionCtx::Targets, DecisionAnswer::Subroutines(subs)) => {
                // CR 1.15.1/1.15.2b: subroutines are announced exactly like
                // objects — validated against the candidate list, chosen at
                // most once each, capped at the count.
                cite!("rule_object_subroutine_targets");
                let keep: Vec<SubKey> = match &spec {
                    DecisionSpec::ChooseSubroutines { candidates, count, .. } => {
                        let mut out: Vec<SubKey> = Vec::new();
                        for k in subs {
                            if candidates.iter().any(|(c, _)| *c == k)
                                && !out.contains(&k)
                                && (out.len() as u32) < *count
                            {
                                out.push(k);
                            }
                        }
                        out
                    }
                    _ => subs,
                };
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                    af.sub_targets.extend(keep);
                    af.announce_slot += 1;
                    af.instructions[af.idx].clone()
                };
                self.begin_imminence(instr);
            }
            (DecisionCtx::Targets, DecisionAnswer::PayNestedCost(pay)) => {
                // The nested-cost choice ends an instruction (9.11.4f).
                cite!("rule_nested_cost_instruction");
                let (source, idx) = {
                    let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                    (af.source, af.idx)
                };
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                    af.instructions[idx].clone()
                };
                let (payer, _) = self.nested_cost_payer(&instr);
                match instr {
                    Instruction::NestedCostThen { cost, effect, .. } => {
                        if pay {
                            cite!("rule_nested_cost_may");
                            self.pay_cost(payer, source.obj, &cost);
                            if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                                af.instructions.insert(idx + 1, (*effect).clone());
                            }
                            self.changes.record(GameChange::AbilityUsed { source: source.obj });
                        }
                    }
                    Instruction::NestedCostUnless { cost, effect, .. } => {
                        cite!("rule_nested_cost_unless");
                        if pay {
                            self.pay_cost(payer, source.obj, &cost);
                        } else if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                            af.instructions.insert(idx + 1, (*effect).clone());
                        }
                    }
                    _ => unreachable!(),
                }
                // The choice instruction itself resolves as a no-op.
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    af.phase = AbilityPhase::Checkpoint;
                }
            }
            (DecisionCtx::StealCost(card), DecisionAnswer::PayNestedCost(pay)) => {
                // CR 1.16.10a/1.17.3d: an additional cost to steal may be
                // declined; declining means the agenda is not stolen.
                cite!("rule_decline_additional_cost");
                if pay {
                    // 1.16.10b: all additional costs are one all-at-once
                    // payment; the frame's PayCost phase pays first, so the
                    // cost-paid checkpoint's reactions resolve BEFORE the
                    // steal becomes imminent.
                    cite!("rule_additonal_cost_simultaenous");
                    let total = self.steal_cost_of(card);
                    self.push_ability_frame_cost(
                        ResolutionKind::Conditional,
                        AbilityRef { obj: card, index: usize::MAX },
                        Side::Runner,
                        vec![Instruction::StealSelfAgenda],
                        None,
                        None,
                        Some(total),
                    );
                }
            }
            (DecisionCtx::OptionalReplacement, DecisionAnswer::ResolveOptional(yes)) => {
                // CR 6.7.4c: the decision was made where the breach would
                // begin. Applying consumes the effect; declining leaves it
                // unapplied for THIS effect (9.9.9c) and the base effect
                // happens normally.
                cite!("rule_if_successful_ability_optional");
                if let Some(lid) = self.pending_optional_replacement.take() {
                    if yes {
                        self.apply_replacement(lid);
                    } else if let Some(seq) = self.imminents.last().map(|i| i.seq) {
                        if let Some(l) = self.lingering.iter_mut().find(|l| l.id == lid) {
                            l.applied_to.push(seq);
                        }
                    }
                }
                // More replacements may still apply to the same effect
                // (9.9.11); when none do, the flow continues exactly as it
                // does after an order Decision.
                if self.resolve_replacements_or_ask() {
                    return;
                }
                if !self.open_interrupt_window_if_relevant() {
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        if af.phase == AbilityPhase::Imminent {
                            af.phase = AbilityPhase::Resolve;
                        }
                    }
                }
            }
            (DecisionCtx::RunActionCost(server), DecisionAnswer::PayNestedCost(pay)) => {
                // CR 9.12.3e: "a singular 'must' ability cannot force a player
                // to pay an additional cost they wish to decline." Being
                // offered the cost is enough to satisfy the requirement, so
                // the player may then spend the click on anything.
                cite!("rule_must_cannot_force_additional_cost");
                self.st.run_requirement_discharged = true;
                if pay {
                    // 6.3.4: the [click] and the additional cost are both paid
                    // to MAKE the run; the run formally begins afterwards.
                    cite!("rule_abilities_during_a_run");
                    let extra = self.run_action_cost();
                    self.spend_click(Side::Runner);
                    self.pay_cost(Side::Runner, ObjectId(0), &extra);
                    self.initiate_run(server);
                }
            }
            (DecisionCtx::ScoreCost(card), DecisionAnswer::PayNestedCost(pay)) => {
                // CR 1.16.10a: the Corp may decline the additional cost, and
                // declining means the agenda is not scored.
                cite!("rule_decline_additional_cost");
                if pay {
                    // 1.16.10c: "the additional cost is paid and a checkpoint
                    // is resolved BEFORE performing the usual procedure to
                    // carry out that effect" — which is exactly what an
                    // ability frame's 9.5.7b PayCost phase does, so scoring
                    // becomes that frame's one instruction.
                    cite!("rule_additional_cost_checkpoint");
                    let total = self.score_cost_of(card);
                    self.push_ability_frame_cost(
                        ResolutionKind::Conditional,
                        AbilityRef { obj: card, index: usize::MAX },
                        Side::Corp,
                        vec![Instruction::ScoreSelfAgenda],
                        None,
                        None,
                        Some(total),
                    );
                    // 1.16.1c: the restriction that made the agenda scorable
                    // must still be met by the way the cost is paid.
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        af.cost_restriction = Some(PaymentRestriction::ScoreRequirement(card));
                    }
                }
            }
            (DecisionCtx::Targets, DecisionAnswer::ResolveOptional(yes)) => {
                let idx = {
                    let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
                    af.idx
                };
                let instr = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { unreachable!() };
                    af.declined = !yes;
                    af.instructions[idx].clone()
                };
                self.begin_imminence(instr);
            }
            (DecisionCtx::MinimalSet, DecisionAnswer::ChooseSet(i)) => {
                cite!("step_checkpoint_card_restrictions");
                // Re-derive the sets is unnecessary: the spec carried them.
                // The answer index was validated by the driver against the
                // spec; trash the chosen set.
                if let Some((_, DecisionSpec::MinimalSet { sets }, _)) = None::<(Side, DecisionSpec, DecisionCtx)> {
                    let _ = (sets, i);
                }
                // Sets are re-fetched from the last asked spec (stored by ask).
                let sets = match &self.last_minimal_sets {
                    Some(s) => s.clone(),
                    None => Vec::new(),
                };
                if let Some(set) = sets.get(i) {
                    for id in set.clone() {
                        let owner = self.st.objects[&id].owner;
                        self.trash_card(id, owner);
                    }
                }
                self.last_minimal_sets = None;
            }
            (DecisionCtx::AccessCost(card), DecisionAnswer::PayNestedCost(pay)) => {
                // 7.4.3 example 2: declining the additional access cost
                // means no access occurs — but the chosen card already
                // ceased to be a candidate.
                cite!("rule_candidates_already_accessed");
                if pay {
                    let cost = self.additional_access_cost(card);
                    self.begin_payment(
                        Side::Runner,
                        card,
                        &cost,
                        PaymentCont::Access(card),
                        None,
                    );
                } else {
                    if let Some(b) = self.breach_ctx_mut() {
                        b.chosen = None;
                    }
                    self.refresh_candidates_after_access();
                }
                // The breach step's Exec already advanced to Checkpoint;
                // a paid access pushed its structure frame on top.
            }
            (DecisionCtx::LoopCount, DecisionAnswer::LoopCount(n)) => {
                // 10.1.6a: the loop resolves that many more times, and ends.
                cite!("rule_mandatory_infinite_loop");
                self.loop_budget = Some(n);
            }
            (DecisionCtx::Arrange { to_top_of }, DecisionAnswer::Arrangement(order)) => {
                // 8.3.3: the declared order is applied to the cards this
                // ability set aside. Anything the answer omitted keeps the
                // order it had, behind what was named — an arrangement
                // repositions cards among their current locations (8.3.1) and
                // cannot lose one.
                let mut cards = self.ability_set_aside_group_cards();
                let mut ordered: Vec<ObjectId> = Vec::new();
                for c in order {
                    if cards.contains(&c) && !ordered.contains(&c) {
                        ordered.push(c);
                    }
                }
                cards.retain(|c| !ordered.contains(c));
                ordered.extend(cards);
                self.finish_arrangement(to_top_of, ordered);
            }
            (DecisionCtx::Sabotage { count }, DecisionAnswer::Targets(from_hq)) => {
                // 10.12.2: the chosen HQ cards and the top of R&D are trashed
                // SIMULTANEOUSLY, and they enter Archives facedown (10.12.2a).
                cite!("rule_sabotage_resolution");
                cite!("rule_sabotage_facedown");
                let hq_now = self.st.hand[&Side::Corp].clone();
                let rnd_len = self.st.deck[&Side::Corp].len() as u32;
                let mut chosen: Vec<ObjectId> = Vec::new();
                for c in from_hq {
                    if hq_now.contains(&c) && !chosen.contains(&c) && (chosen.len() as u32) < count {
                        chosen.push(c);
                    }
                }
                // The 10.12.3a floor is a requirement of the instruction, not
                // a suggestion: a short choice is completed from HQ.
                let need = count.saturating_sub(rnd_len).min(hq_now.len() as u32);
                for c in hq_now {
                    if (chosen.len() as u32) >= need {
                        break;
                    }
                    if !chosen.contains(&c) {
                        chosen.push(c);
                    }
                }
                let from_rnd: Vec<ObjectId> = self.st.deck[&Side::Corp]
                    .iter()
                    .take((count - chosen.len() as u32) as usize)
                    .copied()
                    .collect();
                for c in chosen.into_iter().chain(from_rnd) {
                    self.st.objects.get_mut(&c).unwrap().faceup = false;
                    self.trash_card(c, Side::Corp);
                }
            }
            (DecisionCtx::SearchFind { zone }, DecisionAnswer::Targets(found)) => {
                // 8.7.2: the player has finished looking. Take what they
                // found, set it aside facedown, reshuffle if it was a deck
                // (8.7.3), and only then let resolution resume (8.7.4). The
                // ability frame is already at its post-instruction
                // checkpoint, which is where a search-involving condition
                // becomes pending (8.7.5 / 9.11.4d).
                cite!("rule_continue_after_search");
                self.complete_search(zone, &found, side);
            }
            (DecisionCtx::ReplacementOrder, DecisionAnswer::Option(i)) => {
                // 9.9.11: apply the chosen replacement, then re-evaluate —
                // later replacements only apply if their target effect is
                // still expected (9.9.11a).
                cite!("rule_order_of_replacement_effects");
                let appl = self.applicable_replacements();
                if let Some(&lid) = appl.get(i) {
                    self.apply_replacement(lid);
                }
                if self.resolve_replacements_or_ask() {
                    return; // another order Decision pending
                }
                // Replacement application complete: open the interrupt
                // window (or continue).
                if !self.open_interrupt_window_if_relevant() {
                    if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                        if af.phase == AbilityPhase::Imminent {
                            af.phase = AbilityPhase::Resolve;
                        }
                    }
                    // Structure steps already advanced to Exec; they
                    // continue naturally.
                }
            }
            (DecisionCtx::DamageSelection { kind, amount }, DecisionAnswer::Targets(chosen)) => {
                // 10.4.3a: selected sequentially, trashed simultaneously.
                cite!("rule_multiple_damage_selected_sequentially");
                cite!("rule_multiple_damage_taken_simultaneously");
                self.do_damage_selecting(kind, amount, &chosen, Side::Corp);
            }
            (DecisionCtx::SubroutineOrder, DecisionAnswer::SubroutineOrder(at)) => {
                // 9.8.2c: apply the declared positions to the grant that was
                // just made. The declaration is stored on the lingering
                // effects, so `current_subs` reads it back for the rest of
                // the grant's duration.
                cite!("rule_gain_subroutines_in_any_order");
                let Some((ice, seq)) = self.pending_sub_order.take() else { return };
                let n = self.current_subs(ice).len();
                let mut k = 0usize;
                let mut targets: Vec<(u64, u32)> = self
                    .lingering
                    .iter()
                    .filter_map(|l| match &l.payload {
                        Payload::GrantedSubroutine { to, seq: s, ord, placement: None, .. }
                            if *to == ice && *s == seq =>
                        {
                            Some((l.id, *ord))
                        }
                        _ => None,
                    })
                    .collect();
                targets.sort_by_key(|(_, o)| *o);
                for (lid, _) in targets {
                    let want = at.get(k).copied().unwrap_or(n).min(n);
                    k += 1;
                    for l in self.lingering.iter_mut() {
                        if l.id == lid {
                            if let Payload::GrantedSubroutine { placement, .. } = &mut l.payload {
                                *placement = Some(want);
                            }
                        }
                    }
                }
            }
            (DecisionCtx::CostDivision, DecisionAnswer::DivideReduction(n)) => {
                // 1.16.2f: "the numbers declared this way must be nonnegative
                // numbers whose sum is equal to the number of credits
                // specified by the original modifier" — one number determines
                // both, and it is clamped to the total.
                cite!("rule_install_and_rez_reducing_total");
                if let Some(p) = self.installs.last_mut() {
                    p.reduce_install = n.min(p.reduce_total);
                }
                // (The 1.16.2f division Decision resumes here; the 9.9.6c
                // value modification, if any, was applied to `reduce_install`
                // before the Decision was asked.)
                self.pay_install_cost(None);
                // Step 8.5.16d completes; its checkpoint is the cost-paid one.
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    if af.phase == AbilityPhase::Resolve {
                        af.phase = AbilityPhase::Checkpoint;
                    }
                }
            }
            (DecisionCtx::RezAdditionalCost, DecisionAnswer::PayNestedCost(pay)) => {
                // 8.5.13d / 1.16.4c: pay the rez cost plus additional costs,
                // or decline — declining reveals the card and skips the rez.
                cite!("rule_inherent_and_additional_cost");
                let Some(p) = self.installs.last().cloned() else { return };
                let c = p.card;
                if pay {
                    let base = if p.ignore_costs {
                        Cost::free()
                    } else {
                        Cost::credits(self.st.objects[&c].printed.cost.unwrap_or(0))
                    };
                    let add = self.st.objects[&c]
                        .printed
                        .additional_rez_cost
                        .clone()
                        .unwrap_or_default();
                    let total = base.plus(&add);
                    self.pay_cost(Side::Corp, c, &total);
                } else {
                    cite!("rule_reveal_for_install_and_rez");
                    self.install_reveal(c);
                    if let Some(p) = self.installs.last_mut() {
                        p.rez_skipped = true;
                    }
                }
                // The rez-cost step completes.
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    if af.phase == AbilityPhase::Resolve {
                        af.phase = AbilityPhase::Checkpoint;
                    }
                }
            }
            (DecisionCtx::BreachCandidacy(card), DecisionAnswer::ResolveOptional(yes)) => {
                // CR 10.3.1j / 7.4.6a: the Runner declares candidacy.
                cite!("step_checkpoint_card_entering_root_during_breach");
                cite!("rule_candidates_entering_root");
                if yes {
                    self.add_breach_candidate(card);
                } else if let Some(b) = self.breach_ctx_mut() {
                    // Declined: it cannot become a candidate for the rest of
                    // this breach.
                    b.declined.push(card);
                }
                // Further mid-breach entries from the same checkpoint are
                // declared one at a time.
                if let Some(next) = self.pending_candidacy.pop() {
                    self.ask(
                        Side::Runner,
                        DecisionSpec::DeclareBreachCandidate { card: next },
                        DecisionCtx::BreachCandidacy(next),
                    );
                }
            }
            (DecisionCtx::TraceSpend(side), DecisionAnswer::SpendCredits(n)) => {
                // 10.8.2/10.8.3: openly spend credits; this is a payment, so
                // its checkpoint follows (10.3.4).
                let n = n.min(self.spendable_credits(side));
                self.spend_flexible(side, n);
                if n > 0 {
                    self.changes.record(GameChange::CreditsLost { side, amount: n });
                    self.changes.record(GameChange::CostPaid {
                        side,
                        credits: n,
                        clicks: 0,
                        trashed: Vec::new(),
                    });
                }
                if let Some(t) = self.trace.as_mut() {
                    match side {
                        Side::Corp => t.trace_strength += n as i64,
                        Side::Runner => t.link_strength += n as i64,
                    }
                }
                if n > 0 {
                    cite!("rule_checkpoint_after_paying_cost");
                    self.checkpoint_and_react(None);
                }
                // The spend instruction completes.
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    if af.phase == AbilityPhase::Resolve {
                        af.phase = AbilityPhase::Checkpoint;
                    }
                }
            }
            (DecisionCtx::PsiBid(side), DecisionAnswer::Bid(n)) => {
                cite!("rule_bid_secret");
                let legal = self.psi_legal_bids(side);
                assert!(legal.contains(&n), "illegal bid {n} (10.14.3)");
                match side {
                    Side::Corp => {
                        // Seal the Corp's bid; ask the Runner.
                        self.psi_first_bid = Some(n);
                        let legal = self.psi_legal_bids(Side::Runner);
                        self.ask(
                            Side::Runner,
                            DecisionSpec::PsiBid { legal },
                            DecisionCtx::PsiBid(Side::Runner),
                        );
                    }
                    Side::Runner => {
                        // Reveal: both spend immediately — no checkpoint or
                        // window between reveal and spend (10.14.4a/10.14.6c).
                        cite!("rule_bid_spent_immediately");
                        cite!("rule_bid_is_cost");
                        let corp_bid = self.psi_first_bid.take().unwrap_or(0);
                        let runner_bid = n;
                        self.spend_flexible(Side::Corp, corp_bid);
                        self.spend_flexible(Side::Runner, runner_bid);
                        if corp_bid > 0 {
                            self.changes.record(GameChange::CreditsLost {
                                side: Side::Corp,
                                amount: corp_bid,
                            });
                        }
                        if runner_bid > 0 {
                            self.changes.record(GameChange::CreditsLost {
                                side: Side::Runner,
                                amount: runner_bid,
                            });
                        }
                        // 10.14.6d: branch on match/differ as the following
                        // instructions.
                        cite!("rule_psi_outcome");
                        let (idx, instr) = {
                            let Some(Frame::Ability(af)) = self.frames.last() else {
                                unreachable!()
                            };
                            (af.idx, af.instructions[af.idx].clone())
                        };
                        if let Instruction::PsiGame { on_match, on_differ } = instr {
                            let inject = if corp_bid == runner_bid { on_match } else { on_differ };
                            if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                                for (k, ins) in inject.into_iter().enumerate() {
                                    af.instructions.insert(idx + 1 + k, ins);
                                }
                                if af.phase == AbilityPhase::Resolve {
                                    af.phase = AbilityPhase::Checkpoint;
                                }
                            }
                        }
                    }
                }
            }
            (ctx, ans) => panic!("mismatched decision answer {ans:?} for {ctx:?}"),
        }
    }

    fn take_action(&mut self, side: Side, opt: ActionOption) {
        // Close the action window first (9.2.6c: after the action the window
        // closes; the player does not receive priority again). The action
        // itself may push frames (run structure, ability frame).
        cite!("rule_action_window_closes_after_action");
        let Some(Frame::Window(_)) = self.frames.pop() else { unreachable!() };
        self.after_window_closed();
        // CR 5.2.2: initiate by paying [click] (checkpoint follows, 10.3.4).
        cite!("rule_initiate_action");
        // CR 5.2.5a/b: the action's IDENTITY — what makes two actions the
        // same or different — is recorded as the action is initiated, and it
        // is also the key 1.16.4d attributes clicks to.
        cite!("rule_same_actions");
        cite!("rule_defferent_actions");
        let identity = match &opt {
            ActionOption::BasicCredit => ActionIdentity::Basic(BasicAction::Credit),
            ActionOption::BasicDraw => ActionIdentity::Basic(BasicAction::Draw),
            ActionOption::BasicRun { .. } => ActionIdentity::Basic(BasicAction::Run),
            ActionOption::BasicRemoveTag => ActionIdentity::Basic(BasicAction::RemoveTag),
            ActionOption::BasicPlayOperation { .. } => {
                ActionIdentity::Basic(BasicAction::PlayOperation)
            }
            ActionOption::BasicInstall { .. } => ActionIdentity::Basic(BasicAction::Install),
            ActionOption::BasicAdvance { .. } => ActionIdentity::Basic(BasicAction::Advance),
            ActionOption::BasicTrashResource => {
                ActionIdentity::Basic(BasicAction::TrashResource)
            }
            ActionOption::BasicPurge => ActionIdentity::Basic(BasicAction::Purge),
            ActionOption::CardAction { ability, .. } => ActionIdentity::CardAbility(*ability),
        };
        // 1.16.4d: the clicks spent to TAKE this action, counted from here —
        // "even though other steps take place between initiating the action
        // and paying that cost".
        cite!("rule_inherent_cost_aggregates");
        self.st.current_action = Some((identity, 0));
        self.changes.record(GameChange::ActionTaken { side, action: identity });
        match opt {
            ActionOption::BasicCredit => {
                cite!("rule_corp_basic_action_credit");
                self.spend_click(side);
                self.changes.bump_group();
                self.st.player_mut(side).credits += 1;
                self.changes.record(GameChange::CreditsGained { side, amount: 1 });
            }
            ActionOption::BasicDraw => {
                cite!("rule_corp_basic_action_draw");
                self.spend_click(side);
                self.changes.bump_group();
                self.draw_cards(side, 1, false);
            }
            ActionOption::BasicRun { server } => {
                cite!("runner_basic_action_run");
                // 1.16.10a: an additional cost to make a run may be declined,
                // and declining means the action is not taken at all — the
                // [click] is not spent either (1.16.4c's shape).
                let extra = self.run_action_cost();
                if !extra.is_free() {
                    cite!("rule_additional_cost");
                    cite!("rule_decline_additional_cost");
                    self.ask(
                        side,
                        DecisionSpec::NestedCost { cost: extra },
                        DecisionCtx::RunActionCost(server),
                    );
                    return;
                }
                self.st.run_requirement_discharged = true;
                self.spend_click(side);
                self.initiate_run(server);
            }
            ActionOption::BasicRemoveTag => {
                cite!("runner_basic_action_remove_tag");
                self.spend_click(side);
                self.st.runner.credits -= 2;
                self.changes.record(GameChange::CreditsLost { side, amount: 2 });
                self.checkpoint_and_react(None);
                self.st.runner.tags -= 1;
                self.changes.record(GameChange::TagRemoved);
            }
            ActionOption::BasicPlayOperation { card } => {
                // CR 5.2.6e: "[click]: Play 1 operation from HQ." The play
                // cost — and any additional cost to play (1.16.10b) — is paid
                // inside the 8.6.7 procedure, and 1.16.4d attributes all of
                // it to this action.
                cite!("rule_corp_basic_action_operation");
                self.spend_click(side);
                // The action's effect is the 8.6.7 play procedure; it runs in
                // a rules ability frame, exactly as the 7.2.3 steal does.
                self.push_ability_frame(
                    ResolutionKind::Play,
                    AbilityRef { obj: card, index: usize::MAX },
                    side,
                    vec![Instruction::PlayCard {
                        card: TargetSpec::Objects(vec![card]),
                        ignore_costs: false,
                    }],
                    None,
                    None,
                );
            }
            ActionOption::BasicInstall { card } => {
                // CR 5.2.6d/5.2.7d: the action's effect is the ordinary 8.5.16
                // install procedure, in a rules ability frame — the same shape
                // the basic play action uses. The destination is declared
                // inside it, at step 8.5.16b.
                cite!("rule_corp_basic_action_install");
                cite!("runner_basic_action_install");
                self.spend_click(side);
                self.push_ability_frame(
                    ResolutionKind::Paid,
                    AbilityRef { obj: card, index: usize::MAX },
                    side,
                    vec![Instruction::InstallCard {
                        card: TargetSpec::Objects(vec![card]),
                        dest: crate::instr::InstallDest::DeclaredByInstaller,
                        and_rez: false,
                        ignore_costs: false,
                        reveal_check: None,
                        reduce_total: crate::instr::Quantity::Const(0),
                    }],
                    None,
                    None,
                );
            }
            ActionOption::BasicAdvance { card } => {
                // CR 5.2.6f: "[click], 1[credit]: Advance 1 installed card."
                // The credit is part of the action's cost, so it is paid as
                // the action is initiated (5.2.2) — before the advance.
                cite!("corp_basic_action_advance");
                self.spend_click(side);
                self.st.player_mut(side).credits -= 1;
                self.changes.record(GameChange::CreditsLost { side, amount: 1 });
                self.changes.record(GameChange::CostPaid {
                    side,
                    credits: 1,
                    clicks: 0,
                    trashed: Vec::new(),
                });
                cite!("rule_checkpoint_after_paying_cost");
                self.checkpoint_and_react(None);
                self.push_ability_frame(
                    ResolutionKind::Paid,
                    AbilityRef { obj: card, index: usize::MAX },
                    side,
                    vec![Instruction::AdvanceCard { target: TargetSpec::Objects(vec![card]) }],
                    None,
                    None,
                );
            }
            ActionOption::BasicTrashResource => {
                // CR 5.2.6g / 10.5.3: the resource is chosen while the action
                // resolves — a 1.15.2 target announcement over the installed
                // resources, which is exactly what the criteria say.
                cite!("corp_basic_action_trash_resource");
                cite!("rule_tagged_trash_resource");
                self.spend_click(side);
                self.st.player_mut(side).credits -= 2;
                self.changes.record(GameChange::CreditsLost { side, amount: 2 });
                self.changes.record(GameChange::CostPaid {
                    side,
                    credits: 2,
                    clicks: 0,
                    trashed: Vec::new(),
                });
                cite!("rule_checkpoint_after_paying_cost");
                self.checkpoint_and_react(None);
                self.push_ability_frame(
                    ResolutionKind::Paid,
                    AbilityRef { obj: ObjectId(0), index: usize::MAX },
                    side,
                    vec![Instruction::TrashCards(TargetSpec::Choose {
                        count: crate::instr::Quantity::Const(1),
                        criteria: vec![TargetFilter::InstalledResource], up_to: false })],
                    None,
                    None,
                );
            }
            ActionOption::BasicPurge => {
                // CR 5.2.6h: three clicks, paid one at a time — each is a cost
                // payment with its own checkpoint (1.16.3).
                cite!("corp_basic_action_purge_virus_counters");
                self.spend_click(side);
                self.spend_click(side);
                self.spend_click(side);
                self.push_ability_frame(
                    ResolutionKind::Paid,
                    AbilityRef { obj: ObjectId(0), index: usize::MAX },
                    side,
                    vec![Instruction::PurgeVirusCounters],
                    None,
                    None,
                );
            }
            ActionOption::CardAction { ability, .. } => {
                let def = self.st.objects[&ability.obj].face().abilities[ability.index].clone();
                self.trigger_paid_ability(side, ability, def);
            }
        }
    }

    /// CR 1.18.3: which installed cards the Corp can advance — agendas
    /// always, and any other card whose active abilities say it can be
    /// advanced (9.1.8f keeps that declaration active while the card is
    /// unrezzed, which is the usual case for an Ice Wall).
    pub fn advanceable_cards(&self) -> Vec<ObjectId> {
        cite!("rule_you_can_advance");
        let threat = self.threat_level();
        let mut out: Vec<ObjectId> = self
            .st
            .objects
            .values()
            .filter(|o| {
                if !matches!(o.zone, Zone::Root(_) | Zone::Ice(_)) || o.hosted_not_installed {
                    return false;
                }
                if o.printed.card_type == CardType::Agenda {
                    return true;
                }
                o.face().abilities.iter().any(|a| {
                    a.statics.iter().any(|d| matches!(d, StaticDecl::CanBeAdvancedSelf))
                        && crate::ability::ability_active(
                            o,
                            a,
                            self.st.encounter.as_ref().map(|e| e.ice),
                            self.st.accessed,
                            threat,
                        )
                })
            })
            .map(|o| o.id)
            .collect();
        out.sort();
        out
    }

    /// CR 8.5.16b: every destination this card may legally be installed in —
    /// the option list of the declaration the installing player makes when
    /// the effect states no destination of its own (5.2.6d/5.2.7d).
    pub(crate) fn install_destinations_for(
        &self,
        card: ObjectId,
        side: Side,
    ) -> Vec<crate::instr::InstallDest> {
        use crate::instr::InstallDest as D;
        cite!("rule_steps_installing_destination");
        let Some(o) = self.st.objects.get(&card) else { return Vec::new() };
        let mut out = Vec::new();
        match o.printed.card_type {
            // 8.5.2d: ice is installed protecting a server.
            CardType::Ice => {
                for s in self.all_servers() {
                    out.push(D::Protecting(s));
                }
                if self.can_create_new_remote() {
                    out.push(D::NewRemoteProtecting);
                }
            }
            // 8.5.2b/c: agendas, assets and upgrades go in the root of a
            // server; 4.6.6e/3.6.1 limit which roots will take them, and
            // `may_occupy` is that test.
            CardType::Agenda | CardType::Asset | CardType::Upgrade => {
                for s in self.all_servers() {
                    if self.may_occupy(card, Zone::Root(s), ObjectId(u32::MAX)) {
                        out.push(D::Root(s));
                    }
                }
                if self.can_create_new_remote() {
                    out.push(D::NewRemoteRoot);
                }
            }
            // 8.5.4: Runner cards are installed in the rig.
            _ => {
                if side == Side::Runner {
                    out.push(D::Rig);
                }
            }
        }
        // 8.5.16b's "including any host relationships": an eligible host is a
        // destination like any other (1.13.6a), and where the installee's own
        // ability names its hosts it is the ONLY kind (1.13.6c).
        let hosts = self.eligible_hosts_for(card);
        if self.install_only_hosted_on(card).is_some() {
            out.clear();
        }
        out.extend(hosts.into_iter().map(D::HostedOn));
        out
    }

    fn spend_click(&mut self, side: Side) {
        self.st.player_mut(side).clicks -= 1;
        self.note_click_on_action(1);
        self.changes.record(GameChange::ClickSpent { side });
        self.changes.record(GameChange::CostPaid {
            side,
            credits: 0,
            clicks: 1,
            trashed: Vec::new(),
        });
        cite!("rule_checkpoint_after_paying_cost");
        self.checkpoint_and_react(None);
    }

    /// CR 9.5.7 step (a): announce, then push the shared resolution frame;
    /// the frame's PayCost phase performs step (b) so that cost-paid chain
    /// reactions resolve before the first instruction becomes imminent.
    fn trigger_paid_ability(&mut self, side: Side, ability: AbilityRef, def: AbilityDef) {
        cite!("step_paid_ability_announce");
        cite!("rule_paid_ability_independent");
        if def.has_flag(AbilityFlag::OncePerTurn) {
            self.once_per_turn_used.insert((ability, self.generation(ability.obj)));
        }
        let cost = def.cost.clone().unwrap_or_default();
        self.push_ability_frame_cost(
            ResolutionKind::Paid,
            ability,
            side,
            def.instructions.clone(),
            None,
            None,
            Some(cost),
        );
    }

    fn take_window_option(&mut self, side: Side, wid: u64, opt: WindowOption) {
        match opt {
            WindowOption::TriggerInstance { instance, .. } => {
                cite!("rule_triggering");
                cite!("step_conditional_ability_announce");
                let inst = self.instances.get_mut(&instance).expect("pending instance");
                // 9.6.8a: triggering removes pending status.
                cite!("rule_triggered_ability_loses_pending");
                inst.window = None;
                let def = inst.def.clone();
                let ability = inst.ability;
                let controller = inst.controller;
                // 9.6.13c: a delayed conditional with no stated duration
                // exists "until the next time it resolves" — and it is
                // resolving from the moment it is triggered, not from the
                // moment its last instruction has finished. The difference
                // matters whenever an instruction opens a nested timing
                // structure whose end would meet the condition again (a
                // delayed BREACH, 7.3.8, or a delayed run): reading the rule
                // at frame completion re-arms the effect from inside its own
                // resolution, forever.
                if let Some(lid) = inst.from_lingering {
                    cite!("rule_delayed_conditional_ability_relevant_once");
                    self.lingering
                        .retain(|l| !(l.id == lid && l.duration == Duration::UntilResolved));
                }
                self.push_ability_frame(
                    ResolutionKind::Conditional,
                    ability,
                    controller,
                    def.instructions.clone(),
                    Some(instance),
                    None,
                );
            }
            WindowOption::TriggerPaid { ability, .. } => {
                let def = self.st.objects[&ability.obj].face().abilities[ability.index].clone();
                self.trigger_paid_ability(side, ability, def);
            }
            WindowOption::Rez { card } | WindowOption::RezApproachedIce { card } => {
                self.rez_card(card);
                if let Some(Frame::Window(w)) = self.frames.last_mut() {
                    if w.id == wid {
                        w.option_resolved();
                    }
                }
            }
            WindowOption::Score { card } => {
                let cost = self.score_cost_of(card);
                if let Some(Frame::Window(w)) = self.frames.last_mut() {
                    if w.id == wid {
                        w.option_resolved();
                    }
                }
                if cost.is_free() {
                    self.score_agenda(card);
                    self.checkpoint_and_react(None);
                } else {
                    // 1.16.10a: an additional cost may be declined, and
                    // declining means the agenda is not scored.
                    cite!("rule_decline_additional_cost");
                    self.ask(
                        Side::Corp,
                        DecisionSpec::NestedCost { cost },
                        DecisionCtx::ScoreCost(card),
                    );
                }
            }
            WindowOption::BasicTrash { card, cost } => {
                cite!("rule_basic_trash_ability");
                self.begin_payment(
                    Side::Runner,
                    card,
                    &Cost::credits(cost),
                    PaymentCont::BasicTrash { card, window: wid },
                    None,
                );
            }
        }
    }
}

fn class_key(c: EffectClass) -> u64 {
    // Stable small hash for BTreeMap keys.
    match c {
        EffectClass::Damage(DamageKind::Meat) => 1,
        EffectClass::Damage(DamageKind::Net) => 2,
        EffectClass::Damage(DamageKind::Core) => 3,
        EffectClass::TakeTags => 4,
        EffectClass::GainCredits => 5,
        EffectClass::LoseCredits => 6,
        EffectClass::GainClicks => 7,
        EffectClass::LoseClicks => 8,
        EffectClass::Draw => 9,
        EffectClass::TrashCards => 10,
        EffectClass::EndTheRun => 11,
        EffectClass::Bypass => 12,
        EffectClass::StealAgenda => 13,
        EffectClass::Structural => 14,
        EffectClass::Breach => 15,
        EffectClass::AccessCard => 16,
        EffectClass::PayCost => 17,
    }
}

/// CR 9.12.2c: the effects that are aggregated when performed in a single
/// instruction — gaining, losing or spending credits or clicks; taking,
/// removing or preventing tags or bad publicity; looking at or revealing
/// cards from a specified location; drawing cards; trashing cards from
/// specified locations, including by damage; and shuffling cards from a
/// discard pile into a deck. Everything else is not on the list, and 9.12.2b
/// says one such effect is enough to stop the whole group aggregating.
fn instruction_aggregates(i: &Instruction) -> bool {
    cite!("rule_aggregated_instructions");
    match i {
        Instruction::GainCredits(..)
        | Instruction::LoseCredits(..)
        | Instruction::GainClicks(..)
        | Instruction::LoseClicks(..)
        | Instruction::Draw(..)
        | Instruction::GainTags(..)
        | Instruction::TakeBadPublicity { .. }
        | Instruction::RemoveCountersFromPlayer { .. }
        | Instruction::LookAtCards { .. }
        | Instruction::TrashCards(..)
        | Instruction::Damage { .. } => true,
        // A wrapper aggregates exactly when what it wraps does.
        Instruction::PerformedBy { instr, .. } | Instruction::DeclineableChoice(instr) => {
            instruction_aggregates(instr)
        }
        Instruction::Combined(list) => list.iter().all(instruction_aggregates),
        _ => false,
    }
}

/// CR 9.12.2b: "the values included in the effect aggregated according to the
/// calculated quantity" — the per-unit value of an aggregated effect scaled
/// by the quantity. Only the effects `instruction_aggregates` accepts are
/// ever scaled, and only the ones that carry a numeric value: a set-based
/// aggregated effect (trashing named cards, looking at named cards) has no
/// per-unit number to multiply.
fn scale_instruction(i: &Instruction, x: i64) -> Instruction {
    use crate::instr::Quantity as Q;
    let times = |q: &Q| Q::Times(x, Box::new(q.clone()));
    match i {
        Instruction::GainCredits(s, q) => Instruction::GainCredits(*s, times(q)),
        Instruction::GainClicks(s, q) => Instruction::GainClicks(*s, times(q)),
        Instruction::LoseClicks(s, q) => Instruction::LoseClicks(*s, times(q)),
        Instruction::LoseCredits(s, q) => Instruction::LoseCredits(*s, times(q)),
        Instruction::Draw(s, n) => Instruction::Draw(*s, (*n as i64 * x).max(0) as u32),
        Instruction::GainTags(n) => Instruction::GainTags((*n as i64 * x).max(0) as u32),
        Instruction::TakeBadPublicity { side, amount } => {
            Instruction::TakeBadPublicity { side: *side, amount: times(amount) }
        }
        Instruction::RemoveCountersFromPlayer { side, kind, amount } => {
            Instruction::RemoveCountersFromPlayer { side: *side, kind: *kind, amount: times(amount) }
        }
        Instruction::Damage { kind, amount, responsible } => {
            Instruction::Damage { kind: *kind, amount: times(amount), responsible: *responsible }
        }
        Instruction::PerformedBy { side, instr } => Instruction::PerformedBy {
            side: *side,
            instr: Box::new(scale_instruction(instr, x)),
        },
        Instruction::Combined(list) => {
            Instruction::Combined(list.iter().map(|i| scale_instruction(i, x)).collect())
        }
        other => other.clone(),
    }
}

/// CR 1.14.5: re-wrap a chosen option's instructions so the player named as
/// carrying out the choice also carries out what the choice produces.
fn wrap_all(instrs: Vec<Instruction>, by: Option<Side>) -> Vec<Instruction> {
    match by {
        None => instrs,
        Some(side) => instrs
            .into_iter()
            .map(|i| Instruction::PerformedBy { side, instr: Box::new(i) })
            .collect(),
    }
}

/// CR 1.15.2b/e: turn an answered target announcement into the announced
/// set. Targets outside the candidate list are not valid for the instruction
/// and are dropped; a target repeated in the answer is chosen once; the set
/// is capped at what the instruction asks for and completed from the
/// candidates when the answer is short of the floor ("chooses as many
/// distinct targets as possible").
fn clamp_announcement(spec: &DecisionSpec, answered: Vec<ObjectId>) -> Vec<ObjectId> {
    let DecisionSpec::ChooseTargets { candidates, count, min, .. } = spec else {
        return answered;
    };
    let mut out: Vec<ObjectId> = Vec::new();
    for c in answered {
        if candidates.contains(&c) && !out.contains(&c) && (out.len() as u32) < *count {
            out.push(c);
        }
    }
    for c in candidates {
        if (out.len() as u32) >= *min {
            break;
        }
        if !out.contains(c) {
            out.push(*c);
        }
    }
    out
}

/// CR 1.15.4: rewrite `TargetSpec::EarlierTarget` references into the
/// objects the ability actually announced. Used where an instruction creates
/// ANOTHER ability that refers to one of this ability's targets — the
/// reference has to be resolved at creation time, because the created
/// ability announces nothing of its own.
fn bind_targets(instr: Instruction, announced: &[ObjectId]) -> Instruction {
    fn bind_spec(s: TargetSpec, announced: &[ObjectId]) -> TargetSpec {
        match s {
            TargetSpec::EarlierTarget { nth } => match announced.get(nth) {
                Some(id) => TargetSpec::Objects(vec![*id]),
                // 1.15.3: an unannounced target is simply not acted on.
                None => TargetSpec::Objects(Vec::new()),
            },
            TargetSpec::Each(v) => {
                TargetSpec::Each(v.into_iter().map(|s| bind_spec(s, announced)).collect())
            }
            other => other,
        }
    }
    match instr {
        Instruction::TrashCards(s) => Instruction::TrashCards(bind_spec(s, announced)),
        Instruction::ShuffleCardsIntoDeck { targets, to } => {
            Instruction::ShuffleCardsIntoDeck { targets: bind_spec(targets, announced), to }
        }
        Instruction::RemoveCardsFromGame { targets } => {
            Instruction::RemoveCardsFromGame { targets: bind_spec(targets, announced) }
        }
        Instruction::PlaceCounters { target, kind, amount } => {
            Instruction::PlaceCounters { target: bind_spec(target, announced), kind, amount }
        }
        Instruction::ModifyStrength { target, amount, duration } => {
            Instruction::ModifyStrength { target: bind_spec(target, announced), amount, duration }
        }
        Instruction::Combined(v) => {
            Instruction::Combined(v.into_iter().map(|i| bind_targets(i, announced)).collect())
        }
        Instruction::PerformedBy { side, instr } => Instruction::PerformedBy {
            side,
            instr: Box::new(bind_targets(*instr, announced)),
        },
        other => other,
    }
}
