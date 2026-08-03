//! The control stack: [`StructureFrame`] (timing structures executing §11
//! step tables as data), [`AbilityFrame`] (the ONE parameterized resolution
//! loop shared by 9.5.7 / 9.6.15 / 9.7.2 / 9.8.10), and window frames
//! (window.rs). Chain reactions are LIFO nesting (9.1.2a); the most recent
//! frame always resolves first (9.2.4d).

use crate::ability::AbilityRef;
use crate::instr::Instruction;
use crate::object::{ObjectId, ServerId, Side};
use crate::timing::StructKind;
use crate::window::WindowFrame;

/// Micro-phase of one timing-structure step (9.11.2: each step is one
/// instruction — interrupt window before, checkpoint after).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPhase {
    /// About to open the pre-step interrupt window.
    Enter,
    /// Interrupt window done (or skipped); execute the step.
    Exec,
    /// Step executed; run the post-step checkpoint, then advance the cursor.
    Checkpoint,
}

/// Per-run state carried by the run structure frame (§6).
#[derive(Debug, Clone)]
pub struct RunCtx {
    pub run_id: u64,
    /// CR 6.1.2: the attacked server (announced at initiation).
    pub server: ServerId,
    /// CR 6.2: the Runner's position, as an index into the server's ice
    /// list counted from the innermost (1-based). `None` = no position /
    /// inside the server.
    pub position: Option<usize>,
    /// Whether the current approach/encounter arrived from phase 2/3 (drives
    /// `step_pass_ice`'s "if the run got here from (2) or (3)").
    pub came_from_ice: bool,
    /// Position before the last `step_move_position` (drives 4f).
    pub moved_to_new_position: bool,
    /// CR 6.8.4: reached the Success Phase (not "unsuccessful" even if it
    /// did not breach).
    pub reached_success: bool,
    /// CR 6.7.1: the run was declared successful.
    pub declared_successful: bool,
    /// An "end the run" effect is unwinding toward the Run Ends phase.
    pub jump_to_run_ends: bool,
}

/// Per-breach state (§7.3-7.5).
#[derive(Debug, Clone)]
pub struct BreachCtx {
    pub server: ServerId,
    /// CR 7.4: current candidates.
    pub candidates: Vec<ObjectId>,
    /// The candidate the Runner chose at step 4a.
    pub chosen: Option<ObjectId>,
    /// Cards already accessed this breach (7.4.3).
    pub accessed: Vec<ObjectId>,
    /// HQ/R&D: how many accesses from hand/deck remain (7.3.6).
    pub remaining_from_zone: u32,
    /// CR 7.4.6a: root entries the Runner declared NON-candidates at
    /// 10.3.1j — they cannot become candidates for the rest of the breach.
    pub declined: Vec<ObjectId>,
    /// CR 7.4.3: every candidate the Runner has CHOSEN this breach —
    /// whether or not it was actually accessed, it cannot become a
    /// candidate again (and 7.4.7a's "already chosen" eligibility test).
    pub chosen_ever: Vec<ObjectId>,
}

/// Per-access state (§7.1-7.2).
#[derive(Debug, Clone)]
pub struct AccessCtx {
    pub card: ObjectId,
}

/// Structure-specific context.
#[derive(Debug, Clone)]
pub enum StructCtx {
    Turn { side: Side },
    Run(RunCtx),
    Breach(BreachCtx),
    Access(AccessCtx),
}

/// One timing structure in progress: executes its §11 step table as data.
#[derive(Debug, Clone)]
pub struct StructureFrame {
    pub kind: StructKind,
    /// Unique instance id (durations and 9.2.8f binding).
    pub instance_id: u64,
    /// Cursor: index into the loaded step table.
    pub cursor: usize,
    pub phase: StepPhase,
    /// Branch/goto target applied after the step's checkpoint.
    pub pending_jump: Option<usize>,
    pub ctx: StructCtx,
}

/// Which of the four resolution step-lists an ability frame is running —
/// they share ONE loop, parameterized here (9.5.7/9.6.15/9.7.2/9.8.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionKind {
    /// 9.5.7 steps (c)-(h) — announce/cost happened before the frame.
    Paid,
    /// 9.6.15 steps (b)-(g).
    Conditional,
    /// 9.7.2 steps (a)-(f).
    Play,
    /// 9.8.10 steps (a)-(h) — with the extra sub-level imminence (a)/(b).
    Subroutine,
}

