//! The CR lobby: human vs human on our own server, same VM as the bot mode.
//!
//! A lobby entry is a person sitting down at one seat of an eternal-decks
//! game and waiting for someone to take the other. Nothing is a game until
//! both seats are filled AND both players have said ready; the moment the
//! second player joins, the two seats become a [`Pairing`] — the ready
//! check — and only a completed countdown makes them one
//! [`crate::cr::CrGame`] with a bot in neither seat.
//!
//! **The ready check.** A join no longer starts the game on the spot: it
//! puts both players at one table ([`pair`]), each with a Ready toggle.
//! When both are ready the server — never a client — counts 5,4,3,2,1, one
//! tick a second ([`spawn_countdown`]), and anyone unreadying or leaving
//! mid-count cancels it (the generation counter in [`Pairing`] is how a
//! stale countdown task discovers it has been cancelled). At zero the game
//! is created through the exact code path a join used to take:
//! [`crate::cr::eternal_setup`] behind the same gate, then
//! [`crate::cr::create_two_human_session`].
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
use crate::db::Db;
use crate::timing::TimingConfig;
use jinteki_cr::object::Side;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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
    /// The pairing (ready check) with this id changed — a seat readied,
    /// the countdown ticked, or the pairing dissolved.
    Pair(String),
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
    /// The deck key the creator chose (`GET /api/decks`); `None` means the
    /// side's eternal deck, which is also what the game is built from today.
    pub deck: Option<String>,
    /// The timing mode this table will play under (`crate::timing`). The
    /// joiner sees it in the list and at the ready check; readying up is
    /// consenting to it.
    pub timing: TimingConfig,
    pub seed: u64,
    /// The creator's resume token, minted while they wait, so that closing
    /// the tab before anyone joins loses nothing.
    pub token: String,
    /// How many live sockets are holding this seat (a refresh briefly has
    /// zero, a second tab has two). A dead socket's seat is withdrawn only
    /// after [`ABANDON_GRACE`] with no holder — see [`drop_open_holder`].
    holders: Arc<AtomicU32>,
    /// Strictly increasing arrival order — "oldest" must not depend on a
    /// second-resolution clock that ties.
    seq: u64,
    created: Instant,
    created_unix: u64,
}

