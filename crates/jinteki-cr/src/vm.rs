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
pub use crate::ability::SubKey;
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
    /// 10.3.1j: the Runner declares candidacy of a mid-breach root entry.
    BreachCandidacy(ObjectId),
    /// 8.5.13d/1.16.4c: pay or decline the additional rez cost during an
    /// "install and rez" effect.
    RezAdditionalCost,
    /// 9.9.11: choose the order in which replacement effects apply.
    ReplacementOrder,
    /// 7.4.3 example 2 (Gagarin class): pay or decline an additional cost
    /// to access the chosen candidate.
    AccessCost(ObjectId),
    /// 10.12.2: the Corp chooses which cards to trash from HQ for a
    /// "sabotage N"; the remainder comes off the top of R&D.
    Sabotage { count: u32 },
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
    /// Ice-position insertion index (innermost-first) for inward installs.
    pub ice_insert_at: Option<usize>,
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
    /// In-progress installations (8.5.16), innermost last. Installing is a
    /// procedure (9.2.2e); nested installs stack.
    pub installs: Vec<InstallProgress>,
    /// In-progress plays (8.6.7), innermost last.
    pub plays: Vec<PlayProgress>,
    /// 10.3.1j: mid-breach root entries awaiting the Runner's candidacy
    /// declaration.
    pub pending_candidacy: Vec<ObjectId>,
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
        /// CR 1.15.1: announced SUBROUTINE targets (9.8.6).
        pub sub_targets: Vec<crate::ability::SubKey>,
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
            installs: Vec::new(),
            plays: Vec::new(),
            pending_candidacy: Vec::new(),
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
                hosted_not_installed: false,
                counters: BTreeMap::new(),
                active_since: 0,
                set_aside_for_ability: false,
                staged: false,
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
                        let asked =
                            self.push_imminent(instr, self.st.turn_side, Vec::new(), Vec::new(), atoms);
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
                        if b.remaining_from_zone > 0 {
                            b.remaining_from_zone -= 1;
                        }
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
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            for (i, a) in o.printed.abilities.iter().enumerate() {
                if a.kind != AbilityKind::Static {
                    continue;
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
                        StaticDecl::RemoveHostAbilities => {
                            if let Some(h) = o.host {
                                out.push(CharEffect {
                                    source: o.id,
                                    target: h,
                                    op: CharOp::RemoveAllAbilities,
                                });
                            }
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

    fn begin_encounter(&mut self, ice: ObjectId) {
        cite!("rule_subroutines_initial_status_in_encounter");
        let id = self.next_encounter;
        self.next_encounter += 1;
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

    /// The candidates as of RIGHT NOW. Archives candidates derive
    /// continuously from the discard pile (7.4.6d: cards entering Archives
    /// during the breach become candidates), excluding everything already
    /// chosen (7.4.3) or declined (7.4.6a); other servers track a
    /// maintained list.
    fn breach_candidates_now(&self) -> Vec<ObjectId> {
        let Some(b) = self.breach_ctx() else { return Vec::new() };
        match b.server {
            ServerId::Archives => {
                cite!("rule_candidates_entering_archives");
                self.st.discard[&Side::Corp]
                    .iter()
                    .copied()
                    .filter(|c| !b.chosen_ever.contains(c) && !b.declined.contains(c))
                    .collect()
            }
            _ => b.candidates.clone(),
        }
    }

    fn choose_candidate_body(&mut self) {
        cite!("step_choose_candidate");
        let candidates = self.restrict_candidates(self.breach_candidates_now());
        if candidates.len() == 1 {
            let only = candidates[0];
            if let Some(b) = self.breach_ctx_mut() {
                b.chosen = Some(only);
                // 7.4.3: a chosen candidate ceases to be one for the
                // remainder of the breach, accessed or not.
                cite!("rule_candidates_already_accessed");
                b.chosen_ever.push(only);
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

    /// Candidates are recomputed as accesses happen: R&D presents the
    /// TOPMOST ELIGIBLE card (7.4.7a — eligible = not already chosen, not
    /// prohibited); HQ picks at random (CONVENTION → RNG per the digest
    /// §13).
    pub fn refresh_candidates_after_access(&mut self) {
        let server = match self.breach_ctx() {
            Some(b) => b.server,
            None => return,
        };
        match server {
            ServerId::Rnd => {
                cite!("rule_rnd_candidates_1_at_a_time");
                cite!("rule_rnd_topmost_eligibile_candidate");
                let (remaining, chosen_ever) = {
                    let b = self.breach_ctx().unwrap();
                    (b.remaining_from_zone, b.chosen_ever.clone())
                };
                if remaining > 0 {
                    // All deck cards cease to be candidates, then the
                    // topmost eligible one becomes the candidate.
                    let top = self.st.deck[&Side::Corp]
                        .iter()
                        .copied()
                        .find(|c| !chosen_ever.contains(c));
                    if let Some(b) = self.breach_ctx_mut() {
                        b.candidates = top.into_iter().collect();
                    }
                }
            }
            ServerId::Hq => {
                let (remaining, chosen_ever) = {
                    let b = self.breach_ctx().unwrap();
                    (b.remaining_from_zone, b.chosen_ever.clone())
                };
                if remaining > 0 {
                    let pool: Vec<ObjectId> = self.st.hand[&Side::Corp]
                        .iter()
                        .copied()
                        .filter(|c| !chosen_ever.contains(c))
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
            Q::CountersOnSource(kind) => {
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
            Q::Plus(a, b) => self.eval_quantity(a, source) + self.eval_quantity(b, source),
            Q::Times(n, inner) => n * self.eval_quantity(inner, source),
            Q::XOfSource(inner) => {
                // CR 9.12.2e: X is defined by an ability of the source; while
                // that defining ability is inactive (source in Archives —
                // the ZATO example) or lost, X is treated as 0.
                cite!("rule_values_defined_by_x");
                let defined = source.is_some_and(|s| {
                    self.st.objects.get(&s).is_some_and(|o| {
                        card_active(o)
                            && o.printed.abilities.iter().enumerate().any(|(i, a)| {
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
            TargetFilter::IceProtectingSourceServer => source
                .and_then(|s| self.st.objects.get(&s))
                .and_then(|o| match o.zone {
                    Zone::Ice(sv) => Some(self.ice_at(sv).len() as i64),
                    _ => None,
                })
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
            | Instruction::ModifyStrength { .. }
            | Instruction::ModifySubtypes { .. }
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
            | Instruction::HostCards { .. }
            | Instruction::SwapCards { .. } => {
                vec![EffectAtom::new(EffectClass::Structural, 1, controller)]
            }
            Instruction::RemoveCountersFromPlayer { side, amount, .. } => {
                let n = self.eval_quantity(amount, source);
                vec![EffectAtom::new(EffectClass::Structural, n, *side)]
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

    /// Apply one replacement effect to the top imminence (marks it applied
    /// for this effect — 9.9.9c).
    fn apply_replacement(&mut self, lid: u64) {
        let Some(imm_seq) = self.imminents.last().map(|i| i.seq) else { return };
        let Some(l) = self.lingering.iter_mut().find(|l| l.id == lid) else { return };
        let Payload::ReplacementEffect { applies_to, replace_with } = &l.payload else { return };
        let applies_to = *applies_to;
        let replace_with = replace_with.clone();
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
        }
        // Kernel-wave replacements are one-shot effects (Security Testing,
        // Account Siphon, Showing Off, Immolation Script): applying consumes
        // the lingering effect. Multi-application replacement durations
        // arrive with the card layer.
        self.lingering.retain(|l| l.id != lid);
    }

    /// Apply replacements one at a time; when several could apply, the order
    /// is a Decision (9.9.11: the base effect's controller chooses).
    /// Returns `true` if a Decision was asked.
    fn resolve_replacements_or_ask(&mut self) -> bool {
        loop {
            let appl = self.applicable_replacements();
            match appl.len() {
                0 => return false,
                1 => self.apply_replacement(appl[0]),
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
            for (i, a) in o.printed.abilities.iter().enumerate() {
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
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.printed.abilities.iter().enumerate() {
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
            sub_targets: Vec::new(),
            announce_slot: 0,
            ability_targets: Vec::new(),
            imminent_index: None,
            instance,
            source_move_stamp: self.st.move_seq,
            any_expected_effects: false,
            subroutine_index,
            declined: false,
            cost,
            set_aside_counters: Vec::new(),
            set_aside_cards: Vec::new(),
            found_cards: Vec::new(),
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
                // the cost payments in (c)/(d)). The base quantity selector
                // is evaluated HERE, when the trace initiates (9.12.2e:
                // X-based traces read X at initiation; an orphaned XOfSource
                // selector yields 0 — the ZATO example).
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
                // 1.15.2: the next instruction announces its own targets
                // from scratch; 1.15.4 keeps the ability-wide list.
                af.targets.clear();
                af.sub_targets.clear();
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
            Instruction::LoseCredits(side, n) => self.st.player(*side).credits >= *n,
            Instruction::TrashCards(TargetSpec::Choose { count, criteria }) => {
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
            Instruction::TrashCards(spec)
            | Instruction::AccessCards { cards: spec }
            | Instruction::ModifySubtypes { target: spec, .. } => {
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
            Instruction::NestedCostThen { cost, .. }
            | Instruction::NestedCostUnless { cost, .. } => {
                let (payer, _) = self.nested_cost_payer(instr);
                Some((payer, DecisionSpec::NestedCost { cost: cost.clone() }))
            }
            Instruction::MoveSetAsideCounters {
                target: TargetSpec::Choose { count, criteria }, ..
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
            Instruction::HostCards { cards: TargetSpec::Choose { count, criteria }, .. }
            | Instruction::AddCardsToHand { cards: TargetSpec::Choose { count, criteria } } => {
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
                card: TargetSpec::Choose { count, criteria },
                ..
            } => {
                let candidates = self.filter_candidates(criteria, af.controller);
                let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
                Some((af.controller, self.announcement(candidates, want)))
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
    pub fn swap_cards(&mut self, x: ObjectId, y: ObjectId) {
        cite!("rule_swap_installed_cards");
        cite!("rule_swap_installed_cards_preserves_hosting");
        cite!("rule_swap_score_areas");
        let (zx, zy) = (self.st.objects[&x].zone, self.st.objects[&y].zone);
        if zx == zy {
            return;
        }
        // Simultaneous exchange: neither card's own move can be observed
        // trashing what is hosted on it (8.8.3a), so the host relationships
        // are simply carried along with the cards.
        self.move_card(x, zy);
        self.move_card(y, zx);
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
        o.printed.abilities.iter().any(|a| {
            a.instructions.iter().any(|i| {
                matches!(i, Instruction::HostCards { host: TargetSpec::SelfSource, .. })
            })
        })
    }

    /// Does `host`'s hosting declaration accept `installee`, with room left
    /// (1.13.5: any number unless the ability says otherwise)?
    fn host_accepts(&self, host: &Object, installee: &Object) -> bool {
        host.printed.abilities.iter().enumerate().any(|(i, a)| {
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
                .and_then(|s| self.st.objects.get(&s))
                .and_then(|src| match src.zone {
                    Zone::Ice(sv) => Some(self.ice_at(sv).contains(&o.id)),
                    _ => None,
                })
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
            // CR 4.2.2: the deck is ordered; "the top N cards" are its first
            // N, and a card must still be there to be a valid target.
            TargetFilter::TopOfDeckOf { side, n } => {
                cite!("rule_deck_ordered");
                self.st.deck[&side].iter().take(n as usize).any(|c| *c == o.id)
            }
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
        let (count, criteria) = match spec {
            TargetSpec::Choose { count, criteria } if slot == 0 => (count, criteria),
            TargetSpec::Each(specs) => match specs.get(slot) {
                Some(TargetSpec::Choose { count, criteria }) => (count, criteria),
                Some(_) | None => return None,
            },
            _ => return None,
        };
        cite!("rule_announce_targets");
        let mut candidates = self.filter_candidates(criteria, af.controller);
        candidates.retain(|c| !af.targets.contains(c));
        let want = self.eval_quantity(count, Some(af.source.obj)).max(0) as u32;
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
            _ => {}
        }
        let (controller, targets, sub_targets) = {
            let Some(Frame::Ability(af)) = self.frames.last() else { unreachable!() };
            (af.controller, af.targets.clone(), af.sub_targets.clone())
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
        let asked = self.push_imminent(instr, controller, targets, sub_targets, atoms);
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
        let Instruction::InstallCard { card, dest, and_rez, ignore_costs, reveal_check } = instr
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
            ice_insert_at: None,
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
        ignore_costs
            || self.cost_payable(
                o.printed.side,
                card,
                &Cost::credits(o.printed.cost.unwrap_or(0)),
            )
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

    /// The cards in a searched zone (8.7.1: the player looks at ALL of them).
    fn cards_in_zone(&self, zone: Zone) -> Vec<ObjectId> {
        match zone {
            Zone::Deck(s) => self.st.deck[&s].clone(),
            Zone::Hand(s) => self.st.hand[&s].clone(),
            Zone::Discard(s) => self.st.discard[&s].clone(),
            Zone::ScoreArea(s) => self.st.score_area[&s].clone(),
            Zone::Root(s) => self.st.root.get(&s).cloned().unwrap_or_default(),
            Zone::Ice(s) => self.st.ice.get(&s).cloned().unwrap_or_default(),
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
            self.st.objects.get_mut(&c).unwrap().zone = Zone::SetAside;
            self.st.objects.get_mut(&c).unwrap().set_aside_for_ability = true;
            match zone {
                Zone::Deck(s) => self.st.deck.get_mut(&s).unwrap().retain(|&x| x != c),
                Zone::Hand(s) => self.st.hand.get_mut(&s).unwrap().retain(|&x| x != c),
                Zone::Discard(s) => self.st.discard.get_mut(&s).unwrap().retain(|&x| x != c),
                Zone::Root(s) => {
                    self.st.root.entry(s).or_default().retain(|&x| x != c)
                }
                Zone::Ice(s) => self.st.ice.entry(s).or_default().retain(|&x| x != c),
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
        self.changes.record(GameChange::DeckShuffled { side });
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
            self.changes.record(GameChange::CardRevealed { obj: p.card });
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
                    // 9.6.9d: an optional component was carried out — used;
                    // this expends once-per-turn restrictions (9.3.6g).
                    cite!("rule_optional_conditional_ability_use");
                    cite!("rule_once_per_turn_flag");
                    self.changes.record(GameChange::AbilityUsed { source: source.obj });
                    self.once_per_turn_used.insert(source);
                    let inner_imm = ImminentWrap {
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
                // Thunder-Art-Gallery-class conditions meet on tag
                // avoidance; the chain reaction resolves while the interrupt
                // window is still open (9.9.4c/d examples).
                self.changes.record(GameChange::TagsAvoided { amount: *n });
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
            Instruction::GrantSubroutines { to, count, sub, before, duration } => {
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
                for obj in ice {
                    for _ in 0..*count {
                        let id = self.next_lingering;
                        self.next_lingering += 1;
                        self.lingering.push(LingeringEffect::new(id, source.obj, Payload::GrantedSubroutine {
                                to: obj,
                                sub: (**sub).clone(),
                                before: *before,
                                seq: id,
                            }, dur));
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
                let payload = match payload {
                    crate::instr::LingeringSpec::PreventAllDamage => Payload::DamagePreventionAll,
                    crate::instr::LingeringSpec::Replacement { applies_to, with } => {
                        // 9.9.8c: a replacement effect can be created ahead
                        // of the effect it replaces.
                        cite!("rule_replacement_effect_from_lingering_effect");
                        Payload::ReplacementEffect {
                            applies_to: *applies_to,
                            replace_with: with.clone(),
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
            Instruction::ModifyStrength { target, amount, duration } => {
                let targets = self.resolve_targets(target, Some(source.obj), &imm.targets);
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
                        Payload::StrengthMod { target: t, delta: *amount },
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
                let x = self.resolve_targets(a, Some(source.obj), &imm.targets);
                let y = self.resolve_targets(b, Some(source.obj), &imm.targets);
                if let (Some(&x), Some(&y)) = (x.first(), y.first()) {
                    self.swap_cards(x, y);
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
                        let s = ServerId::Remote(self.next_remote);
                        self.next_remote += 1;
                        Some((Zone::Root(s), None))
                    }
                    crate::instr::InstallDest::Protecting(s) => {
                        cite!("rule_ice_outermost_position");
                        Some((Zone::Ice(s), None))
                    }
                    crate::instr::InstallDest::InwardFromSource => {
                        match self.st.objects.get(&source.obj).map(|o| o.zone) {
                            Some(Zone::Ice(s)) => self
                                .st
                                .ice
                                .get(&s)
                                .and_then(|v| v.iter().position(|&i| i == source.obj))
                                .map(|i| (Zone::Ice(s), Some(i))),
                            // The source is not protecting a server: it has
                            // no position from which "directly inward" can
                            // be evaluated (8.5.14).
                            _ => None,
                        }
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
                if let Some(p) = self.installs.last_mut() {
                    p.resolved_zone = Some(zone);
                    p.ice_insert_at = ice_at;
                }
                // (c) trash like cards — the MUST component of 8.5.6a.
                if let Zone::Root(s) = zone {
                    let new_type = self.st.objects[&c].printed.card_type;
                    let new_is_region =
                        self.st.objects[&c].printed.subtypes.contains(&"region");
                    let must_trash: Vec<ObjectId> = self
                        .st
                        .root
                        .get(&s)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|&other| {
                            let ot = self.st.objects[&other].printed.card_type;
                            let other_region =
                                self.st.objects[&other].printed.subtypes.contains(&"region");
                            (matches!(new_type, CardType::Asset | CardType::Agenda)
                                && matches!(ot, CardType::Asset | CardType::Agenda))
                                || (new_is_region && other_region)
                        })
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
                let payer = self.st.objects[&p.card].printed.side;
                // 1.16.6: a Patchwork-class reduction the player needs is
                // used here, and its own cost is part of the same all-at-once
                // payment (1.16.10b).
                let cost = if p.ignore_costs {
                    Cost::free()
                } else {
                    let (net, extra) =
                        self.install_payment(p.card, p.dest, p.resolved_zone, payer);
                    extra.plus(&Cost::credits(net))
                };
                self.pay_cost(payer, p.card, &cost);
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
                self.move_card(c, zone);
                if let (Zone::Ice(s), Some(at)) = (zone, p.ice_insert_at) {
                    let v = self.st.ice.get_mut(&s).unwrap();
                    v.retain(|&x| x != c);
                    let at = at.min(v.len());
                    v.insert(at, c);
                }
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
                self.changes.record(GameChange::CardInstalled { obj: c, side });
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
                let base_rez = if p.ignore_costs {
                    Cost::free()
                } else {
                    Cost::credits(self.st.objects[&c].printed.cost.unwrap_or(0))
                };
                let full_rez = match &self.st.objects[&c].printed.additional_rez_cost {
                    Some(add) => base_rez.plus(add),
                    None => base_rez,
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
                    let base = if p.ignore_costs {
                        Cost::free()
                    } else {
                        Cost::credits(self.st.objects[&c].printed.cost.unwrap_or(0))
                    };
                    let total = base.plus(&add);
                    self.ask(
                        Side::Corp,
                        DecisionSpec::NestedCost { cost: total },
                        DecisionCtx::RezAdditionalCost,
                    );
                    return;
                }
                let amount = if p.ignore_costs {
                    0
                } else {
                    self.st.objects[&c].printed.cost.unwrap_or(0)
                };
                // The cost-paid checkpoint that follows is the checkpoint
                // that processes the CardInstalled change, while the card is
                // still facedown (the 9.6.5b THG example).
                cite!("rule_cost_checkpoint_cost_zero");
                self.pay_cost(Side::Corp, c, &Cost::credits(amount));
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
                    self.changes.record(GameChange::CardRezzed { obj: c });
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
                let amount =
                    if p.ignore_costs { 0 } else { self.st.objects[&c].printed.cost.unwrap_or(0) };
                self.pay_cost(side, c, &Cost::credits(amount));
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
                    let shielded = self.st.objects[&c].printed.abilities.iter().any(|a| {
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
            Instruction::CorpRearrangesRnd => {
                // 1.12.3 / 7.4.7a example 1: cards returned to R&D are NEW
                // OBJECTS — the breach's already-chosen bookkeeping forgets
                // them, and the topmost eligible card is recomputed.
                cite!("rule_object_move_location");
                cite!("rule_rnd_topmost_eligibile_candidate");
                let deck: Vec<ObjectId> = self.st.deck[&Side::Corp].clone();
                if let Some(b) = self.breach_ctx_mut() {
                    b.chosen_ever.retain(|c| !deck.contains(c));
                    b.accessed.retain(|c| !deck.contains(c));
                }
                self.refresh_candidates_after_access();
            }
            Instruction::MoveToTopOfRnd { card } => {
                cite!("rule_rnd_topmost_eligibile_candidate");
                let targets = self.resolve_targets(card, Some(source.obj), &imm.targets);
                if let Some(&t) = targets.first() {
                    self.move_card(t, Zone::Deck(Side::Corp));
                    let deck = self.st.deck.get_mut(&Side::Corp).unwrap();
                    deck.retain(|&x| x != t);
                    deck.insert(0, t);
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
                for t in targets {
                    let owner = self.st.objects[&t].owner;
                    self.move_card(t, Zone::Hand(owner));
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
        let threat = self.threat_level();
        for o in self.st.objects.values() {
            if o.controller != side {
                continue;
            }
            for (i, a) in o.printed.abilities.iter().enumerate() {
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

    fn paid_window_options(&self, side: Side, classes: PawClasses) -> Vec<WindowOption> {
        cite!("rule_paid_ability_window_options");
        let mut out = Vec::new();
        let threat = self.threat_level();
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
                if !ability_active(o, a, self.st.encounter.as_ref().map(|e| e.ice), self.st.accessed, threat)
                {
                    continue;
                }
                if !self.ability_present(o.id, i) {
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
        let threat = self.threat_level();
        for src in self.st.objects.values() {
            if src.controller != Side::Runner {
                continue;
            }
            for (i, a) in src.printed.abilities.iter().enumerate() {
                if a.kind == AbilityKind::Paid
                    && a.has_flag(AbilityFlag::Access)
                    && ability_active(src, a, None, self.st.accessed, threat)
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
        // 1.10.3c: hosted credits their card lets them spend are part of what
        // this player can pay with (Cyberfeeder class, 9.1.6c).
        let credits_avail = p.credits
            + self.spendable_hosted_credits(side)
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
        // 1.16.1b: a "trash N cards from your grip" component cannot be paid
        // with fewer than N cards there (the Patchwork branch of 8.7.2b).
        if (self.st.hand[&side].len() as u32) < cost.trash_from_hand {
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
        self.st.player(side).credits + self.spendable_hosted_credits(side)
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
        // 1.10.3c: "spend" and "pay" are the same thing — the credits go
        // back to the bank, by default from the pool, and from credits hosted
        // on a card only where an ability allows it. 9.1.6c then makes that
        // card used alongside the card whose ability is being paid for.
        cite!("rule_spend_credits");
        self.spend_flexible(side, credits_to_pay);
        self.st.player_mut(side).clicks -= cost.clicks;
        let mut trashed = Vec::new();
        if cost.trash_self {
            // 1.19.4: [trash] on a card means "trash this object", used as a
            // trigger cost.
            cite!("rule_trash_symbol");
            self.trash_card(source, side);
            trashed.push(source);
            self.changes.record(GameChange::TrashAbilityUsed { source, side });
        }
        // "Trash N cards from your grip" as a cost (Patchwork class). Which
        // cards is the payer's choice in the CR; the kernel takes the front
        // of the hand (documented on `Cost::trash_from_hand`).
        for _ in 0..cost.trash_from_hand {
            let Some(&c) = self.st.hand[&side].first() else { break };
            self.trash_card(c, side);
            trashed.push(c);
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
        // CR 4.8: leaving the set-aside zone ends the set-aside state, so a
        // 9.5.5 hosted card or an 8.7.2 found card becomes visible to every
        // ability again the moment it is put somewhere else.
        if to != Zone::SetAside {
            o.set_aside_for_ability = false;
        }
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
                    self.lingering.push(LingeringEffect::new(lid, id, Payload::PersistedAbility { def, run_id }, Duration::PersistUntilAfterRun(run_id)));
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
        self.st.score_area[&side]
            .iter()
            .filter_map(|id| self.st.objects.get(id))
            .filter_map(|o| o.printed.agenda_points)
            .sum()
    }

    /// CR 1.17.1a: the threat level is the greatest score of any player.
    /// It is what the "threat N" ability flag reads (9.3.6f).
    pub fn threat_level(&self) -> i32 {
        cite!("rule_threat_level");
        self.score(Side::Corp).max(self.score(Side::Runner))
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
                    // 7.4.3: chosen → never a candidate again this breach.
                    cite!("rule_candidates_already_accessed");
                    b.chosen_ever.push(c);
                    b.candidates.retain(|&x| x != c);
                }
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
                    self.pay_cost(Side::Runner, card, &cost);
                    self.push_access(card);
                } else {
                    if let Some(b) = self.breach_ctx_mut() {
                        b.chosen = None;
                        if b.remaining_from_zone > 0 {
                            b.remaining_from_zone -= 1;
                        }
                    }
                    self.refresh_candidates_after_access();
                }
                // The breach step's Exec already advanced to Checkpoint;
                // a paid access pushed its structure frame on top.
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
        EffectClass::Breach => 15,
        EffectClass::AccessCard => 16,
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
