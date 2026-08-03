//! The CR lobby: human vs human on our own server, same VM as the bot mode.
//!
//! A lobby entry is a person sitting down at one seat of an eternal-decks
//! game and waiting for someone to take the other. Nothing is a game until
//! both seats are filled; the moment the second player joins, the two seats
//! become one [`crate::cr::CrGame`] with a bot in neither of them.
//!
//! **The gate is the same gate (SYS-D-12).** `lobby-create` evaluates
//! [`crate::cr::readiness`] and refuses with the identical payload
//! `start` refuses with, so the UI shows the identical honest screen. The
//! mode opens by itself the moment the card layer closes — no deploy, no flag.
//!
//! **Two seats, two viewpoints, two tokens.** Each player gets their own
//! resume token (the creator's is minted while they wait, so closing the tab
//! before anyone joins loses nothing), and every frame either of them ever
//! receives comes from `view_of(their own side)` (SYS-S-1). There is no "the
//! state" in this file: there are two states, and neither is derived from the
//! other.
//!
//! **The nudge bus.** One socket cannot poll another's game, so a change
//! announces itself: [`Nudge::Lobby`] when the open-games list moves, and
//! [`Nudge::Game`] when a game with that key advances. `local::handle`
//! selects over its socket and this bus, and answers a nudge by pushing its
//! OWN seat's view — a nudge carries no game data, only a name.

use crate::cr::{self, SeatState};
use jinteki_cr::object::Side;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, Mutex};

// ───────────────────────────────────────────────────────────────────────────
// The nudge bus
// ───────────────────────────────────────────────────────────────────────────

/// A thing that changed. Never carries state — a socket answers a nudge by
/// serializing its own seat's view, which is the only path that exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nudge {
    /// The open-games list changed (created, joined, cancelled, GC'd).
    Lobby,
    /// The game with this key advanced.
    Game(String),
}

fn bus() -> &'static broadcast::Sender<Nudge> {
    static BUS: OnceLock<broadcast::Sender<Nudge>> = OnceLock::new();
    BUS.get_or_init(|| broadcast::channel(256).0)
}

pub fn subscribe() -> broadcast::Receiver<Nudge> {
    bus().subscribe()
}

pub fn nudge(n: Nudge) {
    // No subscribers is not an error: a game with one player in it is a
    // perfectly ordinary state of the world.
    let _ = bus().send(n);
}

// ───────────────────────────────────────────────────────────────────────────
// Open games
// ───────────────────────────────────────────────────────────────────────────

/// A seat waiting for an opponent.
#[derive(Clone, Debug)]
pub struct Open {
    pub id: String,
    pub title: String,
    /// The creator's display name (`auth.rs`), as the other player sees it.
    pub creator: String,
    pub creator_user: Option<String>,
    /// The side the creator took; the joiner takes the other.
    pub side: Side,
    pub seed: u64,
    /// The creator's resume token, minted while they wait, so that closing
    /// the tab before anyone joins loses nothing.
    pub token: String,
    created: Instant,
    created_unix: u64,
}

impl Open {
    pub fn to_json(&self) -> Value {
        let age = self.created.elapsed().as_secs();
        json!({
            "gameid": self.id,
            "title": self.title,
            "creator": self.creator,
            // The creator's side, and the seat still going begging.
            "side": side_key(self.side),
            "open-side": side_key(self.side.other()),
            "open-deck": deck_title(self.side.other()),
            "format": "eternal",
            "started": false,
            "created-at": self.created_unix,
            "age-seconds": age,
            // Bridge-shaped too, so a lobby row renders with the same code.
            "players": [{"user": {"username": self.creator}, "side": side_name(self.side)}],
        })
    }
}

type Opens = Arc<Mutex<HashMap<String, Open>>>;