impl Open {
    /// One more live socket is holding this seat.
    pub fn hold(&self) {
        self.holders.fetch_add(1, Ordering::Relaxed);
    }
    pub fn to_json(&self) -> Value {
        let age = self.created.elapsed().as_secs();
        json!({
            "gameid": self.id,
            "title": self.title,
            "creator": self.creator,
            // The creator's side, and the seat still going begging.
            "side": side_key(self.side),
            "deck": self.deck,
            "deck-name": deck_name(self.side, self.deck.as_deref()),
            "timing": self.timing.to_json(),
            "timing-label": self.timing.label(),
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

fn next_seq() -> u64 {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    SEQ.fetch_add(1, Ordering::Relaxed)
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
    rows.sort_by(|a, b| b.seq.cmp(&a.seq));
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
    deck: Option<String>,
    timing: TimingConfig,
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
        deck: clean_deck_key(deck),
        timing,
        seed,
        token: cr::new_token(),
        // The caller's socket (if it has one) says so with [`Open::hold`];
        // a seat created without a socket (a test) is held by nobody.
        holders: Arc::new(AtomicU32::new(0)),
        seq: next_seq(),
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

/// "Play anyone": which open seat an autopairing joiner takes. Pure, so the
/// pairing RULE is testable without the shared registry: the OLDEST open
/// seat (arrival order, not a tying clock) whose free side is one the
/// joiner can play — sides must oppose, a corp host needs a runner joiner —
/// and never the joiner's own seat. ROPED TABLES ONLY: a lobby without a
/// rope can stall forever on one absent player, so autopair never volunteers
/// anyone into it — an unroped table is joined by hand, from the list, with
/// the label read (with or without a main clock, the rope is the floor).
fn pick_oldest<'a>(
    rows: impl Iterator<Item = &'a Open>,
    can_sides: &[Side],
    exclude_token: Option<&str>,
) -> Option<&'a Open> {
    rows.filter(|o| o.timing.rope.is_some())
        .filter(|o| can_sides.contains(&o.side.other()))
        .filter(|o| exclude_token != Some(o.token.as_str()))
        .min_by_key(|o| o.seq)
}

/// Claim the seat [`pick_oldest`] chooses, atomically — the same race a
/// named join runs, won by exactly one player.
pub async fn claim_oldest_compatible(
    can_sides: &[Side],
    exclude_token: Option<&str>,
) -> Option<Open> {
    let reg = opens();
    let mut map = reg.lock().await;
    prune(&mut map).await;
    let id = pick_oldest(map.values(), can_sides, exclude_token)?.id.clone();
    map.remove(&id)
}

// ───────────────────────────────────────────────────────────────────────────
// The ready check: both seats filled, nobody committed yet
// ───────────────────────────────────────────────────────────────────────────

/// One person at the pairing table.
#[derive(Clone, Debug)]
pub struct PairSeat {
    pub name: String,
    pub user: Option<String>,
    /// This seat's resume token — the creator keeps the one they held while
    /// waiting; the joiner's is minted the moment they sit down.
    pub token: String,
    /// The deck key this seat chose; `None` is the side's eternal deck.
    pub deck: Option<String>,
    pub ready: bool,
    /// Live sockets in this seat (the creator's counter survives from their
    /// [`Open`]); see [`drop_pairing_holder`].
    holders: Arc<AtomicU32>,
}

/// Two seats at one table, deciding whether to play. Not a game yet: the
/// countdown finishing is what makes it one, and anything else unwinds it.
#[derive(Clone, Debug)]
pub struct Pairing {
    pub id: String,
    pub title: String,
    /// The host's chosen timing, shown at the table: readying up consents.
    pub timing: TimingConfig,
    pub seed: u64,
    /// The side the lobby's creator took (their leaving dissolves the table;
    /// the joiner leaving puts the creator back on the open list).
    pub creator_side: Side,
    /// Indexed by side: `[corp, runner]`.
    pub seats: [PairSeat; 2],
    /// The tick most recently announced (5…1), `None` outside a countdown.
    pub count: Option<u8>,
    /// Bumped by every unready/leave; a countdown task that finds a bump
    /// knows it was cancelled. This is the ONLY cancellation channel.
    generation: u64,
    seq: u64,
}

impl Pairing {
    /// This pairing as one viewer sees it. `you` marks their own seat, so
    /// the client never has to know which token is whose.
    pub fn to_json(&self, viewer_token: &str) -> Value {
        let seats: Vec<Value> = [Side::Corp, Side::Runner]
            .into_iter()
            .map(|side| {
                let s = &self.seats[ix(side)];
                json!({
                    "side": side_key(side),
                    "name": s.name,
                    "deck": s.deck,
                    "deck-name": deck_name(side, s.deck.as_deref()),
                    "ready": s.ready,
                    "you": s.token == viewer_token,
                })
            })
            .collect();
        json!({
            "id": self.id,
            "title": self.title,
            "timing": self.timing.to_json(),
            "timing-label": self.timing.label(),
            "count": self.count,
            "seats": seats,
        })
    }
    fn seat_of(&self, token: &str) -> Option<Side> {
        [Side::Corp, Side::Runner]
            .into_iter()
            .find(|s| self.seats[ix(*s)].token == token)
    }
    fn both_ready(&self) -> bool {
        self.seats.iter().all(|s| s.ready)
    }
    pub fn seat(&self, side: Side) -> &PairSeat {
        &self.seats[ix(side)]
    }
    /// The side the joiner sat down in — the one the creator left free.
    pub fn joiner_side(&self) -> Side {
        self.creator_side.other()
    }
    /// One more live socket is sitting in this token's seat.
    pub fn hold(&self, token: &str) {
        if let Some(side) = self.seat_of(token) {
            self.seats[ix(side)].holders.fetch_add(1, Ordering::Relaxed);
        }
    }
}

type Pairings = Arc<Mutex<HashMap<String, Pairing>>>;

fn pairings() -> Pairings {
    static PAIRINGS: OnceLock<Pairings> = OnceLock::new();
    PAIRINGS.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

/// Take the second seat: a claimed open lobby becomes a pairing. The
/// joiner's resume token is minted here, so a mid-ready-check refresh
/// resumes into the same table.
pub async fn pair(
    open: Open,
    joiner: &str,
    joiner_user: Option<String>,
    joiner_deck: Option<String>,
) -> Pairing {
    let creator_seat = PairSeat {
        name: open.creator.clone(),
        user: open.creator_user.clone(),
        token: open.token.clone(),
        deck: open.deck.clone(),
        ready: false,
        // The creator's live-socket count carries over from their wait.
        holders: open.holders.clone(),
    };
    let joiner_seat = PairSeat {
        name: joiner.to_string(),
        user: joiner_user,
        token: cr::new_token(),
        deck: clean_deck_key(joiner_deck),
        ready: false,
        // The joining socket is this seat's first holder.
        holders: Arc::new(AtomicU32::new(1)),
    };
    let mut seats_by_side = [creator_seat.clone(), joiner_seat.clone()];
    seats_by_side[ix(open.side)] = creator_seat;
    seats_by_side[ix(open.side.other())] = joiner_seat;
    let p = Pairing {
        id: open.id.clone(),
        title: open.title.clone(),
        timing: open.timing,
        seed: open.seed,
        creator_side: open.side,
        seats: seats_by_side,
        count: None,
        generation: 0,
        seq: open.seq,
    };
    pairings().lock().await.insert(p.id.clone(), p.clone());
    nudge(Nudge::Lobby);
    nudge(Nudge::Pair(p.id.clone()));
    p
}

/// The pairing a token is sitting in, if any (a refreshed tab resuming).
pub async fn pairing_by_token(token: &str) -> Option<Pairing> {
    let reg = pairings();
    let map = reg.lock().await;
    map.values().find(|p| p.seat_of(token).is_some()).cloned()
}

pub async fn pairing_snapshot(id: &str) -> Option<Pairing> {
    pairings().lock().await.get(id).cloned()
}

/// What a ready toggle changed.
#[derive(Debug, PartialEq, Eq)]
pub enum ReadyOutcome {
    /// The toggle flipped this table into both-ready: the caller starts the
    /// countdown. Returned exactly once per transition, however the two
    /// sockets race.
    BothReadyNow(String),
    /// The toggle landed (including an unready, which cancelled any count).
    Updated(String),
    /// The token is not at any pairing table.
    NotPaired,
}

/// Flip one seat's ready state. Unreadying cancels a running countdown by
/// bumping the generation — the countdown task notices on its next tick.
pub async fn set_ready(token: &str, ready: bool) -> ReadyOutcome {
    let reg = pairings();
    let mut map = reg.lock().await;
    let Some(p) = map.values_mut().find(|p| p.seat_of(token).is_some()) else {
        return ReadyOutcome::NotPaired;
    };
    let side = p.seat_of(token).expect("found by seat_of");
    let was_both = p.both_ready();
    p.seats[ix(side)].ready = ready;
    let id = p.id.clone();
    if !ready {
        p.generation += 1;
        p.count = None;
    }
    let out = if p.both_ready() && !was_both {
        ReadyOutcome::BothReadyNow(id.clone())
    } else {
        ReadyOutcome::Updated(id.clone())
    };
    drop(map);
    nudge(Nudge::Pair(id));
    out
}

/// Who walked away from a pairing, and what became of the table.
#[derive(Debug, PartialEq, Eq)]
pub enum Left {
    /// The joiner left (or dropped): the creator is back on the open list,
    /// holding the same token, seed and title they always had.
    CreatorReopened(String),
    /// The creator left: the table dissolves and the joiner is told so.
    Dissolved(String),
    NotPaired,
}

/// Leave a pairing (a Leave tap, or the socket dying). Any leave cancels a
/// running countdown, because the generation bumps with the table.
pub async fn leave_pairing(token: &str) -> Left {
    let p = {
        let reg = pairings();
        let mut map = reg.lock().await;
        let Some(id) = map
            .values()
            .find(|p| p.seat_of(token).is_some())
            .map(|p| p.id.clone())
        else {
            return Left::NotPaired;
        };
        map.remove(&id).expect("found just above")
    };
    let id = p.id.clone();
    let leaver = p.seat_of(token).expect("found by seat_of");
    let out = if leaver == p.creator_side {
        Left::Dissolved(id.clone())
    } else {
        let c = &p.seats[ix(p.creator_side)];
        let reopened = Open {
            id: p.id.clone(),
            title: p.title.clone(),
            creator: c.name.clone(),
            creator_user: c.user.clone(),
            side: p.creator_side,
            deck: c.deck.clone(),
            timing: p.timing,
            seed: p.seed,
            token: c.token.clone(),
            // Their socket count rides back out with them.
            holders: c.holders.clone(),
            seq: p.seq,
            created: Instant::now(),
            created_unix: now_unix(),
        };
        opens().lock().await.insert(reopened.id.clone(), reopened);
        Left::CreatorReopened(id.clone())
    };
    nudge(Nudge::Lobby);
    nudge(Nudge::Pair(id));
    out
}

// ───────────────────────────────────────────────────────────────────────────
// Dead sockets: an invitation from nobody is withdrawn — after a grace
// ───────────────────────────────────────────────────────────────────────────

/// How long a seat survives with no live socket in it. A refresh reconnects
/// well inside this; a closed tab does not, and its seat is withdrawn — a
/// lobby whose host is gone must not keep catching joiners.
pub const ABANDON_GRACE: Duration = Duration::from_secs(3);

/// A socket holding a lobby seat died. One fewer socket holds it; if that
/// was the last one, then after [`ABANDON_GRACE`] with the seat still
/// unheld it is withdrawn — cancelled if it is (by then) an open seat,
/// walked away from if it is a ready-check seat (which cancels any
/// countdown and puts a still-present creator back on the open list). The
/// re-lookup after the sleep is deliberate: a seat can change rooms while
/// the grace runs (a joiner leaving turns a pairing back into an open
/// seat), and the counter travels with it.
pub fn drop_holder(token: String) {
    tokio::spawn(async move {
        let Some(h) = holders_of(&token).await else { return };
        let _ = h.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            Some(v.saturating_sub(1))
        });
        if h.load(Ordering::Relaxed) > 0 {
            return;
        }
        tokio::time::sleep(ABANDON_GRACE).await;
        match holders_of(&token).await {
            Some(h) if h.load(Ordering::Relaxed) == 0 => {
                if !cancel(&token).await {
                    leave_pairing(&token).await;
                }
            }
            _ => {}
        }
    });
}

/// The live-socket counter of whatever seat a token holds right now — an
/// open seat's, or a pairing seat's.
async fn holders_of(token: &str) -> Option<Arc<AtomicU32>> {
    if let Some(o) = by_token(token).await {
        return Some(o.holders.clone());
    }
    let reg = pairings();
    let map = reg.lock().await;
    map.values().find_map(|p| {
        let side = p.seat_of(token)?;
        Some(p.seats[ix(side)].holders.clone())
    })
}

// ───────────────────────────────────────────────────────────────────────────
// The countdown: 5,4,3,2,1 — spoken by the server, never by a client
// ───────────────────────────────────────────────────────────────────────────

/// Announce one tick iff the pairing still stands, both seats are still
/// ready, and no unready has bumped the generation since the count began.
/// Returns false the moment any of that stops being true.
async fn tick(id: &str, gen: u64, n: u8) -> bool {
    let reg = pairings();
    let mut map = reg.lock().await;
    let Some(p) = map.get_mut(id) else { return false };
    if p.generation != gen || !p.both_ready() {
        return false;
    }
    p.count = Some(n);
    drop(map);
    nudge(Nudge::Pair(id.to_string()));
    true
}

/// The generation to count under — captured BEFORE the task spawns, so an
/// unready that lands between the spawn and the first tick still cancels.
async fn countdown_generation(id: &str) -> Option<u64> {
    pairings().lock().await.get(id).map(|p| p.generation)
}

/// Both players said ready: count 5,4,3,2,1 at one-second ticks, then make
/// the game. Cancelled by any unready or leave (the generation bump); the
/// task simply stops, leaving the table exactly as the cancel left it.
pub async fn spawn_countdown(id: String, db: Arc<Db>) {
    let Some(gen) = countdown_generation(&id).await else { return };
    tokio::spawn(async move {
        for n in (1..=5u8).rev() {
            if !tick(&id, gen, n).await {
                return;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        finish_countdown(&id, gen, &db).await;
    });
}

/// The countdown reached zero: the gate one more time (it is evaluated per
/// start, at this door as at every other), then the game — the same
/// [`crate::cr::create_two_human_session`] every two-human game goes
/// through. A gate refusal dissolves the table honestly rather than leaving
/// two ready players in front of a stuck 1.
async fn finish_countdown(id: &str, gen: u64, db: &Arc<Db>) {
    let setup = {
        let reg = pairings();
        let map = reg.lock().await;
        let Some(p) = map.get(id) else { return };
        if p.generation != gen || !p.both_ready() {
            return;
        }
        match cr::eternal_setup(p.seed) {
            Ok(s) => s,
            Err(_) => {
                drop(map);
                let _ = leave_pairing_dissolve(id).await;
                return;
            }
        }
    };
    if let Some(started) = finish_pairing_with(id, gen, setup).await {
        for side in [Side::Corp, Side::Runner] {
            let s = &started.seats[ix(side)];
            if let Some(uid) = s.user.as_deref() {
                cr::record_start(db, &s.token, uid, side, started.seed).await;
            }
        }
    }
}

/// Dissolve a pairing outright (both players get `lobby-gone`).
async fn leave_pairing_dissolve(id: &str) -> bool {
    let gone = pairings().lock().await.remove(id).is_some();
    if gone {
        nudge(Nudge::Lobby);
        nudge(Nudge::Pair(id.to_string()));
    }
    gone
}

/// A pairing that became a game: what the countdown's finish knows.
pub struct PairedStart {
    pub key: String,
    pub seed: u64,
    /// By side: name, user, token — the tokens are now CR session tokens.
    pub seats: [PairSeat; 2],
}

/// Turn a pairing into a game with the given setup. Public with the setup
/// injected so a test can play the whole ready-check through on small decks
/// that are entirely implemented today — the same code path minus the gate,
/// exactly like [`crate::cr::create_session`]'s test seam.
pub async fn finish_pairing_with(
    id: &str,
    gen: u64,
    setup: jinteki_cr::GameSetup,
) -> Option<PairedStart> {
    let p = {
        let reg = pairings();
        let mut map = reg.lock().await;
        let stands = map
            .get(id)
            .is_some_and(|p| p.generation == gen && p.both_ready());
        if !stands {
            return None;
        }
        map.remove(id).expect("checked just above")
    };
    let mut by_side = [SeatState::bot(), SeatState::bot()];
    for side in [Side::Corp, Side::Runner] {
        let s = &p.seats[ix(side)];
        by_side[ix(side)] =
            SeatState::human(&s.name, s.user.clone()).with_token(s.token.clone());
    }
    let [corp_seat, runner_seat] = by_side;
    let (game, _tokens) =
        cr::create_two_human_session(setup, corp_seat, runner_seat, p.timing).await;
    let key = game.lock().await.key().to_string();
    // Both sockets find out the same way a waiting seat always has: the
    // nudge sends them to the registry, where their tokens are games now.
    nudge(Nudge::Lobby);
    nudge(Nudge::Pair(id.to_string()));
    Some(PairedStart { key, seed: p.seed, seats: p.seats })
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
    let (game, tokens) =
        cr::create_two_human_session(setup, corp_seat, runner_seat, open.timing).await;
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
pub fn side_key(s: Side) -> &'static str {
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
/// Display layer: players see the deck's display name, never its internal
/// key or printed provenance title.
pub fn deck_title(s: Side) -> &'static str {
    match s {
        Side::Corp => cr::GAUNTLET.display_name,
        Side::Runner => cr::ANDROMEDA.display_name,
    }
}

/// The display name a deck choice renders as when the client has nothing
/// better: the chosen key verbatim, or the side's eternal deck title. The
/// deck CATALOG (names, legality) belongs to `GET /api/decks`; the lobby
/// stores keys and never pretends to know more.
fn deck_name(side: Side, key: Option<&str>) -> String {
    match key {
        Some(k) => k.to_string(),
        None => deck_title(side).to_string(),
    }
}

/// A deck key off the wire: trimmed, bounded, and never an empty string.
fn clean_deck_key(k: Option<String>) -> Option<String> {
    let k = k?;
    let k: String = k.trim().chars().take(64).collect();
    if k.is_empty() {
        None
    } else {
        Some(k)
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
        let o = create("test game", "guest-abcd", None, Side::Corp, None,
            TimingConfig::default(), 7).await;
        let list = list_json().await;
        let rows = list["list"].as_array().unwrap();
        let row = rows.iter().find(|r| r["gameid"] == json!(o.id)).expect("listed");
        assert_eq!(row["creator"], json!("guest-abcd"));
        assert_eq!(row["side"], json!("corp"));
        assert_eq!(row["open-side"], json!("runner"));
        assert_eq!(
            row["open-deck"],
            json!("Mezzie's Andromeda"),
            "the open seat advertises the display name"
        );
        assert_eq!(row["deck"], Value::Null);
        assert_eq!(
            row["deck-name"],
            json!("Mezzie's Making Stars"),
            "the listed deck speaks the display name too"
        );
        assert_eq!(row["timing-label"], json!("30m + rope"), "the default mode, labelled");
        assert_eq!(row["timing"]["main_clock_secs"], json!(1800));
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
        let o = create("   ", "guest-9999", None, Side::Runner, None,
            TimingConfig::default(), 1).await;
        assert!(o.title.contains("guest-9999"));
        cancel(&o.token).await;
    }

    #[tokio::test]
    async fn a_chosen_deck_key_rides_the_listing() {
        let o = create("deck ride", "guest-deck", None, Side::Runner,
            Some("  mezzie-andromeda  ".into()), TimingConfig::default(), 3).await;
        assert_eq!(o.deck.as_deref(), Some("mezzie-andromeda"));
        let list = list_json().await;
        let row = list["list"].as_array().unwrap().iter()
            .find(|r| r["gameid"] == json!(o.id)).expect("listed").clone();
        assert_eq!(row["deck"], json!("mezzie-andromeda"));
        cancel(&o.token).await;
    }

    /// A synthetic open seat, aged by its seq — the pure picker never reads
    /// the registry, so these never race the other tests' entries. Roped
    /// (the default timing) unless the test says otherwise.
    fn open(seq: u64, side: Side, token: &str) -> Open {
        open_timed(seq, side, token, TimingConfig::default())
    }

    fn open_timed(seq: u64, side: Side, token: &str, timing: TimingConfig) -> Open {
        Open {
            id: format!("test-{seq}"),
            title: "t".into(),
            creator: "c".into(),
            creator_user: None,
            side,
            deck: None,
            timing,
            seed: 1,
            token: token.into(),
            holders: Arc::new(AtomicU32::new(0)),
            seq,
            created: Instant::now(),
            created_unix: now_unix(),
        }
    }

    #[test]
    fn autopair_picks_the_oldest_compatible_seat() {
        // Oldest is a runner host (free seat: corp); two corp hosts follow.
        let rows = [
            open(1, Side::Runner, "t1"),
            open(2, Side::Corp, "t2"),
            open(3, Side::Corp, "t3"),
        ];
        // A joiner who can only play runner needs a corp host: the oldest
        // CORP host wins, not the older runner host.
        let got = pick_oldest(rows.iter(), &[Side::Runner], None).expect("a match");
        assert_eq!(got.seq, 2, "oldest compatible, not oldest overall");
        // A joiner who can play both sides takes the oldest overall.
        let got = pick_oldest(rows.iter(), &[Side::Corp, Side::Runner], None).unwrap();
        assert_eq!(got.seq, 1);
    }

    #[test]
    fn autopair_requires_opposite_sides() {
        // Only corp hosts are waiting: a corp-deck joiner matches nobody —
        // a corp host needs a runner joiner.
        let rows = [open(1, Side::Corp, "t1"), open(2, Side::Corp, "t2")];
        assert!(pick_oldest(rows.iter(), &[Side::Corp], None).is_none());
        assert!(pick_oldest(rows.iter(), &[], None).is_none(), "no deck, no seat");
    }

    #[test]
    fn autopair_never_picks_your_own_seat() {
        let rows = [open(1, Side::Corp, "mine"), open(2, Side::Corp, "theirs")];
        let got = pick_oldest(rows.iter(), &[Side::Runner], Some("mine")).unwrap();
        assert_eq!(got.token, "theirs");
        let rows = [open(1, Side::Corp, "mine")];
        assert!(pick_oldest(rows.iter(), &[Side::Runner], Some("mine")).is_none());
    }

    #[test]
    fn autopair_seats_nobody_at_an_unroped_table() {
        use crate::timing::RopeConfig;
        let unroped_timed =
            TimingConfig { main_clock_secs: Some(1500), rope: None };
        let unroped_untimed = TimingConfig { main_clock_secs: None, rope: None };
        let roped_untimed =
            TimingConfig { main_clock_secs: None, rope: Some(RopeConfig::default()) };
        // The two oldest tables have no rope — joinable by hand, never by
        // autopair. The roped-but-untimed one qualifies: rope is the floor,
        // the main clock is not.
        let rows = [
            open_timed(1, Side::Corp, "t1", unroped_timed),
            open_timed(2, Side::Corp, "t2", unroped_untimed),
            open_timed(3, Side::Corp, "t3", roped_untimed),
            open(4, Side::Corp, "t4"),
        ];
        let got = pick_oldest(rows.iter(), &[Side::Runner], None).expect("a roped table");
        assert_eq!(got.seq, 3, "the oldest ROPED table, not the oldest table");
        // Nothing roped at all: autopair walks away (and opens its own —
        // default-roped — seat; that half lives in the wire tests).
        let rows = [
            open_timed(1, Side::Corp, "t1", unroped_timed),
            open_timed(2, Side::Corp, "t2", unroped_untimed),
        ];
        assert!(pick_oldest(rows.iter(), &[Side::Runner], None).is_none());
    }

    #[tokio::test]
    async fn both_ready_is_announced_exactly_once_and_unready_cancels_the_count() {
        let o = create("ready check", "host", None, Side::Corp,
            Some("mezzie-making-stars".into()), TimingConfig::default(), 9).await;
        let claimed = claim(&o.id).await.expect("our own open seat");
        let p = pair(claimed, "joiner", None, Some("mezzie-andromeda".into())).await;
        let host_token = p.seats[ix(Side::Corp)].token.clone();
        let join_token = p.seats[ix(Side::Runner)].token.clone();
        assert_eq!(host_token, o.token, "the creator keeps the token they held");
        assert_ne!(join_token, host_token);

        // One ready is an update; the second is the both-ready transition,
        // and only that one starts a countdown.
        assert_eq!(set_ready(&host_token, true).await, ReadyOutcome::Updated(p.id.clone()));
        assert_eq!(
            set_ready(&join_token, true).await,
            ReadyOutcome::BothReadyNow(p.id.clone())
        );
        // Re-readying an already-ready seat is not a second transition.
        assert_eq!(set_ready(&join_token, true).await, ReadyOutcome::Updated(p.id.clone()));

        // The countdown ticks under the generation it started with…
        let gen = countdown_generation(&p.id).await.unwrap();
        assert!(tick(&p.id, gen, 5).await, "both ready: the count speaks");
        assert_eq!(pairing_snapshot(&p.id).await.unwrap().count, Some(5));

        // …and an unready mid-count cancels it: the stale task's next tick
        // refuses, and the announced count is withdrawn.
        assert_eq!(set_ready(&host_token, false).await, ReadyOutcome::Updated(p.id.clone()));
        assert!(!tick(&p.id, gen, 4).await, "unready cancels the countdown");
        assert_eq!(pairing_snapshot(&p.id).await.unwrap().count, None);

        // Readying again is a fresh transition with a fresh generation.
        assert_eq!(
            set_ready(&host_token, true).await,
            ReadyOutcome::BothReadyNow(p.id.clone())
        );
        let gen2 = countdown_generation(&p.id).await.unwrap();
        assert_ne!(gen, gen2);
        assert!(!tick(&p.id, gen, 5).await, "the old task stays cancelled");
        assert!(tick(&p.id, gen2, 5).await, "the new one counts");

        leave_pairing(&join_token).await;
        cancel(&host_token).await;
    }

    #[tokio::test]
    async fn a_joiner_leaving_puts_the_creator_back_on_the_open_list() {
        let o = create("abandoned table", "host2", None, Side::Runner, None,
            TimingConfig::default(), 4).await;
        let claimed = claim(&o.id).await.unwrap();
        let p = pair(claimed, "joiner2", None, None).await;
        let join_token = p.seats[ix(Side::Corp)].token.clone();

        assert_eq!(leave_pairing(&join_token).await, Left::CreatorReopened(p.id.clone()));
        assert!(pairing_snapshot(&p.id).await.is_none(), "the table is gone");
        let back = by_token(&o.token).await.expect("the creator waits again");
        assert_eq!(back.id, o.id, "same lobby, same token, same seat");
        assert_eq!(back.side, Side::Runner);

        // The creator leaving instead dissolves the table entirely.
        let claimed = claim(&back.id).await.unwrap();
        let p = pair(claimed, "joiner3", None, None).await;
        assert_eq!(leave_pairing(&o.token).await, Left::Dissolved(p.id.clone()));
        assert!(pairing_snapshot(&p.id).await.is_none());
        assert!(by_token(&o.token).await.is_none(), "nothing left behind");
    }
}
