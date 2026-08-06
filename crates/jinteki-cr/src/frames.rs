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
    /// CR 6.2.6: the Runner's position — the IDENTITY of one element of the
    /// attacked server's sequence of positions, never an index into it, so
    /// adding or removing other positions cannot move the Runner. `None` =
    /// no position (6.2.5c/d/e).
    pub position: Option<u64>,
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
    /// CR 6.7.4: the "If successful" ability the effect that initiated this
    /// run carried, with the 6.7.4a set of servers that effect allowed.
    pub if_successful: Option<crate::vm::IfSuccessful>,
    /// CR 9.9.1: the "…would be declared successful" interrupt the effect
    /// that initiated this run carried. Cleared when it becomes pending, so
    /// the one declaration offers it once.
    pub if_would_be_successful: Option<crate::vm::WouldBeSuccessful>,
    /// CR 6.9.1c: what the effect that initiated this run STATED about it,
    /// unconditionally — "the Corp cannot rez ice during that run". Resolved
    /// as the run formally begins, and cleared then, so the one sentence
    /// resolves once.
    pub stated_about_run: Option<crate::vm::WouldBeSuccessful>,
    /// CR 6.1.3e: the Encounter Ice Phase the run has come DIRECTLY from,
    /// which is what makes a pass a pass "after an encounter" — `(ice, all
    /// its subroutines were broken during that encounter (6.1.3f/6.5.7), any
    /// of its subroutines resolved during it (9.8.9))`. Set when an Encounter
    /// Ice Phase completes normally, cleared whenever the run reaches the
    /// Approach Ice Phase by any other route, so "the standard progression of
    /// the run" is what decides it.
    pub last_encounter: Option<(ObjectId, bool, bool)>,
}

/// Per-encounter-phase state (§6.5). The phase is a timing structure of its
/// own (9.2.2b), so a forced encounter (6.5.9a) is a frame pushed anywhere —
/// during another encounter, during a breach, or outside a run entirely.
#[derive(Debug, Clone)]
pub struct EncounterCtx {
    /// The piece of ice being encountered. It need not be installed
    /// (9.1.8h keeps its subroutines active for exactly this phase).
    pub ice: ObjectId,
    /// CR 6.5.9a: this phase is a FORCED encounter — resolved outside the
    /// run's normal progression, without changing the Runner's position.
    pub forced: bool,
    /// CR 6.5.9a: the encounter this one interrupted, restored when this
    /// phase completes ("return to the effect that caused the encounter and
    /// proceed from there"). Only one encounter is "in progress" at a time
    /// for everything that reads it, and it is the innermost.
    pub outer: Option<crate::vm::EncounterState>,
    /// Imminence-stack depth when the phase opened: an "end the run" that
    /// unwinds this phase (6.1.4b) drops exactly the imminences raised inside
    /// it.
    pub imminents_at_open: usize,
    /// CR 6.5.8a / 6.2.7c / 6.1.4b: the phase has been aborted and completes
    /// without following any of its remaining steps, as soon as the
    /// instruction that aborted it has finished resolving.
    pub aborted: bool,
}