fn opens() -> Opens {
    static OPENS: OnceLock<Opens> = OnceLock::new();
    OPENS.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

/// An open seat nobody took in a day is not an invitation any more.
const OPEN_TTL: Duration = Duration::from_secs(24 * 3600);

/// Drop every open game older than [`OPEN_TTL`]. Returns how many went, so a
/// sweep that changed the list can say so.
async fn prune(map: &mut HashMap<String, Open>) -> usize {
    let before = map.len();
    map.retain(|_, o| o.created.elapsed() <= OPEN_TTL);
    before - map.len()
}

/// The daily sweep: an open seat nobody took in 24h stops being an
/// invitation. Every lobby operation prunes as it goes; this is for the
/// server that nobody has opened the lobby on for days.
pub async fn gc() -> usize {
    let reg = opens();
    let mut map = reg.lock().await;
    let gone = prune(&mut map).await;
    drop(map);
    if gone > 0 {
        nudge(Nudge::Lobby);
    }
    gone
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The open-games list, newest first.
pub async fn list_json() -> Value {
    let reg = opens();
    let mut map = reg.lock().await;
    let gone = prune(&mut map).await;
    let mut rows: Vec<&Open> = map.values().collect();
    rows.sort_by(|a, b| b.created_unix.cmp(&a.created_unix).then(a.id.cmp(&b.id)));
    let list: Vec<Value> = rows.iter().map(|o| o.to_json()).collect();
    drop(map);
    if gone > 0 {
        nudge(Nudge::Lobby);
    }
    json!({"type": "lobby-list", "list": list})
}

/// The open game a token is waiting in, if any.
pub async fn by_token(token: &str) -> Option<Open> {
    let reg = opens();
    let map = reg.lock().await;
    map.values().find(|o| o.token == token).cloned()
}

/// Sit down at one seat of a new eternal game and wait. The gate is checked
/// by the caller (which owns the socket the refusal goes down).
pub async fn create(
    title: &str,
    creator: &str,
    creator_user: Option<String>,
    side: Side,
    seed: u64,
) -> Open {
    let title = {
        let t = title.trim();
        let t: String = t.chars().take(60).collect();
        if t.is_empty() {
            format!("{}'s eternal game", creator)
        } else {
            t
        }
    };
    let o = Open {
        id: cr::new_token()[..16].to_string(),
        title,
        creator: creator.to_string(),
        creator_user,
        side,
        seed,
        token: cr::new_token(),
        created: Instant::now(),
        created_unix: now_unix(),
    };
    let reg = opens();
    {
        let mut map = reg.lock().await;
        prune(&mut map).await;
        map.insert(o.id.clone(), o.clone());
    }
    nudge(Nudge::Lobby);
    o
}

/// Give up a waiting seat. Only the token that created it may.
pub async fn cancel(token: &str) -> bool {
    let reg = opens();
    let dropped = {
        let mut map = reg.lock().await;
        let id = map.values().find(|o| o.token == token).map(|o| o.id.clone());
        match id {
            Some(id) => map.remove(&id).is_some(),
            None => false,
        }
    };
    if dropped {
        nudge(Nudge::Lobby);
    }
    dropped
}

/// Both seats filled. The lobby entry becomes a game: one VM, two seats, two
/// tokens (the creator keeps the one they have been holding).
pub struct Started {
    pub key: String,
    pub seat: cr::Seat,
    /// The joiner's own resume token.
    pub token: String,
    pub side: Side,
    pub creator_token: String,
    pub creator_side: Side,
    pub creator_user: Option<String>,
    pub seed: u64,
}

/// Take an open game off the list — a join is a race and exactly one player
/// may win it. The caller then either [`start`]s it or [`restore`]s it.
pub async fn claim(id: &str) -> Option<Open> {
    let reg = opens();
    let mut map = reg.lock().await;
    prune(&mut map).await;
    map.remove(id)
}

/// Put a claimed seat back. The gate refusing is about the card layer, not
/// about the player who tried, so their game survives the attempt.
pub async fn restore(open: Open) {
    let reg = opens();
    reg.lock().await.insert(open.id.clone(), open);
    nudge(Nudge::Lobby);
}

/// Fill the free seat and start the game. The SETUP is the caller's: the ws
/// surface passes `cr::eternal_setup` (and refuses on its gate before ever
/// getting here), which is also what lets a test play the lobby through with
/// decks whose behaviour is entirely implemented today.
pub async fn start(
    open: Open,
    joiner: &str,
    joiner_user: Option<String>,
    setup: jinteki_cr::GameSetup,
) -> Started {
    // Seats are indexed by SIDE, not by who arrived first.
    let (creator_side, joiner_side) = (open.side, open.side.other());
    let mut by_side = [SeatState::bot(), SeatState::bot()];
    by_side[ix(creator_side)] = SeatState::human(&open.creator, open.creator_user.clone())
        .with_token(open.token.clone());
    by_side[ix(joiner_side)] = SeatState::human(joiner, joiner_user);
    let [corp_seat, runner_seat] = by_side;
    let (game, tokens) = cr::create_two_human_session(setup, corp_seat, runner_seat).await;
    let joiner_token = tokens[ix(joiner_side)].clone();
    let key = game.lock().await.key().to_string();
    // The open list moved, and the creator's socket finds out its token is a
    // game now by asking the registry (see `local::on_nudge`).
    nudge(Nudge::Lobby);
    Started {
        key: key.clone(),
        seat: cr::Seat { side: joiner_side, key, game },
        token: joiner_token,
        side: joiner_side,
        creator_token: open.token,
        creator_side,
        creator_user: open.creator_user,
        seed: open.seed,
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Labels
// ───────────────────────────────────────────────────────────────────────────

fn ix(s: Side) -> usize {
    match s {
        Side::Corp => 0,
        Side::Runner => 1,
    }
}
fn side_key(s: Side) -> &'static str {
    match s {
        Side::Corp => "corp",
        Side::Runner => "runner",
    }
}
fn side_name(s: Side) -> &'static str {
    match s {
        Side::Corp => "Corp",
        Side::Runner => "Runner",
    }
}
/// The eternal deck that comes with a seat — picking a side picks a deck.
pub fn deck_title(s: Side) -> &'static str {
    match s {
        Side::Corp => cr::GAUNTLET.title,
        Side::Runner => cr::ANDROMEDA.title,
    }
}

pub fn side_from_key(k: &str) -> Side {
    match k {
        "corp" => Side::Corp,
        _ => Side::Runner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_open_seat_lists_itself_and_cancels() {
        let o = create("test game", "guest-abcd", None, Side::Corp, 7).await;
        let list = list_json().await;
        let rows = list["list"].as_array().unwrap();
        let row = rows.iter().find(|r| r["gameid"] == json!(o.id)).expect("listed");
        assert_eq!(row["creator"], json!("guest-abcd"));
        assert_eq!(row["side"], json!("corp"));
        assert_eq!(row["open-side"], json!("runner"));
        assert_eq!(row["open-deck"], json!(cr::ANDROMEDA.title));
        assert!(cancel(&o.token).await);
        let list = list_json().await;
        assert!(!list["list"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["gameid"] == json!(o.id)));
    }

    #[tokio::test]
    async fn a_blank_title_gets_the_creators_name() {
        let o = create("   ", "guest-9999", None, Side::Runner, 1).await;
        assert!(o.title.contains("guest-9999"));
        cancel(&o.token).await;
    }
}