/// Micro-phase of the shared ability-resolution loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityPhase {
    /// 9.5.7b: pay the trigger cost (paid abilities only). The cost-paid
    /// checkpoint may open reaction windows that resolve BEFORE this
    /// ability's instructions become imminent (chain reactions, 9.1.2a).
    PayCost,
    /// 9.8.10a: the subroutine itself becomes imminent (subroutines only).
    SubImminent,
    /// 9.8.10b: the subroutine-level interrupt window is open.
    SubInterrupt,
    /// Announce targets for the next instruction (9.5.7c / 9.6.15b /
    /// 9.7.2a / 9.8.10c,g).
    Targets,
    /// The instruction is imminent; its interrupt window is open
    /// (9.5.7d / 9.6.15c / 9.7.2b / 9.8.10d).
    Imminent,
    /// Resolve the instruction (9.5.7e / 9.6.15d / 9.7.2c / 9.8.10e).
    Resolve,
    /// The post-instruction checkpoint (9.5.7f / 9.6.15e / 9.7.2d / 9.8.10f).
    Checkpoint,
}

/// One resolving ability.
#[derive(Debug, Clone)]
pub struct AbilityFrame {
    pub kind: ResolutionKind,
    /// Source ability (may be stranded per 9.1.4 once independent).
    pub source: AbilityRef,
    pub controller: Side,
    pub instructions: Vec<Instruction>,
    pub idx: usize,
    pub phase: AbilityPhase,
    /// Announced targets for the current instruction (9.3.4b), in
    /// announcement order. An instruction that requires several
    /// announcements (1.15.2, "Trash 1 program and 1 resource") appends one
    /// round per announcement, so this is the whole set the instruction acts
    /// on once every slot is filled.
    pub targets: Vec<ObjectId>,
    /// CR 1.15.2: which announcement of the current instruction comes next.
    /// Reset when the frame moves on to the next instruction.
    pub announce_slot: usize,
    /// CR 1.15.1 / 9.8.6: announced SUBROUTINE targets for the current
    /// instruction — the other kind of target.
    pub sub_targets: Vec<crate::ability::SubKey>,
    /// CR 1.15.4: every target this ABILITY has announced, across all its
    /// instructions — "subsequent instructions of the same ability can
    /// continue to act on that target without needing to select it again".
    pub ability_targets: Vec<ObjectId>,
    /// Index into the VM's imminence stack while Imminent.
    pub imminent_index: Option<usize>,
    /// The conditional-instance id this frame resolves (drops pending).
    pub instance: Option<u64>,
    /// CR 9.1.4: source zone-move stamp at independence; if the source moved
    /// since, self-referencing effects are stranded.
    pub source_move_stamp: u64,
    /// CR 9.6.7d bookkeeping: did ANY instruction of this ability have
    /// expected effects when its interrupt window opened?
    pub any_expected_effects: bool,
    /// Subroutine bookkeeping: which subroutine index on the ice.
    pub subroutine_index: Option<usize>,
    /// Optional-effect declination state for DeclineableChoice.
    pub declined: bool,
    /// Paid abilities: the trigger cost to pay in the PayCost phase.
    pub cost: Option<crate::ability::Cost>,
    /// CR 9.5.5: counters set aside as a [trash] trigger cost was paid —
    /// still "hosted" for this ability, invisible to others.
    pub set_aside_counters: Vec<(crate::object::CounterKind, u32)>,
    /// CR 9.5.5: cards set aside the same way.
    pub set_aside_cards: Vec<crate::object::ObjectId>,
    /// CR 4.8.4 / 8.7.2: cards FOUND by this ability's search, set aside
    /// facedown with the zone they were taken from. `TargetSpec::FoundBySearch`
    /// resolves to these; anything still here when the ability finishes goes
    /// back where it came from (nothing in the CR trashes an unused find).
    pub found_cards: Vec<(crate::object::ObjectId, crate::object::Zone)>,
}

/// A frame on the control stack.
#[derive(Debug, Clone)]
pub enum Frame {
    Structure(StructureFrame),
    Ability(AbilityFrame),
    Window(WindowFrame),
}
