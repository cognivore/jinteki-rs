//! The timing vocabulary a game is played under — SHARED GROUND.
//!
//! The lobby writes a [`TimingConfig`] (the host picks it, the joiner sees
//! it at the ready check and consents by readying up), and game creation
//! carries it onto the created [`crate::cr::CrGame`] verbatim, where the
//! in-game timing enforcement reads it. The struct is the contract between
//! those two halves: nothing here enforces anything, and nothing in the
//! enforcement re-derives what is written here.
//!
//! Four modes, one shape: `main_clock_secs` is the per-side chess clock
//! (`None` = untimed), `rope` is the per-decision overrun discipline
//! (`None` = no rope). The DEFAULT — what a lobby gets when the host
//! touches nothing — is 30 minutes a side, roped.

use jinteki_cr::object::Side;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Per-decision overrun discipline: how long an action window or a decision
/// may dawdle before the rope starts burning, and how much fuse a timed-out
/// player has before the game concludes they are gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RopeConfig {
    /// Seconds an ACTION (a turn's own click) may take before the rope.
    pub action_secs: u32,
    /// Seconds any other decision (a prompt) may take before the rope.
    pub decision_secs: u32,
    /// Seconds of burning rope before the timeout concludes.
    pub timeout_fuse_secs: u32,
}

impl Default for RopeConfig {
    fn default() -> Self {
        Self { action_secs: 60, decision_secs: 10, timeout_fuse_secs: 30 }
    }
}

/// The whole timing mode of one game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Per side, in seconds; `None` = untimed.
    pub main_clock_secs: Option<u32>,
    /// `None` = no rope. NOTE: autopair ("play anyone") only ever seats
    /// players at roped tables — see `lobby::pick_oldest`.
    pub rope: Option<RopeConfig>,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self { main_clock_secs: Some(1800), rope: Some(RopeConfig::default()) }
    }
}

/// The wire clamps: a clock is 1 minute to 6 hours a side, a rope number is
/// 1 second to 10 minutes. Outside numbers are pulled to the near edge —
/// refusing a lobby over a typo would be ceremony.
const MAIN_CLOCK_RANGE: (u32, u32) = (60, 6 * 3600);
const ROPE_RANGE: (u32, u32) = (1, 600);

fn clamp((lo, hi): (u32, u32), v: u64) -> u32 {
    (v.min(hi as u64) as u32).max(lo)
}

impl TimingConfig {
    /// The lobby row's compact label: "30m + rope", "untimed + rope",
    /// "25m", "untimed". Odd second counts print as "25m30s".
    pub fn label(&self) -> String {
        let clock = match self.main_clock_secs {
            None => "untimed".to_string(),
            Some(s) if s % 60 == 0 => format!("{}m", s / 60),
            Some(s) => format!("{}m{}s", s / 60, s % 60),
        };
        match &self.rope {
            Some(_) => format!("{clock} + rope"),
            None => clock,
        }
    }

    /// A config off the wire (`{"timing":{…}}` on `lobby-create`). A missing
    /// or malformed object is the DEFAULT (timed 30 + rope) — the mode you
    /// get by not asking for one. Inside an object, absent/null fields mean
    /// what they mean in the struct: no clock, no rope.
    pub fn from_wire(v: &Value) -> Self {
        let Some(obj) = v.as_object() else { return Self::default() };
        let main_clock_secs = obj
            .get("main_clock_secs")
            .and_then(Value::as_u64)
            .map(|s| clamp(MAIN_CLOCK_RANGE, s));
        let rope = obj.get("rope").and_then(Value::as_object).map(|r| {
            let d = RopeConfig::default();
            let f = |k: &str, dflt: u32| {
                r.get(k).and_then(Value::as_u64).map_or(dflt, |v| clamp(ROPE_RANGE, v))
            };
            RopeConfig {
                action_secs: f("action_secs", d.action_secs),
                decision_secs: f("decision_secs", d.decision_secs),
                timeout_fuse_secs: f("timeout_fuse_secs", d.timeout_fuse_secs),
            }
        });
        Self { main_clock_secs, rope }
    }

