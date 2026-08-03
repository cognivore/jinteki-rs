//! The coroutine surface: the VM never blocks — every player decision
//! suspends the machine as a typed, defunctionalized [`DecisionSpec`]
//! (ARCHITECTURE §3: decisions are DATA, never callbacks). The driver
//! answers with a [`DecisionAnswer`] and steps again.

use crate::ability::{AbilityRef, SubKey};
use crate::object::{ObjectId, ServerId, Side};
use crate::window::PawClasses;

/// What `Vm::step()` yields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Yield {
    /// A player must decide. The VM is suspended until `Vm::answer`.
    Decision(Side, DecisionSpec),
    /// The machine advanced but wants to be stepped again.
    Progressed,
    /// CR 1.7: the game has ended.
    GameOver(GameResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    /// CR 1.7.2a: 7+ agenda points.
    AgendaPoints(Side),
    /// CR 1.7.2b: the Runner is flatlined.
    Flatline,
    /// CR 1.7.2c: the Corp must draw from an empty R&D.
    RndEmpty,
    /// CR 1.7.1a / 10.3.1c: simultaneous win.
    Draw,
}

/// An option a player can take with priority in some window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowOption {
    /// Trigger a pending conditional instance (reaction/interrupt windows).
    TriggerInstance { instance: u64, label: &'static str, mandatory: bool },
    /// Trigger a paid ability (paid/interrupt/mid-access windows).
    TriggerPaid { ability: AbilityRef, label: &'static str },
    /// (R): rez an asset/upgrade (9.2.7c).
    Rez { card: ObjectId },
    /// Approach-ice special: rez the approached ice (9.2.7e).
    RezApproachedIce { card: ObjectId },
    /// (S): score an agenda (9.2.7d).
    Score { card: ObjectId },
    /// Mid-access basic trash ability (7.1.5).
    BasicTrash { card: ObjectId, cost: u32 },
}

/// Basic actions + card actions available in an action window (5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionOption {
    /// 5.2.6b / 5.2.7b: "[click]: Gain 1[credit]."
    BasicCredit,
    /// 5.2.6c / 5.2.7c: "[click]: Draw 1 card."
    BasicDraw,
    /// 5.2.7f: "[click]: Run any server."
    BasicRun { server: ServerId },
    /// 5.2.7g: "[click], 2[credit]: Remove 1 tag."
    BasicRemoveTag,
    /// A [click]-cost card paid ability (an action, 5.2.1).
    CardAction { ability: AbilityRef, label: &'static str },
}

/// The typed decision requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionSpec {
    /// CR 1.6.6a: keep or mulligan the starting hand.
    Mulligan,
    /// Action window (9.2.6): must choose one; no pass.
    TakeAction { options: Vec<ActionOption> },
    /// Paid ability window (9.2.7): any option or pass.
    PaidWindow { classes: PawClasses, options: Vec<WindowOption> },
    /// Reaction window (9.2.8): trigger a pending instance or pass;
    /// `can_pass` is false while mandatory instances remain (9.2.8e).
    ReactionWindow { options: Vec<WindowOption>, can_pass: bool },
    /// Interrupt window (9.2.9): relevant interrupts or pass (9.2.9f).
    InterruptWindow { options: Vec<WindowOption>, can_pass: bool },
    /// Mid-access window (9.2.10): one ability / basic trash / pass.
    MidAccessWindow { options: Vec<WindowOption> },
    /// Choose targets for an instruction (9.3.4b), one of `candidates`,
    /// `count` times (or fewer if `up_to`). CR 1.15.2e / 10.12.3a: `min` is
    /// the number of targets the instruction REQUIRES where fewer than
    /// `count` may be chosen — 0 for a plain "up to N", and non-zero where
    /// the rules force a floor (a sabotage that must take enough from HQ).
    ChooseTargets { candidates: Vec<ObjectId>, count: u32, up_to: bool, min: u32 },
    /// CR 1.15.1 / 9.8.6: announce SUBROUTINES as targets — the other kind
    /// of thing that can be a target. Each candidate carries its label so a
    /// driver can show it. 9.8.6 restricts the candidates for an ability
    /// that would BREAK them to unbroken subroutines; 9.8.6b's "all but N"
    /// ability targets the subroutines it will NOT break, so its candidates
    /// include the broken ones.
    ChooseSubroutines { candidates: Vec<(SubKey, &'static str)>, count: u32, up_to: bool },
    /// CR 9.11.4g: choose between optioned effects; each option is its own
    /// instruction chain.
    ChooseOption { options: Vec<&'static str> },
    /// CR 9.11.4f / 1.16.10-11: pay a (nested or additional) cost or decline.
    NestedCost { cost: crate::ability::Cost },
    /// Decline or resolve an optional part (9.6.9c).
    OptionalEffect { label: &'static str },
    /// CR 11.5 step 4a: the Runner chooses a candidate to access.
    ChooseCandidate { candidates: Vec<ObjectId> },
    /// CR 10.3.1j / 7.4.6a: a card entered the breached server's root since
    /// the previous checkpoint; the Runner declares whether it becomes a
    /// candidate. Answer with `ResolveOptional(bool)`.
    DeclareBreachCandidate { card: ObjectId },
    /// `step_jack_out_choice` (6.9.4c).
    JackOut,
    /// Discard down to hand size (5.5.4c): choose `count` cards.
    DiscardCards { count: u32, hand: Vec<ObjectId> },
    /// CR 10.3.1e: choose which minimal appropriate set is trashed.
    MinimalSet { sets: Vec<Vec<ObjectId>> },
    /// 10.8.6c/d: openly spend credits on a trace (0..=max).
    TraceSpend { max: u32, strength_so_far: i64, corp_side: bool },
    /// 10.14.6b: a sealed Psi-Game bid; `legal` per 10.14.3.
    PsiBid { legal: Vec<u32> },
}

/// Defunctionalized answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionAnswer {
    KeepHand,
    TakeMulligan,
    Action(ActionOption),
    /// Take a window option.
    Take(WindowOption),
    Pass,
    Targets(Vec<ObjectId>),
    /// CR 1.15.1: announced subroutine targets (9.8.6).
    Subroutines(Vec<SubKey>),
    Option(usize),
    PayNestedCost(bool),
    ResolveOptional(bool),
    Candidate(ObjectId),
    JackOut(bool),
    Discard(Vec<ObjectId>),
    ChooseSet(usize),
    SpendCredits(u32),
    Bid(u32),
}
