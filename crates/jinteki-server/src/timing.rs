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

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Per-decision overrun discipline: how long an action window or a decision
/// may dawdle before the rope starts burning, and how much fuse a timed-out
/// player has before the game concludes they are gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
