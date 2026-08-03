//! Priority windows (§9.2): the five kinds with exact pass/close semantics,
//! LIFO nesting (9.2.4d), checkpoint-on-priority (9.2.4e/10.3.3), reaction
//! windows bound to fixed pending sets with mandatory-before-pass (9.2.8e)
//! and structure-ended immediate close (9.2.8f), interrupt windows with the
//! open-time fixed conditional set but open paid-interrupt participation
//! (9.9.4b/c/d), and the one-shot mid-access window (9.2.10).

use crate::object::Side;

/// Option classes available in a paid ability window (9.2.7b/c/d/e).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PawClasses {
    /// (P): trigger paid abilities (always).
    pub paid: bool,
    /// (R): rez non-ice (9.2.7c).
    pub rez: bool,
    /// (S): score agendas (9.2.7d).
    pub score: bool,
    /// Approach-ice special: rez the approached ice (9.2.7e).
    pub rez_approached_ice: bool,
}

impl PawClasses {
    /// CR 9.2.7g: paid ability windows occur throughout the timing steps of
    /// turns and runs — which is why the classes are DATA on the step tables
    /// (§11) rather than a property of any one call site.
    pub fn occurrences() {
        cite!("rule_paid_ability_window_occurrence");
    }
    pub fn p() -> Self {
        PawClasses { paid: true, ..Default::default() }
    }
    pub fn pr() -> Self {
        PawClasses { paid: true, rez: true, ..Default::default() }
    }
    pub fn prs() -> Self {
        PawClasses { paid: true, rez: true, score: true, ..Default::default() }
    }
    pub fn approach_ice() -> Self {
        PawClasses { paid: true, rez: true, rez_approached_ice: true, ..Default::default() }
    }
}

/// CR 9.2.5: the five priority window types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowKind {
    /// 9.2.6: active player only, must act, no pass, closes after one action.
    Action,
    /// 9.2.7: both players, active first; closes when a player passes right
    /// after receiving priority from a pass.
    Paid(PawClasses),
    /// 9.2.8: fixed pending set captured at open.
    Reaction,
    /// 9.2.9/§9.9: bound to the single imminent instruction (index into the
    /// VM's imminence stack).
    Interrupt,
    /// 9.2.10: Runner only, one ability or pass, once.
    MidAccess,
}

/// One open priority window (a frame on the control stack).
#[derive(Debug, Clone)]
pub struct WindowFrame {
    pub id: u64,
    pub kind: WindowKind,
    /// Player currently holding priority (9.2.3: at most one at a time).
    pub priority: Side,
    /// The active player when the window opened (priority starts with them:
    /// 9.2.7a/9.2.8b/9.2.9c).
    pub active_player: Side,
    /// Paid/interrupt close rule (9.2.7a/9.2.9c): set when the previous
    /// player passed; a pass while true closes the window.
    pub previous_passed: bool,
    /// Reaction close rule (9.2.8b): the active player has passed; when the
    /// inactive player passes, the window closes.
    pub active_passed: bool,
    /// Reaction/interrupt: instance ids pending in this window (fixed at
    /// open: 9.2.8a / 9.9.4b).
    pub pending: Vec<u64>,
    /// Interrupt: which imminence this window modifies (top-of-stack index).
    /// CR 9.2.9a: "each interrupt window is associated with the single
    /// imminent instruction being modified by the abilities in that window."
    pub imminent_index: Option<usize>,
    /// 9.2.8f: the structure instance whose beginning opened this reaction
    /// window; if that structure ends mid-window the window closes at once.
    pub originating_structure: Option<u64>,
    /// Mid-access one-shot latch (9.2.10c).
    pub used: bool,
    /// Whether the current priority-holder has received a checkpoint before
    /// acting (9.2.4e) — consumed each time priority is handed over.
    pub checkpoint_done_for_priority: bool,
    /// CR 6.8.2c: this window was open when the run was ended and is being
    /// "completed normally, except that new timing structures (including a
    /// breach that was delayed according to rule 7.3.8) cannot be initiated".
    /// The flag is what makes the exception real: everything else about the
    /// window is unchanged.
    pub no_new_timing_structures: bool,
}

