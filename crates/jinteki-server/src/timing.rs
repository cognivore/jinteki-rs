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
//! (`None` = untimed), `rope` is the overrun discipline (`None` = no rope).
//! The DEFAULT — what a lobby gets when the host touches nothing — is 30
//! minutes a side, roped.
//!
//! # The rope is a RESERVOIR, not a per-prompt fuse
//!
//! Each player holds ONE bank of calm time. It drains in real time only
//! while the game is waiting on them, and while it is positive NOTHING
//! about the rope is on their screen. Every action they COMPLETE (CR 5.2.5,
//! the [`jinteki_cr::change::GameChange::ActionCompleted`] record — not a
//! mere decision, which is what the bank pays for) adds `action_increment`
//! to it, with no ceiling. Only when the bank is empty does a rope appear
//! and burn.
//!
//! EVERY TURN IS A FRESH MINUTE: at each turn boundary both players' banks
//! are SET to `calm` ([`TimingState::note_turn`]). A minute to start the
//! turn with, ten more seconds for every action taken in it, and none of it
//! carried into the next turn — the reset is what stops a fast player
//! hoarding an hour of rope-proofing, and it is also what lifts a player who
//! was roped a moment ago back into calm when their new turn begins.
//!
//! A player who is playing therefore never sees a rope at all; a player who
//! has stopped playing sees one, and taking an action lifts them off it —
//! whoever pressed the button. The house's auto-played credits pay the bank
//! exactly as a human's do; what they do NOT do is break the
//! consecutive-burn-out chain, because only a player answering for
//! themselves does that. That is what keeps the ladder terminating for a
//! player who has walked away: their banked ⌛ fire one by one, and then two
//! burn-outs with no human answer between them end the game.
//!
//! The consequence ladder for a rope that burns all the way out is
//! unchanged — see [`PopOutcome`].
//!
//! The per-prompt fuse this replaced was a real bug and not only a design
//! mistake: the board's two-tap targeting grammar (arm, then confirm) needs
//! more seconds than a 10-second decision fuse gave it, so choosing which
//! remote to install into was routinely auto-answered out from under the
//! player between their first and second click.

use jinteki_cr::object::Side;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// The overrun discipline: how much calm time a player banks, what an
/// action pays back into it, and how long the rope burns once it is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RopeConfig {
    /// THE MINUTE EVERY TURN STARTS WITH. At each turn boundary both
    /// players' banks are SET to this — "before rope even appears the
    /// player has 1 minute" — and from there "every action adds 10
    /// ADDITIONAL seconds", with no ceiling inside the turn.
    ///
    /// Set, not raised: the reset is what stops a bank being hoarded across
    /// turns, so a long turn's earnings are spent in the turn that earned
    /// them. A player who is on the rope when a turn boundary arrives is
    /// lifted off it by the reset, which is right — a new turn is a new
    /// minute. See [`TimingState::note_turn`], the one place it happens.
    pub calm_secs: u32,
    /// The bank both players start the game holding, before any turn has
    /// begun — the opening keep/mulligan window is allowed to be a long
    /// think. The first turn boundary hands over to `calm_secs`.
    pub opening_calm_secs: u32,
    /// What one COMPLETED action pays back into the bank.
    pub action_increment_secs: u32,
    /// Seconds of burning rope once the bank is empty, before the burn-out
    /// consequence lands.
    pub rope_secs: u32,
}

impl Default for RopeConfig {
    fn default() -> Self {
        Self {
            calm_secs: 60,
            opening_calm_secs: 120,
            action_increment_secs: 10,
            rope_secs: 30,
        }
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
                calm_secs: f("calm_secs", d.calm_secs),
                opening_calm_secs: f("opening_calm_secs", d.opening_calm_secs),
                action_increment_secs: f("action_increment_secs", d.action_increment_secs),
                rope_secs: f("rope_secs", d.rope_secs),
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
        assert_eq!(
            t.rope,
            Some(RopeConfig {
                calm_secs: 60,
                opening_calm_secs: 120,
                action_increment_secs: 10,
                rope_secs: 30
            })
        );
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
            "rope": {"calm_secs": 99999, "opening_calm_secs": 0,
                     "action_increment_secs": 10, "rope_secs": 30}
        }));
        assert_eq!(t.main_clock_secs, Some(60));
        let r = t.rope.unwrap();
        assert_eq!(
            (r.calm_secs, r.opening_calm_secs, r.action_increment_secs, r.rope_secs),
            (600, 1, 10, 30)
        );
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
// Params (Duration-based, so tests run millisecond reservoirs)
// ───────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct TimingParams {
    pub main_clock: Option<Duration>,
    pub rope: Option<RopeParams>,
}

