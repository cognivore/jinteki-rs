//! Timing structures (§11) as DATA: `docs/rules/timing-structures.json` is
//! embedded and deserialized into typed step tables; [`step_op`] gives each
//! step id its operational semantics. Each step is a single instruction
//! (9.11.2): preceded by an interrupt window, followed by a checkpoint.
//! 10.3.6: the frame pops BEFORE the closing checkpoint runs.

use crate::window::PawClasses;
use serde::Deserialize;

/// The embedded §11 tables (SYS-F-9: ordered, data-driven step tables).
pub const TIMING_STRUCTURES_JSON: &str =
    include_str!("../../../docs/rules/timing-structures.json");

/// CR 9.2.2a-d: the five timing structures of the appendix tables. Phases
/// are sub-structures (9.2.2a/b) tracked as spans inside the parent table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructKind {
    CorpTurn,
    RunnerTurn,
    Run,
    Breach,
    Access,
}

impl StructKind {
    pub fn json_name(self) -> &'static str {
        match self {
            StructKind::CorpTurn => "corp_turn",
            StructKind::RunnerTurn => "runner_turn",
            StructKind::Run => "run",
            StructKind::Breach => "breach",
            StructKind::Access => "access",
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawFile {
    structures: Vec<RawStructure>,
}

#[derive(Debug, Deserialize)]
struct RawStructure {
    structure: String,
    steps: Vec<RawStep>,
}

#[derive(Debug, Deserialize)]
struct RawStep {
    number: String,
    id: Option<String>,
    text: String,
    #[serde(default)]
    substeps: Vec<RawStep>,
}

/// One executable step row (flattened; branch arms with null ids fold into
/// their parent step's semantics).
#[derive(Debug, Clone)]
pub struct StepEntry {
    pub id: String,
    pub number: String,
    pub text: String,
}

/// A loaded, typed step table for one structure.
#[derive(Debug, Clone)]
pub struct StepTable {
    pub kind: StructKind,
    pub steps: Vec<StepEntry>,
}

impl StepTable {
    pub fn index_of(&self, id: &str) -> usize {
        self.steps
            .iter()
            .position(|s| s.id == id)
            .unwrap_or_else(|| panic!("step id {id} not in table {:?}", self.kind))
    }
}

fn flatten(prefix: &str, steps: &[RawStep], out: &mut Vec<StepEntry>) {
    for s in steps {
        let number = if prefix.is_empty() {
            s.number.clone()
        } else {
            format!("{prefix}{}", s.number)
        };
        if let Some(id) = &s.id {
            if id.starts_with("step_") {
                out.push(StepEntry {
                    id: id.clone(),
                    number: number.clone(),
                    text: s.text.clone(),
                });
            }
        }
        flatten(&number, &s.substeps, out);
    }
}

/// Load all five tables from the embedded JSON.
pub fn load_tables() -> Vec<StepTable> {
    cite!("rule_timing_structure");
    cite!("rule_turn_timing_structure");
    cite!("rule_run_timing_structure");
    cite!("rule_breaching_timing_structure");
    cite!("rule_accessing_timing_structure");
    let raw: RawFile =
        serde_json::from_str(TIMING_STRUCTURES_JSON).expect("timing-structures.json parses");
    let mut out = Vec::new();
    for s in &raw.structures {
        let kind = match s.structure.as_str() {
            "corp_turn" => StructKind::CorpTurn,
            "runner_turn" => StructKind::RunnerTurn,
            "run" => StructKind::Run,
            "breach" => StructKind::Breach,
            "access" => StructKind::Access,
            other => panic!("unknown structure {other}"),
        };
        let mut steps = Vec::new();
        flatten("", &s.steps, &mut steps);
        out.push(StepTable { kind, steps });
    }
    out
}

/// Context-free step instruction kinds; the VM instantiates them with frame
/// context (attacked server, chosen candidate, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    GainAllottedClicks,
    RefillRecurring,
    TurnFormallyBegins,
    MandatoryDraw,
    ActionPhaseEnds,
    DiscardToHandSize,
    LoseUnspentClicks,
    TurnFormallyEnds,
    AnnounceAttackedServer,
    FillBadPubFund,
    RunFormallyBegins,
    SetPositionOutermost,
    ApproachIce,
    EncounterIce,
    EncounterComplete,
    PassIce,
    JackOutChoice,
    MovePositionInward,
    ApproachServer,
    DeclareRunSuccessful,
    BreachAttackedServer,
    CloseRunPriorityWindows,
    EmptyBadPubFund,
    DeclareRunUnsuccessfulIfApplicable,
    BreachBegins,
    FlipArchivesFaceup,
    DetermineAccessLimit,
    AccessChosenCandidate,
    CardBecomesAccessed,
    StealIfAgenda,
}

/// Bodies of branch steps that do something on the "yes" arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepBody {
    /// 9.2.6d: the action window occurs here.
    ActionWindow,
    /// 11.5 step 4a: the Runner chooses a candidate.
    ChooseCandidate,
    /// 6.9.3c-i: the Corp resolves the next unbroken subroutine.
    ResolveNextSubroutine,
}

