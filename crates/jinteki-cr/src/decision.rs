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
    /// CR 9.2.10: the Runner uses one mid-access ability or passes.
    /// 9.12.3a/b: `can_pass` is false while a "must trash this card"
    /// requirement is in force and a permitted means is among the options.
    MidAccessWindow { options: Vec<WindowOption>, can_pass: bool },
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
    /// CR 9.8.2c: "in the order of your choice" — the granting player declares
    /// where each newly granted subroutine goes relative to every subroutine
    /// the ice has at that time, regardless of categories. `existing` is that
    /// list, in order, and `granted` the labels being placed; the answer is
    /// one insertion index per granted subroutine, in `0..=existing.len()`.
    DeclareSubroutineOrder {
        existing: Vec<(SubKey, &'static str)>,
        granted: Vec<&'static str>,
    },
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
    /// CR 1.16.2f: an install-and-rez effect reducing the TOTAL cost — the
    /// Corp declares how many of the `total` credits come off the install
    /// cost. The remainder comes off the rez cost, so one number is the whole
    /// declaration.
    DivideCostReduction { total: u32 },
    /// CR 1.16.2c: announce the value of X for the cost about to be paid.
    /// `max` is the greatest value the ability's own restriction allows, so
    /// the legal answers are exactly `0..=max`.
    DeclareX { max: u32 },
    /// CR 1.16.2e: an alternate way to pay part of the cost being paid. The
    /// payer may use it or not; using it covers `covers` credits of the cost
    /// in exchange for `instead`.
    AlternatePayment { label: &'static str, covers: u32, instead: crate::ability::Cost },
    /// CR 1.16.1: which cards the payer spends for a cost component that
    /// names a number of cards. CR 1.15.1b: cards chosen to PAY A COST are
    /// not targets, so this is not a 1.15.2 announcement and does not share
    /// its decision. `label` names the component ("forfeit", "trash").
    PaymentCards { candidates: Vec<ObjectId>, count: u32, label: &'static str },
    /// CR 1.10.3c: "a player spending credits chooses how to divide the
    /// credits they are spending from among the allowed locations". Each
    /// location is `(where, how many are there)`, with `None` the credit pool
    /// and `Some(card)` credits hosted on that card; the answer is one number
    /// per location, in order, summing to `total`.
    DivideCreditPayment { total: u32, locations: Vec<(Option<ObjectId>, u32)> },
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
    /// CR 1.16.2f: credits of the "total" modifier applied to the install
    /// cost; the rest goes to the rez cost.
    DivideReduction(u32),
    /// CR 9.8.2c: one insertion index per granted subroutine.
    SubroutineOrder(Vec<usize>),
    /// CR 1.16.2c: the announced value of X.
    DeclaredX(u32),
    /// CR 1.10.3c: credits taken from each allowed location, in the order the
    /// spec listed them.
    Division(Vec<u32>),
}