#[derive(Debug, Clone)]
pub struct RopeParams {
    /// The bank's cap, and so the steady-state calm time.
    pub calm: Duration,
    /// The bank both sides open the game holding.
    pub opening_calm: Duration,
    /// What one completed action pays back into the bank.
    pub action_increment: Duration,
    /// How long the rope burns once the bank is empty.
    pub rope: Duration,
}

impl From<&TimingConfig> for TimingParams {
    fn from(c: &TimingConfig) -> Self {
        TimingParams {
            main_clock: c.main_clock_secs.map(|s| Duration::from_secs(s as u64)),
            rope: c.rope.as_ref().map(|r| RopeParams {
                calm: Duration::from_secs(r.calm_secs as u64),
                opening_calm: Duration::from_secs(r.opening_calm_secs as u64),
                action_increment: Duration::from_secs(r.action_increment_secs as u64),
                rope: Duration::from_secs(r.rope_secs as u64),
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

/// What a burn-out resolves to, decided HERE so the consecutive-burn-out and
/// token bookkeeping cannot drift from the answer the caller then gives the
/// VM. The ladder is unchanged from the fuse the reservoir replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopOutcome {
    /// A banked ⌛ fired: the rope has been relit at `rope`, the token is
    /// consumed, the streak is reset. NOT a burn-out.
    TimeoutFired,
    /// First burn-out: auto-resolve the prompt (credits for an action
    /// window, the neutral default for anything else).
    AutoResolve,
    /// Second consecutive burn-out: that player loses.
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
    /// Whose clocks are running, and since when (the last settle point).
    /// `Some` exactly while that side owes a decision.
    running: Option<(Side, Instant)>,
    /// THE RESERVOIR: calm time in hand, per side. Drains only while that
    /// side is the one being waited on; while it is positive there is no
    /// rope on their screen at all.
    bank: [Duration; 2],
    /// What is left of the rope that lights when the bank empties. Relit to
    /// `params.rope` whenever the bank is credited or a burn-out resolves,
    /// so a player who comes off the rope gets a whole one next time.
    rope_left: [Duration; 2],
    /// Did this side's rope burn out with nothing answered BY THEM since?
    ///
    /// The whole of the consecutive-burn-out ladder, and the reason it
    /// terminates for an absent player: nothing the game does on their
    /// behalf touches this. Not an auto-resolve, not a house-played credit,
    /// not the turn boundary refilling their bank. Only [`Self::answered`]
    /// — a human answering a prompt — clears it, so two burn-outs with no
    /// human answer between them are always a loss, however much calm time
    /// the reservoir handed back in between.
    burnt_last: [bool; 2],
    /// Banked ⌛ per side.
    tokens: [u32; 2],
    /// Consecutive clean own-turns per side (3 banks a token and resets).
    streak: [u32; 2],
    /// Has the current turn already been dirtied for its owner (a burn-out
    /// or a timeout fire anywhere resets the streak; this flag additionally
    /// keeps the turn it happened in from counting when it ends).
    turn_dirty: [bool; 2],
    /// Last observed `(turn_seq, turn_side)`, to notice turn boundaries.
    seen_turn: (u64, Side),
}

impl TimingState {
    pub fn new(params: TimingParams) -> Self {
        let main = params.main_clock.unwrap_or(Duration::ZERO);
        let (open, rope) = match params.rope.as_ref() {
            Some(r) => (r.opening_calm, r.rope),
            None => (Duration::ZERO, Duration::ZERO),
        };
        TimingState {
            params,
            remaining: [main, main],
            running: None,
            bank: [open, open],
            rope_left: [rope, rope],
            burnt_last: [false, false],
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
    /// The side the game is currently waiting on, if any — the only side
    /// whose clocks are moving.
    pub fn running_side(&self) -> Option<Side> {
        self.running.map(|(s, _)| s)
    }
    /// That side's calm bank as last settled.
    pub fn bank_of(&self, side: Side) -> Duration {
        self.bank[six(side)]
    }
    /// Is a rope VISIBLY burning on someone's screen right now? (Only the
    /// side being waited on can be on one, and only with an empty bank.)
    pub fn roped(&self) -> Option<Side> {
        self.params.rope.as_ref()?;
        let side = self.running_side()?;
        self.bank[six(side)].is_zero().then_some(side)
    }

    /// Move the running clocks forward to `now`. Linear and drift-free:
    /// elapsed time is charged to the running side's main clock and to
    /// their reservoir — the bank first, and only what the bank could not
    /// pay for comes off the rope.
    pub fn settle(&mut self, now: Instant) {
        let Some((side, since)) = self.running else { return };
        let spent = now.saturating_duration_since(since);
        let i = six(side);
        if self.params.main_clock.is_some() {
            self.remaining[i] = self.remaining[i].saturating_sub(spent);
        }
        if self.params.rope.is_some() {
            let from_bank = spent.min(self.bank[i]);
            self.bank[i] -= from_bank;
            self.rope_left[i] = self.rope_left[i].saturating_sub(spent - from_bank);
        }
        self.running = Some((side, now));
    }

    /// The engine just put a decision to `side`: their main clock starts and
    /// their reservoir starts draining. Nothing is sized per prompt — the
    /// bank is the whole budget, and it is the same bank as a moment ago.
    pub fn arm(&mut self, side: Side, now: Instant) {
        self.settle(now);
        self.running = Some((side, now));
    }

    /// The decision is no longer waiting on anyone (answered, auto-resolved,
    /// or the game ended): the clocks stop where they are.
    pub fn disarm(&mut self, now: Instant) {
        self.settle(now);
        self.running = None;
    }

    /// `side` ANSWERED a prompt themselves. That is what breaks a
    /// consecutive-burn-out chain, and what puts them back in charge of
    /// their own reservoir (see [`Self::burnt_last`]). Auto-resolutions
    /// never call this.
    pub fn answered(&mut self, side: Side) {
        self.burnt_last[six(side)] = false;
    }

    /// `side` COMPLETED AN ACTION (CR 5.2.5): their bank is paid
    /// `action_increment` ADDITIONAL seconds — no ceiling; the turn boundary
    /// is what bounds a bank, not a cap — and the rope they would meet at
    /// the bottom of it is relit whole.
    ///
    /// It does not matter whose hand pressed the button. An action the house
    /// plays for a roped player is still an action completed, and the bank
    /// is a budget for deciding, not a reward for attention. What the house
    /// cannot do on a player's behalf is BREAK THE CHAIN — see
    /// [`Self::answered`] — so a player who has walked away still runs out
    /// of ladder.
    pub fn credit_action(&mut self, side: Side, now: Instant) {
        let Some(r) = self.params.rope.as_ref() else { return };
        let (inc, rope) = (r.action_increment, r.rope);
        let i = six(side);
        self.settle(now);
        self.bank[i] += inc;
        self.rope_left[i] = rope;
    }

    /// A side whose main clock has reached zero, if any.
    pub fn flagged(&mut self, now: Instant) -> Option<Side> {
        self.params.main_clock?;
        self.settle(now);
        [Side::Corp, Side::Runner]
            .into_iter()
            .find(|s| self.remaining[six(*s)] == Duration::ZERO)
    }

    /// Has the running side's rope burnt all the way out? Reads the SETTLED
    /// state, so callers settle first (the ticker does).
    pub fn rope_burnt(&self) -> bool {
        self.roped().is_some_and(|s| self.rope_left[six(s)].is_zero())
    }

    /// The rope burnt out: decide what that means for its owner, and do the
    /// token/streak/consecutive-burn-out bookkeeping for that meaning. The
    /// caller (who owns the VM) acts on the outcome. The rope is relit
    /// either way — the ladder's second step is a SECOND burn-out, so a
    /// player who has just had one must be given another rope to burn.
    pub fn pop(&mut self, now: Instant) -> Option<(Side, PopOutcome)> {
        let side = self.roped()?;
        let i = six(side);
        if self.rope_left[i] > Duration::ZERO {
            return None;
        }
        let _ = now;
        // The rope ran out — however this resolves, the current turn is no
        // longer clean and the streak restarts (see the module doc for why
        // this includes a timeout fire).
        self.streak[i] = 0;
        self.turn_dirty[i] = true;
        self.rope_left[i] = self.params.rope.as_ref().map_or(Duration::ZERO, |r| r.rope);
        if self.tokens[i] > 0 {
            // A banked ⌛ fires instead of a burn-out: consume it, and the
            // relit rope above is the restart it buys.
            self.tokens[i] -= 1;
            return Some((side, PopOutcome::TimeoutFired));
        }
        if self.burnt_last[i] {
            return Some((side, PopOutcome::Loss));
        }
        self.burnt_last[i] = true;
        Some((side, PopOutcome::AutoResolve))
    }

    /// Watch the VM's `(turn_seq, turn_side)` for turn boundaries — THE ONE
    /// PLACE A BANK IS RESET, and where a clean own-turn feeds the ⌛ streak.
    ///
    /// A new turn sets BOTH banks to `calm` and relights both ropes. Both,
    /// not just the turn's owner: the player who is not on the move spends
    /// their bank on paid windows and reactions all through the opponent's
    /// turn, so they need the same fresh minute. SET, not raised — a bank is
    /// spent in the turn that earned it, and nothing is hoarded across one.
    ///
    /// Returns the side that just banked a ⌛, if any. `turn_seq` 0 is setup
    /// and belongs to nobody, but it is still a boundary: the opening
    /// `opening_calm` gives way to `calm` when the first turn begins.
    pub fn note_turn(&mut self, turn_seq: u64, turn_side: Side, now: Instant) -> Option<Side> {
        let Some(r) = self.params.rope.as_ref() else { return None };
        let (calm, rope) = (r.calm, r.rope);
        if (turn_seq, turn_side) == self.seen_turn {
            return None;
        }
        let (old_seq, old_side) = self.seen_turn;
        self.seen_turn = (turn_seq, turn_side);
        // Charge the drain up to the boundary before the reset overwrites it,
        // so the anchor moves too: without this the seconds spent deciding in
        // the OLD turn would be taken out of the new turn's fresh minute.
        self.settle(now);
        self.bank = [calm, calm];
        self.rope_left = [rope, rope];
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

    /// The reservoir of the side on the clock, projected to `now` without
    /// mutating: `(bank, rope_left)`.
    fn projected(&self, side: Side, now: Instant) -> (Duration, Duration) {
        let i = six(side);
        let (mut bank, mut rope) = (self.bank[i], self.rope_left[i]);
        if let Some((rs, since)) = self.running {
            if rs == side {
                let spent = now.saturating_duration_since(since);
                let from_bank = spent.min(bank);
                bank -= from_bank;
                rope = rope.saturating_sub(spent - from_bank);
            }
        }
        (bank, rope)
    }

    /// The wire shape, from this VIEWER's seat: both main clocks and whose is
    /// running are open information; the ⌛ count is the viewer's OWN only
    /// (the opponent's is never shown — nor sent). `None` when the game is
    /// untimed, so untimed games carry no timing key at all.
    ///
    /// The `rope` key describes the RESERVOIR of the side being waited on:
    /// `bank_ms` is their calm time left, `visible` says whether the rope is
    /// actually burning (which is the only thing the client draws), and
    /// `rope_ms_left`/`rope_total_ms` size that burn.
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
        if let Some(r) = self.params.rope.as_ref() {
            m.insert(
                "rope".into(),
                match (self.running, over) {
                    (Some((side, _)), false) => {
                        let (bank, rope) = self.projected(side, now);
                        json!({
                            "side": side_key(side),
                            "bank_ms": bank.as_millis() as u64,
                            "visible": bank.is_zero(),
                            "rope_ms_left": rope.as_millis() as u64,
                            "rope_total_ms": r.rope.as_millis() as u64,
                        })
                    }
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

    const MS: fn(u64) -> Duration = Duration::from_millis;

    /// A millisecond-scale reservoir: 100ms of calm, 200ms to open with,
    /// 40ms an action, a 60ms rope.
    fn reservoir() -> TimingParams {
        TimingParams {
            main_clock: None,
            rope: Some(RopeParams {
                calm: MS(100),
                opening_calm: MS(200),
                action_increment: MS(40),
                rope: MS(60),
            }),
        }
    }

    #[test]
    fn the_defaults_are_the_spec_defaults() {
        let r = RopeConfig::default();
        assert_eq!(
            (r.calm_secs, r.opening_calm_secs, r.action_increment_secs, r.rope_secs),
            (60, 120, 10, 30)
        );
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
        assert_eq!(
            (r.calm_secs, r.opening_calm_secs, r.action_increment_secs, r.rope_secs),
            (60, 120, 10, 30)
        );
    }

    #[test]
    fn the_opening_bank_is_the_opening_calm_until_the_first_turn_begins() {
        let mut ts = TimingState::new(reservoir());
        assert_eq!(ts.bank_of(Side::Corp), MS(200), "both sides open on the long bank");
        assert_eq!(ts.bank_of(Side::Runner), MS(200));
        let t0 = Instant::now();
        ts.arm(Side::Runner, t0);
        // The keep/mulligan think spends the LONG bank…
        ts.settle(t0 + MS(50));
        assert_eq!(ts.bank_of(Side::Runner), MS(150));
        // …and then the first turn hands over to the steady minute.
        ts.note_turn(1, Side::Corp, t0 + MS(50));
        assert_eq!(ts.bank_of(Side::Runner), MS(100));
        assert_eq!(ts.bank_of(Side::Corp), MS(100));
    }

    #[test]
    fn actions_add_without_a_ceiling_and_every_turn_sets_the_bank_back() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        ts.note_turn(1, Side::Runner, t0);
        ts.arm(Side::Runner, t0);
        assert_eq!(ts.bank_of(Side::Runner), MS(100), "the turn opens on the calm minute");
        // Five actions inside one turn: +40 each, and NOTHING caps them.
        for n in 1..=5 {
            ts.credit_action(Side::Runner, t0);
            assert_eq!(
                ts.bank_of(Side::Runner),
                MS(100 + 40 * n),
                "action {n} adds 40 additional, with no ceiling"
            );
        }
        assert_eq!(ts.bank_of(Side::Runner), MS(300), "well past `calm`");
        // The next turn SETS it back — a big bank is not hoarded across one.
        ts.note_turn(2, Side::Corp, t0);
        assert_eq!(ts.bank_of(Side::Runner), MS(100), "set, not raised");
        assert_eq!(ts.bank_of(Side::Corp), MS(100), "both sides, not just the mover");
    }

    #[test]
    fn a_turn_boundary_lifts_a_roped_player_back_into_calm() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        ts.note_turn(1, Side::Runner, t0);
        ts.arm(Side::Runner, t0);
        // Idle away the whole minute: on the rope, halfway through it.
        ts.settle(t0 + MS(130));
        assert_eq!(ts.roped(), Some(Side::Runner));
        assert_eq!(ts.rope_left[1], MS(30));
        // A new turn is a new minute, so the rope goes out.
        ts.note_turn(2, Side::Corp, t0 + MS(130));
        assert_eq!(ts.roped(), None, "un-roped by the reset");
        assert_eq!(ts.bank_of(Side::Runner), MS(100));
        assert_eq!(ts.rope_left[1], MS(60), "with a whole rope waiting under it");
    }

    #[test]
    fn a_fast_player_never_sees_a_rope() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        let mut now = t0;
        ts.arm(Side::Corp, now);
        // Ten decisions, each answered in 30ms, each an action: the bank
        // never empties, so nothing is ever visible.
        for _ in 0..10 {
            now += MS(30);
            ts.settle(now);
            assert_eq!(ts.roped(), None, "the bank is still positive");
            assert!(!ts.rope_burnt());
            ts.credit_action(Side::Corp, now);
            ts.disarm(now);
            ts.arm(Side::Corp, now);
        }
        assert!(ts.bank_of(Side::Corp) > Duration::ZERO);
        let j = ts.json(Side::Corp, now, false).unwrap();
        assert_eq!(j["rope"]["visible"], json!(false), "no rope on a fast player's screen");
        assert!(j["rope"]["bank_ms"].as_u64().unwrap() > 0);
    }

    #[test]
    fn the_bank_empties_the_rope_shows_and_an_action_lifts_you_off_it() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        ts.arm(Side::Runner, t0);
        // Idle through the whole opening bank: the rope appears.
        ts.settle(t0 + MS(220));
        assert_eq!(ts.roped(), Some(Side::Runner));
        assert!(!ts.rope_burnt(), "it is burning, not burnt");
        let j = ts.json(Side::Runner, t0 + MS(220), false).unwrap();
        assert_eq!(j["rope"]["visible"], json!(true));
        assert_eq!(j["rope"]["bank_ms"], json!(0));
        assert_eq!(j["rope"]["rope_total_ms"], json!(60));
        assert_eq!(j["rope"]["rope_ms_left"], json!(40), "20ms of the 60ms rope is gone");
        // Acting mid-rope lifts them off it, with a whole rope waiting below.
        ts.credit_action(Side::Runner, t0 + MS(220));
        assert_eq!(ts.roped(), None, "off the rope");
        assert_eq!(ts.bank_of(Side::Runner), MS(40));
        let j = ts.json(Side::Runner, t0 + MS(220), false).unwrap();
        assert_eq!(j["rope"]["visible"], json!(false));
        assert_eq!(j["rope"]["rope_ms_left"], json!(60), "the rope is whole again");
    }

    #[test]
    fn the_bank_drains_only_while_the_game_waits_on_that_player() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        ts.arm(Side::Runner, t0);
        ts.settle(t0 + MS(50));
        assert_eq!(ts.bank_of(Side::Runner), MS(150));
        assert_eq!(ts.bank_of(Side::Corp), MS(200), "the Corp was never waited on");
        // The Corp's turn: the Runner's bank stands still.
        ts.disarm(t0 + MS(50));
        ts.arm(Side::Corp, t0 + MS(50));
        ts.settle(t0 + MS(500));
        assert_eq!(ts.bank_of(Side::Runner), MS(150), "held while it was not their decision");
        assert_eq!(ts.bank_of(Side::Corp), Duration::ZERO);
    }

    #[test]
    fn the_chess_clock_charges_only_the_side_on_the_move() {
        let t0 = Instant::now();
        let mut ts = TimingState::new(TimingParams { main_clock: Some(MS(1000)), rope: None });
        ts.arm(Side::Runner, t0);
        ts.settle(t0 + MS(400));
        ts.disarm(t0 + MS(400));
        assert_eq!(ts.remaining[1], MS(600));
        assert_eq!(ts.remaining[0], MS(1000), "the Corp was never waited on");
        assert_eq!(ts.flagged(t0 + MS(500)), None);
        ts.arm(Side::Runner, t0 + MS(500));
        assert_eq!(ts.flagged(t0 + MS(2000)), Some(Side::Runner), "a clock at zero flags");
    }

    #[test]
    fn a_streak_of_three_clean_turns_banks_and_a_burn_out_resets() {
        let mut ts = TimingState::new(reservoir());
        let t = Instant::now();
        // Setup (seq 0) belongs to nobody.
        assert_eq!(ts.note_turn(1, Side::Corp, t), None);
        // Corp 1 → Runner 2 → Corp 3 → Runner 4 …: each old turn credits its owner.
        assert_eq!(ts.note_turn(2, Side::Runner, t), None); // corp streak 1
        assert_eq!(ts.note_turn(3, Side::Corp, t), None); // runner streak 1
        assert_eq!(ts.note_turn(4, Side::Runner, t), None); // corp streak 2
        assert_eq!(ts.note_turn(5, Side::Corp, t), None); // runner streak 2
        assert_eq!(ts.note_turn(6, Side::Runner, t), Some(Side::Corp), "3 clean corp turns bank");
        assert_eq!(ts.tokens_of(Side::Corp), 1);
        // The runner burns out mid-turn: streak gone, and the turn will not count.
        let now = Instant::now();
        ts.arm(Side::Runner, now);
        ts.settle(now + MS(300));
        assert_eq!(ts.pop(now + MS(300)), Some((Side::Runner, PopOutcome::AutoResolve)));
        assert_eq!(ts.note_turn(7, Side::Corp, now + MS(300)), None, "the burnt turn is not clean");
        assert_eq!(ts.streak[1], 0);
    }

    #[test]
    fn a_second_consecutive_burn_out_is_a_loss_and_an_answer_breaks_the_chain() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        ts.arm(Side::Runner, t0);
        ts.settle(t0 + MS(300));
        assert_eq!(ts.pop(t0 + MS(300)), Some((Side::Runner, PopOutcome::AutoResolve)));
        // The rope was relit, so a second burn-out takes another rope's worth.
        assert_eq!(ts.rope_left[1], MS(60));
        ts.settle(t0 + MS(400));
        assert_eq!(ts.pop(t0 + MS(400)), Some((Side::Runner, PopOutcome::Loss)), "second in a row");
        // Again, but with an answer in between: no loss.
        let mut ts2 = TimingState::new(reservoir());
        ts2.arm(Side::Runner, t0);
        ts2.settle(t0 + MS(300));
        assert_eq!(ts2.pop(t0 + MS(300)), Some((Side::Runner, PopOutcome::AutoResolve)));
        ts2.answered(Side::Runner);
        ts2.settle(t0 + MS(400));
        assert_eq!(ts2.pop(t0 + MS(400)), Some((Side::Runner, PopOutcome::AutoResolve)));
    }

    #[test]
    fn the_house_playing_for_a_roped_player_fills_the_bank_as_if_clicked() {
        let mut ts = TimingState::new(reservoir());
        let t0 = Instant::now();
        ts.arm(Side::Runner, t0);
        ts.settle(t0 + MS(300));
        assert_eq!(ts.pop(t0 + MS(300)), Some((Side::Runner, PopOutcome::AutoResolve)));
        // The house plays out the turn on their behalf: four basic credits.
        // The bank does not care whose hand pressed the button.
        for n in 1..=4 {
            ts.credit_action(Side::Runner, t0 + MS(300));
            assert_eq!(ts.bank_of(Side::Runner), MS(40 * n), "auto-credit {n} pays like any other");
        }
        assert_eq!(ts.roped(), None, "and it lifts them off the rope, as clicking would");
        // What the house CANNOT do is speak for them: the chain is still open,
        // so the next burn-out is still the second in a row.
        assert!(ts.burnt_last[1], "no auto-play breaks the chain");
        ts.settle(t0 + MS(600));
        assert_eq!(ts.pop(t0 + MS(600)), Some((Side::Runner, PopOutcome::Loss)));
    }

    #[test]
    fn an_absent_player_runs_out_of_ladder_in_a_finite_number_of_turns() {
        // The whole arc the rules describe for someone who walked away: the
        // rope burns out, their banked ⌛ fire one at a time, and then two
        // burn-outs with no human answer between them end it. Nothing the
        // game does on their behalf — the turn reset refilling their minute,
        // the house's auto-credits, the auto-resolves — may keep it going.
        let mut ts = TimingState::new(reservoir());
        ts.tokens[1] = 3;
        let mut now = Instant::now();
        let mut fired = 0;
        let mut auto = 0;
        let mut outcome = None;
        for turn in 1..200u64 {
            // A new turn hands them a fresh minute and a whole rope.
            ts.note_turn(turn, Side::Runner, now);
            ts.arm(Side::Runner, now);
            // They are not there, so it all burns: bank, then rope.
            now += MS(200);
            ts.settle(now);
            assert!(ts.rope_burnt(), "turn {turn}: an untouched reservoir empties");
            let (side, what) = ts.pop(now).expect("a burnt rope resolves");
            assert_eq!(side, Side::Runner);
            match what {
                PopOutcome::TimeoutFired => fired += 1,
                PopOutcome::AutoResolve => {
                    auto += 1;
                    // The house plays the rest of the turn as credits, which
                    // now DOES pay the bank — and still does not save them.
                    for _ in 0..4 {
                        ts.credit_action(Side::Runner, now);
                    }
                }
                PopOutcome::Loss => {
                    outcome = Some(turn);
                    break;
                }
            }
            ts.disarm(now);
        }
        assert_eq!(fired, 3, "every banked ⌛ fires, one per burn-out");
        assert_eq!(auto, 1, "then one auto-resolve");
        assert_eq!(outcome, Some(5), "and the fifth burn-out is the loss");
    }

    #[test]
    fn a_banked_timeout_fires_instead_of_burning_out_and_is_consumed() {
        let mut ts = TimingState::new(reservoir());
        ts.tokens[1] = 1;
        ts.streak[1] = 2;
        let t0 = Instant::now();
        ts.arm(Side::Runner, t0);
        ts.settle(t0 + MS(300));
        assert_eq!(ts.pop(t0 + MS(300)), Some((Side::Runner, PopOutcome::TimeoutFired)));
        assert_eq!(ts.tokens_of(Side::Runner), 0, "consumed");
        assert_eq!(ts.rope_left[1], MS(60), "the rope restarted whole");
        assert_eq!(ts.roped(), Some(Side::Runner), "and it is still their rope to burn");
        assert_eq!(ts.streak[1], 0, "the rope ran out, so the streak resets (module doc)");
        assert!(!ts.burnt_last[1], "a timeout fire is not a burn-out");
    }
}