/// Per-breach state (§7.3-7.5).
#[derive(Debug, Clone)]
pub struct BreachCtx {
    pub server: ServerId,
    /// CR 7.4.1a: the MAINTAINED candidate list — the cards in the root of
    /// the breached server, plus (for Archives) the continuously-derived
    /// discard pile. Every server has a root, so this is never empty by
    /// construction of the server type.
    pub candidates: Vec<ObjectId>,
    /// CR 7.4.1b/c: the ONE candidate currently presented from the zone
    /// corresponding to a central server — a random card in the Corp's hand,
    /// the topmost eligible card of R&D. It is a candidate ALONGSIDE the root
    /// cards, not instead of them, and it is the only kind of candidate the
    /// random access limit counts (7.3.5).
    pub zone_candidate: Option<ObjectId>,
    /// The candidate the Runner chose at step 4a.
    pub chosen: Option<ObjectId>,
    /// Cards already accessed this breach (7.4.3).
    pub accessed: Vec<ObjectId>,
    /// CR 7.3.5: how many more times the Runner may choose a candidate from
    /// the zone corresponding to the breached central server — the random
    /// access limit, decremented as those choices are made (7.3.5c: a chosen
    /// candidate counts even if it is never accessed).
    pub remaining_from_zone: u32,
    /// CR 7.3.5b: additional accesses granted BY AN ABILITY for this breach.
    /// Kept separately from the remainder because step 11.5.3 computes the
    /// limit from scratch and would otherwise erase what was granted before
    /// it ran — and 11.5.1's reaction window, which is where an ability
    /// triggered by the breach BEGINNING resolves (Cupellation, Akiko Nisei),
    /// is two steps earlier. That is also the only place such an ability may
    /// act: 7.3.5b says it "can only be applied at the beginning of the
    /// breach, before the value of the random access limit is set", and 7.3.5
    /// says the limit "will not change for the remainder of that breach".
    pub granted_extra: u32,
    /// CR 7.3.5: whether step 11.5.3 has set the limit for this breach, after
    /// which 7.3.5b's grants no longer apply.
    pub limit_determined: bool,
    /// CR 7.4.6a: root entries the Runner declared NON-candidates at
    /// 10.3.1j — they cannot become candidates for the rest of the breach.
    pub declined: Vec<ObjectId>,
    /// CR 7.4.3: every candidate the Runner has CHOSEN this breach —
    /// whether or not it was actually accessed, it cannot become a
    /// candidate again (and 7.4.7a's "already chosen" eligibility test).
    pub chosen_ever: Vec<(ObjectId, u32)>,
}

/// Per-access state (§7.1-7.2).
#[derive(Debug, Clone)]
pub struct AccessCtx {
    pub card: ObjectId,
    /// CR 9.12.3a/b: a "the Runner must trash this card, if able" requirement
    /// in force for THIS access, and the means it stipulates (if any). The
    /// requirement lives exactly as long as the access does.
    pub must_trash: Option<crate::instr::TrashMeans>,
    /// "…you cannot steal or trash it during this access." (Pinhole class.)
    pub restricted: bool,
}

