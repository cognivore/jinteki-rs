//! Abuse guard for the claim endpoint and outbound requests — a port of
//! draftroom's `guard.go` (itself ported from Cubehall's `auth-guard.ts`),
//! minus the proof-of-work layer held in reserve (ACCOUNTS-AND-DECKS.md
//! §4.4, OI-7).
//!
//! In-memory by design: "heuristics, not correctness-critical". A restart
//! forgets counters; the SendGrid global budget is the hard backstop.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Email send limits (draftroom guard.go:32-34): per-address cooldown 60 s,
/// daily cap 5 per address, global hourly budget 80 as a circuit breaker on
/// the shared SendGrid account's reputation.
const ADDR_COOLDOWN: Duration = Duration::from_secs(60);
const ADDR_DAILY_CAP: u32 = 5;
const GLOBAL_HOURLY_BUDGET: u32 = 80;

/// Per-IP backoff on claim POSTs (guard.go:267-303): 3 free per 10-minute
/// window, then 2 s, 8 s, 32 s ... capped at 5 minutes.
const IP_FREE_HITS: u32 = 3;
const IP_WINDOW: Duration = Duration::from_secs(600);
const IP_MAX_DELAY: Duration = Duration::from_secs(300);

/// NRDB import limit: 10 per user per minute (§7.2 — polite-guest manners).
const NRDB_PER_MIN: u32 = 10;

#[derive(Default)]
struct AddrState {
    last_send: Option<Instant>,
    day_start: Option<Instant>,
    day_count: u32,
}

#[derive(Default)]
struct IpState {
    window_start: Option<Instant>,
    hits: u32,
    blocked_until: Option<Instant>,
}

#[derive(Default)]
struct WindowCount {
    start: Option<Instant>,
    count: u32,
}

#[derive(Default)]
pub struct Guard {
    addr: Mutex<HashMap<String, AddrState>>,
    global_hour: Mutex<WindowCount>,
    ip: Mutex<HashMap<String, IpState>>,
    nrdb: Mutex<HashMap<String, WindowCount>>,
}

pub enum IpVerdict {
    Ok,
    /// Backed off; retry after this many seconds (429 + Retry-After).
    RetryAfter(u64),
}

impl Guard {
    pub fn new() -> Guard {
        Guard::default()
    }

    /// May we send a magic-link email to `email` right now? A `false` is
    /// logged and dropped by the caller; the HTTP response does not change
    /// (enumeration safety — draftroom email.go:50-56).
    pub fn allow_email(&self, email: &str) -> bool {
        let now = Instant::now();
        {
            let mut g = self.global_hour.lock().unwrap();
            match g.start {
                Some(s) if now.duration_since(s) < Duration::from_secs(3600) => {
                    if g.count >= GLOBAL_HOURLY_BUDGET {
                        return false;
                    }
                }
                _ => {
                    g.start = Some(now);
                    g.count = 0;
                }
            }
        }
        let mut map = self.addr.lock().unwrap();
        let st = map.entry(email.to_string()).or_default();
        if let Some(last) = st.last_send {
            if now.duration_since(last) < ADDR_COOLDOWN {
                return false;
            }
        }
        match st.day_start {
            Some(s) if now.duration_since(s) < Duration::from_secs(86_400) => {
                if st.day_count >= ADDR_DAILY_CAP {
                    return false;
                }
            }
            _ => {
                st.day_start = Some(now);
                st.day_count = 0;
            }
        }
        st.last_send = Some(now);
        st.day_count += 1;
        self.global_hour.lock().unwrap().count += 1;
        true
    }

    /// Per-IP exponential backoff on auth POSTs.
    pub fn check_ip(&self, ip: &str) -> IpVerdict {
        let now = Instant::now();
        let mut map = self.ip.lock().unwrap();
        let st = map.entry(ip.to_string()).or_default();
        if let Some(until) = st.blocked_until {
            if now < until {
                // Ceil: telling a client "retry after 1" when 1.9s remain
                // guarantees a second rejection.
                let remaining = until - now;
                return IpVerdict::RetryAfter(remaining.as_millis().div_ceil(1000).max(1) as u64);
            }
        }
        match st.window_start {
            Some(s) if now.duration_since(s) < IP_WINDOW => {}
            _ => {
                st.window_start = Some(now);
                st.hits = 0;
                st.blocked_until = None;
            }
        }
        st.hits += 1;
        if st.hits > IP_FREE_HITS {
            // 2 s, 8 s, 32 s ... x4 per extra hit, capped.
            let over = st.hits - IP_FREE_HITS - 1;
            let delay = Duration::from_secs(2)
                .saturating_mul(4u32.saturating_pow(over))
                .min(IP_MAX_DELAY);
            st.blocked_until = Some(now + delay);
            return IpVerdict::RetryAfter(delay.as_secs().max(1));
        }
        IpVerdict::Ok
    }

    /// Per-user NRDB import limit.
    pub fn allow_nrdb(&self, user_id: &str) -> bool {
        let now = Instant::now();
        let mut map = self.nrdb.lock().unwrap();
        let st = map.entry(user_id.to_string()).or_default();
        match st.start {
            Some(s) if now.duration_since(s) < Duration::from_secs(60) => {
                if st.count >= NRDB_PER_MIN {
                    return false;
                }
            }
            _ => {
                st.start = Some(now);
                st.count = 0;
            }
        }
        st.count += 1;
        true
    }
}

/// Client IP: `X-Real-IP`, else first `X-Forwarded-For` hop (Caddy sets
/// them), else the placeholder for direct connections (guard.go:65-76).
pub fn client_ip(headers: &axum::http::HeaderMap) -> String {
    if let Some(v) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let v = v.trim();
        if !v.is_empty() {
            return v.to_string();
        }
    }
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            let first = first.trim();
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    "direct".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_cooldown_blocks_immediate_resend() {
        let g = Guard::new();
        assert!(g.allow_email("a@example.com"));
        assert!(!g.allow_email("a@example.com")); // 60 s cooldown
        assert!(g.allow_email("b@example.com")); // other addresses unaffected
    }

    #[test]
    fn ip_backoff_after_three_free_hits() {
        let g = Guard::new();
        for _ in 0..3 {
            assert!(matches!(g.check_ip("1.2.3.4"), IpVerdict::Ok));
        }
        let IpVerdict::RetryAfter(s) = g.check_ip("1.2.3.4") else {
            panic!("fourth hit should back off");
        };
        assert_eq!(s, 2);
        let IpVerdict::RetryAfter(s2) = g.check_ip("1.2.3.4") else {
            panic!("still backed off");
        };
        assert!(s2 >= 2);
        assert!(matches!(g.check_ip("5.6.7.8"), IpVerdict::Ok));
    }

    #[test]
    fn nrdb_limit_ten_per_minute() {
        let g = Guard::new();
        for _ in 0..10 {
            assert!(g.allow_nrdb("u1"));
        }
        assert!(!g.allow_nrdb("u1"));
        assert!(g.allow_nrdb("u2"));
    }
}