impl WindowFrame {
    pub fn new(id: u64, kind: WindowKind, active_player: Side) -> Self {
        // 9.2.4: a priority window is a timing step in which one or both
        // players receive priority; 9.2.3: priority is a player's opportunity
        // to act, and at most one player has it at a time — which is why this
        // is one field. 9.2.1: the active player is the player whose turn it
        // is, and 9.2.4d: windows NEST, which is what makes them frames.
        cite!("rule_priority_window");
        cite!("rule_priority");
        cite!("rule_active_player");
        cite!("rule_nested_priority_window");
        cite!("rule_reaction_window");
        cite!("rule_priority_window_types");
        let priority = match kind {
            // 9.2.10a: only the Runner receives priority mid-access.
            WindowKind::MidAccess => {
                cite!("rule_mid_access_window_priority");
                Side::Runner
            }
            // 9.2.6a: action windows give only the active player priority.
            // 9.2.7a/9.2.8b/9.2.9c: both players, starting with the active.
            _ => {
                cite!("rule_action_window_priority");
                active_player
            }
        };
        WindowFrame {
            id,
            kind,
            priority,
            active_player,
            previous_passed: false,
            active_passed: false,
            pending: Vec::new(),
            imminent_index: None,
            originating_structure: None,
            used: false,
            checkpoint_done_for_priority: false,
            no_new_timing_structures: false,
        }
    }

    /// Handle a pass by the current priority holder. Returns `true` when the
    /// window closes.
    pub fn pass(&mut self) -> bool {
        cite!("rule_pass");
        match self.kind {
            WindowKind::Action => {
                // 9.2.6b: no option to pass in an action window. The VM never
                // routes a pass here; closing happens after the action.
                unreachable_pass()
            }
            WindowKind::Paid(_) | WindowKind::Interrupt => {
                cite!("rule_ability_window_priority");
                cite!("rule_interrupt_window_priority");
                // 9.2.7a: players exchange priority until a player who
                // received priority from their opponent's pass passes.
                if self.previous_passed {
                    true
                } else {
                    self.previous_passed = true;
                    self.priority = self.priority.other();
                    self.checkpoint_done_for_priority = false;
                    false
                }
            }
            WindowKind::Reaction => {
                cite!("rule_reaction_window_priority");
                // 9.2.8b: active passes → inactive receives; inactive passes
                // → window closes.
                if self.priority == self.active_player && !self.active_passed {
                    self.active_passed = true;
                    self.priority = self.active_player.other();
                    self.checkpoint_done_for_priority = false;
                    false
                } else {
                    true
                }
            }
            WindowKind::MidAccess => {
                cite!("rule_mid_access_window_one_ability");
                // 9.2.10c: pass closes it.
                true
            }
        }
    }

    /// CR 9.2.4c: after resolving an option the same player receives
    /// priority again (except action/mid-access one-shots). Resets the
    /// consecutive-pass latch because an option was resolved.
    pub fn option_resolved(&mut self) {
        // 9.2.7f: the player with priority in a paid ability window may use
        // any of the options available to them any number of times; 9.2.8d and
        // 9.2.9e say the same for pending abilities and interrupts — they are
        // triggered in any order until the player passes, and each must fully
        // resolve before another is chosen, which is what returning priority
        // to the same player after a resolution means.
        cite!("rule_paid_ability_window_multiple_options");
        cite!("rule_reaction_window_pending_abilities_unordered");
        cite!("rule_interrupt_window_abilities_unordered");
        cite!("rule_keep_priority_until_pass");
        self.previous_passed = false;
        self.checkpoint_done_for_priority = false;
        if matches!(self.kind, WindowKind::MidAccess) {
            self.used = true;
        }
    }
}

fn unreachable_pass() -> bool {
    debug_assert!(false, "action windows have no pass option (9.2.6b)");
    true
}
