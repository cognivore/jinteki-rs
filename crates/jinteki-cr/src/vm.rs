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
use crate::instr::{Instruction, TargetFilter, TargetSpec};
use crate::lingering::{Duration, LingeringEffect, Payload};
use crate::change::{ChangeBuffer, GameChange};
use crate::object::{
    card_active, CardType, CounterKind, Object, ObjectId, PrintedCard, ServerId, Side, Zone,
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
}

/// Stable identity of one subroutine on a piece of ice: (category rank per
/// 9.8.2/9.8.3, source key, ordinal within that source). Category-d counts
/// shrink last-first (9.8.3d), which is exactly highest-ordinal-first here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubKey {
    pub category: u8,
    pub src: u64,
    pub ord: u32,
}

/// The pure game state (cloneable for the 9.6.6a snapshot).
#[derive(Debug, Clone)]
pub struct CoreState {
    pub objects: BTreeMap<ObjectId, Object>,
    pub deck: BTreeMap<Side, Vec<ObjectId>>,
    pub hand: BTreeMap<Side, Vec<ObjectId>>,
    pub discard: BTreeMap<Side, Vec<ObjectId>>,
    pub score_area: BTreeMap<Side, Vec<ObjectId>>,
    /// Ice per server, INNERMOST FIRST (position k = index k-1).
    pub ice: BTreeMap<ServerId, Vec<ObjectId>>,
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
    /// 10.8.6c/d trace spends.
    TraceSpend(Side),
    /// 10.14.6 sealed psi bids.
    PsiBid(Side),
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
    pub once_per_turn_used: HashSet<AbilityRef>,
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
    /// Run context mirror for conditions when the run frame is deep in the
    /// stack: (run_id, server, reached_success) while a run is in progress.
    pub current_run: Option<(u64, ServerId, bool)>,
    /// Trace of resolutions for tests: labels of resolved ability frames.
    pub resolution_log: Vec<String>,
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
            for _ in 0..5 {
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
            snapshot: None,
            last_scan_window: Vec::new(),
            last_minimal_sets: None,
            orphan_set_aside_counters: Vec::new(),
            set_aside_card_cleanup: Vec::new(),
            trace: None,
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
                zone,
                faceup: false,
                owner,
                controller: owner,
                host: None,
                hosted: Vec::new(),
                counters: BTreeMap::new(),
                active_since: 0,
                set_aside_for_ability: false,
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

    pub fn add_breach_candidate(&mut self, obj: ObjectId) {
        if let Some(b) = self.breach_ctx_mut() {
            if !b.candidates.contains(&obj) && !b.accessed.contains(&obj) {
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
                        self.push_imminent(instr, self.st.turn_side, Vec::new(), atoms);
                        if self.open_interrupt_window_if_relevant() {
                            self.set_structure_phase(StepPhase::Exec);
                            return; // window frame now on top
                        }
                        self.set_structure_phase(StepPhase::Exec);
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
                StepOp::Paw(classes) => {
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
            BranchPred::RunnerHasIcePosition => self
                .run_ctx()
                .map(|r| {
                    r.position
                        .map(|p| p >= 1 && p <= self.ice_at(r.server).len())
                        .unwrap_or(false)
                })
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
            BranchPred::CandidatesRemain => self
                .breach_ctx()
                .map(|b| !self.restrict_candidates(b.candidates.clone()).is_empty())
                .unwrap_or(false),
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

    pub fn ice_at(&self, server: ServerId) -> &[ObjectId] {
        self.st.ice.get(&server).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn approached_ice(&self, r: &RunCtx) -> Option<ObjectId> {
        let pos = r.position?;
        self.ice_at(r.server).get(pos - 1).copied()
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
            StepKind::EncounterComplete => Instruction::PassIce, // marker; exec handles
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
                // CR 1.10.5: recurring credits refill to the printed amount.
                cite!("rule_recurring_credits");
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
                    let n = self.st.objects[&id].printed.recurring_credits.unwrap();
                    self.st
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .counters
                        .insert(CounterKind::Credit, n);
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
                let n = self.ice_at(server).len();
                let r = self.run_ctx_mut().unwrap();
                r.position = if n > 0 { Some(n) } else { None };
            }
            StepKind::ApproachIce => {
                let (server, pos) = {
                    let r = self.run_ctx().unwrap();
                    (r.server, r.position)
                };
                if let Some(p) = pos {
                    if let Some(&ice) = self.ice_at(server).get(p - 1) {
                        self.changes.record(GameChange::IceApproached { ice });
                    }
                }
                if let Some(r) = self.run_ctx_mut() {
                    r.came_from_ice = true;
                }
            }
            StepKind::EncounterIce => {
                let ice = {
                    let r = self.run_ctx().unwrap();
                    self.approached_ice(r).expect("encounter requires approached ice")
                };
                self.begin_encounter(ice);
            }
            StepKind::EncounterComplete => {
                self.end_encounter();
            }
            StepKind::PassIce => {
                let (came, server, pos) = {
                    let r = self.run_ctx().unwrap();
                    (r.came_from_ice, r.server, r.position)
                };
                if came {
                    if let Some(p) = pos {
                        if let Some(&ice) = self.ice_at(server).get(p - 1) {
                            cite!("rule_pass_ice");
                            self.changes.record(GameChange::IcePassed { ice });
                        }
                    }
                }
            }
            StepKind::JackOutChoice => {
                cite!("rule_jack_out_after_passing_ice");
                self.ask(Side::Runner, DecisionSpec::JackOut, DecisionCtx::JackOut);
            }
            StepKind::MovePositionInward => {
                cite!("rule_position_progression");
                let r = self.run_ctx_mut().unwrap();
                match r.position {
                    Some(p) if p > 1 => {
                        r.position = Some(p - 1);
                        r.moved_to_new_position = true;
                    }
                    Some(_) => {
                        r.position = None;
                        r.moved_to_new_position = false;
                    }
                    None => r.moved_to_new_position = false,
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
                }
            }
            StepKind::BreachAttackedServer => {
                let server = self.run_ctx().unwrap().server;
                self.push_breach(server);
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
                let n = match server {
                    ServerId::Hq | ServerId::Rnd => 1,
                    _ => 0,
                };
                if let Some(b) = self.breach_ctx_mut() {
                    b.remaining_from_zone = n;
                }
                // Populate the first hand/deck candidate (7.4.6/7.4.7).
                self.refresh_candidates_after_access();
            }
            StepKind::AccessChosenCandidate => {
                let card = self.breach_ctx().unwrap().chosen.expect("candidate chosen");
                self.push_access(card);
            }
            StepKind::CardBecomesAccessed => {
                cite!("rule_accessing");
                let card = self.access_card().unwrap();
                self.st.accessed = Some(card);
                self.changes.record(GameChange::CardAccessed { obj: card });
            }
            StepKind::StealIfAgenda => {
                cite!("rule_after_mid_access_agenda");
                let card = self.access_card().unwrap();
                if self.st.objects[&card].printed.card_type == CardType::Agenda {
                    let total = self.steal_cost_of(card);
                    if total.is_free() {
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

    /// All (source object, declaration) pairs of active static abilities.
    pub fn active_statics(&self) -> Vec<(ObjectId, StaticDecl)> {
        cite!("rule_static_ability");
        let mut out = Vec::new();
        for o in self.st.objects.values() {
            for (i, a) in o.printed.abilities.iter().enumerate() {
                if a.kind != AbilityKind::Static {
                    continue;
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed)
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
        for o in self.st.objects.values() {
            if !card_active(o) {
                continue;
            }
            for a in &o.printed.abilities {
                if a.kind != AbilityKind::Static {
                    continue;
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
                        StaticDecl::RemoveHostAbilities => {
                            if let Some(h) = o.host {
                                out.push(CharEffect {
                                    source: o.id,
                                    target: h,
                                    op: CharOp::RemoveAllAbilities,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        for l in &self.lingering {
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

    // ------------------------------------------------------------------
    // Encounters and subroutines
    // ------------------------------------------------------------------

    fn begin_encounter(&mut self, ice: ObjectId) {
        cite!("rule_subroutines_initial_status_in_encounter");
        let id = self.next_encounter;
        self.next_encounter += 1;
        self.st.encounter = Some(EncounterState {
            id,
            ice,
            broken: std::collections::BTreeSet::new(),
            resolved: std::collections::BTreeSet::new(),
        });
        self.changes.record(GameChange::EncounterBegan { ice, encounter_id: id });
    }

    fn end_encounter(&mut self) {
        if let Some(e) = self.st.encounter.take() {
            self.changes.record(GameChange::EncounterEnded {
                ice: e.ice,
                encounter_id: e.id,
            });
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
        let mut befores: Vec<(u64, AbilityDef)> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::GrantedSubroutine { to, sub, before: true, seq } if *to == ice => {
                    Some((*seq, sub.clone()))
                }
                _ => None,
            })
            .collect();
        befores.sort_by(|x, y| y.0.cmp(&x.0));
        for (seq, def) in befores {
            out.push((SubKey { category: 1, src: seq, ord: 0 }, def));
        }
        // (c) printed, in printed order (9.8.3c), honoring 9.1.9 losses.
        cite!("rule_subroutine_origin_printed");
        for (i, a) in self.st.objects[&ice].printed.abilities.iter().enumerate() {
            if a.kind == AbilityKind::Subroutine && self.ability_present(ice, i) {
                out.push((SubKey { category: 3, src: 0, ord: i as u32 }, a.clone()));
            }
        }
        // (d) self-static count-linked (9.8.3d): Ashigaru class.
        cite!("rule_subroutine_origin_static_after");
        for a in &self.st.objects[&ice].printed.abilities {
            if a.kind != AbilityKind::Static {
                continue;
            }
            if !crate::object::card_active(&self.st.objects[&ice]) {
                continue;
            }
            for d in &a.statics {
                if let StaticDecl::GainSubroutinePerHqCard { sub } = d {
                    let n = self.st.hand[&Side::Corp].len() as u32;
                    for k in 0..n {
                        out.push((SubKey { category: 4, src: 0, ord: k }, (**sub).clone()));
                    }
                }
            }
        }
        // (e) external after/unspecified, oldest first (9.8.3e).
        cite!("rule_subroutine_origin_external_after");
        let mut afters: Vec<(u64, AbilityDef)> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::GrantedSubroutine { to, sub, before: false, seq } if *to == ice => {
                    Some((*seq, sub.clone()))
                }
                _ => None,
            })
            .collect();
        afters.sort_by(|x, y| x.0.cmp(&y.0));
        for (seq, def) in afters {
            out.push((SubKey { category: 5, src: seq, ord: 0 }, def));
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
        let run_id = self.next_run;
        self.next_run += 1;
        let id = self.next_structure;
        self.next_structure += 1;
        self.would.reset_scope(WouldScope::Run);
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
            }),
        }));
    }

    fn push_breach(&mut self, server: ServerId) {
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
                chosen: None,
                accessed: Vec::new(),
                remaining_from_zone: 0,
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
            ctx: StructCtx::Access(AccessCtx { card }),
        }));
    }

    /// CR 7.4: candidates per server type.
    fn compute_candidates(&mut self) {
        cite!("sec_determining_candidates");
        let server = self.breach_ctx().unwrap().server;
        let cands: Vec<ObjectId> = match server {
            ServerId::Remote(_) => {
                let mut v: Vec<ObjectId> =
                    self.st.root.get(&server).cloned().unwrap_or_default();
                // Ice protecting the server is not a candidate (7.4.1c-ish);
                // roots only in the kernel wave.
                v.reverse();
                v
            }
            ServerId::Archives => self.st.discard[&Side::Corp].clone(),
            ServerId::Rnd => Vec::new(),  // filled per-access from the top
            ServerId::Hq => Vec::new(),   // filled per-access at random
        };
        if let Some(b) = self.breach_ctx_mut() {
            b.candidates = cands;
        }
    }

    /// CR 7.4.2: apply active access prohibitions to a candidate list.
    fn restrict_candidates(&self, list: Vec<ObjectId>) -> Vec<ObjectId> {
        let only: Vec<ObjectId> = self
            .lingering
            .iter()
            .filter_map(|l| match &l.payload {
                Payload::RestrictCandidatesTo(x) => Some(*x),
                _ => None,
            })
            .collect();
        if only.is_empty() {
            list
        } else {
            cite!("rule_prohibiting_access");
            list.into_iter().filter(|c| only.contains(c)).collect()
        }
    }

    fn choose_candidate_body(&mut self) {
        cite!("step_choose_candidate");
        let b = self.breach_ctx().unwrap();
        let candidates = self.restrict_candidates(b.candidates.clone());
        if candidates.len() == 1 {
            let only = candidates[0];
            if let Some(b) = self.breach_ctx_mut() {
                b.chosen = Some(only);
                b.candidates.retain(|&c| c != only);
            }
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

    /// Candidates are recomputed as accesses happen: R&D reveals the next
    /// top card; HQ picks at random (CONVENTION → RNG per the digest §13).
    fn refresh_candidates_after_access(&mut self) {
        let server = match self.breach_ctx() {
            Some(b) => b.server,
            None => return,
        };
        match server {
            ServerId::Rnd => {
                cite!("rule_rnd_topmost_eligibile_candidate");
                let (remaining, accessed) = {
                    let b = self.breach_ctx().unwrap();
                    (b.remaining_from_zone, b.accessed.clone())
                };
                if remaining > 0 {
                    let top = self.st.deck[&Side::Corp]
                        .first()
                        .copied()
                        .filter(|c| !accessed.contains(c));
                    if let Some(b) = self.breach_ctx_mut() {
                        b.candidates = top.into_iter().collect();
                    }
                }
            }
            ServerId::Hq => {
                let (remaining, accessed) = {
                    let b = self.breach_ctx().unwrap();
                    (b.remaining_from_zone, b.accessed.clone())
                };
                if remaining > 0 {
                    let pool: Vec<ObjectId> = self.st.hand[&Side::Corp]
                        .iter()
                        .copied()
                        .filter(|c| !accessed.contains(c))
                        .collect();
                    let pick = if pool.is_empty() {
                        None
                    } else {
                        let i = self.rng.random_range(0..pool.len());
                        Some(pool[i])
                    };
                    if let Some(b) = self.breach_ctx_mut() {
                        b.candidates = pick.into_iter().collect();
                    }
                }
            }
            _ => {}
        }
    }

    fn steal_agenda(&mut self, card: ObjectId) {
        cite!("rule_score_steal");
        let points = self.st.objects[&card].printed.agenda_points.unwrap_or(0);
        self.move_card(card, Zone::ScoreArea(Side::Runner));
        self.st.objects.get_mut(&card).unwrap().faceup = true;
        self.changes.record(GameChange::AgendaStolen { obj: card, points });
    }

    /// CR 1.17.4-adjacent scoring via the (S) window option.
    fn score_agenda(&mut self, card: ObjectId) {
        cite!("rule_score");
        let points = self.st.objects[&card].printed.agenda_points.unwrap_or(0);
        self.move_card(card, Zone::ScoreArea(Side::Corp));
        self.st.objects.get_mut(&card).unwrap().faceup = true;
        self.changes.record(GameChange::AgendaScored { obj: card, points });
    }

    /// "End the run" (6.8.1/`rule_end_the_run`): unwind every frame above the
    /// run structure (windows drop their pendings — 9.2.8f/6.8.2), end any
    /// encounter, and jump to the Run Ends Phase.
    pub fn end_the_run(&mut self) {
        cite!("rule_end_the_run");
        let Some(run_pos) = self.frames.iter().rposition(|f| {
            matches!(f, Frame::Structure(StructureFrame { ctx: StructCtx::Run(_), .. }))
        }) else {
            // CR 6.1.4c: "end the run" with no run — if there is an
            // encounter, it ends; otherwise nothing.
            cite!("rule_end_run_no_run_or_encounter");
            self.end_encounter();
            return;
        };
        while self.frames.len() > run_pos + 1 {
            let f = self.frames.pop().unwrap();
            if let Frame::Window(w) = f {
                // 6.8.2/9.2.8f: open windows close; pendings die untriggered.
                cite!("rule_run_ends_close_paws");
                cite!("rule_run_ends_close_reaction_window");
                self.drop_window_pendings(&w);
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
        // The ETR instruction finished resolving: checkpoint (10.3.5).
        self.checkpoint_and_react(None);
    }

    fn complete_structure(&mut self) {
        let Some(Frame::Structure(sf)) = self.frames.pop() else { unreachable!() };
        match &sf.ctx {
            StructCtx::Turn { .. } => {
                // "…is complete, and the game moves to the other turn."
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
                    if b.remaining_from_zone > 0 {
                        b.remaining_from_zone -= 1;
                    }
                }
                self.refresh_candidates_after_access();
            }
        }
    }

    // ------------------------------------------------------------------
    // Imminence, expected effects, interrupt windows (§9.9)
    // ------------------------------------------------------------------

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
            Instruction::GainCredits(side, n) => {
                vec![EffectAtom::new(EffectClass::GainCredits, *n as i64, *side)]
            }
            Instruction::LoseCredits(side, n) => {
                vec![EffectAtom::new(EffectClass::LoseCredits, *n as i64, *side)]
            }
            Instruction::Draw(side, n) => {
                // 9.9.2: statics modify expected effects — a Lockdown-class
                // "cannot draw" removes the draw entirely.
                if self.draw_prohibited(*side) {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::Draw, *n as i64, *side)]
                }
            }
            Instruction::DamageUnpreventable { kind, amount, responsible } => {
                cite!("rule_static_modification_keep_restrictions");
                let mut v = *amount as i64;
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
                let mut v = *amount as i64;
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
            Instruction::GainCreditsPerCounter { kind, per } => {
                // CR 9.5.5: set-aside counters still count as hosted for
                // this ability. 9.12.2b/c: credits aggregate into ONE atom.
                cite!("rule_trash_ability_keeps_track_of_hosted_objects");
                cite!("rule_calculated_quantity");
                let on_card = source
                    .and_then(|s| self.st.objects.get(&s))
                    .map(|o| o.counter(*kind))
                    .unwrap_or(0);
                let set_aside: u32 = self
                    .frames
                    .iter()
                    .rev()
                    .find_map(|f| match f {
                        Frame::Ability(af) => Some(
                            af.set_aside_counters
                                .iter()
                                .filter(|(k, _)| k == kind)
                                .map(|(_, n)| *n)
                                .sum::<u32>(),
                        ),
                        _ => None,
                    })
                    .unwrap_or(0);
                let total = (on_card + set_aside) as i64 * *per as i64;
                vec![EffectAtom::new(EffectClass::GainCredits, total, controller)]
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
            | Instruction::PumpStrengthSelf { .. }
            | Instruction::BreakSubroutines { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::PlaceCounters { amount, .. } => {
                vec![EffectAtom::new(EffectClass::Structural, *amount as i64, controller)]
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
            Instruction::StealSelfAgenda => {
                vec![EffectAtom::new(EffectClass::StealAgenda, 1, controller)]
            }
            Instruction::MandatoryDraw => {
                if self.draw_prohibited(Side::Corp) {
                    vec![]
                } else {
                    vec![EffectAtom::new(EffectClass::Draw, 1, Side::Corp)]
                }
            }
            Instruction::ReplaceImminentDamageKind { .. } | Instruction::InitiateRun(_) => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::TraceInitiate { base } => {
                // 9.9.6d: the base trace strength is a modifiable value (it
                // need not be positive).
                cite!("rule_modifiable_value_base_trace_strength");
                vec![EffectAtom::new(EffectClass::Structural, *base, controller)]
            }
            Instruction::TraceCorpSpend
            | Instruction::TraceRunnerSpend
            | Instruction::TraceDetermine { .. }
            | Instruction::PsiGame { .. }
            | Instruction::GrantSubroutinesToSelf { .. }
            | Instruction::CorpDiscards { .. }
            | Instruction::RestrictAccessToSelf => {
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
    /// apply pre-existing replacement effects (9.9.9b).
    fn push_imminent(
        &mut self,
        instr: Instruction,
        controller: Side,
        targets: Vec<ObjectId>,
        mut atoms: Vec<EffectAtom>,
    ) {
        // CR 9.9.9b: active replacement effects apply as the window opens,
        // before pending interrupts are determined.
        cite!("rule_replacement_effects_apply_as_interrupt_window_opens");
        let seq = self.changes.next_group + 1_000_000; // distinct key-space
        let mut applied: Vec<u64> = Vec::new();
        for l in &mut self.lingering {
            if let Payload::ReplacementEffect { applies_to, replace_with } = &l.payload {
                // 9.9.9c: at most once per effect; 9.9.11a: must have
                // something to replace.
                cite!("rule_replacement_effect_only_applies_once_per_effect");
                cite!("rule_replacement_effect_must_have_something_to_replace");
                if l.applied_to.contains(&seq) {
                    continue;
                }
                let target = atoms.iter_mut().find(|a| a.expected() && a.class == *applies_to);
                if let Some(atom) = target {
                    match replace_with {
                        crate::lingering::ReplacementTransform::Suppress => atom.removed = true,
                        crate::lingering::ReplacementTransform::ChangeDamageKind(k) => {
                            if let EffectClass::Damage(_) = atom.class {
                                atom.class = EffectClass::Damage(*k);
                            }
                        }
                    }
                    l.applied_to.push(seq);
                    applied.push(l.id);
                }
            }
        }
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
            run_ordinal,
            turn_ordinal,
            seq,
        });
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
        for o in self.st.objects.values() {
            for (i, a) in o.printed.abilities.iter().enumerate() {
                if a.kind != AbilityKind::Conditional || !a.is_interrupt() {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed)
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
                    source_move_stamp: self.st.move_seq,
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
                TriggerCond::SelfWouldBeTrashed => {
                    // Harbinger class: relevant while the expected effects
                    // still include this source being trashed (9.9.4c).
                    return atoms.iter().any(|a| {
                        a.expected()
                            && a.class == EffectClass::TrashCards
                            && a.targets.contains(&source)
                    });
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
                Instruction::PreventDamage { kind, .. }
                | Instruction::PreventAllDamage { kind } => {
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
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.printed.abilities.iter().enumerate() {
                if a.kind != AbilityKind::Paid || !a.is_interrupt() {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed)
                {
                    continue;
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                if !self.cost_payable(side, o.id, a.cost.as_ref().unwrap_or(&Cost::default())) {
                    continue;
                }
                if a.has_flag(AbilityFlag::OncePerTurn)
                    && self.once_per_turn_used.contains(&AbilityRef { obj: o.id, index: i })
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
            imminent_index: None,
            instance,
            source_move_stamp: self.st.move_seq,
            any_expected_effects: false,
            subroutine_index,
            declined: false,
            cost,
            set_aside_counters: Vec::new(),
            set_aside_cards: Vec::new(),
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
                let (source, controller, cost) = {
                    let Some(Frame::Ability(af)) = self.frames.last_mut() else { unreachable!() };
                    af.phase = AbilityPhase::Targets;
                    (af.source, af.controller, af.cost.clone().unwrap_or_default())
                };
                // CR 9.5.5: if the trigger cost uninstalls the source, set
                // aside its hosted counters and cards as the cost is paid.
                // They still count as "hosted" for this ability and are
                // invisible to everything else (4.8.3).
                if cost.trash_self {
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
                self.pay_cost(controller, source.obj, &cost);
            }
            AbilityPhase::SubImminent => {
                cite!("step_subroutine_becomes_imminent");
                cite!("step_subroutine_interrupt_subroutine_resolution");
                // Kernel wave: no prevent-the-subroutine interrupts in the
                // vocabulary yet; the imminence point exists and proceeds.
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
                // the cost payments in (c)/(d)).
                if let Instruction::Trace { base, if_successful, if_unsuccessful, determined_min } =
                    &instr
                {
                    cite!("rule_steps_of_resolving_trace_attempt");
                    cite!("rule_not_timing_structures");
                    let (b, isucc, iunsucc, dmin) = (
                        *base,
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
                if let Some((side, spec)) = self.targets_needed(&instr) {
                    self.ask(side, spec, DecisionCtx::Targets);
                    return;
                }
                self.begin_imminence(instr);
            }
            AbilityPhase::Imminent => {
                // The interrupt window above us closed → resolve.
                self.set_ability_phase(AbilityPhase::Resolve);
            }
            AbilityPhase::Resolve => {
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
                self.checkpoint_and_react(None);
                let Some(Frame::Ability(af)) = self.frames.last_mut() else { return };
                af.idx += 1;
                af.phase = AbilityPhase::Targets;
            }
        }
    }

    fn set_ability_phase(&mut self, p: AbilityPhase) {
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.phase = p;
        }
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

    /// Compute targets that need a Decision (9.3.4b).
    fn targets_needed(&self, instr: &Instruction) -> Option<(Side, DecisionSpec)> {
        let Some(Frame::Ability(af)) = self.frames.last() else { return None };
        match instr {
            Instruction::TrashCards(TargetSpec::Choose { count, filter }) => {
                let candidates = self.filter_candidates(*filter, af.controller);
                Some((
                    af.controller,
                    DecisionSpec::ChooseTargets {
                        candidates,
                        count: *count,
                        up_to: false,
                    },
                ))
            }
            Instruction::NestedCostThen { cost, .. }
            | Instruction::NestedCostUnless { cost, .. } => {
                let (payer, _) = self.nested_cost_payer(instr);
                Some((payer, DecisionSpec::NestedCost { cost: cost.clone() }))
            }
            Instruction::MoveSetAsideCounters { target: TargetSpec::Choose { count, filter }, .. } => {
                let candidates = self.filter_candidates(*filter, af.controller);
                Some((
                    af.controller,
                    DecisionSpec::ChooseTargets { candidates, count: *count, up_to: false },
                ))
            }
            Instruction::DeclineableChoice(_) => Some((
                af.controller,
                DecisionSpec::OptionalEffect { label: "optional effect" },
            )),
            _ => None,
        }
    }

    fn filter_candidates(&self, f: TargetFilter, _controller: Side) -> Vec<ObjectId> {
        self.st
            .objects
            .values()
            .filter(|o| match f {
                TargetFilter::InstalledCorpCard => {
                    o.zone.is_installed() && is_corp_card(o.printed.card_type)
                }
                TargetFilter::InstalledRunnerCard => {
                    o.zone.is_installed() && !is_corp_card(o.printed.card_type)
                }
                TargetFilter::InstalledResource => {
                    o.zone == Zone::Rig && o.printed.card_type == CardType::Resource
                }
            })
            .map(|o| o.id)
            .collect()
    }

    /// Make the current instruction imminent: compute expected effects, open
    /// the interrupt window if relevant.
    fn begin_imminence(&mut self, instr: Instruction) {
        let (controller, targets) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (af.controller, af.targets.clone())
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
        self.push_imminent(instr, controller, targets, atoms);
        if let Some(Frame::Ability(af)) = self.frames.last_mut() {
            af.imminent_index = Some(0);
        }
        if !self.open_interrupt_window_if_relevant() {
            self.set_ability_phase(AbilityPhase::Resolve);
        }
    }

    /// Resolve the imminent instruction of the top ability frame.
    fn resolve_current_instruction(&mut self) {
        let imm = self.imminents.pop().expect("imminent instruction to resolve");
        self.changes.bump_group();
        let (frame_idx, controller, source, stamp) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (self.frames.len() - 1, af.controller, af.source, af.source_move_stamp)
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

    fn source_moved_since(&self, obj: ObjectId, _stamp: u64) -> bool {
        // Kernel-wave approximation: track via move stamps on the object.
        // Objects record moves by bumping st.move_seq; ability frames record
        // the seq at push. A finer per-object stamp arrives with the card
        // layer; for now compare zones recorded at frame push.
        let _ = obj;
        false
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
        match &instr {
            Instruction::GainCredits(side, _) => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let n = a.value.max(0) as u32;
                    self.st.player_mut(*side).credits += n;
                    self.changes.record(GameChange::CreditsGained { side: *side, amount: n });
                }
            }
            Instruction::LoseCredits(side, _) => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let have = self.st.player(*side).credits;
                    let n = (a.value.max(0) as u32).min(have);
                    self.st.player_mut(*side).credits -= n;
                    self.changes.record(GameChange::CreditsLost { side: *side, amount: n });
                }
            }
            Instruction::Draw(side, _) => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    self.draw_cards(*side, a.value.max(0) as u32, false);
                }
            }
            Instruction::Damage { responsible, .. } => {
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
            Instruction::Combined(_) => {
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
                    // 9.6.9d: an optional component was carried out — used.
                    cite!("rule_optional_conditional_ability_use");
                    self.changes.record(GameChange::AbilityUsed { source: source.obj });
                    let inner_imm = ImminentWrap {
                        instr: (**inner).clone(),
                        atoms: imm.atoms.clone(),
                        controller,
                        targets: imm.targets.clone(),
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
            Instruction::GainCreditsPerCounter { .. } => {
                for a in imm.atoms.iter().filter(|a| a.occurs_at_resolution()) {
                    let n = a.value.max(0) as u32;
                    self.st.player_mut(controller).credits += n;
                    self.changes.record(GameChange::CreditsGained { side: controller, amount: n });
                }
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
                self.modify_parent_imminent(|atom| {
                    if atom.class == EffectClass::Damage(*kind) {
                        atom.prevent(*amount as i64);
                        true
                    } else {
                        false
                    }
                });
            }
            Instruction::PreventAllDamage { kind } => {
                self.modify_parent_imminent(|atom| {
                    if atom.class == EffectClass::Damage(*kind) {
                        atom.prevent_all();
                        true
                    } else {
                        false
                    }
                });
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
            Instruction::InitiateRun(server) => {
                cite!("rule_run_timing_structure");
                self.initiate_run(*server);
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
            Instruction::GrantSubroutinesToSelf { count, sub, before } => {
                // 9.8.3a/e: externally-granted subroutines, ordered by grant
                // time within their category; they arrive unbroken (9.8.4b).
                cite!("rule_subroutine_origins");
                let dur = crate::lingering::bind_duration(
                    crate::lingering::WantedDuration::ThisEncounter,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                for _ in 0..*count {
                    let id = self.next_lingering;
                    self.next_lingering += 1;
                    self.lingering.push(LingeringEffect {
                        id,
                        source: source.obj,
                        payload: Payload::GrantedSubroutine {
                            to: source.obj,
                            sub: (**sub).clone(),
                            before: *before,
                            seq: id,
                        },
                        duration: dur,
                        applied_to: Vec::new(),
                    });
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
                self.lingering.push(LingeringEffect {
                    id,
                    source: source.obj,
                    payload: Payload::RestrictCandidatesTo(source.obj),
                    duration: dur,
                    applied_to: Vec::new(),
                });
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
                cite!("rule_bypass_during_encounter");
                self.end_encounter();
                // The run proceeds to the Movement Phase (6.5.8): jump the
                // run frame to step_pass_ice.
                let idx = self
                    .table(crate::timing::StructKind::Run)
                    .index_of("step_pass_ice");
                for f in self.frames.iter_mut().rev() {
                    if let Frame::Structure(sf) = f {
                        if matches!(sf.ctx, StructCtx::Run(_)) {
                            sf.pending_jump = Some(idx);
                            break;
                        }
                    }
                }
            }
            Instruction::PumpStrengthSelf { amount } => {
                // 9.10.4a: implicit duration = remainder of the current
                // encounter.
                cite!("rule_icebreaker_strength_increase_implicit_link");
                let dur = crate::lingering::bind_duration(
                    crate::lingering::WantedDuration::ThisEncounter,
                    self.st.encounter.as_ref().map(|e| e.id),
                    self.current_run.map(|(r, _, _)| r),
                    self.st.turn_seq,
                );
                let id = self.next_lingering;
                self.next_lingering += 1;
                self.lingering.push(LingeringEffect {
                    id,
                    source: source.obj,
                    payload: Payload::StrengthMod { target: source.obj, delta: *amount },
                    duration: dur,
                    applied_to: Vec::new(),
                });
            }
            Instruction::BreakSubroutines { count } => {
                cite!("rule_break_subroutine");
                cite!("rule_unbroken_subroutines_target_for_break_abilities");
                if let Some(ice) = self.st.encounter.as_ref().map(|e| e.ice) {
                    let subs = self.current_subs(ice);
                    let broken_now: Vec<SubKey> = {
                        let e = self.st.encounter.as_ref().unwrap();
                        subs.iter()
                            .filter(|(k, _)| !e.broken.contains(k))
                            .take(*count as usize)
                            .map(|(k, _)| *k)
                            .collect()
                    };
                    if let Some(e) = self.st.encounter.as_mut() {
                        for k in broken_now {
                            e.broken.insert(k);
                        }
                    }
                }
            }
            Instruction::PlaceCounters { target, kind, amount } => {
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
                for t in targets {
                    let obj = self.st.objects.get_mut(&t).unwrap();
                    *obj.counters.entry(*kind).or_insert(0) += amount;
                    self.changes.record(GameChange::CounterPlaced {
                        obj: t,
                        kind: *kind,
                        amount: *amount,
                    });
                }
            }
            Instruction::StealSelfAgenda => {
                if !source_moved {
                    self.steal_agenda(source.obj);
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
            TargetSpec::Choose { .. } => announced.to_vec(),
            TargetSpec::TopOfDeck(side, n) => self.st.deck[side]
                .iter()
                .take(*n as usize)
                .copied()
                .collect(),
        }
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
        if !af.instructions.is_empty() {
            let label = self.st.objects.get(&af.source.obj)
                .map(|o| o.printed.name)
                .unwrap_or("?");
            self.resolution_log.push(format!("{label}#{}", af.source.index));
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
                    self.window_pass();
                } else {
                    self.ask(
                        Side::Runner,
                        DecisionSpec::MidAccessWindow { options },
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
            self.drop_window_pendings(&w);
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
                out.push(ActionOption::BasicRun { server });
            }
            if self.st.runner.tags > 0 && self.st.runner.credits >= 2 {
                cite!("runner_basic_action_remove_tag");
                out.push(ActionOption::BasicRemoveTag);
            }
        }
        // Card actions ([click]-cost paid abilities, 5.2.1).
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.printed.abilities.iter().enumerate() {
                if !a.is_action() {
                    continue;
                }
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed)
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

    fn paid_window_options(&self, side: Side, classes: PawClasses) -> Vec<WindowOption> {
        cite!("rule_paid_ability_window_options");
        let mut out = Vec::new();
        // (P): regular paid abilities (not actions/interrupts/mid-access).
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.printed.abilities.iter().enumerate() {
                if a.kind != AbilityKind::Paid
                    || a.is_action()
                    || a.is_interrupt()
                    || a.has_flag(AbilityFlag::Access)
                {
                    continue;
                }
                cite!("rule_other_paid_abilities");
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed)
                {
                    continue;
                }
                if !self.ability_present(o.id, i) {
                    continue;
                }
                // 9.5.6: effect-based timing restrictions.
                match a.timing {
                    Some(crate::ability::TimingRestriction::EncounterOnly) => {
                        cite!("rule_paid_ability_refers_to_encountered_ice");
                        if self.st.encounter.is_none() {
                            continue;
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
                    && self.once_per_turn_used.contains(&AbilityRef { obj: o.id, index: i })
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
                    && self.st.corp.credits >= o.printed.cost.unwrap_or(0)
                {
                    out.push(WindowOption::Rez { card: o.id });
                }
            }
        }
        if side == Side::Corp && classes.rez_approached_ice {
            cite!("rule_paid_ability_window_corp_rez_ice");
            if let Some(r) = self.run_ctx() {
                if let Some(ice) = self.approached_ice(r) {
                    let o = &self.st.objects[&ice];
                    if !o.faceup && self.st.corp.credits >= o.printed.cost.unwrap_or(0) {
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
                    && o.counter(CounterKind::Advancement)
                        >= o.printed.advancement_requirement.unwrap_or(u32::MAX)
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

    fn mid_access_options(&self) -> Vec<WindowOption> {
        cite!("rule_mid_access_window_options");
        let mut out = Vec::new();
        let Some(card) = self.st.accessed else { return out };
        let o = &self.st.objects[&card];
        // 7.1.5: the basic trash ability — pay the trash cost, trash it.
        if let Some(tc) = o.printed.trash_cost {
            cite!("rule_basic_trash_ability");
            if self.st.runner.credits + self.st.bp_fund >= tc {
                out.push(WindowOption::BasicTrash { card, cost: tc });
            }
        }
        // Access-flagged paid abilities (9.3.6b).
        for src in self.st.objects.values() {
            if src.controller != Side::Runner {
                continue;
            }
            for (i, a) in src.printed.abilities.iter().enumerate() {
                if a.kind == AbilityKind::Paid
                    && a.has_flag(AbilityFlag::Access)
                    && ability_active(src, a, None, self.st.accessed)
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

    // ------------------------------------------------------------------
    // Costs (§1.16)
    // ------------------------------------------------------------------

    /// CR 1.16.1: a cost must be payable all at once.
    pub fn cost_payable(&self, side: Side, source: ObjectId, cost: &Cost) -> bool {
        cite!("rule_cost");
        let p = self.st.player(side);
        let credits_avail = p.credits
            + if side == Side::Runner && self.current_run.is_some() {
                self.st.bp_fund
            } else {
                0
            };
        if credits_avail < cost.credits || p.clicks < cost.clicks {
            return false;
        }
        if cost.trash_self && !self.st.objects[&source].zone.is_installed() {
            return false;
        }
        // CR 1.16.1b: if a static ability or a MANDATORY conditional
        // interrupt would prevent the steps of payment, the cost cannot be
        // paid (Jesminder vs Funhouse's take-a-tag nested cost).
        if cost.tags > 0 && self.tag_cost_blocked() {
            cite!("rule_cost_interrupt_static_mandatory");
            return false;
        }
        true
    }

    /// CR 1.10.3c-adjacent: credits a player can actually spend — pool plus
    /// hosted credits on cards that allow spending them, minus prohibitions
    /// (RSVP class → 0).
    pub fn spendable_credits(&self, side: Side) -> u32 {
        if self
            .active_statics()
            .iter()
            .any(|(_, d)| matches!(d, StaticDecl::CannotSpendCredits(s) if *s == side))
        {
            cite!("rule_bid_possible");
            return 0;
        }
        let hosted: u32 = self
            .st
            .objects
            .values()
            .filter(|o| o.controller == side && card_active(o) && o.printed.hosted_credits_spendable)
            .map(|o| o.counter(CounterKind::Credit))
            .sum();
        self.st.player(side).credits + hosted
    }

    /// 10.14.6b + 10.14.3: legal Psi bids — 0, 1, or 2, capped by what the
    /// player can actually spend; 0 is always legal.
    pub fn psi_legal_bids(&self, side: Side) -> Vec<u32> {
        cite!("rule_psi_bid_options");
        cite!("rule_bid_possible");
        let max = self.spendable_credits(side).min(2);
        (0..=max).collect()
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
                    self.st
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .counters
                        .insert(CounterKind::Credit, have - take);
                    self.changes.record(GameChange::AbilityUsed { source: id });
                    n -= take;
                }
            }
        }
    }

    /// Would an active MANDATORY interrupt avoid a tag the Runner takes now?
    fn tag_cost_blocked(&self) -> bool {
        for o in self.st.objects.values() {
            for (i, a) in o.printed.abilities.iter().enumerate() {
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
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed)
                    || !self.ability_present(o.id, i)
                {
                    continue;
                }
                return true;
            }
        }
        false
    }

    /// Pay a cost; CR 1.16.3/10.3.4: a checkpoint occurs immediately after —
    /// zero costs included (1.16.1d).
    pub fn pay_cost(&mut self, side: Side, source: ObjectId, cost: &Cost) {
        cite!("rule_cost_zero");
        cite!("rule_checkpoint_after_paying_cost");
        let mut credits_to_pay = cost.credits;
        // Bad publicity fund credits spend first during runs (6.4.3-ish).
        if side == Side::Runner && self.current_run.is_some() && self.st.bp_fund > 0 {
            cite!("rule_bad_publicity_fund");
            let from_fund = credits_to_pay.min(self.st.bp_fund);
            self.st.bp_fund -= from_fund;
            credits_to_pay -= from_fund;
        }
        {
            let p = self.st.player_mut(side);
            p.credits -= credits_to_pay;
            p.clicks -= cost.clicks;
        }
        let mut trashed = Vec::new();
        if cost.trash_self {
            self.trash_card(source, side);
            trashed.push(source);
            self.changes.record(GameChange::TrashAbilityUsed { source, side });
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
            self.changes.record(GameChange::ClickSpent { side });
        }
        self.changes.record(GameChange::CostPaid {
            side,
            credits: cost.credits,
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
    pub fn do_damage(&mut self, kind: DamageKind, amount: u32, _responsible: Side) {
        cite!("rule_meat_net_damage");
        if amount == 0 {
            return;
        }
        let grip_len = self.st.hand[&Side::Runner].len() as u32;
        if amount > grip_len {
            cite!("rule_flatline");
            self.game_over = Some(GameResult::Flatline);
        }
        let mut trashed = Vec::new();
        for _ in 0..amount.min(grip_len) {
            let hand = &self.st.hand[&Side::Runner];
            let i = self.rng.random_range(0..hand.len());
            let card = hand[i];
            self.move_card(card, Zone::Discard(Side::Runner));
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
        for v in self.st.ice.values_mut() {
            v.retain(|&c| c != id);
        }
        for v in self.st.root.values_mut() {
            v.retain(|&c| c != id);
        }
        match to {
            Zone::Deck(s) => self.st.deck.get_mut(&s).unwrap().push(id),
            Zone::Hand(s) => self.st.hand.get_mut(&s).unwrap().push(id),
            Zone::Discard(s) => self.st.discard.get_mut(&s).unwrap().push(id),
            Zone::ScoreArea(s) => self.st.score_area.get_mut(&s).unwrap().push(id),
            Zone::Ice(s) => self.st.ice.entry(s).or_default().push(id),
            Zone::Root(s) => self.st.root.entry(s).or_default().push(id),
            _ => {}
        }
        self.st.move_seq += 1;
        let o = self.st.objects.get_mut(&id).unwrap();
        o.zone = to;
        self.changes.record(GameChange::CardMoved { obj: id, from, to });
        if from.is_installed() && !to.is_installed() {
            self.changes.record(GameChange::CardUninstalled { obj: id, was_zone: from });
        }
        // Mid-breach root entries (10.3.1j).
        if let Zone::Root(server) = to {
            self.changes.record(GameChange::CardEnteredRoot { obj: id, server });
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
                    self.lingering.push(LingeringEffect {
                        id: lid,
                        source: id,
                        payload: Payload::PersistedAbility { def, run_id },
                        duration: Duration::PersistUntilAfterRun(run_id),
                        applied_to: Vec::new(),
                    });
                }
            }
        }
        self.changes.record(GameChange::CardTrashed { obj: id, by, was_zone: was });
        self.move_card(id, Zone::Discard(owner));
    }

    /// Rez: pay cost (checkpoint per 8.1.2e), turn faceup, active stamp.
    pub fn rez_card(&mut self, id: ObjectId) {
        cite!("rule_rez_procedure");
        let cost = Cost::credits(self.st.objects[&id].printed.cost.unwrap_or(0));
        self.pay_cost(Side::Corp, id, &cost);
        let seq = {
            self.st.active_seq += 1;
            self.st.active_seq
        };
        let o = self.st.objects.get_mut(&id).unwrap();
        o.faceup = true;
        o.active_since = seq;
        self.changes.record(GameChange::CardRezzed { obj: id });
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
            let orig = originating_structure.or_else(|| {
                let last_changes: Vec<&GameChange> = self
                    .changes
                    .log
                    .iter()
                    .rev()
                    .take(12)
                    .collect();
                last_changes.iter().find_map(|c| match c {
                    GameChange::EncounterBegan { encounter_id, .. } => Some(*encounter_id),
                    _ => None,
                })
            });
            self.open_reaction_window(newly, orig);
        }
    }

    // ------------------------------------------------------------------
    // Answers
    // ------------------------------------------------------------------

    fn apply_answer(&mut self) {
        let (side, _spec, ctx) = self.pending_decision.take().unwrap();
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
                    for _ in 0..5 {
                        self.draw_card_silent(s);
                    }
                }
                self.setup = match s {
                    Side::Corp => SetupPhase::RunnerMulligan,
                    Side::Runner => SetupPhase::Done,
                };
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
                if let Some(b) = self.breach_ctx_mut() {
                    b.chosen = Some(c);
                    b.candidates.retain(|&x| x != c);
                }
                self.set_structure_phase(StepPhase::Checkpoint);
            }
            (DecisionCtx::Targets, DecisionAnswer::Targets(t)) => {
                if let Some(Frame::Ability(af)) = self.frames.last_mut() {
                    af.targets = t;
                    let instr = af.instructions[af.idx].clone();
                    self.begin_imminence(instr);
                }
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
            ActionOption::CardAction { ability, .. } => {
                let def = self.st.objects[&ability.obj].printed.abilities[ability.index].clone();
                self.trigger_paid_ability(side, ability, def);
            }
        }
    }

    fn spend_click(&mut self, side: Side) {
        self.st.player_mut(side).clicks -= 1;
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
            self.once_per_turn_used.insert(ability);
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
                let def = self.st.objects[&ability.obj].printed.abilities[ability.index].clone();
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
                self.score_agenda(card);
                self.checkpoint_and_react(None);
                if let Some(Frame::Window(w)) = self.frames.last_mut() {
                    if w.id == wid {
                        w.option_resolved();
                    }
                }
            }
            WindowOption::BasicTrash { card, cost } => {
                cite!("rule_basic_trash_ability");
                self.pay_cost(Side::Runner, card, &Cost::credits(cost));
                self.trash_card(card, Side::Runner);
                if let Some(Frame::Window(w)) = self.frames.last_mut() {
                    if w.id == wid {
                        w.option_resolved();
                    }
                }
                self.changes.record(GameChange::TrashAbilityUsed {
                    source: card,
                    side: Side::Runner,
                });
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
    }
}