/// Branch predicates of the §11 tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPred {
    ActivePlayerHasClicks,
    RunnerHasIcePosition,
    ApproachedIceRezzed,
    UnbrokenSubsRemain,
    MovedToNewPosition,
    CandidatesRemain,
}

/// Operational semantics of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOp {
    /// A single game instruction (9.11.2): interrupt window before,
    /// checkpoint after.
    Instr(StepKind),
    /// Instruction, then jump ("Go to (N)" steps).
    InstrThenGoto(StepKind, &'static str),
    /// A paid ability window step (9.2.7g).
    Paw(PawClasses),
    /// The mid-access ability window step (9.2.10d).
    MidAccessWindow,
    /// Branch step whose yes-arm performs a body then falls through, and
    /// whose no-arm jumps.
    BodyOrGoto { pred: BranchPred, body: StepBody, skip_to: &'static str },
    /// Pure branch: jump either way.
    BranchGoto { pred: BranchPred, yes: &'static str, no: &'static str },
    /// Unconditional jump ("Return to (x)").
    Goto(&'static str),
    /// Final step: the structure completes. The VM pops the frame BEFORE the
    /// closing checkpoint (10.3.6).
    Complete,
}

/// The semantics of every step id in the §11 tables. Every arm cites the
/// step id it implements (the id doubles as the CR anchor).
pub fn step_op(kind: StructKind, id: &str) -> StepOp {
    use PawClasses as C;
    use StepKind as K;
    use StepOp as O;
    match (kind, id) {
        // ---- Corp turn (11.2 / 5.6) ------------------------------------
        (StructKind::CorpTurn, "step_corp_turn_allotted_clicks") => {
            cite!("step_corp_turn_allotted_clicks");
            O::Instr(K::GainAllottedClicks)
        }
        (StructKind::CorpTurn, "step_corp_turn_draw_phase_paw") => {
            cite!("step_corp_turn_draw_phase_paw");
            O::Paw(C::prs())
        }
        (StructKind::CorpTurn, "step_corp_turn_recurring_credits_refill") => {
            cite!("step_corp_turn_recurring_credits_refill");
            O::Instr(K::RefillRecurring)
        }
        (StructKind::CorpTurn, "step_corp_turn_turn_formal_begin") => {
            cite!("step_corp_turn_turn_formal_begin");
            O::Instr(K::TurnFormallyBegins)
        }
        (StructKind::CorpTurn, "step_corp_turn_mandatory_draw") => {
            cite!("step_corp_turn_mandatory_draw");
            O::Instr(K::MandatoryDraw)
        }
        (StructKind::CorpTurn, "step_corp_turn_action_phase_paw") => {
            cite!("step_corp_turn_action_phase_paw");
            O::Paw(C::prs())
        }
        (StructKind::CorpTurn, "step_corp_turn_action") => {
            cite!("step_corp_turn_action");
            cite!("rule_action_window_occurrence");
            O::BodyOrGoto {
                pred: BranchPred::ActivePlayerHasClicks,
                body: StepBody::ActionWindow,
                skip_to: "step_corp_turn_action_phase_end",
            }
        }
        (StructKind::CorpTurn, "step_corp_turn_action_phase_loop") => {
            cite!("step_corp_turn_action_phase_loop");
            O::Goto("step_corp_turn_action_phase_paw")
        }
        (StructKind::CorpTurn, "step_corp_turn_action_phase_end") => {
            cite!("step_corp_turn_action_phase_end");
            O::Instr(K::ActionPhaseEnds)
        }
        (StructKind::CorpTurn, "step_corp_turn_discard") => {
            cite!("step_corp_turn_discard");
            O::Instr(K::DiscardToHandSize)
        }
        (StructKind::CorpTurn, "step_corp_turn_discard_phase_paw") => {
            cite!("step_corp_turn_discard_phase_paw");
            O::Paw(C::pr())
        }
        (StructKind::CorpTurn, "step_corp_turn_lose_unspent_clicks") => {
            cite!("step_corp_turn_lose_unspent_clicks");
            O::Instr(K::LoseUnspentClicks)
        }
        (StructKind::CorpTurn, "step_corp_turn_formal_end") => {
            cite!("step_corp_turn_formal_end");
            O::Instr(K::TurnFormallyEnds)
        }
        (StructKind::CorpTurn, "step_corp_turn_complete") => {
            cite!("step_corp_turn_complete");
            O::Complete
        }

        // ---- Runner turn (11.3 / 5.7) ----------------------------------
        (StructKind::RunnerTurn, "step_runner_turn_allotted_clicks") => {
            cite!("step_runner_turn_allotted_clicks");
            O::Instr(K::GainAllottedClicks)
        }
        (StructKind::RunnerTurn, "step_runner_turn_action_phase_paw") => {
            cite!("step_runner_turn_action_phase_paw");
            O::Paw(C::pr())
        }
        (StructKind::RunnerTurn, "step_runner_turn_recurring_credits_refill") => {
            cite!("step_runner_turn_recurring_credits_refill");
            O::Instr(K::RefillRecurring)
        }
        (StructKind::RunnerTurn, "step_runner_turn_recurring_formal_begin") => {
            cite!("step_runner_turn_recurring_formal_begin");
            O::Instr(K::TurnFormallyBegins)
        }
        (StructKind::RunnerTurn, "step_runner_turn_loop_paw") => {
            cite!("step_runner_turn_loop_paw");
            O::Paw(C::pr())
        }
        (StructKind::RunnerTurn, "step_runner_turn_action") => {
            cite!("step_runner_turn_action");
            cite!("rule_action_window_occurrence");
            O::BodyOrGoto {
                pred: BranchPred::ActivePlayerHasClicks,
                body: StepBody::ActionWindow,
                skip_to: "step_runner_turn_action_phase_end",
            }
        }
        (StructKind::RunnerTurn, "step_runner_turn_action_loop") => {
            cite!("step_runner_turn_action_loop");
            O::Goto("step_runner_turn_loop_paw")
        }
        (StructKind::RunnerTurn, "step_runner_turn_action_phase_end") => {
            cite!("step_runner_turn_action_phase_end");
            O::Instr(K::ActionPhaseEnds)
        }
        (StructKind::RunnerTurn, "step_runner_turn_discard") => {
            cite!("step_runner_turn_discard");
            O::Instr(K::DiscardToHandSize)
        }
        (StructKind::RunnerTurn, "step_runner_turn_discard_phase_paw") => {
            cite!("step_runner_turn_discard_phase_paw");
            O::Paw(C::pr())
        }
        (StructKind::RunnerTurn, "step_runner_turn_lose_unspent_clicks") => {
            cite!("step_runner_turn_lose_unspent_clicks");
            O::Instr(K::LoseUnspentClicks)
        }
        (StructKind::RunnerTurn, "step_runner_turn_formal_end") => {
            cite!("step_runner_turn_formal_end");
            O::Instr(K::TurnFormallyEnds)
        }
        (StructKind::RunnerTurn, "step_runner_turn_complete") => {
            cite!("step_runner_turn_complete");
            O::Complete
        }

        // ---- Run (11.4 / 6.9) ------------------------------------------
        (StructKind::Run, "step_initiation_announce") => {
            cite!("step_initiation_announce");
            O::Instr(K::AnnounceAttackedServer)
        }
        (StructKind::Run, "step_initiation_bad_publicity") => {
            cite!("step_initiation_bad_publicity");
            O::Instr(K::FillBadPubFund)
        }
        (StructKind::Run, "step_initiation_formal_begin") => {
            cite!("step_initiation_formal_begin");
            O::Instr(K::RunFormallyBegins)
        }
        (StructKind::Run, "step_runner_position") => {
            cite!("step_runner_position");
            O::Instr(K::SetPositionOutermost)
        }
        (StructKind::Run, "step_initiation_paw") => {
            cite!("step_initiation_paw");
            O::Paw(C::pr())
        }
        (StructKind::Run, "step_initiation_complete") => {
            cite!("step_initiation_complete");
            O::BranchGoto {
                pred: BranchPred::RunnerHasIcePosition,
                yes: "step_approach_begins",
                no: "step_pass_ice",
            }
        }
        (StructKind::Run, "step_approach_begins") => {
            cite!("step_approach_begins");
            O::Instr(K::ApproachIce)
        }
        (StructKind::Run, "step_approach_paw") => {
            cite!("step_approach_paw");
            cite!("rule_paid_ability_window_corp_rez_ice");
            O::Paw(C::approach_ice())
        }
        (StructKind::Run, "step_approach_complete") => {
            cite!("step_approach_complete");
            O::BranchGoto {
                pred: BranchPred::ApproachedIceRezzed,
                yes: "step_encounter_begins",
                no: "step_pass_ice",
            }
        }
        (StructKind::Run, "step_encounter_begins") => {
            cite!("step_encounter_begins");
            O::Instr(K::EncounterIce)
        }
        (StructKind::Run, "step_encounter_paw") => {
            cite!("step_encounter_paw");
            O::Paw(C::p())
        }
        (StructKind::Run, "step_resolve_subroutine") => {
            cite!("step_resolve_subroutine");
            O::BodyOrGoto {
                pred: BranchPred::UnbrokenSubsRemain,
                body: StepBody::ResolveNextSubroutine,
                skip_to: "step_encounter_complete",
            }
        }
        (StructKind::Run, "step_resolve_subroutine_loop") => {
            cite!("step_resolve_subroutine_loop");
            O::Goto("step_resolve_subroutine")
        }
        (StructKind::Run, "step_encounter_complete") => {
            cite!("step_encounter_complete");
            O::InstrThenGoto(K::EncounterComplete, "step_pass_ice")
        }
        (StructKind::Run, "step_pass_ice") => {
            cite!("step_pass_ice");
            O::Instr(K::PassIce)
        }
        (StructKind::Run, "step_before_jack_out_paw") => {
            cite!("step_before_jack_out_paw");
            O::Paw(C::p())
        }
        (StructKind::Run, "step_jack_out_choice") => {
            cite!("step_jack_out_choice");
            O::Instr(K::JackOutChoice)
        }
        (StructKind::Run, "step_move_position") => {
            cite!("step_move_position");
            O::Instr(K::MovePositionInward)
        }
        (StructKind::Run, "step_after_jack_out_paw") => {
            cite!("step_after_jack_out_paw");
            O::Paw(C::pr())
        }
        (StructKind::Run, "step_approach_new_ice") => {
            cite!("step_approach_new_ice");
            O::BranchGoto {
                pred: BranchPred::MovedToNewPosition,
                yes: "step_approach_begins",
                no: "step_approach_server",
            }
        }
        (StructKind::Run, "step_approach_server") => {
            cite!("step_approach_server");
            O::Instr(K::ApproachServer)
        }
        (StructKind::Run, "step_movement_complete") => {
            cite!("step_movement_complete");
            O::Goto("step_run_declared_successful")
        }
        (StructKind::Run, "step_run_declared_successful") => {
            cite!("step_run_declared_successful");
            O::Instr(K::DeclareRunSuccessful)
        }
        (StructKind::Run, "step_breach") => {
            cite!("step_breach");
            O::Instr(K::BreachAttackedServer)
        }
        (StructKind::Run, "step_success_complete") => {
            cite!("step_success_complete");
            O::Goto("step_open_priority_windows_closed")
        }
        (StructKind::Run, "step_open_priority_windows_closed") => {
            cite!("step_open_priority_windows_closed");
            O::Instr(K::CloseRunPriorityWindows)
        }
        (StructKind::Run, "step_run_ends_bad_publicity") => {
            cite!("step_run_ends_bad_publicity");
            O::Instr(K::EmptyBadPubFund)
        }
        (StructKind::Run, "step_run_declared_unsuccessful") => {
            cite!("step_run_declared_unsuccessful");
            O::Instr(K::DeclareRunUnsuccessfulIfApplicable)
        }
        (StructKind::Run, "step_run_complete") => {
            cite!("step_run_complete");
            O::Complete
        }

        // ---- Breach (11.5 / 7.5) ---------------------------------------
        (StructKind::Breach, "step_breaching_begins") => {
            cite!("step_breaching_begins");
            O::Instr(K::BreachBegins)
        }
        (StructKind::Breach, "step_flip_archives") => {
            cite!("step_flip_archives");
            O::Instr(K::FlipArchivesFaceup)
        }
        (StructKind::Breach, "step_determine_candidates_limit") => {
            cite!("step_determine_candidates_limit");
            O::Instr(K::DetermineAccessLimit)
        }
        (StructKind::Breach, "step_choose_candidate") => {
            cite!("step_choose_candidate");
            O::BodyOrGoto {
                pred: BranchPred::CandidatesRemain,
                body: StepBody::ChooseCandidate,
                skip_to: "step_breach_complete",
            }
        }
        (StructKind::Breach, "step_access_candidate") => {
            cite!("step_access_candidate");
            O::Instr(K::AccessChosenCandidate)
        }
        (StructKind::Breach, "step_repeat_candidate_selection") => {
            cite!("step_repeat_candidate_selection");
            O::Goto("step_choose_candidate")
        }
        (StructKind::Breach, "step_breach_complete") => {
            cite!("step_breach_complete");
            O::Complete
        }

        // ---- Access (11.6 / 7.2) ---------------------------------------
        (StructKind::Access, "step_card_accessed") => {
            cite!("step_card_accessed");
            O::Instr(K::CardBecomesAccessed)
        }
        (StructKind::Access, "step_mid_access_ability") => {
            cite!("step_mid_access_ability");
            cite!("rule_mid_access_window_occurrence");
            O::MidAccessWindow
        }
        (StructKind::Access, "step_access_agenda") => {
            cite!("step_access_agenda");
            O::Instr(K::StealIfAgenda)
        }
        (StructKind::Access, "step_access_complete") => {
            cite!("step_access_complete");
            O::Complete
        }

        _ => panic!("no semantics for {kind:?} step {id}"),
    }
}