    /// The untimed, unroped mode — what a bot game runs under.
    pub fn none() -> Self {
        Self { main_clock_secs: None, rope: None }
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_default_is_timed_thirty_roped() {
        let t = TimingConfig::default();
        assert_eq!(t.main_clock_secs, Some(1800));
        assert_eq!(t.rope, Some(RopeConfig { action_secs: 60, decision_secs: 10, timeout_fuse_secs: 30 }));
        assert_eq!(t.label(), "30m + rope");
    }

    #[test]
    fn labels_cover_all_four_modes() {
        let rope = Some(RopeConfig::default());
        let m = |c, r| TimingConfig { main_clock_secs: c, rope: r }.label();
        assert_eq!(m(Some(1800), rope), "30m + rope");
        assert_eq!(m(None, rope), "untimed + rope");
        assert_eq!(m(Some(1500), None), "25m");
        assert_eq!(m(None, None), "untimed");
        assert_eq!(m(Some(1530), None), "25m30s");
    }

    #[test]
    fn the_wire_defaults_omissions_and_clamps_typos() {
        // No timing at all: the default mode.
        assert_eq!(TimingConfig::from_wire(&Value::Null), TimingConfig::default());
        assert_eq!(TimingConfig::from_wire(&json!("garbage")), TimingConfig::default());
        // An object speaks for itself: absent fields are absences.
        assert_eq!(TimingConfig::from_wire(&json!({})), TimingConfig::none());
        let t = TimingConfig::from_wire(&json!({"main_clock_secs": 900, "rope": {}}));
        assert_eq!(t.main_clock_secs, Some(900));
        assert_eq!(t.rope, Some(RopeConfig::default()), "an empty rope is the default rope");
        // Typos are pulled to the near edge, not refused.
        let t = TimingConfig::from_wire(&json!({
            "main_clock_secs": 1,
            "rope": {"action_secs": 99999, "decision_secs": 0, "timeout_fuse_secs": 30}
        }));
        assert_eq!(t.main_clock_secs, Some(60));
        let r = t.rope.unwrap();
        assert_eq!((r.action_secs, r.decision_secs, r.timeout_fuse_secs), (600, 1, 30));
    }

    #[test]
    fn the_config_round_trips_through_serde() {
        let t = TimingConfig::default();
        let j = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<TimingConfig>(&j).unwrap(), t);
        let u = TimingConfig::none();
        let j = serde_json::to_string(&u).unwrap();
        assert_eq!(serde_json::from_str::<TimingConfig>(&j).unwrap(), u);
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Params (Duration-based, so tests run millisecond fuses)
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TimingParams {
    pub main_clock: Option<Duration>,
    pub rope: Option<RopeParams>,
}

#[derive(Debug, Clone)]
pub struct RopeParams {
    pub action: Duration,
    pub decision: Duration,
    pub timeout_fuse: Duration,
}

impl From<&TimingConfig> for TimingParams {
    fn from(c: &TimingConfig) -> Self {
        TimingParams {
            main_clock: c.main_clock_secs.map(|s| Duration::from_secs(s as u64)),
            rope: c.rope.as_ref().map(|r| RopeParams {
                action: Duration::from_secs(r.action_secs as u64),
                decision: Duration::from_secs(r.decision_secs as u64),
                timeout_fuse: Duration::from_secs(r.timeout_fuse_secs as u64),
            }),
        }
    }
}

impl TimingParams {
    pub fn untimed() -> Self {
        Self::default()
    }
    pub fn enabled(&self) -> bool {
        self.main_clock.is_some() || self.rope.is_some()
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Live state
// ───────────────────────────────────────────────────────────────────────────

/// Which duration the live fuse was lit with — the client renders the burn
/// against `total`, and "timeout" tells it (and the tests) that a token fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseKind {
    Action,
    Decision,
    Timeout,
}

impl FuseKind {
    fn key(self) -> &'static str {
        match self {
            FuseKind::Action => "action",
            FuseKind::Decision => "decision",
            FuseKind::Timeout => "timeout",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fuse {
    pub side: Side,
    pub deadline: Instant,
    pub total: Duration,
    pub kind: FuseKind,
}

/// What a pop resolves to, decided HERE so the consecutive-pop and token
/// bookkeeping cannot drift from the answer the caller then gives the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopOutcome {
    /// A banked ⌛ fired: the fuse has been re-armed at `timeout_fuse`, the
    /// token is consumed, the streak is reset. NOT a pop.
    TimeoutFired,
    /// First pop: auto-resolve the prompt (credits for an action window, the
    /// neutral default for anything else).
    AutoResolve,
    /// Second consecutive pop: that player loses.
    Loss,
}

fn six(s: Side) -> usize {
    match s {
        Side::Corp => 0,
        Side::Runner => 1,
    }
}

/// All timing state of one game. Owned by `CrGame`; every transition is a
/// method here so the invariants live in one file.
#[derive(Debug)]
pub struct TimingState {
    pub params: TimingParams,
    /// Main-clock time remaining per side (`six`-indexed). Meaningless when
    /// `params.main_clock` is `None`.
    remaining: [Duration; 2],
    /// Whose main clock is running, and since when (the last settle point).
    running: Option<(Side, Instant)>,
    /// The live fuse, armed iff a person owes a decision and rope is on.
    fuse: Option<Fuse>,
    /// Did this side's LAST prompt end in a pop, with nothing answered since?
    popped_last: [bool; 2],
    /// Banked ⌛ per side.
    tokens: [u32; 2],
    /// Consecutive clean own-turns per side (3 banks a token and resets).
    streak: [u32; 2],
    /// Has the current turn already been dirtied for its owner (a pop or a
    /// timeout fire anywhere resets the streak; this flag additionally keeps
    /// the turn it happened in from counting when it ends).
    turn_dirty: [bool; 2],
    /// Last observed `(turn_seq, turn_side)`, to notice turn boundaries.
    seen_turn: (u64, Side),
}

impl TimingState {
    pub fn new(params: TimingParams) -> Self {
        let main = params.main_clock.unwrap_or(Duration::ZERO);
        TimingState {
            params,
            remaining: [main, main],
            running: None,
            fuse: None,
            popped_last: [false, false],
            tokens: [0, 0],
            streak: [0, 0],
            turn_dirty: [false, false],
            // turn_seq 0 is setup — nobody's turn; see `note_turn`.
            seen_turn: (0, Side::Corp),
        }
    }

    pub fn enabled(&self) -> bool {
        self.params.enabled()
    }
    pub fn tokens_of(&self, side: Side) -> u32 {
        self.tokens[six(side)]
    }
    pub fn fuse(&self) -> Option<&Fuse> {
        self.fuse.as_ref()
    }

    /// Move the running clock forward to `now`. Linear, drift-free: elapsed
    /// time is subtracted from the running side and the anchor advances.
    pub fn settle(&mut self, now: Instant) {
        if let Some((side, since)) = self.running {
            if self.params.main_clock.is_some() {
                let spent = now.saturating_duration_since(since);
                let r = &mut self.remaining[six(side)];
                *r = r.saturating_sub(spent);
            }
            self.running = Some((side, now));
        }
    }

    /// The engine just put a decision to `side`: their clock starts, and the
    /// rope lights a fuse sized by what kind of prompt it is.
    pub fn arm(&mut self, side: Side, is_action: bool, now: Instant) {
        self.settle(now);
        self.running = Some((side, now));
        if let Some(r) = &self.params.rope {
            let total = if is_action { r.action } else { r.decision };
            self.fuse = Some(Fuse { side, deadline: now + total, total, kind: if is_action { FuseKind::Action } else { FuseKind::Decision } });
        }
    }

    /// The decision is no longer waiting on anyone (answered, auto-resolved,
    /// or the game ended): the clock stops, the fuse goes out.
    pub fn disarm(&mut self, now: Instant) {
        self.settle(now);
        self.running = None;
        self.fuse = None;
    }

    /// `side` ANSWERED a prompt themselves — which is what breaks a
    /// consecutive-pop chain. Auto-resolutions never call this.
    pub fn answered(&mut self, side: Side) {
        self.popped_last[six(side)] = false;
    }

    /// A side whose main clock has reached zero, if any.
    pub fn flagged(&mut self, now: Instant) -> Option<Side> {
        self.params.main_clock?;
        self.settle(now);
        [Side::Corp, Side::Runner]
            .into_iter()
            .find(|s| self.remaining[six(*s)] == Duration::ZERO)
    }

    /// Whether the live fuse has burnt out.
    pub fn fuse_popped(&self, now: Instant) -> bool {
        self.fuse.as_ref().is_some_and(|f| now >= f.deadline)
    }

    /// The fuse burnt out: decide what that means for its owner, and do the
    /// token/streak/consecutive-pop bookkeeping for that meaning. The caller
    /// (who owns the VM) acts on the outcome; on `TimeoutFired` the fuse has
    /// already been re-armed here, on the other two it is out.
    pub fn pop(&mut self, now: Instant) -> Option<(Side, PopOutcome)> {
        let f = self.fuse.take()?;
        let side = f.side;
        let i = six(side);
        // The rope ran out — however this resolves, the current turn is no
        // longer clean and the streak restarts (see the module doc for why
        // this includes a timeout fire).
        self.streak[i] = 0;
        self.turn_dirty[i] = true;
        if self.tokens[i] > 0 {
            // A banked ⌛ fires instead of a pop: consume it, relight the fuse.
            self.tokens[i] -= 1;
            let total = self.params.rope.as_ref().map(|r| r.timeout_fuse).unwrap_or(Duration::ZERO);
            self.fuse = Some(Fuse { side, deadline: now + total, total, kind: FuseKind::Timeout });
            return Some((side, PopOutcome::TimeoutFired));
        }
        if self.popped_last[i] {
            return Some((side, PopOutcome::Loss));
        }
        self.popped_last[i] = true;
        Some((side, PopOutcome::AutoResolve))
    }

    /// Watch the VM's `(turn_seq, turn_side)` for turn boundaries. When a
    /// turn of `side`'s own ends clean, their streak grows; at three, a ⌛ is
    /// banked and the streak restarts. Returns the side that just banked one,
    /// if any. `turn_seq` 0 is setup and belongs to nobody.
    pub fn note_turn(&mut self, turn_seq: u64, turn_side: Side) -> Option<Side> {
        if self.params.rope.is_none() || (turn_seq, turn_side) == self.seen_turn {
            return None;
        }
        let (old_seq, old_side) = self.seen_turn;
        self.seen_turn = (turn_seq, turn_side);
        // The new turn starts fresh for its owner.
        self.turn_dirty[six(turn_side)] = false;
        if old_seq == 0 {
            return None;
        }
        let i = six(old_side);
        if self.turn_dirty[i] {
            self.turn_dirty[i] = false;
            return None;
        }
        self.streak[i] += 1;
        if self.streak[i] >= 3 {
            self.streak[i] = 0;
            self.tokens[i] += 1;
            return Some(old_side);
        }
        None
    }

    /// The wire shape, from this VIEWER's seat: both main clocks and whose is
    /// running are open information; the ⌛ count is the viewer's OWN only
    /// (the opponent's is never shown — nor sent). `None` when the game is
    /// untimed, so untimed games carry no timing key at all.
    pub fn json(&self, viewer: Side, now: Instant, over: bool) -> Option<Value> {
        if !self.enabled() {
            return None;
        }
        let mut m = serde_json::Map::new();
        if self.params.main_clock.is_some() {
            let left = |s: Side| -> u64 {
                let mut r = self.remaining[six(s)];
                if !over {
                    if let Some((rs, since)) = self.running {
                        if rs == s {
                            r = r.saturating_sub(now.saturating_duration_since(since));
                        }
                    }
                }
                r.as_millis() as u64
            };
            m.insert(
                "main".into(),
                json!({
                    "corp_ms": left(Side::Corp),
                    "runner_ms": left(Side::Runner),
                    "running": match (over, self.running) {
                        (false, Some((s, _))) => Value::String(side_key(s).into()),
                        _ => Value::Null,
                    },
                }),
            );
        }
        if self.params.rope.is_some() {
            m.insert(
                "rope".into(),
                match (&self.fuse, over) {
                    (Some(f), false) => json!({
                        "side": side_key(f.side),
                        "remaining_ms": f.deadline.saturating_duration_since(now).as_millis() as u64,
                        "total_ms": f.total.as_millis() as u64,
                        "kind": f.kind.key(),
                    }),
                    _ => Value::Null,
                },
            );
            m.insert("timeouts".into(), json!(self.tokens[six(viewer)]));
        }
        Some(Value::Object(m))
    }
}

fn side_key(s: Side) -> &'static str {
    match s {
        Side::Corp => "corp",
        Side::Runner => "runner",
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_spec_defaults() {
        let r = RopeConfig::default();
        assert_eq!((r.action_secs, r.decision_secs, r.timeout_fuse_secs), (60, 10, 30));
        let c = TimingConfig::none();
        assert!(c.main_clock_secs.is_none() && c.rope.is_none());
        assert!(!TimingParams::from(&c).enabled());
    }

    #[test]
    fn config_json_with_missing_fields_fills_the_defaults() {
        let c: TimingConfig =
            serde_json::from_value(json!({"main_clock_secs": 300, "rope": {}})).unwrap();
        assert_eq!(c.main_clock_secs, Some(300));
        let r = c.rope.unwrap();
        assert_eq!((r.action_secs, r.decision_secs, r.timeout_fuse_secs), (60, 10, 30));
    }

    #[test]
    fn the_chess_clock_charges_only_the_side_on_the_move() {
        let t0 = Instant::now();
        let mut ts = TimingState::new(TimingParams {
            main_clock: Some(Duration::from_millis(1000)),
            rope: None,
        });
        ts.arm(Side::Runner, true, t0);
        ts.settle(t0 + Duration::from_millis(400));
        ts.disarm(t0 + Duration::from_millis(400));
        assert_eq!(ts.remaining[1], Duration::from_millis(600));
        assert_eq!(ts.remaining[0], Duration::from_millis(1000), "the Corp was never waited on");
        assert_eq!(ts.flagged(t0 + Duration::from_millis(500)), None);
        ts.arm(Side::Runner, true, t0 + Duration::from_millis(500));
        assert_eq!(
            ts.flagged(t0 + Duration::from_millis(2000)),
            Some(Side::Runner),
            "a clock at zero flags"
        );
    }

    #[test]
    fn a_streak_of_three_clean_turns_banks_and_a_pop_resets() {
        let mut ts = TimingState::new(TimingParams {
            main_clock: None,
            rope: Some(RopeParams {
                action: Duration::from_millis(10),
                decision: Duration::from_millis(10),
                timeout_fuse: Duration::from_millis(10),
            }),
        });
        // Setup (seq 0) belongs to nobody.
        assert_eq!(ts.note_turn(1, Side::Corp), None);
        // Corp 1 → Runner 2 → Corp 3 → Runner 4 …: each old turn credits its owner.
        assert_eq!(ts.note_turn(2, Side::Runner), None); // corp streak 1
        assert_eq!(ts.note_turn(3, Side::Corp), None); // runner streak 1
        assert_eq!(ts.note_turn(4, Side::Runner), None); // corp streak 2
        assert_eq!(ts.note_turn(5, Side::Corp), None); // runner streak 2
        assert_eq!(ts.note_turn(6, Side::Runner), Some(Side::Corp), "3 clean corp turns bank");
        assert_eq!(ts.tokens_of(Side::Corp), 1);
        // The runner pops mid-turn: streak gone, and the turn will not count.
        let now = Instant::now();
        ts.arm(Side::Runner, false, now);
        assert_eq!(
            ts.pop(now + Duration::from_millis(20)),
            Some((Side::Runner, PopOutcome::AutoResolve))
        );
        assert_eq!(ts.note_turn(7, Side::Corp), None, "the popped turn is not clean");
        assert_eq!(ts.streak[1], 0);
    }

    #[test]
    fn a_second_consecutive_pop_is_a_loss_and_an_answer_breaks_the_chain() {
        let mut ts = TimingState::new(TimingParams {
            main_clock: None,
            rope: Some(RopeParams {
                action: Duration::from_millis(10),
                decision: Duration::from_millis(10),
                timeout_fuse: Duration::from_millis(10),
            }),
        });
        let t0 = Instant::now();
        ts.arm(Side::Runner, true, t0);
        assert_eq!(ts.pop(t0), Some((Side::Runner, PopOutcome::AutoResolve)));
        ts.arm(Side::Runner, false, t0);
        assert_eq!(ts.pop(t0), Some((Side::Runner, PopOutcome::Loss)), "second in a row");
        // Again, but with an answer in between: no loss.
        let mut ts2 = TimingState::new(ts.params.clone());
        ts2.arm(Side::Runner, true, t0);
        assert_eq!(ts2.pop(t0), Some((Side::Runner, PopOutcome::AutoResolve)));
        ts2.answered(Side::Runner);
        ts2.arm(Side::Runner, false, t0);
        assert_eq!(ts2.pop(t0), Some((Side::Runner, PopOutcome::AutoResolve)));
    }

    #[test]
    fn a_banked_timeout_fires_instead_of_popping_and_is_consumed() {
        let mut ts = TimingState::new(TimingParams {
            main_clock: None,
            rope: Some(RopeParams {
                action: Duration::from_millis(10),
                decision: Duration::from_millis(10),
                timeout_fuse: Duration::from_millis(70),
            }),
        });
        ts.tokens[1] = 1;
        ts.streak[1] = 2;
        let t0 = Instant::now();
        ts.arm(Side::Runner, true, t0);
        assert_eq!(ts.pop(t0), Some((Side::Runner, PopOutcome::TimeoutFired)));
        assert_eq!(ts.tokens_of(Side::Runner), 0, "consumed");
        let f = ts.fuse().expect("the fuse restarted");
        assert_eq!(f.kind, FuseKind::Timeout);
        assert_eq!(f.total, Duration::from_millis(70));
        assert_eq!(ts.streak[1], 0, "the rope ran out, so the streak resets (module doc)");
        assert!(!ts.popped_last[1], "a timeout fire is not a pop");
    }
}