/// Structure-specific context.
#[derive(Debug, Clone)]
pub enum StructCtx {
    Turn { side: Side },
    Run(RunCtx),
    Encounter(EncounterCtx),
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
    /// Index into the change log at the moment this frame was pushed —
    /// what scopes "…this way" quantities (credits lost by this ability).
    pub log_mark: usize,
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
    /// CR 1.15.2: how many objects each announcement of the current
    /// instruction named, in order. An instruction with SEVERAL target
    /// POSITIONS (a swap's two cards, a hosting instruction's guests and its
    /// host) has to read its announcements positionally — the union
    /// `targets` holds cannot say which position named what.
    pub target_spans: Vec<usize>,
    /// CR 1.15.2: which announcement of the current instruction comes next.
    /// Reset when the frame moves on to the next instruction.
    pub announce_slot: usize,
    /// CR 1.15.1 / 9.8.6: announced SUBROUTINE targets for the current
    /// instruction — the other kind of target.
    pub sub_targets: Vec<crate::ability::SubKey>,
    /// CR 1.15.1 / 1.12.1: announced COUNTER targets for the current
    /// instruction — the third kind of target (Trick of Light class).
    pub counter_targets: Vec<crate::object::CounterRef>,
    /// CR 1.15.4: every target this ABILITY has announced, across all its
    /// instructions — "subsequent instructions of the same ability can
    /// continue to act on that target without needing to select it again".
    pub ability_targets: Vec<ObjectId>,
    /// Index into the VM's imminence stack while Imminent.
    pub imminent_index: Option<usize>,
    /// The conditional-instance id this frame resolves (drops pending).
    pub instance: Option<u64>,
    /// CR 1.15.4: the card the occurrence that met this ability's condition
    /// NAMED — inherited from the instance when the frame is pushed, because
    /// the instance stops being pending as soon as it starts resolving. This
    /// is what a printed "it" or "that card" reads.
    pub triggering_card: Option<ObjectId>,
    /// CR 1.15.4 in the plural: every card that occurrence named, inherited
    /// from the instance the same way and for the same reason. What a printed
    /// "those cards" reads; see [`crate::ability::AbilityInstance::triggering_cards`].
    pub triggering_cards: Vec<ObjectId>,
    /// CR 9.1.4: source zone-move stamp at independence; if the source moved
    /// since, self-referencing effects are stranded.
    pub source_generation: u32,
    /// CR 9.6.7d bookkeeping: did ANY instruction of this ability have
    /// expected effects when its interrupt window opened?
    pub any_expected_effects: bool,
    /// Subroutine bookkeeping: which subroutine index on the ice.
    pub subroutine_index: Option<usize>,
    /// Optional-effect declination state for DeclineableChoice.
    pub declined: bool,
    /// Paid abilities: the trigger cost to pay in the PayCost phase.
    pub cost: Option<crate::ability::Cost>,
    /// CR 1.16.1c: a restriction the effect this frame carries out is subject
    /// to, which the PayCost phase's payment must not break.
    pub cost_restriction: Option<crate::vm::PaymentRestriction>,
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
    /// CR 1.12.3: the cards this ability is LOOKING AT, with the generation
    /// each had when it was looked at. A card moved to an unknown location —
    /// a shuffle, a rearrangement — becomes a NEW object, so the ability can
    /// no longer act on it, and the stale entry is exactly how that shows.
    pub looked_at: Vec<(crate::object::ObjectId, u32)>,
    /// CR 1.21.6: the cards this ability has REVEALED, with the generation
    /// each had when it was revealed.
    ///
    /// 1.21.6 is one rule over two verbs — "if a resolving ability directs one
    /// or both players to **look at or reveal** a card or set of cards, each
    /// such card remains visible … until the entire ability is finished
    /// resolving or the card moves to a different location" — so the reveal
    /// keeps its cards on the resolving ability exactly as
    /// [`AbilityFrame::looked_at`] does. Kept separately because 1.21.5 says
    /// the two effects "are not the same": a sentence saying "the revealed
    /// cards" must not reach cards this ability only looked at, and the kernel
    /// already keeps them apart everywhere else (two instructions, two
    /// `GameChange` records).
    ///
    /// EXTENDED rather than assigned, which is the other difference: 1.21.6
    /// keeps EVERY revealed card visible for the whole ability, so a second
    /// reveal adds to the set instead of replacing it.
    pub revealed: Vec<(crate::object::ObjectId, u32)>,
    /// CR 8.5.16f: the cards THIS ability's own install instructions have
    /// installed, in the order they became installed. What
    /// [`crate::instr::TargetFilter::InstalledByThisAbility`] describes and
    /// [`crate::instr::TargetSpec::InstalledByThisAbility`] points at.
    ///
    /// Deliberately NOT [`AbilityFrame::ability_targets`]: 1.15.4 is about the
    /// targets an ability ANNOUNCED, and an install whose card came from a
    /// search announced none — 8.7.4's find is not 1.15.2's announcement — so
    /// putting it there would both lie about what was announced and shift every
    /// [`crate::instr::TargetSpec::EarlierTarget`] index after it.
    ///
    /// Seeded from [`crate::ability::AbilityInstance::bound_installs`] when the
    /// frame is pushed, which is how a delayed conditional (9.6.13) still knows
    /// which card its creator installed.
    pub installed_cards: Vec<crate::object::ObjectId>,
    /// CR 8.3.3 / 4.8.7: the facedown set-aside GROUP this ability created —
    /// the cards it set aside from the top of a deck to arrange them. It is
    /// what `TargetFilter::SetAsideByThisAbility` names while 8.3.3b's "other
    /// effects on cards in a deck" are performed, and what
    /// `Instruction::ArrangeSetAside` returns to the deck.
    pub set_aside_group: Option<u64>,
    /// CR 1.16.2c: the value this ability's controller announced for X when
    /// they paid its trigger cost. The announcement belongs to the USE of the
    /// ability (9.5.1), not to the transient payment, so "remove X tags" reads
    /// it here once the payment has committed. 1.16.2d stays true: an ability
    /// asking about a cost NOT being paid finds neither a payment nor a frame
    /// and sees 0.
    pub announced_x: Option<u32>,
}

/// A frame on the control stack.
#[derive(Debug, Clone)]
pub enum Frame {
    Structure(StructureFrame),
    Ability(AbilityFrame),
    Window(WindowFrame),
}
