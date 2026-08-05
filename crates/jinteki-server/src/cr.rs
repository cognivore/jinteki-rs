//! CR mode: the Comprehensive Rules VM (`jinteki-cr`) as the "Play vs Bot"
//! backend for the two eternal decks — estrike Andromeda vs Gauntlet.
//!
//! Three things live here and nothing else does.
//!
//! **The completeness gate (SYS-D-12).** A CR game refuses to start unless
//! every card of both decks is `is_complete()`, and the refusal names each
//! incomplete card with the exact printed sentences the card vocabulary
//! cannot yet say. The gate is evaluated PER START, so the mode goes live the
//! moment the card layer closes — no deploy, no flag. `GET /api/cr-readiness`
//! serves the same payload so the home screen can show the true fraction.
//!
//! **The adapter.** The VM is a decision-yielding coroutine: `step()` yields a
//! typed [`DecisionSpec`], `answer()` resumes it. The human's decisions become
//! the existing UI's prompt shapes (prompt-state msg/choices, select, the
//! access reader's card focus); the bot's are answered by `plan::default_answer`
//! — the plan driver's neutral policy, which is *literally* the second
//! interpreter of the player algebra, not a second bot.
//!
//! **The shim.** The UI's board renderer consumes jnet-shaped keys. Everything
//! it is given here is derived from `Vm::view_of(side)` and nothing else
//! (SYS-S-1): the view decides, per card, whether this player is entitled to
//! its front face; the object table is consulted only for the cards the view
//! already said `Seen`, plus for the ids of cards in PUBLIC zones, whose
//! presence is open information (CR 10.2.3a) even where their identity is not.
//! Nothing in a hidden zone — the opponent's grip, either deck — reaches the
//! wire at all, not even a card id.

use crate::db::Db;
use axum::extract::ws::{Message, WebSocket};
use jinteki_cr::ability::AbilityRef;
use jinteki_cr::decision::{ActionOption, WindowOption};
use jinteki_cr::frames::Frame;
use jinteki_cr::instr::InstallDest;
use jinteki_cr::object::{CardType, CounterKind, ObjectId, PrintedCard, ServerId, Side, Zone};
use jinteki_cr::plan::default_answer;
use jinteki_cr::timing::StructKind;
use jinteki_cr::view::View;
use jinteki_cr::{DecisionAnswer, DecisionSpec, GameResult, GameSetup, Vm, Yield};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// ───────────────────────────────────────────────────────────────────────────
// The decks, with copy counts
// ───────────────────────────────────────────────────────────────────────────

/// estrike Regular Andromeda — 45 cards + identity.
///
/// Titles and copy counts transcribed from the deck photo facts
/// (`VERSUS_NETRUNNER_DECKS_FULL_CARD_TEXTS.TXT`, the `Nx <title>` headers).
/// `jinteki-cards` holds one entry per DISTINCT card; the multiplicity is a
/// property of the deck LIST, which is this.
pub const ANDROMEDA_LIST: &[(&str, u32)] = &[
    ("Andromeda: Dispossessed Ristie", 1),
    // Events (20)
    ("Account Siphon", 3),
    ("Career Fair", 1),
    ("Clean Getaway", 3),
    ("Diesel", 2),
    ("Employee Strike", 3),
    ("Mutual Favor", 1),
    ("Pinhole Threading", 3),
    ("Rebirth", 1),
    ("Sure Gamble", 3),
    // Hardware (5)
    ("Boomerang", 3),
    ("Desperado", 2),
    // Programs (5)
    ("Bukhgalter", 1),
    ("Cupellation", 2),
    ("Paperclip", 1),
    ("Shibboleth", 1),
    // Resources (15)
    ("Bloo Moose", 3),
    ("Citadel Sanctuary", 1),
    ("Daily Casts", 3),
    ("Earthrise Hotel", 1),
    ("Film Critic", 2),
    ("Miss Bones", 1),
    ("The Class Act", 3),
    ("The Source", 1),
];

/// Gauntlet — 49 cards + identity.
pub const GAUNTLET_LIST: &[(&str, u32)] = &[
    ("Nebula Talent Management: Making Stars", 1),
    // Agendas (10)
    ("AstroScript Pilot Program", 3),
    ("Bellona", 3),
    ("Breaking News", 3),
    ("Tomorrow's Headline", 1),
    // Assets (7)
    ("Humanoid Resources", 1),
    ("Jackson Howard", 3),
    ("Rashida Jaheem", 3),
    // Ice (12)
    ("Archangel", 1),
    ("Data Raven", 2),
    ("Gold Farmer", 2),
    ("IP Block", 3),
    ("Resistor", 1),
    ("Slot Machine", 3),
    // Operations (18)
    ("24/7 News Cycle", 1),
    ("Archived Memories", 2),
    ("BOOM!", 1),
    ("Closed Accounts", 1),
    ("Hard-Hitting News", 3),
    ("Petty Cash", 3),
    ("Predictive Planogram", 2),
    ("Seamless Launch", 2),
    ("Self-Growth Program", 1),
    ("Subliminal Messaging", 1),
    ("Targeted Marketing", 1),
    // Upgrades (2)
    ("Crisium Grid", 2),
];

/// CR 1.5.4a: the additional identity cards this table brings along with the
/// Andromeda deck, kept in a pile outside the game.
///
/// Not part of the printed deck list — 1.5.4a makes the pile a choice a
/// player makes at the table, and the deck photo records no such choice. This
/// deck plays Rebirth, and 1.5.4b's "another identity from the same faction"
/// has to have something to name, so it brings every Criminal the card layer
/// carries WHOLE (`docs/vm/IDENTITY-QUEUE.md`). One identity is not a choice;
/// this list is what makes Rebirth a decision.
///
/// An identity is added here only once it is_complete(): `readiness()` holds
/// a pile card to exactly the same bar as a deck card, so a partial one would
/// make both priority decks unplayable.
pub const ANDROMEDA_PILE: &[&str] = &[
    "Ken \"Express\" Tenma: Disappeared Clone",
    "419: Amoral Scammer",
    "Armand \"Geist\" Walker: Tech Lord",
    "Barry \"Baz\" Wong: Tri-Maf Veteran",
    "Iain Stirling: Retired Spook",
    "Silhouette: Stealth Operative",
    "Gabriel Santiago: Consummate Professional",
    "Los: Data Hijacker",
    "Liza Talking Thunder: Prominent Legislator",
    "Laramy Fisk: Savvy Investor",
    "Leela Patel: Trained Pragmatist",
    "Nyusha \"Sable\" Sintashta: Symphonic Prodigy",
    "Virtual Intelligence, P.I.: \"You Can Call Me Vic\"",
    "Mercury: Chrome Libertador",
    "MuslihaT: Multifarious Marketeer",
    "Zahya Sadeghi: Versatile Smuggler",
    "Az McCaffrey: Mechanical Prodigy",
    "Khan: Savvy Skiptracer",
    "Nero Severn: Information Broker",
    "Steve Cambridge: Master Grifter",
    // Complete, and enlisted for the same reason as the rest — with the one
    // caveat that its first printed line is "Draft format only." and Andromeda
    // is not a draft deck. Format legality is settled before the game begins
    // (1.4.2) and nothing reads it afterwards, so it changes no play here; a
    // pile that wanted to honour it would filter on that printed line, which
    // is a decision about deck construction and not about this card.
    "Boris \"Syfr\" Kovac: Crafty Veteran",
];

/// One of the two eternal decks: the `jinteki-cards` module name, the printed
/// deck name, the list with its copy counts, and CR 1.5.4a's pile.
pub struct DeckSpec {
    pub key: &'static str,
    pub title: &'static str,
    pub side: Side,
    pub list: &'static [(&'static str, u32)],
    /// CR 1.5.4a: additional identities brought along with the deck. One
    /// entry per card — the pile is a set of distinct identities, not a list
    /// with copy counts.
    pub pile: &'static [&'static str],
}

pub const ANDROMEDA: DeckSpec = DeckSpec {
    key: "andromeda",
    title: "estrike Regular Andromeda",
    side: Side::Runner,
    list: ANDROMEDA_LIST,
    pile: ANDROMEDA_PILE,
};
pub const GAUNTLET: DeckSpec = DeckSpec {
    key: "gauntlet",
    title: "Gauntlet",
    side: Side::Corp,
    list: GAUNTLET_LIST,
    // 1.5.4a: the pile is the Runner's.
    pile: &[],
};

pub fn deck_specs() -> [&'static DeckSpec; 2] {
    [&ANDROMEDA, &GAUNTLET]
}

// ───────────────────────────────────────────────────────────────────────────
// The completeness gate (SYS-D-12)
// ───────────────────────────────────────────────────────────────────────────

/// One card that cannot yet be played, and exactly what about it cannot be
/// said. The sentences are quoted from the printed card.
#[derive(Debug, Clone, Serialize)]
pub struct MissingCard {
    pub deck: &'static str,
    pub title: String,
    pub copies: u32,
    /// The printed sentences with no expression in the card vocabulary yet.
    pub unimplemented: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeckReadiness {
    pub key: &'static str,
    pub title: &'static str,
    pub side: &'static str,
    pub identity: String,
    /// Distinct cards in the deck (what the fraction counts).
    pub distinct: usize,
    pub complete: usize,
    /// Cards including copies — 46 and 50 for these two.
    pub copies: u32,
}

/// The honest "not yet" payload: the fraction, and the whole gap.
#[derive(Debug, Clone, Serialize)]
pub struct Readiness {
    /// True iff a CR game may start right now.
    pub ready: bool,
    pub complete: usize,
    pub total: usize,
    pub decks: Vec<DeckReadiness>,
    pub missing: Vec<MissingCard>,
    /// Deck-list/card-layer disagreements (a listed title the card layer does
    /// not carry, or vice versa). Always empty in a healthy build; a problem
    /// blocks the gate exactly like an unimplemented sentence does.
    pub problems: Vec<String>,
}

impl Readiness {
    pub fn fraction(&self) -> String {
        format!("{}/{}", self.complete, self.total)
    }
}

/// The gate, evaluated from `jinteki-cards` every time it is asked.
pub fn readiness() -> Readiness {
    let mut r = Readiness {
        ready: false,
        complete: 0,
        total: 0,
        decks: Vec::new(),
        missing: Vec::new(),
        problems: Vec::new(),
    };
    for spec in deck_specs() {
        let Some(cards) = jinteki_cards::deck_named(spec.key) else {
            r.problems.push(format!("the card layer has no deck named {:?}", spec.key));
            continue;
        };
        let counts: HashMap<&str, u32> = spec.list.iter().copied().collect();
        let mut complete = 0usize;
        let mut copies = 0u32;
        let mut identity = String::new();
        for c in &cards {
            let n = match counts.get(c.name()) {
                Some(n) => *n,
                None => {
                    r.problems.push(format!(
                        "{}: the card layer carries {:?}, which the printed deck list does not",
                        spec.title,
                        c.name()
                    ));
                    0
                }
            };
            copies += n;
            if c.printed.card_type == CardType::Identity {
                identity = c.name().to_string();
            }
            if c.is_complete() {
                complete += 1;
            } else {
                r.missing.push(MissingCard {
                    deck: spec.key,
                    title: c.name().to_string(),
                    copies: n,
                    unimplemented: c.unimplemented.iter().map(|s| s.to_string()).collect(),
                });
            }
        }
        for (title, _) in spec.list {
            if !cards.iter().any(|c| c.name() == *title) {
                r.problems.push(format!(
                    "{}: the printed deck list names {title:?}, which the card layer does not carry",
                    spec.title
                ));
            }
        }
        // CR 1.5.4a: the additional identities come to the table with the
        // deck, so they are gated exactly like the deck is — a pile card that
        // cannot be played is a Rebirth that cannot resolve. They carry no
        // copy count: the pile is a set of distinct identities.
        let pile = jinteki_cards::pile_named(spec.key).unwrap_or_default();
        for title in spec.pile {
            match pile.iter().find(|c| c.name() == *title) {
                Some(c) => {
                    if c.is_complete() {
                        complete += 1;
                    } else {
                        r.missing.push(MissingCard {
                            deck: spec.key,
                            title: c.name().to_string(),
                            copies: 1,
                            unimplemented: c.unimplemented.iter().map(|s| s.to_string()).collect(),
                        });
                    }
                }
                None => r.problems.push(format!(
                    "{}: the additional-identities pile names {title:?}, which the card layer \
                     does not carry",
                    spec.title
                )),
            }
        }
        r.total += cards.len() + spec.pile.len();
        r.complete += complete;
        r.decks.push(DeckReadiness {
            key: spec.key,
            title: spec.title,
            side: side_key(spec.side),
            identity,
            distinct: cards.len() + spec.pile.len(),
            complete,
            copies,
        });
    }
    r.ready = r.problems.is_empty() && r.total > 0 && r.complete == r.total;
    r
}

/// Expand one deck into one `PrintedCard` per COPY, plus the identity and
/// CR 1.5.4a's pile of additional identities.
/// Refuses (via the caller's gate) rather than dropping anything.
fn expand(spec: &DeckSpec) -> (Vec<PrintedCard>, Option<PrintedCard>, Vec<PrintedCard>) {
    let mut deck = Vec::new();
    let mut identity = None;
    let cards = jinteki_cards::deck_named(spec.key).unwrap_or_default();
    let counts: HashMap<&str, u32> = spec.list.iter().copied().collect();
    for c in cards {
        let n = counts.get(c.name()).copied().unwrap_or(0);
        if c.printed.card_type == CardType::Identity {
            identity = Some(c.printed.clone());
            continue;
        }
        for _ in 0..n {
            deck.push(c.printed.clone());
        }
    }
    // 1.5.4a: one copy of each named identity, in the order the spec names
    // them. The pile never enters a zone, so nothing is shuffled into it.
    let carried = jinteki_cards::pile_named(spec.key).unwrap_or_default();
    let pile = spec
        .pile
        .iter()
        .filter_map(|t| carried.iter().find(|c| c.name() == *t))
        .map(|c| c.printed.clone())
        .collect();
    (deck, identity, pile)
}

/// The two eternal decks as a VM setup — or the refusal that says why not.
pub fn eternal_setup(seed: u64) -> Result<GameSetup, Readiness> {
    let r = readiness();
    if !r.ready {
        return Err(r);
    }
    let (runner_deck, runner_identity, runner_pile) = expand(&ANDROMEDA);
    let (corp_deck, corp_identity, corp_pile) = expand(&GAUNTLET);
    Ok(GameSetup {
        corp_deck,
        runner_deck,
        corp_identity,
        runner_identity,
        additional_identities: [(Side::Corp, corp_pile), (Side::Runner, runner_pile)]
            .into_iter()
            .collect(),
        seed,
        shuffle: true,
    })
}

/// Oracle text by title, from the card layer (SYS-D-10: the text a card was
/// implemented from is the text the player is shown). Public to the crate so
/// the card API serves the same text the board does — the "name a card"
/// picker asks the player to READ a card before committing to it, which it
/// cannot do if the only thing the lookup returns is a title.
pub(crate) fn oracle_text(title: &str) -> Option<&'static str> {
    static TEXTS: OnceLock<HashMap<String, String>> = OnceLock::new();
    TEXTS
        .get_or_init(|| {
            jinteki_cards::priority_decks()
                .into_iter()
                .map(|c| (c.name().to_string(), c.oracle_text.clone()))
                .collect()
        })
        .get(title)
        // The map lives in a `static`, so its strings are `'static` too.
        .map(|s| s.as_str())
        // Outside the two decks (the test decks, the demo harness) the
        // generated printed database still knows the text.
        .or_else(|| jinteki_core::printed::printed_text(title))
}

// ───────────────────────────────────────────────────────────────────────────
// The session
// ───────────────────────────────────────────────────────────────────────────

/// A decision put to a human seat, pre-rendered into the UI's vocabulary.
struct Pending {
    /// Whose decision it is. Only that seat may answer it; the other seat's
    /// view carries the waiting prompt instead.
    side: Side,
    spec: DecisionSpec,
    msg: String,
    /// (uuid, label, the answer taking it produces, the card it is about).
    /// The card rides WITH the option because a decision about cards has to
    /// render as cards (UX.md THE LAW §1), and the answer alone cannot always
    /// say which card an option is about — a division of credits answers with
    /// numbers, a minimal set with an index.
    choices: Vec<(String, String, DecisionAnswer, Option<ObjectId>)>,
    /// Card-tap selection, where the decision is about cards on the board.
    select: Option<Select>,
    /// A decision ABOUT one card puts the card itself in front of the player.
    focus: Option<Focus>,
    /// Action-window affordances (the chips and hand glow).
    actions: Vec<Value>,
    /// CR 8.3.3: the cards this decision arranges, in their current order.
    /// The client renders them as draggable cards and answers with an order.
    arrange: Option<Vec<ObjectId>>,
}

struct Select {
    candidates: Vec<ObjectId>,
    count: u32,
    min: u32,
    up_to: bool,
    kind: SelectKind,
}

impl Select {
    /// Is the set the player has already a COMPLETE answer — one they may
    /// submit without picking anything more?
    ///
    /// Two ways to be complete. CR 1.15.2e lets "up to N" finish at any size
    /// from its floor upward, which is why the chooser gets a button at all.
    /// A FIXED N is normally completed by the last pick committing on its own
    /// — except where 1.15.2b clamped N to ZERO because fewer eligible
    /// targets exist than the instruction asked for. Then the empty set is
    /// complete the moment the question is asked and no pick will ever arrive
    /// to commit it, so without a button the prompt cannot be answered at
    /// all. Both the field the client reads and the command the client sends
    /// come from here, so the button's presence and the server's willingness
    /// to honour it cannot drift apart.
    fn complete_at(&self, picked: usize) -> bool {
        picked as u32 >= self.min && (self.up_to || picked as u32 >= self.count)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectKind {
    Targets,
    Discard,
    Candidate,
}

struct Focus {
    card: ObjectId,
    kind: &'static str,
    trash_cost: Option<u32>,
}

/// One of the two seats at the table. A seat is a bot or a person; a person
/// has a name (the account's display name — `auth.rs`), maybe an account, a
/// resume token, and a socket that is either attached or not.
#[derive(Clone, Debug)]
pub struct SeatState {
    /// The plan driver's neutral policy answers this seat's decisions.
    pub bot: bool,
    /// What the other player sees this seat called.
    pub name: String,
    /// The account this seat plays for, if any (attribution in `games`).
    pub user: Option<String>,
    /// The registry key that finds this seat — one per player, so either can
    /// resume independently after a refresh or a closed tab.
    pub token: Option<String>,
    /// Whether a socket is attached right now. `false` is shown to the other
    /// player: a held game is honest, a silently stalled one is not.
    pub connected: bool,
}

impl SeatState {
    pub fn bot() -> Self {
        SeatState { bot: true, name: "bot".into(), user: None, token: None, connected: true }
    }
    pub fn human(name: impl Into<String>, user: Option<String>) -> Self {
        SeatState {
            bot: false,
            name: name.into(),
            user,
            token: None,
            connected: false,
        }
    }
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }
}

/// Seats are addressed by side, so index by it.
fn six(s: Side) -> usize {
    match s {
        Side::Corp => 0,
        Side::Runner => 1,
    }
}

pub struct CrGame {
    vm: Vm,
    /// Indexed by [`six`].
    seats: [SeatState; 2],
    /// The game's own id — what a state-changed nudge names, so the other
    /// seat's socket knows the push is for it.
    key: String,
    seed: u64,
    pending: Option<Pending>,
    picked: Vec<ObjectId>,
    /// Who the engine last put on the clock — PRIORITY, as the board shows it.
    ///
    /// `pending` is only Some while a PERSON owes the decision; the bot's are
    /// answered inside `drive`, and a state pushed mid-drive would therefore
    /// say nobody is deciding while the bot plainly is. This is the one
    /// notion, written at the single place the engine asks (`Yield::Decision`),
    /// so the white glow can never disagree with whose turn to speak it is.
    asked: Option<Side>,
    /// CR 7.1.2: cards the Runner has accessed and has not yet been shown.
    ///
    /// An access is over in a step or two (`vm.st.accessed` is cleared the
    /// moment it ends) and `drive` only pushes a state when the machine
    /// STOPS — so an access that asks the Runner nothing has no state to
    /// appear in at all, which is exactly the complaint: the card existed
    /// only in the log. The card is therefore SNAPSHOT here, at the instant
    /// the entitlement is live, and carried until the client has shown it.
    accesses: Vec<Value>,
    access_seq: u64,
    /// ONE LOG PER SIDE (SYS-S-1). A game event is rendered once per viewer,
    /// each from that viewer's own `view_of`, so a line can never name a card
    /// its reader is not entitled to see. Chat is the one thing written to
    /// both logs verbatim.
    log: [Vec<Value>; 2],
    /// How far into `vm.changes.log` both logs have been narrated. The
    /// kernel's change buffer is the authoritative event stream; this is the
    /// cursor that turns it into two readable ones (`crlog`).
    narrated: usize,
    /// The operator's unfiltered copy (`transcript`) — off unless the process
    /// configured a data dir, and never served to anyone.
    transcript: crate::transcript::Transcript,
    result: Option<GameResult>,
    conceded: Option<Side>,
    last_seen: Instant,
    bot_delay: Duration,
    outcome_recorded: bool,
}

impl CrGame {
    pub fn key(&self) -> &str {
        &self.key
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn seat(&self, side: Side) -> &SeatState {
        &self.seats[six(side)]
    }
    /// A game with a bot in it (the "vs Bot" mode).
    pub fn has_bot(&self) -> bool {
        self.seats.iter().any(|s| s.bot)
    }
    pub fn set_connected(&mut self, side: Side, on: bool) {
        self.seats[six(side)].connected = on;
    }
    fn over(&self) -> bool {
        self.result.is_some() || self.conceded.is_some()
    }
    /// A line both players may read (system notices, chat, the result).
    fn say(&mut self, text: impl Into<String>) {
        let t = text.into();
        self.say_to(Side::Corp, t.clone());
        self.say_to(Side::Runner, t);
    }
    fn say_to(&mut self, side: Side, text: impl Into<String>) {
        self.push_line(side, json!({"user": "__system__", "text": text.into()}));
    }
    /// The same event, rendered once per viewer.
    fn say_each(&mut self, lines: [Option<String>; 2]) {
        for (i, l) in lines.into_iter().enumerate() {
            let Some(l) = l else { continue };
            let side = if i == 0 { Side::Corp } else { Side::Runner };
            self.say_to(side, l);
        }
    }
    /// A player's chat line: identical text in both logs, attributed.
    fn chat_line(&mut self, who: &str, text: &str) {
        let v = json!({"user": who, "text": text});
        self.push_line(Side::Corp, v.clone());
        self.push_line(Side::Runner, v);
    }
    fn push_line(&mut self, side: Side, v: Value) {
        let log = &mut self.log[six(side)];
        log.push(v);
        // A narrated game is a much longer log than a list of taken actions
        // was, and the whole of it rides every state push. The bound is what
        // a player can usefully scroll back through; the operator's copy on
        // disk is the one that keeps everything.
        if log.len() > LOG_LINES {
            log.drain(..LOG_LINES / 4);
        }
    }

    /// The kernel's change log, narrated into BOTH logs — one rendering per
    /// viewer, each decided by that viewer's own entitlement (CR §10.2, via
    /// `crlog::narrate`) — and appended verbatim to the operator's transcript.
    ///
    /// Called after EVERY VM step, and the promptness is load-bearing:
    /// visibility is evaluated when a line is rendered, and 7.1.2's "while the
    /// Runner is accessing a card, the Runner is allowed to look at that card"
    /// is true only while the access is in progress. A line rendered late
    /// would have to be vague about the very card the player asked to see.
    fn narrate(&mut self) {
        let upto = self.vm.changes.log.len();
        if upto <= self.narrated {
            return;
        }
        let mut lines: Vec<(Side, String)> = Vec::new();
        let mut seen: Vec<Value> = Vec::new();
        let view = self.vm.view_of(Side::Runner);
        for c in &self.vm.changes.log[self.narrated..upto] {
            self.transcript.change(c);
            // CR 7.1.2, as a CARD rather than a sentence. The log already said
            // "Runner: accesses X from R&D" and that was the only place the
            // card appeared — a player had to read the drawer to learn what
            // they had just looked at. The entitlement is live exactly here,
            // so the card is rendered here and carried to the client.
            if let jinteki_cr::change::GameChange::CardAccessed { obj } = c {
                seen.push(json!({
                    "card": card_json(&self.vm, &view, *obj, true),
                    "from": accessed_from(&self.vm, *obj),
                }));
            }
            for viewer in [Side::Corp, Side::Runner] {
                if let Some(l) = crate::crlog::narrate(&self.vm, c, viewer) {
                    lines.push((viewer, l));
                }
            }
        }
        self.narrated = upto;
        for mut s in seen {
            self.access_seq += 1;
            if let Some(m) = s.as_object_mut() {
                m.insert("seq".into(), json!(self.access_seq));
            }
            self.accesses.push(s);
        }
        // A breach of R&D 3 is three cards to show; a whole game of them is
        // not. Old reveals fall off the front — the log still has the line.
        if self.accesses.len() > ACCESS_REVEALS {
            let drop = self.accesses.len() - ACCESS_REVEALS;
            self.accesses.drain(..drop);
        }
        for (side, l) in lines {
            self.say_to(side, l);
        }
    }
}

/// How many accessed cards the client is offered at once. Deep R&D digs are
/// the reason this is not 1; a bounded queue is the reason it is not the game.
const ACCESS_REVEALS: usize = 6;

/// Which server an accessed card came out of (CR 4.6.2/10.2.3a: WHERE a card
/// is, is open information even where WHAT it is is not). Read at access time,
/// because the card is about to move.
fn accessed_from(vm: &Vm, id: ObjectId) -> Option<String> {
    let z = vm.st.objects.get(&id)?.zone;
    let s = match z {
        Zone::Hand(Side::Corp) => ServerId::Hq,
        Zone::Deck(Side::Corp) => ServerId::Rnd,
        Zone::Discard(Side::Corp) => ServerId::Archives,
        Zone::Root(s) | Zone::Ice(s) => s,
        _ => return None,
    };
    Some(server_label(s))
}

/// Lines kept per side. Older ones fall off the front; nothing is lost that
/// the server-side transcript does not still have.
const LOG_LINES: usize = 600;

/// A seat as the registry hands it back: which side the token sits in, the
/// game both tokens share, and that game's key — carried here so a socket can
/// tell whether a nudge is about its own game without taking the lock.
#[derive(Clone)]
pub struct Seat {
    pub side: Side,
    pub key: String,
    pub game: Arc<Mutex<CrGame>>,
}

type Registry = Arc<Mutex<HashMap<String, Seat>>>;

fn registry() -> Registry {
    static REG: OnceLock<Registry> = OnceLock::new();
    REG.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

/// Sessions idle longer than this are pruned — the same 72h the local engine
/// gives (phones sleep; give them days).
const SESSION_TTL: Duration = Duration::from_secs(72 * 3600);

async fn prune_and_insert(token: String, seat: Seat) {
    let reg = registry();
    let mut map = reg.lock().await;
    let mut dead = Vec::new();
    for (t, s) in map.iter() {
        if let Ok(g) = s.game.try_lock() {
            if g.last_seen.elapsed() > SESSION_TTL {
                dead.push(t.clone());
            }
        }
    }
    for t in dead {
        map.remove(&t);
    }
    map.insert(token, seat);
}

pub async fn lookup(token: &str) -> Option<Seat> {
    registry().lock().await.get(token).cloned()
}

pub fn new_token() -> String {
    format!("{:016x}{:016x}", rand::random::<u64>(), rand::random::<u64>())
}

fn greeting(g: &CrGame, side: Side) -> String {
    let opp = g.seat(side.other());
    if opp.bot {
        format!("CR engine — you are the {}. Seed {}.", side_name(side), g.seed)
    } else {
        format!(
            "CR engine — you are the {} against {}. Seed {}.",
            side_name(side),
            opp.name,
            g.seed
        )
    }
}

fn new_game(setup: GameSetup, seats: [SeatState; 2], bot_delay_ms: u64) -> CrGame {
    let seed = setup.seed;
    let key = new_token();
    // The opening record carries what the game was BUILT from, so the
    // transcript replays: the seed, the seats, and both deck lists.
    let mut transcript = crate::transcript::Transcript::open(&key);
    transcript.started(
        seed,
        &seats[six(Side::Corp)].name,
        &seats[six(Side::Runner)].name,
        json!({
            "corp_identity": setup.corp_identity.as_ref().map(|c| c.name),
            "runner_identity": setup.runner_identity.as_ref().map(|c| c.name),
            "corp_deck": setup.corp_deck.iter().map(|c| c.name).collect::<Vec<_>>(),
            "runner_deck": setup.runner_deck.iter().map(|c| c.name).collect::<Vec<_>>(),
        }),
    );
    let mut g = CrGame {
        vm: Vm::new_game(setup),
        seats,
        key,
        seed,
        pending: None,
        picked: Vec::new(),
        asked: None,
        accesses: Vec::new(),
        access_seq: 0,
        log: [Vec::new(), Vec::new()],
        narrated: 0,
        transcript,
        result: None,
        conceded: None,
        last_seen: Instant::now(),
        bot_delay: Duration::from_millis(bot_delay_ms),
        outcome_recorded: false,
    };
    for side in [Side::Corp, Side::Runner] {
        let line = greeting(&g, side);
        g.say_to(side, line);
    }
    g
}

/// Register every human seat's token against one game.
async fn register(game: &Arc<Mutex<CrGame>>, seats: &[SeatState; 2]) {
    let key = game.lock().await.key.clone();
    for side in [Side::Corp, Side::Runner] {
        let s = &seats[six(side)];
        if let Some(t) = s.token.clone() {
            let seat = Seat { side, key: key.clone(), game: game.clone() };
            prune_and_insert(t, seat).await;
        }
    }
}

/// Create a session from an arbitrary setup. The eternal-deck path goes
/// through [`eternal_setup`]; tests use this directly with small all-complete
/// decks, which is the same code path minus the gate.
pub async fn create_session(setup: GameSetup, human: Side, bot_delay_ms: u64) -> String {
    let token = new_token();
    let mut seats = [SeatState::bot(), SeatState::bot()];
    seats[six(human)] = SeatState::human("you", None).with_token(token.clone());
    let g = new_game(setup, seats.clone(), bot_delay_ms);
    let game = Arc::new(Mutex::new(g));
    register(&game, &seats).await;
    token
}

/// Create a two-human session: one VM, two seats, two resume tokens. A seat
/// that already carries a token (the lobby mints the creator's when they sit
/// down to wait) keeps it, so a waiting player's token survives the start.
pub async fn create_two_human_session(
    setup: GameSetup,
    corp: SeatState,
    runner: SeatState,
) -> (Arc<Mutex<CrGame>>, [String; 2]) {
    let mut seats = [corp, runner];
    for s in seats.iter_mut() {
        if s.token.is_none() {
            s.token = Some(new_token());
        }
    }
    let tokens = [
        seats[0].token.clone().unwrap_or_default(),
        seats[1].token.clone().unwrap_or_default(),
    ];
    let game = Arc::new(Mutex::new(new_game(setup, seats.clone(), 0)));
    register(&game, &seats).await;
    (game, tokens)
}

/// The seat token for one side, if that seat is a person.
pub fn seat_token(g: &CrGame, side: Side) -> Option<String> {
    g.seats[six(side)].token.clone()
}

// ───────────────────────────────────────────────────────────────────────────
// The ws surface (called by `local::handle`, which owns the socket)
// ───────────────────────────────────────────────────────────────────────────

/// The SYS-D-12 refusal, byte for byte the same whichever door it is asked
/// through — `start` (vs Bot) and `lobby-create` (vs Human) share it.
pub async fn refuse_gate(ws: &mut WebSocket, r: &Readiness) {
    let _ = ws
        .send(Message::Text(
            json!({
                "type": "error",
                "error": format!(
                    "the eternal decks are not playable yet: {} cards implemented",
                    r.fraction()
                ),
                "cr_readiness": r,
            })
            .to_string()
            .into(),
        ))
        .await;
}

/// `{"type":"start","engine":"cr",…}` — the gate, then the game.
pub async fn start(
    ws: &mut WebSocket,
    db: &Db,
    user: Option<&str>,
    v: &Value,
) -> Option<(String, Seat)> {
    let human = match v["side"].as_str() {
        Some("corp") => Side::Corp,
        _ => Side::Runner,
    };
    let seed = v["seed"].as_u64().unwrap_or_else(rand::random);
    let setup = match eternal_setup(seed) {
        Ok(s) => s,
        Err(r) => {
            // SYS-D-12: refuse, and say exactly what is missing.
            refuse_gate(ws, &r).await;
            return None;
        }
    };
    let token = create_session(setup, human, 300).await;
    let seat = lookup(&token).await?;
    if let Some(uid) = user {
        record_start(db, &token, uid, human, seed).await;
    }
    let _ = ws
        .send(Message::Text(
            json!({"type":"session","token": token, "side": side_key(human), "engine":"cr"})
                .to_string()
                .into(),
        ))
        .await;
    {
        let mut g = seat.game.lock().await;
        g.set_connected(human, true);
        drive(&mut g, ws, human).await;
        push_state(&g, ws, human).await;
        record_outcome_if_over(db, &mut g).await;
    }
    Some((token, seat))
}

/// One `games` row per SEAT: each player's own token is that player's own
/// game id, so "my games" is honest on both sides of a lobby game.
pub async fn record_start(db: &Db, token: &str, user: &str, side: Side, seed: u64) {
    let conn = db.lock().await;
    let _ = conn.execute(
        "INSERT INTO games (id, owner_id, side, deck_id, seed, started_at)
         VALUES (?1, ?2, ?3, NULL, ?4, datetime('now'))",
        rusqlite::params![token, user, side_key(side), seed as i64],
    );
}

/// Put a socket in a seat: announce the session, run the machine to the next
/// decision anyone owes, and push THIS seat's view. Used by `resume` (a
/// refreshed tab) and by the lobby (a game that has just started).
pub async fn attach(ws: &mut WebSocket, db: &Db, token: &str, seat: &Seat) {
    let mut g = seat.game.lock().await;
    g.last_seen = Instant::now();
    g.set_connected(seat.side, true);
    let side = side_key(seat.side);
    let _ = ws
        .send(Message::Text(
            json!({"type":"session","token": token, "side": side, "engine":"cr"})
                .to_string()
                .into(),
        ))
        .await;
    // The bot may have been mid-move when the old tab died.
    drive(&mut g, ws, seat.side).await;
    push_state(&g, ws, seat.side).await;
    record_outcome_if_over(db, &mut g).await;
}

/// `{"type":"resume","token":…}` for a CR session — either seat's.
pub async fn resume(ws: &mut WebSocket, db: &Db, token: &str, seat: Seat) -> Seat {
    attach(ws, db, token, &seat).await;
    seat
}

/// `{"type":"action",…}` for a CR session. `Ok(true)` means the other seat
/// (if there is a person in it) must be told to redraw.
pub async fn action(ws: &mut WebSocket, db: &Db, seat: &Seat, v: &Value) -> bool {
    let mut g = seat.game.lock().await;
    g.last_seen = Instant::now();
    match apply_command(&mut g, seat.side, v) {
        Ok(true) => {
            push_state(&g, ws, seat.side).await;
            drive(&mut g, ws, seat.side).await;
            push_state(&g, ws, seat.side).await;
            record_outcome_if_over(db, &mut g).await;
            true
        }
        // A partial selection: nothing to resume yet, just re-render.
        Ok(false) => {
            push_state(&g, ws, seat.side).await;
            false
        }
        Err(e) => {
            let _ = ws
                .send(Message::Text(json!({"type":"error","error": e}).to_string().into()))
                .await;
            false
        }
    }
}

/// `{"type":"say","msg":…}` — a chat line, in both players' logs verbatim.
/// Chat is the ONLY thing that crosses the per-side log boundary, and it
/// carries no game information the sender did not choose to give away.
pub async fn chat(seat: &Seat, msg: &str) -> bool {
    let msg = msg.trim();
    if msg.is_empty() {
        return false;
    }
    let msg: String = msg.chars().take(280).collect();
    let mut g = seat.game.lock().await;
    g.last_seen = Instant::now();
    let who = g.seat(seat.side).name.clone();
    g.chat_line(&who, &msg);
    true
}

/// Push this seat's own view down its own socket (the state-changed nudge).
pub async fn push_seat(seat: &Seat, ws: &mut WebSocket) {
    let g = seat.game.lock().await;
    push_state(&g, ws, seat.side).await;
}

/// Attach/detach a socket to a seat. The other player is shown the truth
/// either way — a held game is honest, a silently stalled one is not.
pub async fn set_connected(seat: &Seat, on: bool) {
    let mut g = seat.game.lock().await;
    g.set_connected(seat.side, on);
    if on {
        g.last_seen = Instant::now();
    }
}

async fn record_outcome_if_over(db: &Db, g: &mut CrGame) {
    if g.outcome_recorded || !g.over() {
        return;
    }
    g.outcome_recorded = true;
    let (winner, reason) = outcome(g);
    let tokens: Vec<String> = g.seats.iter().filter_map(|s| s.token.clone()).collect();
    let conn = db.lock().await;
    for t in tokens {
        let _ = conn.execute(
            "UPDATE games SET finished_at = datetime('now'), winner = ?1, reason = ?2
             WHERE id = ?3 AND finished_at IS NULL",
            rusqlite::params![winner, reason, t],
        );
    }
}

/// Run the machine until a PERSON owes a decision, pushing a state (and
/// pausing) whenever the bot does something worth watching. With two people
/// at the table nothing is auto-answered: the loop simply stops at whichever
/// seat is asked, and that seat's socket is the one that finds a prompt.
async fn drive(g: &mut CrGame, ws: &mut WebSocket, viewer: Side) {
    // Anything the last answer set in motion is said before anything else
    // happens, and before the early return below can swallow it.
    g.narrate();
    // A decision already on the table is not ours to re-ask: the other seat's
    // socket may be mid-selection, and re-presenting would discard its picks.
    if g.over() || g.pending.is_some() {
        return;
    }
    for _ in 0..20_000 {
        match g.vm.step() {
            Yield::Progressed => {
                // Every step, so a line is rendered in the state the record
                // was made in (see `CrGame::narrate`).
                g.narrate();
                continue;
            }
            Yield::GameOver(r) => {
                g.narrate();
                g.result = Some(r);
                let (w, why) = outcome(g);
                g.say(format!("Game over — {} wins ({}).", w, why));
                g.transcript.note(&format!("game over: {w} wins ({why})"));
                g.transcript.flush();
                return;
            }
            Yield::Decision(side, spec) => {
                g.narrate();
                // PRIORITY, written once, where the engine actually hands it
                // over. Everything the board says about whose decision it is
                // reads off this — including the bot's, so a state pushed
                // between two of its moves still says who is speaking.
                g.asked = Some(side);
                g.transcript.decision(side, &spec);
                if !g.seats[six(side)].bot {
                    let p = present(&g.vm, side, &spec);
                    g.picked.clear();
                    g.pending = Some(p);
                    g.transcript.flush();
                    return;
                }
                let answer = default_answer(&spec);
                let noteworthy = describe_move(g, &spec, &answer, side);
                let worth_a_frame = noteworthy.iter().any(|l| l.is_some());
                g.transcript.answer(side, "bot", &answer);
                g.vm.answer(answer);
                if worth_a_frame {
                    g.say_each(noteworthy);
                }
                g.narrate();
                if worth_a_frame {
                    push_state(g, ws, viewer).await;
                    if !g.bot_delay.is_zero() {
                        tokio::time::sleep(g.bot_delay).await;
                    }
                }
            }
        }
    }
    g.say("The engine ran 20000 steps without asking anyone anything — stopping.");
    g.transcript.note("the engine ran 20000 steps without asking anyone anything");
    g.transcript.flush();
}

/// What a player just did, rendered ONCE PER VIEWER from that viewer's own
/// `view_of` (a card a reader is not entitled to see is never named in their
/// log). `None` = not worth a frame: passing a window is declining to act,
/// and the CR opens a great many windows.
/// The card an offered choice lives on, where it lives on one.
///
/// Every window option and action that names a card yields it, so the client
/// can outline that card as usable instead of printing its label as a button.
/// A `TriggerInstance` names a pending ability rather than a card, so the
/// instance is resolved back to the source it belongs to.
///
/// A TARGET ANNOUNCEMENT (1.15.2) names cards too, and answering it with one
/// card is the commonest decision in the game — "Choose 1 card" over two
/// copies of Boomerang is two identical text buttons unless the card travels
/// with the choice. Only single-card answers map: an answer naming several
/// cards is not ABOUT one card, and the client picks those by tapping.
fn choice_card(vm: &Vm, a: &DecisionAnswer) -> Option<ObjectId> {
    match a {
        DecisionAnswer::Take(w) => match w {
            WindowOption::TriggerPaid { ability, .. } => Some(ability.obj),
            WindowOption::Rez { card }
            | WindowOption::RezApproachedIce { card }
            | WindowOption::Score { card }
            | WindowOption::BasicTrash { card, .. } => Some(*card),
            WindowOption::TriggerInstance { instance, .. } => vm.instance_source(*instance),
        },
        DecisionAnswer::Action(o) => match o {
            ActionOption::BasicPlayOperation { card }
            | ActionOption::BasicInstall { card }
            | ActionOption::BasicAdvance { card } => Some(*card),
            ActionOption::CardAction { ability, .. } => Some(ability.obj),
            _ => None,
        },
        // 1.15.2 target announcements, 1.16.1 cards spent for a cost, 5.5.4c
        // discards and 11.5's access candidate: each one card, each a card.
        DecisionAnswer::Targets(cs) | DecisionAnswer::Discard(cs) => match cs.as_slice() {
            [c] => Some(*c),
            _ => None,
        },
        DecisionAnswer::Candidate(c) => Some(*c),
        // 1.12.1: a counter is an object, and the object a player recognises
        // is the card holding it. All from one host, or it is not one card.
        DecisionAnswer::Counters(cs) => match cs.split_first() {
            Some((first, rest)) if rest.iter().all(|c| c.host == first.host) => Some(first.host),
            _ => None,
        },
        _ => None,
    }
}

fn describe_move(
    g: &CrGame,
    spec: &DecisionSpec,
    answer: &DecisionAnswer,
    actor: Side,
) -> [Option<String>; 2] {
    let who = side_name(actor);
    let mut out = [None, None];
    for viewer in [Side::Corp, Side::Runner] {
        let view = g.vm.view_of(viewer);
        out[six(viewer)] = match (spec, answer) {
            (_, DecisionAnswer::Pass) => None,
            (DecisionSpec::TakeAction { .. }, DecisionAnswer::Action(a)) => {
                Some(format!("{who}: {}", action_label(&g.vm, &view, a)))
            }
            (_, DecisionAnswer::Take(o)) => {
                Some(format!("{who}: {}", window_label(&g.vm, &view, o)))
            }
            (DecisionSpec::Mulligan, DecisionAnswer::TakeMulligan) => {
                Some(format!("{who}: takes a mulligan."))
            }
            (DecisionSpec::Mulligan, _) => Some(format!("{who}: keeps their opening hand.")),
            _ => None,
        };
    }
    out
}

fn outcome(g: &CrGame) -> (String, String) {
    if let Some(loser) = g.conceded {
        return (side_key(loser.other()).into(), "conceded".into());
    }
    match g.result {
        Some(GameResult::AgendaPoints(s)) => {
            (side_key(s).into(), "7 agenda points".into())
        }
        Some(GameResult::Flatline) => ("corp".into(), "the Runner is flatlined".into()),
        Some(GameResult::RndEmpty) => {
            ("runner".into(), "the Corp must draw from an empty R&D".into())
        }
        Some(GameResult::Draw) => ("draw".into(), "a simultaneous win".into()),
        None => (String::new(), String::new()),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Client command → DecisionAnswer
// ───────────────────────────────────────────────────────────────────────────

/// Apply one client command from `actor`'s seat. `Ok(true)` = the VM was
/// resumed and should be driven; `Ok(false)` = the command changed only the
/// pending selection.
fn apply_command(g: &mut CrGame, actor: Side, v: &Value) -> Result<bool, String> {
    let cmd = v["command"].as_str().ok_or("missing command")?;
    let args = &v["args"];
    if cmd == "concede" {
        g.conceded = Some(actor);
        g.pending = None;
        let who = side_name(actor);
        g.say(format!("{who}: concedes."));
        g.transcript.note(&format!("{who} conceded"));
        g.transcript.flush();
        return Ok(true);
    }
    if g.over() {
        return Err("the game is over".into());
    }
    let Some(p) = g.pending.as_ref() else {
        return Err("nothing to decide right now".into());
    };
    // Two people at one table: a seat answers its OWN decisions and no others.
    if p.side != actor {
        return Err("it is not your decision right now".into());
    }
    let cid = || -> Option<ObjectId> {
        args["card"]["cid"]
            .as_u64()
            .or_else(|| args["cid"].as_u64())
            .map(|c| ObjectId(c as u32))
    };

    // 1. A choice button.
    if cmd == "choice" {
        let uuid = args["choice"]["uuid"].as_str().ok_or("missing choice uuid")?;
        let answer = p
            .choices
            .iter()
            .find(|(u, ..)| u == uuid)
            .map(|(_, _, a, _)| a.clone())
            .ok_or("that choice is not on offer")?;
        return Ok(answer_now(g, answer));
    }

    // CR 8.3.3: the arranged order, answered wholesale. The client sends the
    // cids in the order it wants them, topmost first; anything missing or
    // foreign is rejected rather than silently reordered.
    if cmd == "arrange" {
        let want = p.arrange.clone().ok_or("nothing to arrange right now")?;
        let got: Vec<ObjectId> = args["order"]
            .as_array()
            .ok_or("missing order")?
            .iter()
            .filter_map(|v| v.as_u64().map(|c| ObjectId(c as u32)))
            .collect();
        if got.len() != want.len() || !want.iter().all(|c| got.contains(c)) {
            return Err("that is not an arrangement of these cards".into());
        }
        return Ok(answer_now(g, DecisionAnswer::Arrangement(got)));
    }

    // CR 1.15.2e: "choose UP TO n" is finished when the chooser says it is,
    // any time from `min` picks onward. Without this the only expressible
    // answer is exactly `count` cards, so a player who wants one of the two
    // targets on offer is forced to announce both.
    if cmd == "select-done" {
        let sel = p.select.as_ref().ok_or("nothing to select right now")?;
        if !sel.complete_at(g.picked.len()) {
            return Err("you have not chosen enough cards yet".into());
        }
        let picks = std::mem::take(&mut g.picked);
        let answer = match sel.kind {
            SelectKind::Targets => DecisionAnswer::Targets(picks),
            SelectKind::Discard => DecisionAnswer::Discard(picks),
            // 11.5 step 4a always names exactly one candidate; there is no
            // "up to" access, so this is unreachable rather than a policy.
            SelectKind::Candidate => return Err("an access candidate is not optional".into()),
        };
        return Ok(answer_now(g, answer));
    }

    // 2. A card tap in select mode.
    if cmd == "select" {
        let id = cid().ok_or("missing card")?;
        let sel = p.select.as_ref().ok_or("nothing to select right now")?;
        if !sel.candidates.contains(&id) {
            return Err("that card is not a legal choice".into());
        }
        if let Some(i) = g.picked.iter().position(|c| *c == id) {
            g.picked.remove(i); // tap again to unpick
            return Ok(false);
        }
        g.picked.push(id);
        let (count, kind) = (sel.count, sel.kind);
        // 5.5.4c is IRREVERSIBLE and it is the one decision a player makes
        // while they are out of clicks and thinking about next turn. So a
        // discard STAGES: the cards are marked, nothing has happened yet, and
        // a separate confirmation is what actually throws them away. Every
        // other select still fires on its last pick — those are announcements
        // (1.15.2), and an announcement the player is still assembling has no
        // effect to take back.
        if kind == SelectKind::Discard {
            return Ok(false);
        }
        if g.picked.len() as u32 >= count {
            let picks = std::mem::take(&mut g.picked);
            let answer = match kind {
                SelectKind::Targets => DecisionAnswer::Targets(picks),
                SelectKind::Discard => DecisionAnswer::Discard(picks),
                SelectKind::Candidate => DecisionAnswer::Candidate(picks[0]),
            };
            return Ok(answer_now(g, answer));
        }
        return Ok(false);
    }

    // The second half of a staged discard: the player has looked at what
    // they marked and said yes. Refused unless the set is exactly the size
    // 5.5.4c asks for, so the button can never commit a half-answer.
    if cmd == "select-confirm" {
        let sel = p.select.as_ref().ok_or("nothing to select right now")?;
        if sel.kind != SelectKind::Discard {
            return Err("that choice is not staged".into());
        }
        if g.picked.len() as u32 != sel.count {
            return Err("that is not the number of cards being discarded".into());
        }
        let picks = std::mem::take(&mut g.picked);
        return Ok(answer_now(g, DecisionAnswer::Discard(picks)));
    }

    // …and the way back out of it: unstage everything and start again.
    if cmd == "select-clear" {
        p.select.as_ref().ok_or("nothing to select right now")?;
        g.picked.clear();
        return Ok(false);
    }

    // 3. Run controls.
    if cmd == "jack-out" || cmd == "continue" {
        if matches!(p.spec, DecisionSpec::JackOut) {
            return Ok(answer_now(g, DecisionAnswer::JackOut(cmd == "jack-out")));
        }
        return Err("there is nothing to jack out of".into());
    }

    // 4. An action-window affordance.
    let DecisionSpec::TakeAction { options } = &p.spec else {
        return Err(format!("{cmd} is not what is being asked of you"));
    };
    let want = |o: &ActionOption| -> bool {
        match (cmd, o) {
            ("credit", ActionOption::BasicCredit) => true,
            ("draw", ActionOption::BasicDraw) => true,
            ("remove-tag", ActionOption::BasicRemoveTag) => true,
            ("purge", ActionOption::BasicPurge) => true,
            ("trash-resource", ActionOption::BasicTrashResource) => true,
            ("run", ActionOption::BasicRun { server }) => {
                args["server"].as_str().and_then(server_from_key) == Some(*server)
            }
            ("play", ActionOption::BasicPlayOperation { card }) => Some(*card) == cid(),
            ("corp-install" | "runner-install", ActionOption::BasicInstall { card }) => {
                Some(*card) == cid()
            }
            ("advance", ActionOption::BasicAdvance { card }) => Some(*card) == cid(),
            ("ability", ActionOption::CardAction { ability, .. }) => {
                Some(ability.obj) == cid()
                    && args["ability"].as_u64().unwrap_or(0) as usize == ability.index
            }
            _ => false,
        }
    };
    let opt = options
        .iter()
        .find(|o| want(o))
        .cloned()
        .ok_or("that action is not legal right now")?;
    // One line per reader, each from that reader's own view (SYS-S-1).
    let mut lines = [None, None];
    for viewer in [Side::Corp, Side::Runner] {
        let view = g.vm.view_of(viewer);
        lines[six(viewer)] =
            Some(format!("{}: {}", side_name(actor), action_label(&g.vm, &view, &opt)));
    }
    g.say_each(lines);
    Ok(answer_now(g, DecisionAnswer::Action(opt)))
}

fn answer_now(g: &mut CrGame, a: DecisionAnswer) -> bool {
    let side = g.pending.as_ref().map(|p| p.side).unwrap_or(g.vm.st.turn_side);
    g.transcript.answer(side, "human", &a);
    g.pending = None;
    g.picked.clear();
    g.vm.answer(a);
    true
}

// ───────────────────────────────────────────────────────────────────────────
// DecisionSpec → the UI's prompt shapes
// ───────────────────────────────────────────────────────────────────────────

fn present(vm: &Vm, asked: Side, spec: &DecisionSpec) -> Pending {
    let view = vm.view_of(asked);
    let mut p = Pending {
        side: asked,
        spec: spec.clone(),
        msg: String::new(),
        choices: Vec::new(),
        select: None,
        focus: None,
        arrange: None,
        actions: Vec::new(),
    };
    // A choice uuid is unique to the DECISION, not just to its position in
    // the list. Two prompts in a row both numbering their options from 0 mean
    // a tap that lands a frame late answers the NEXT question with whatever
    // happens to sit at that index — a misplay the player never made and
    // cannot see. A per-decision stamp makes a stale uuid simply not match.
    static DECISION_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = DECISION_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let push = |p: &mut Pending, label: String, a: DecisionAnswer| {
        let card = choice_card(vm, &a);
        p.choices.push((format!("{seq}-{}", p.choices.len()), label, a, card));
    };
    // The same, where the option is about a card the ANSWER does not name
    // (a division of credits, a set to trash): the arm knows, so it says.
    let push_on = |p: &mut Pending, label: String, a: DecisionAnswer, card: ObjectId| {
        p.choices.push((format!("{seq}-{}", p.choices.len()), label, a, Some(card)));
    };
    // Every card the board should light up GOLD for this decision, as an
    // affordance the client already knows how to draw and tap (THE LAW §3).
    // Without these the CR engine asked "tap the cards" and lit nothing:
    // a multi-card choice, whose per-card buttons are dropped below, had no
    // answer the player could give at all.
    let selectable = |p: &mut Pending, candidates: &[ObjectId]| {
        p.actions = candidates
            .iter()
            .map(|c| json!({"command":"select","cid": c.0, "label": name_of(vm, &view, *c)}))
            .collect();
    };
    match spec {
        DecisionSpec::Mulligan => {
            p.msg = "Keep this opening hand? (CR 1.6.6a)".into();
            push(&mut p, "Keep".into(), DecisionAnswer::KeepHand);
            push(&mut p, "Mulligan".into(), DecisionAnswer::TakeMulligan);
        }
        // The action window drives the board itself: chips, hand glow, the
        // card sheets. No prompt sheet, exactly as the local engine.
        DecisionSpec::TakeAction { options } => {
            p.actions = options.iter().map(|o| action_json(vm, &view, o)).collect();
        }
        DecisionSpec::PaidWindow { classes, options } => {
            p.msg = if classes.rez_approached_ice {
                "Paid ability window — the approached ice may be rezzed (9.2.7e).".into()
            } else if classes.score {
                "Paid ability window — you may score (9.2.7d).".into()
            } else {
                "Paid ability window (9.2.7).".into()
            };
            for o in options {
                push(&mut p, window_label(vm, &view, o), DecisionAnswer::Take(o.clone()));
            }
            push(&mut p, "Pass".into(), DecisionAnswer::Pass);
        }
        DecisionSpec::ReactionWindow { options, can_pass } => {
            p.msg = "Reaction window (9.2.8) — trigger an ability?".into();
            for o in options {
                push(&mut p, window_label(vm, &view, o), DecisionAnswer::Take(o.clone()));
            }
            if *can_pass {
                push(&mut p, "Pass".into(), DecisionAnswer::Pass);
            }
        }
        DecisionSpec::InterruptWindow { options, can_pass } => {
            p.msg = "Interrupt window (9.2.9).".into();
            for o in options {
                push(&mut p, window_label(vm, &view, o), DecisionAnswer::Take(o.clone()));
            }
            if *can_pass {
                push(&mut p, "Pass".into(), DecisionAnswer::Pass);
            }
        }
        DecisionSpec::MidAccessWindow { options, can_pass } => {
            p.msg = "You may trash the card or use a mid-access ability (9.2.10).".into();
            let trash_cost = options.iter().find_map(|o| match o {
                WindowOption::BasicTrash { cost, .. } => Some(*cost),
                _ => None,
            });
            // The card you are accessing is the card you most need to SEE:
            // hand it to the access reader instead of a sentence about it.
            if let Some(card) = vm.st.accessed.filter(|c| view.sees(*c)) {
                p.focus = Some(Focus { card, kind: "access", trash_cost });
            }
            for o in options {
                push(&mut p, window_label(vm, &view, o), DecisionAnswer::Take(o.clone()));
            }
            if *can_pass {
                push(&mut p, "No action".into(), DecisionAnswer::Pass);
            }
        }
        DecisionSpec::ChooseTargets { candidates, count, up_to, min, distinct_names } => {
            p.msg = format!(
                "Choose {}{} card{}{} (CR 1.15.2).",
                if *up_to { "up to " } else { "" },
                count,
                if *count == 1 { "" } else { "s" },
                // CR 2.1.5: the constraint is on the set, so it belongs in
                // the prompt rather than in the candidate list.
                if *distinct_names { " with different names" } else { "" }
            );
            p.select = Some(Select {
                candidates: candidates.clone(),
                count: *count,
                min: *min,
                up_to: *up_to,
                kind: SelectKind::Targets,
            });
            selectable(&mut p, candidates);
            for c in candidates {
                push(
                    &mut p,
                    name_of(vm, &view, *c),
                    DecisionAnswer::Targets(vec![*c]),
                );
            }
            if *up_to && *min == 0 {
                push(&mut p, "None".into(), DecisionAnswer::Targets(Vec::new()));
            }
            // A multi-card choice is made by tapping; the buttons above answer
            // only the one-card case, so drop them when more are needed. The
            // candidate list still travels (as `select`), so the prompt shows
            // the cards and the board lights them — the tap is not a guess.
            if *count > 1 {
                p.choices.retain(|(_, _, a, _)| {
                    matches!(a, DecisionAnswer::Targets(t) if t.is_empty())
                });
                p.msg.push_str(" Tap the cards.");
            }
        }
        DecisionSpec::PaymentCards { candidates, count, label } => {
            p.msg = format!(
                "Choose {count} card{} to {label} (CR 1.16.1).{}",
                if *count == 1 { "" } else { "s" },
                if *count == 1 { "" } else { " Tap the cards." }
            );
            p.select = Some(Select {
                candidates: candidates.clone(),
                count: *count,
                min: *count,
                up_to: false,
                kind: SelectKind::Targets,
            });
            selectable(&mut p, candidates);
            if *count == 1 {
                for c in candidates {
                    push(
                        &mut p,
                        name_of(vm, &view, *c),
                        DecisionAnswer::Targets(vec![*c]),
                    );
                }
            }
        }
        DecisionSpec::DiscardCards { count, hand } => {
            // Two phases, and the sentence says which one you are in — the
            // client appends the confirmation. Nothing is discarded until the
            // set is complete AND confirmed (5.5.4c is irreversible).
            p.msg = format!(
                "Choose {count} card{} to discard down to your maximum hand size (5.5.4c).",
                if *count == 1 { "" } else { "s" },
            );
            p.select = Some(Select {
                candidates: hand.clone(),
                count: *count,
                min: *count,
                up_to: false,
                kind: SelectKind::Discard,
            });
            selectable(&mut p, hand);
            if *count == 1 {
                for c in hand {
                    push(
                        &mut p,
                        name_of(vm, &view, *c),
                        DecisionAnswer::Discard(vec![*c]),
                    );
                }
            }
        }
        DecisionSpec::ChooseCandidate { candidates } => {
            p.msg = "Choose a card to access (11.5 step 4a).".into();
            p.select = Some(Select {
                candidates: candidates.clone(),
                count: 1,
                min: 1,
                up_to: false,
                kind: SelectKind::Candidate,
            });
            selectable(&mut p, candidates);
            for (i, c) in candidates.iter().enumerate() {
                let label = if view.sees(*c) {
                    name_of(vm, &view, *c)
                } else {
                    format!("Unseen card {}", i + 1)
                };
                push(&mut p, label, DecisionAnswer::Candidate(*c));
            }
        }
        DecisionSpec::DeclareBreachCandidate { card } => {
            p.msg = format!(
                "{} entered this server — access it too? (7.4.6a)",
                name_of(vm, &view, *card)
            );
            // The question is about ONE card, so the card is the prompt.
            p.focus = Some(Focus { card: *card, kind: "candidate", trash_cost: None });
            push(&mut p, "Yes".into(), DecisionAnswer::ResolveOptional(true));
            push(&mut p, "No".into(), DecisionAnswer::ResolveOptional(false));
        }
        DecisionSpec::DeclareInstallDestination { options } => {
            p.msg = "Where does it go? (8.5.16b)".into();
            for d in options {
                let label = install_dest_label(vm, &view, d);
                let a = DecisionAnswer::InstallDestination(*d);
                // 1.13.6a: hosting names a card, and "Hosted on Sahasrara"
                // is a card the player should see rather than read.
                match d {
                    jinteki_cr::instr::InstallDest::HostedOn(c) => push_on(&mut p, label, a, *c),
                    _ => push(&mut p, label, a),
                }
            }
        }
        DecisionSpec::DeclareAttackedServer { options } => {
            p.msg = "Which server are you running? (6.9.1a)".into();
            for s in options {
                push(&mut p, server_label(*s), DecisionAnswer::AttackedServer(*s));
            }
        }
        // CR 1.15.1b: naming a card or a number. The kernel offers no
        // candidate list — the namespace is open, and the only list it could
        // build from its own state is the union of both decks, which §10.2
        // does not entitle the naming player to see. Resolving a player's
        // input to a real printed card is the DRIVER's job, so the list comes
        // from the card layer's own registry of printed titles.
        DecisionSpec::NameValue { of, excluding } => match of {
            jinteki_cr::instr::NameSpace::CardName => {
                p.msg = match excluding {
                    Some(jinteki_cr::instr::NameExclusion::SourceName) => {
                        "Name a card other than this one (1.15.1b).".into()
                    }
                    None => "Name a card (1.15.1b).".into(),
                };
                let mut names: Vec<&'static str> =
                    jinteki_cards::all_cards().iter().map(|c| c.name()).collect();
                names.sort_unstable();
                names.dedup();
                for n in names {
                    push(
                        &mut p,
                        n.to_string(),
                        DecisionAnswer::NamedValue(jinteki_cr::instr::NamedValue::CardName(n)),
                    );
                }
            }
            // 1.1.3: numbers in this game are integers. The prompt offers the
            // span a printed rez cost, play cost or advancement requirement
            // can actually take; the kernel accepts any integer.
            jinteki_cr::instr::NameSpace::Number => {
                p.msg = "Name a number (1.15.1b).".into();
                for n in 0..=12i64 {
                    push(
                        &mut p,
                        n.to_string(),
                        DecisionAnswer::NamedValue(jinteki_cr::instr::NamedValue::Number(n)),
                    );
                }
            }
        },
        DecisionSpec::ChooseSubroutines { candidates, count, up_to } => {
            p.msg = format!(
                "Announce {}{count} subroutine{} (1.15.1).",
                if *up_to { "up to " } else { "" },
                if *count == 1 { "" } else { "s" }
            );
            if *count == 1 {
                for (k, l) in candidates {
                    push(&mut p, sym_label(l), DecisionAnswer::Subroutines(vec![*k]));
                }
            } else if *count > 0 {
                // Break-from-the-top, and every contiguous run of `count`:
                // enough to say "break these", without a reorderable list.
                // `count` must be non-zero here: `count - 1` is unsigned, and
                // at zero it underflows — a panic in debug and a wrap to
                // `usize::MAX` in release. 1.15.2b clamps this count to the
                // subroutines that exist, so zero is a state the kernel can
                // produce (an ability announcing against ice whose
                // subroutines are all broken already).
                for start in 0..candidates.len().saturating_sub(*count as usize - 1) {
                    let chunk: Vec<_> = candidates[start..start + *count as usize].to_vec();
                    let label = chunk
                        .iter()
                        .map(|(_, l)| sym_label(l))
                        .collect::<Vec<_>>()
                        .join(" + ");
                    push(
                        &mut p,
                        label,
                        DecisionAnswer::Subroutines(chunk.iter().map(|(k, _)| *k).collect()),
                    );
                }
            }
            // …and the empty announcement is an answer whenever it is a legal
            // one: an explicit "up to", or a count 1.15.2b already clamped to
            // nothing. Without the second case a zero-count announcement
            // renders no buttons at all, which is the dead end this whole
            // change is about.
            if *up_to || *count == 0 {
                push(&mut p, "None".into(), DecisionAnswer::Subroutines(Vec::new()));
            }
        }
        DecisionSpec::ChooseOption { options } => {
            p.msg = "Choose (9.11.4g).".into();
            for (i, l) in options.iter().enumerate() {
                push(&mut p, sym_label(l), DecisionAnswer::Option(i));
            }
        }
        DecisionSpec::NestedCost { cost } => {
            p.msg = format!("Pay {}? (9.11.4f)", cost_label(cost));
            push(&mut p, "Pay".into(), DecisionAnswer::PayNestedCost(true));
            push(&mut p, "Decline".into(), DecisionAnswer::PayNestedCost(false));
        }
        DecisionSpec::OptionalEffect { label } => {
            p.msg = sym_label(label);
            push(&mut p, "Yes".into(), DecisionAnswer::ResolveOptional(true));
            push(&mut p, "No".into(), DecisionAnswer::ResolveOptional(false));
        }
        DecisionSpec::AlternatePayment { label, covers, instead } => {
            p.msg = format!(
                "{} — covers {covers}[c], costs {} instead? (1.16.2e)",
                sym_label(label),
                cost_label(instead)
            );
            push(&mut p, "Use it".into(), DecisionAnswer::ResolveOptional(true));
            push(&mut p, "No".into(), DecisionAnswer::ResolveOptional(false));
        }
        DecisionSpec::JackOut => {
            p.msg = "Jack out? (6.9.4c)".into();
            // The run controls already carry these two buttons.
            p.actions = vec![
                json!({"command":"continue","label":"Continue"}),
                json!({"command":"jack-out","label":"Jack out"}),
            ];
            push(&mut p, "Jack out".into(), DecisionAnswer::JackOut(true));
            push(&mut p, "Continue the run".into(), DecisionAnswer::JackOut(false));
        }
        DecisionSpec::MinimalSet { sets } => {
            p.msg = "Which of these is trashed? (10.3.1e)".into();
            for (i, s) in sets.iter().enumerate() {
                let label =
                    s.iter().map(|c| name_of(vm, &view, *c)).collect::<Vec<_>>().join(", ");
                let a = DecisionAnswer::ChooseSet(i);
                // The common case by far is one card per set — "trash this or
                // that" — and that case is a choice about cards.
                match s.as_slice() {
                    [c] => push_on(&mut p, label, a, *c),
                    _ => push(&mut p, label, a),
                }
            }
        }
        DecisionSpec::TraceSpend { max, strength_so_far, corp_side } => {
            p.msg = format!(
                "Trace — {} so far is {strength_so_far}. Spend how many credits? (10.8.6)",
                if *corp_side { "the trace strength" } else { "the link strength" }
            );
            for n in 0..=*max {
                push(&mut p, format!("{n}[c]"), DecisionAnswer::SpendCredits(n));
            }
        }
        DecisionSpec::PsiBid { legal } => {
            p.msg = "Psi game — bid secretly (10.14.6b).".into();
            for n in legal {
                push(&mut p, format!("{n}[c]"), DecisionAnswer::Bid(*n));
            }
        }
        DecisionSpec::DeclareX { max } => {
            p.msg = "Announce the value of X (1.16.2c).".into();
            for n in 0..=*max {
                push(&mut p, format!("X = {n}"), DecisionAnswer::DeclaredX(n));
            }
        }
        DecisionSpec::DivideCostReduction { total } => {
            p.msg = format!("How many of the {total} credits come off the install cost? (1.16.2f)");
            for n in 0..=*total {
                push(&mut p, format!("{n} on install"), DecisionAnswer::DivideReduction(n));
            }
        }
        DecisionSpec::LoopCount { period } => {
            p.msg = format!(
                "A mandatory loop of {period} abilities — how many more times? (10.1.6a)"
            );
            for n in [0u32, 1, 2, 3, 5, 10] {
                push(&mut p, format!("{n}×"), DecisionAnswer::LoopCount(n));
            }
        }
        DecisionSpec::ChooseCounters { candidates, count, up_to } => {
            p.msg = format!(
                "Announce {}{count} counter{} (1.15.1).",
                if *up_to { "up to " } else { "" },
                if *count == 1 { "" } else { "s" }
            );
            // Counters are addressed by (host, kind, ordinal); the player
            // chooses a HOST and takes them from the top.
            let mut hosts: Vec<ObjectId> = Vec::new();
            for c in candidates {
                if !hosts.contains(&c.host) {
                    hosts.push(c.host);
                }
            }
            for h in hosts {
                let mine: Vec<_> =
                    candidates.iter().filter(|c| c.host == h).take(*count as usize).copied().collect();
                if mine.len() < *count as usize && !*up_to {
                    continue;
                }
                push_on(
                    &mut p,
                    format!("{count} from {}", name_of(vm, &view, h)),
                    DecisionAnswer::Counters(mine),
                    h,
                );
            }
            if *up_to {
                push(&mut p, "None".into(), DecisionAnswer::Counters(Vec::new()));
            }
        }
        DecisionSpec::DivideCreditPayment { total, locations } => {
            p.msg = format!("Where do the {total} credits come from? (1.10.3c)");
            // One choice per "spend from here first", which is the shape real
            // recurring-credit decisions take. A mixed division that is not a
            // priority order has no expression in this UI (see the handoff).
            for (i, (loc, _)) in locations.iter().enumerate() {
                let mut left = *total;
                let mut div = vec![0u32; locations.len()];
                let order = std::iter::once(i).chain((0..locations.len()).filter(|j| *j != i));
                for j in order {
                    let take = locations[j].1.min(left);
                    div[j] = take;
                    left -= take;
                }
                if left > 0 {
                    continue;
                }
                // Recurring credits sit ON a card, and which card is the whole
                // question — Net Mercur or the pool is a real decision.
                match loc {
                    None => push(
                        &mut p,
                        "Credit pool first".into(),
                        DecisionAnswer::Division(div),
                    ),
                    Some(c) => push_on(
                        &mut p,
                        format!("{} first", name_of(vm, &view, *c)),
                        DecisionAnswer::Division(div),
                        *c,
                    ),
                }
            }
        }
        // Ordering declarations have no shape in this UI yet. They are offered
        // honestly, labelled as such, and answered by the engine's own neutral
        // policy — never silently.
        DecisionSpec::DeclareSubroutineOrder { granted, .. } => {
            p.msg = format!(
                "Order {} granted subroutine(s) — not yet expressible here; the engine's default (last) applies. (9.8.2c)",
                granted.len()
            );
            push(&mut p, "Continue".into(), default_answer(spec));
        }
        // CR 8.3.3: the player "secretly puts them in the order of their
        // choice, and returns them to the top of that deck". The order is a
        // DECLARATION, not a target announcement — nothing is chosen to be
        // acted on — so it is answered wholesale by the arranged list rather
        // than card by card.
        DecisionSpec::ArrangeCards { cards } => {
            p.msg = format!(
                "Arrange {} card(s) — first is topmost. Drag to reorder (8.3.3).",
                cards.len()
            );
            p.arrange = Some(cards.clone());
            // 8.3.1a: arranging 1 or fewer cards does nothing, so there is
            // nothing to drag and the only sensible offer is to go on.
            if cards.len() <= 1 {
                push(&mut p, "Continue".into(), default_answer(spec));
            }
        }
    }
    p
}

// ───────────────────────────────────────────────────────────────────────────
// Labels
// ───────────────────────────────────────────────────────────────────────────

fn side_key(s: Side) -> &'static str {
    match s {
        Side::Corp => "corp",
        Side::Runner => "runner",
    }
}
pub(crate) fn side_name(s: Side) -> &'static str {
    match s {
        Side::Corp => "Corp",
        Side::Runner => "Runner",
    }
}
fn server_key(s: ServerId) -> String {
    match s {
        ServerId::Hq => "hq".into(),
        ServerId::Rnd => "rd".into(),
        ServerId::Archives => "archives".into(),
        ServerId::Remote(n) => format!("remote{n}"),
    }
}
fn server_from_key(k: &str) -> Option<ServerId> {
    match k {
        "hq" => Some(ServerId::Hq),
        "rd" => Some(ServerId::Rnd),
        "archives" => Some(ServerId::Archives),
        _ => k.strip_prefix("remote").and_then(|n| n.parse().ok()).map(ServerId::Remote),
    }
}
pub(crate) fn server_label(s: ServerId) -> String {
    match s {
        ServerId::Hq => "HQ".into(),
        ServerId::Rnd => "R&D".into(),
        ServerId::Archives => "Archives".into(),
        ServerId::Remote(n) => format!("Server {n}"),
    }
}
fn type_name(t: CardType) -> &'static str {
    match t {
        CardType::Identity => "Identity",
        CardType::Agenda => "Agenda",
        CardType::Asset => "Asset",
        CardType::Ice => "ICE",
        CardType::Operation => "Operation",
        CardType::Upgrade => "Upgrade",
        CardType::Event => "Event",
        CardType::Hardware => "Hardware",
        CardType::Program => "Program",
        CardType::Resource => "Resource",
    }
}

/// CR 10.2.2b: a card this player has not been shown is not named to them.
fn name_of(vm: &Vm, view: &View, id: ObjectId) -> String {
    if view.sees(id) {
        vm.st
            .objects
            .get(&id)
            .map(|o| o.printed.name.to_string())
            .unwrap_or_else(|| "a card".into())
    } else {
        "a card".into()
    }
}

fn sym_label(s: &str) -> String {
    s.to_string()
}

fn cost_label(c: &jinteki_cr::ability::Cost) -> String {
    // The Cost algebra has no display of its own; a debug rendering is honest
    // and readable enough for a prompt ("Credits(3)").
    let s = format!("{c:?}");
    s.replace("Cost ", "")
}

fn window_label(vm: &Vm, view: &View, o: &WindowOption) -> String {
    match o {
        WindowOption::TriggerInstance { label, mandatory, .. } => {
            if *mandatory {
                format!("{label} (must)")
            } else {
                sym_label(label)
            }
        }
        WindowOption::TriggerPaid { ability, label } => {
            format!("{} — {}", sym_label(label), name_of(vm, view, ability.obj))
        }
        WindowOption::Rez { card } => {
            let cost = vm.st.objects.get(card).and_then(|o| o.printed.cost);
            match cost {
                Some(c) => format!("Rez {} ({c}[c])", name_of(vm, view, *card)),
                None => format!("Rez {}", name_of(vm, view, *card)),
            }
        }
        WindowOption::RezApproachedIce { card } => {
            let cost = vm.st.objects.get(card).and_then(|o| o.printed.cost);
            match cost {
                Some(c) => format!("Rez the approached ice ({c}[c])"),
                None => format!("Rez {}", name_of(vm, view, *card)),
            }
        }
        WindowOption::Score { card } => format!("Score {}", name_of(vm, view, *card)),
        WindowOption::BasicTrash { card, cost } => {
            format!("Pay {cost}[c] to trash {}", name_of(vm, view, *card))
        }
    }
}

fn action_label(vm: &Vm, view: &View, a: &ActionOption) -> String {
    match a {
        ActionOption::BasicCredit => "Gain 1[c]".into(),
        ActionOption::BasicDraw => "Draw 1 card".into(),
        ActionOption::BasicRun { server } => format!("Run {}", server_label(*server)),
        ActionOption::BasicRemoveTag => "Remove 1 tag".into(),
        ActionOption::BasicPlayOperation { card } => {
            format!("Play {}", name_of(vm, view, *card))
        }
        ActionOption::BasicInstall { card } => format!("Install {}", name_of(vm, view, *card)),
        ActionOption::BasicAdvance { card } => format!("Advance {}", name_of(vm, view, *card)),
        ActionOption::BasicTrashResource => "Trash 1 resource".into(),
        ActionOption::BasicPurge => "Purge virus counters".into(),
        ActionOption::CardAction { ability, label } => {
            format!("{} — {}", sym_label(label), name_of(vm, view, ability.obj))
        }
    }
}

fn install_dest_label(vm: &Vm, view: &View, d: &InstallDest) -> String {
    match d {
        InstallDest::Root(s) => server_label(*s),
        InstallDest::NewRemoteRoot => "A new remote server".into(),
        InstallDest::NewRemoteProtecting => "Protecting a new remote server".into(),
        InstallDest::Protecting(s) => format!("Protecting {}", server_label(*s)),
        InstallDest::InwardFromSource => "Directly inward".into(),
        InstallDest::Rig => "Your rig".into(),
        InstallDest::HostedOn(c) => format!("Hosted on {}", name_of(vm, view, *c)),
        InstallDest::BreachedServerRoot => "The breached server".into(),
        other => format!("{other:?}"),
    }
}

/// One action-window option as a UI affordance (the same command vocabulary
/// the local engine emits, so the board's chips and sheets need no new code).
fn action_json(vm: &Vm, view: &View, a: &ActionOption) -> Value {
    let label = action_label(vm, view, a);
    match a {
        ActionOption::BasicCredit => json!({"command":"credit","label":label}),
        ActionOption::BasicDraw => json!({"command":"draw","label":label}),
        ActionOption::BasicRun { server } => {
            json!({"command":"run","server":server_key(*server),"label":label})
        }
        ActionOption::BasicRemoveTag => json!({"command":"remove-tag","label":label}),
        ActionOption::BasicPurge => json!({"command":"purge","label":label}),
        ActionOption::BasicTrashResource => json!({"command":"trash-resource","label":label}),
        ActionOption::BasicPlayOperation { card } => {
            json!({"command":"play","cid":card.0,"label":label})
        }
        ActionOption::BasicInstall { card } => {
            let side = vm.st.objects.get(card).map(|o| o.printed.side).unwrap_or(Side::Runner);
            match side {
                // 8.5.16b: the destination is declared inside the procedure,
                // so the affordance names no server.
                Side::Corp => {
                    json!({"command":"corp-install","cid":card.0,"server":"New remote","label":"Install…"})
                }
                Side::Runner => json!({"command":"runner-install","cid":card.0,"label":label}),
            }
        }
        ActionOption::BasicAdvance { card } => {
            json!({"command":"advance","cid":card.0,"label":label})
        }
        ActionOption::CardAction { ability, .. } => {
            json!({"command":"ability","cid":ability.obj.0,"ability":ability.index,"label":label})
        }
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The shim: View → the jnet-shaped state the board renderer eats
// ───────────────────────────────────────────────────────────────────────────

async fn push_state(g: &CrGame, ws: &mut WebSocket, viewer: Side) {
    // A seat's affordances are its own: the action window belongs to whoever
    // was asked, and the other seat is given nothing to click.
    let actions = g
        .pending
        .as_ref()
        .filter(|p| p.side == viewer)
        .map(|p| p.actions.clone())
        .unwrap_or_default();
    let msg = json!({
        "type": "state",
        "engine": "cr",
        "state": state_json(g, viewer),
        "actions": actions,
    });
    let _ = ws.send(Message::Text(msg.to_string().into())).await;
}

/// The whole client-bound payload, derived from `view_of(viewer)` and nothing
/// else (SYS-S-1). Called once per seat, with that seat's own viewpoint —
/// there is no "the state" here, only two states.
pub fn state_json(g: &CrGame, viewer: Side) -> Value {
    let vm = &g.vm;
    let view = vm.view_of(viewer);
    let opp = g.seat(viewer.other());
    let mut root = Map::new();
    root.insert("gameid".into(), json!("cr"));
    root.insert("turn".into(), json!(vm.st.turn_seq));
    root.insert("active-player".into(), json!(side_key(vm.st.turn_side)));
    root.insert("turn-state".into(), json!("acting"));
    // Whose decision it is, continuously — the active player is not the same
    // question (the Corp answers paid windows all through the Runner's turn),
    // and a board that only says "waiting for the Runner" in a sheet leaves
    // the commonest case of all — your own action window — saying nothing.
    root.insert(
        "priority".into(),
        match (g.over(), g.asked) {
            (false, Some(s)) => json!(side_key(s)),
            _ => Value::Null,
        },
    );
    root.insert("run".into(), run_json(vm));
    // CR 7.1.2: the cards the Runner has accessed, as cards. The Corp's copy
    // is deliberately empty — they are looking at their own board already,
    // and an R&D access is theirs to see no more than the deck was (4.2.2).
    if viewer == Side::Runner {
        root.insert("accessed".into(), Value::Array(g.accesses.clone()));
    }
    root.insert("corp".into(), corp_json(g, &view, viewer));
    root.insert("runner".into(), runner_json(g, &view, viewer));
    root.insert("log".into(), Value::Array(g.log[six(viewer)].clone()));
    // Who is across the table, and whether they are still there.
    root.insert("opponent".into(), json!(opp.name));
    root.insert("opponent-bot".into(), json!(opp.bot));
    root.insert("opponent-connected".into(), json!(opp.bot || opp.connected));
    let (winner, reason) = outcome(g);
    if g.over() {
        root.insert("winner".into(), json!(winner));
        root.insert("reason".into(), json!(reason));
    } else {
        root.insert("winner".into(), Value::Null);
        root.insert("reason".into(), Value::Null);
    }
    Value::Object(root)
}

fn run_json(vm: &Vm) -> Value {
    let Some(rc) = vm.run_ctx() else { return Value::Null };
    let positions = vm.positions_at(rc.server);
    let idx = rc.position.and_then(|p| positions.iter().position(|q| q.id == p));
    json!({
        "server": [server_key(rc.server)],
        "position": idx.map(|i| i + 1),
        "phase": run_phase(vm),
        "successful": rc.declared_successful,
        "run-credits": 0,
    })
}

/// The innermost RUN structure frame's §11 step, as the phase name the board
/// already knows (jnet's run phases).
///
/// The step table is FLAT — one row per leaf step — and each row's `number`
/// carries §6's phase as its first character ("4c" is the Movement Phase's
/// jack-out choice), so the phase is read off the number rather than kept in
/// a second place. An encounter is its own frame over the same table (9.2.2b),
/// which is why the innermost run-table frame, not the run frame, decides.
fn run_phase(vm: &Vm) -> &'static str {
    let mut number = None;
    for f in vm.frames.iter().rev() {
        if let Frame::Structure(sf) = f {
            if matches!(sf.kind, StructKind::Run | StructKind::Encounter) {
                number = vm
                    .tables
                    .iter()
                    .find(|t| t.kind == sf.kind)
                    .and_then(|t| t.steps.get(sf.cursor))
                    .map(|s| s.number.clone());
                break;
            }
        }
    }
    match number.as_deref().and_then(|n| n.chars().next()) {
        Some('1') => "initiation",
        Some('2') => "approach-ice",
        Some('3') => "encounter-ice",
        Some('4') => "movement",
        Some('5') => "success",
        Some('6') => "run-ends",
        _ => "initiation",
    }
}

/// Is this card ALREADY ON `viewer`'S SCREEN, readable, where it lies?
///
/// THE ONE NOTION. Both "the sheet may keep quiet and let the board answer"
/// questions read off this and nothing else, so they cannot disagree: a
/// prompt that hides its cards because the board has them, and a board that
/// never draws them, is a question with no answer on screen anywhere.
///
/// Two halves, and both are required:
///
/// * The zone is one the board DRAWS as cards — the servers, the ice, the
///   rig, the play-area rail, the viewer's own hand. A discard pile and a
///   deck are counts you tap to open, and the score areas are an AP chip
///   that opens a list, so a card in any of those is nowhere an outline
///   could land. (The score area used to count as drawn, and did not, so a
///   choice among scored agendas showed nothing and lit nothing.)
/// * The viewer may SEE it (§10.2). An unrezzed asset is drawn as a facedown
///   rectangle: repeating it in the sheet is not a repetition at all, it is
///   the only place its face appears. This is why the accessed card's reader
///   survives — 7.1.2 hands the Runner a face the board would not show.
fn on_screen(vm: &Vm, view: &View, viewer: Side, id: ObjectId) -> bool {
    let Some(o) = vm.st.objects.get(&id) else { return false };
    let drawn = match o.zone {
        Zone::Root(_) | Zone::Ice(_) | Zone::Rig | Zone::PlayArea(_) => true,
        Zone::Hand(s) => s == viewer,
        _ => false,
    };
    drawn && view.sees(id)
}

fn prompt_json(g: &CrGame, view: &View, viewer: Side) -> Value {
    if g.over() {
        return Value::Null;
    }
    // The waiting variant covers both "they are thinking" and "they are gone":
    // a held game says so rather than looking stuck.
    let waiting = |msg: String| {
        json!({"msg": msg, "prompt-type": "waiting", "choices": [], "select": false})
    };
    let opp = g.seat(viewer.other());
    let p = match g.pending.as_ref() {
        Some(p) if p.side == viewer => p,
        _ => {
            return waiting(if !opp.bot && !opp.connected {
                format!("{} disconnected — the game is held.", opp.name)
            } else {
                format!("Waiting for the {}", side_name(viewer.other()))
            })
        }
    };
    // The action window is the board itself, not a sheet.
    if matches!(p.spec, DecisionSpec::TakeAction { .. }) {
        return Value::Null;
    }
    // Jacking out rides the run controls.
    if matches!(p.spec, DecisionSpec::JackOut) {
        return Value::Null;
    }
    let mut msg = p.msg.clone();
    if let Some(s) = p.select.as_ref() {
        if s.count > 1 {
            msg.push_str(&format!(" ({} of {} chosen)", g.picked.len(), s.count));
        }
    }
    // …and, for exactly the same reason `select-onboard` exists, whether the
    // board is ALREADY SHOWING every card these options live on.
    //
    // A reaction window on Daily Casts drew a large Daily Casts in the sheet
    // while Daily Casts sat two inches away in the rig, already outlined green
    // — the same card twice, the second copy over the board it was asking
    // about. Where the board has them, the board answers (THE LAW §3) and the
    // sheet keeps only the sentence and the labels. All or nothing, and the
    // same `on_screen` the select path uses, so a prompt can never hide a card
    // the board is not drawing.
    let option_cards: Vec<ObjectId> = p.choices.iter().filter_map(|(_, _, _, c)| *c).collect();
    let choices_onboard = !option_cards.is_empty()
        && option_cards.iter().all(|c| on_screen(&g.vm, view, viewer, *c));
    let mut obj = json!({
        "msg": msg,
        "prompt-type": "prompt",
        "choices": p.choices.iter().map(|(u, l, _, card)| {
            // The card the option LIVES ON, where it has one. Without this a
            // client can only render a wall of text buttons: nothing maps
            // "use this ability" back to the card it belongs to, so the board
            // cannot light up and the player has to read instead of look.
            // The CARD, not just its id — a choice about cards must be able
            // to render as cards, and the candidates are often somewhere the
            // client cannot otherwise see (the stack during a search, the
            // heap, the opponent's HQ mid-access). `card_json` applies §10.2
            // itself: a card this viewer may not see travels as a FACEDOWN
            // card, which is what it looks like on the table too.
            match card {
                Some(c) => json!({"uuid": u, "value": l, "cid": c.0,
                                  "card": card_json(&g.vm, view, *c, view.sees(*c))}),
                None => json!({"uuid": u, "value": l}),
            }
        }).collect::<Vec<_>>(),
        "select": p.select.is_some(),
        // A long list is only a SEARCH when the thing being searched is the
        // card pool (1.15.1b). A trace with 15 credits to spend also offers
        // more than a dozen options, and putting "0…15" behind a search box
        // that looks each number up in the card database is a worse prompt
        // than the buttons it replaced.
        "picker": matches!(
            p.spec,
            DecisionSpec::NameValue { of: jinteki_cr::instr::NameSpace::CardName, .. }
        ),
        "choices-onboard": choices_onboard,
    });
    // Tapping a highlighted card is one of the two ways a target announcement
    // is answered, and the other one is the prompt itself — so the prompt
    // carries the candidates AS CARDS. Half the time the candidates are
    // somewhere the board cannot show them (the stack mid-search, the heap,
    // HQ during an access), where a board highlight highlights nothing at
    // all; and where they ARE on the board, both paths are drawn from this
    // one list, so they cannot disagree.
    if let (Some(s), Some(m)) = (p.select.as_ref(), obj.as_object_mut()) {
        // …and it says whether the board is showing them, so the client can
        // let the board answer where the board CAN answer (THE LAW §3) and
        // only put the cards in the sheet when it cannot. All or nothing: a
        // set split between the sheet and the board cannot be compared.
        m.insert(
            "select-onboard".into(),
            json!(!s.candidates.is_empty()
                && s.candidates.iter().all(|c| on_screen(&g.vm, view, viewer, *c))),
        );
        m.insert(
            "select-cards".into(),
            Value::Array(
                s.candidates
                    .iter()
                    .map(|c| card_json(&g.vm, view, *c, view.sees(*c)))
                    .collect(),
            ),
        );
        m.insert("select-count".into(), json!(s.count));
        m.insert("select-min".into(), json!(s.min));
        m.insert("select-up-to".into(), json!(s.up_to));
        m.insert(
            "select-picked".into(),
            Value::Array(g.picked.iter().map(|c| json!(c.0)).collect()),
        );
        // Whether what the player already holds is an answer they may send.
        // Nothing else in the prompt says so, and a player who has taken the
        // one target they wanted out of two must be able to finish, as must a
        // player whose announcement 1.15.2b already clamped to nothing at all
        // (THE LAW §6: an empty answer is stated, never implied — and it has
        // to be GIVABLE, or stating it just names the dead end).
        m.insert("select-done".into(), json!(s.complete_at(g.picked.len())));
        m.insert(
            "select-kind".into(),
            json!(match s.kind {
                SelectKind::Targets => "targets",
                SelectKind::Discard => "discard",
                SelectKind::Candidate => "candidate",
            }),
        );
        // Staged, complete, and waiting to be confirmed — the second phase of
        // an irreversible choice.
        m.insert(
            "select-confirm".into(),
            json!(s.kind == SelectKind::Discard && g.picked.len() as u32 == s.count),
        );
    }
    // CR 8.3.3: the cards being arranged, in their current order, as CARDS —
    // the player is choosing an order and cannot do that from titles alone.
    if let (Some(ids), Some(m)) = (p.arrange.as_ref(), obj.as_object_mut()) {
        m.insert(
            "arrange".into(),
            Value::Array(ids.iter().map(|c| card_json(&g.vm, view, *c, true)).collect()),
        );
    }
    if let (Some(f), Some(m)) = (p.focus.as_ref(), obj.as_object_mut()) {
        m.insert("card".into(), card_json(&g.vm, view, f.card, true));
        m.insert("focus".into(), json!(f.kind));
        if let Some(tc) = f.trash_cost {
            m.insert("trash-cost".into(), json!(tc));
        }
    }
    obj
}

fn corp_json(g: &CrGame, view: &View, viewer: Side) -> Value {
    let vm = &g.vm;
    let side = Side::Corp;
    let own = viewer == side;
    let mut m = Map::new();
    m.insert(
        "user".into(),
        json!({"username": if own { "you".to_string() } else { g.seat(side).name.clone() }}),
    );
    m.insert("identity".into(), identity_json(vm, view, side));
    m.insert("credit".into(), json!(vm.st.corp.credits));
    m.insert("click".into(), json!(vm.st.corp.clicks));
    m.insert("agenda-point".into(), json!(vm.score(side)));
    m.insert("bad-publicity".into(), json!({"base": vm.st.corp.bad_publicity}));
    // CR 10.2.2a: HQ is hidden information to the Runner — no ids, no cards.
    m.insert("hand".into(), hand_json(vm, view, side, own));
    m.insert("hand-count".into(), json!(view.count_in(Zone::Hand(side))));
    m.insert("deck".into(), json!([]));
    m.insert("deck-count".into(), json!(view.count_in(Zone::Deck(side))));
    m.insert("discard".into(), zone_json(vm, view, Zone::Discard(side)));
    m.insert("scored".into(), zone_json(vm, view, Zone::ScoreArea(side)));
    // CR 8.6.7g / 3.7.1: the play area is not a holding pen. A card being
    // played SITS there while it resolves, and an active current STAYS there
    // until another current replaces it — both are active, both are open
    // information, and neither was ever sent, so a run event mid-resolution
    // and every current were invisible. The identity lives here too and is
    // sent separately, so it is excluded.
    m.insert("play-area".into(), play_area_json(vm, view, side));
    let mut servers = Map::new();
    let mut ids: Vec<ServerId> = vec![ServerId::Hq, ServerId::Rnd, ServerId::Archives];
    ids.extend(vm.remote_servers());
    for s in ids {
        servers.insert(
            server_key(s),
            json!({
                "content": zone_json(vm, view, Zone::Root(s)),
                "ices": zone_json(vm, view, Zone::Ice(s)),
            }),
        );
    }
    m.insert("servers".into(), Value::Object(servers));
    m.insert(
        "prompt-state".into(),
        if own { prompt_json(g, view, viewer) } else { Value::Null },
    );
    Value::Object(m)
}

fn runner_json(g: &CrGame, view: &View, viewer: Side) -> Value {
    let vm = &g.vm;
    let side = Side::Runner;
    let own = viewer == side;
    let mut m = Map::new();
    m.insert(
        "user".into(),
        json!({"username": if own { "you".to_string() } else { g.seat(side).name.clone() }}),
    );
    m.insert("identity".into(), identity_json(vm, view, side));
    m.insert("credit".into(), json!(vm.st.runner.credits));
    m.insert("click".into(), json!(vm.st.runner.clicks));
    m.insert("agenda-point".into(), json!(vm.score(side)));
    m.insert(
        "tag".into(),
        json!({"base": vm.st.runner.tags, "total": vm.st.runner.tags}),
    );
    let limit = vm.memory_limit();
    let used: i32 = vm
        .cards_in_zone(Zone::Rig)
        .iter()
        .filter_map(|c| vm.st.objects.get(c))
        .filter_map(|o| o.printed.memory_cost)
        .sum::<u32>() as i32;
    m.insert(
        "memory".into(),
        json!({"base": 4, "limit": limit, "used": used, "available": limit - used}),
    );
    m.insert(
        "hand-size".into(),
        json!({"base": 5, "total": vm.max_hand_size(side)}),
    );
    m.insert("hand".into(), hand_json(vm, view, side, own));
    m.insert("hand-count".into(), json!(view.count_in(Zone::Hand(side))));
    m.insert("deck".into(), json!([]));
    m.insert("deck-count".into(), json!(view.count_in(Zone::Deck(side))));
    m.insert("discard".into(), zone_json(vm, view, Zone::Discard(side)));
    m.insert("scored".into(), zone_json(vm, view, Zone::ScoreArea(side)));
    // CR 8.6.7g / 3.7.1: the play area is not a holding pen. A card being
    // played SITS there while it resolves, and an active current STAYS there
    // until another current replaces it — both are active, both are open
    // information, and neither was ever sent, so a run event mid-resolution
    // and every current were invisible. The identity lives here too and is
    // sent separately, so it is excluded.
    m.insert("play-area".into(), play_area_json(vm, view, side));
    // The rig is one zone in the CR; the board draws it in three rows.
    let rig = vm.cards_in_zone(Zone::Rig);
    let seen = view.in_zone(Zone::Rig);
    let mut programs = Vec::new();
    let mut hardware = Vec::new();
    let mut resources = Vec::new();
    for (i, id) in rig.iter().enumerate() {
        let visible = seen.get(i).map(|c| c.is_seen()).unwrap_or(false);
        let j = card_json(vm, view, *id, visible);
        match vm.st.objects.get(id).map(|o| o.printed.card_type) {
            Some(CardType::Program) => programs.push(j),
            Some(CardType::Hardware) => hardware.push(j),
            _ => resources.push(j),
        }
    }
    m.insert(
        "rig".into(),
        json!({"program": programs, "hardware": hardware, "resource": resources}),
    );
    m.insert(
        "prompt-state".into(),
        if own { prompt_json(g, view, viewer) } else { Value::Null },
    );
    Value::Object(m)
}

/// CR 10.2.3a: the number of cards in a hand is open; their identities are
/// not. The opponent's grip therefore travels as a COUNT and nothing else —
/// not even a card id, which would let a client track a card out of it.
fn hand_json(vm: &Vm, view: &View, side: Side, own: bool) -> Value {
    if !own {
        return json!([]);
    }
    zone_json(vm, view, Zone::Hand(side))
}

/// One public zone. The view decides identity; the id travels because CR
/// 4.6.2/4.6.3 make presence and location open information for the play area,
/// and an unrezzed card is an object both players can point at.
fn zone_json(vm: &Vm, view: &View, z: Zone) -> Value {
    let ids = vm.cards_in_zone(z);
    let seen = view.in_zone(z);
    Value::Array(
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                let visible = seen.get(i).map(|c| c.is_seen()).unwrap_or(false);
                card_json(vm, view, *id, visible)
            })
            .collect(),
    )
}

/// CR 4.6.7: the play area, minus the identity — a card resolving right now
/// (8.6.7g) and any active current (3.7.1b / 3.5.1b).
fn play_area_json(vm: &Vm, view: &View, side: Side) -> Value {
    let z = Zone::PlayArea(side);
    let ids = vm.cards_in_zone(z);
    let seen = view.in_zone(z);
    Value::Array(
        ids.iter()
            .enumerate()
            .filter(|(_, id)| {
                vm.st.objects.get(id).map(|o| o.printed.card_type) != Some(CardType::Identity)
            })
            .map(|(i, id)| {
                let visible = seen.get(i).map(|c| c.is_seen()).unwrap_or(false);
                card_json(vm, view, *id, visible)
            })
            .collect(),
    )
}

fn identity_json(vm: &Vm, view: &View, side: Side) -> Value {
    let z = Zone::PlayArea(side);
    let ids = vm.cards_in_zone(z);
    let seen = view.in_zone(z);
    for (i, id) in ids.iter().enumerate() {
        if vm.st.objects.get(id).map(|o| o.printed.card_type) == Some(CardType::Identity) {
            let visible = seen.get(i).map(|c| c.is_seen()).unwrap_or(false);
            return card_json(vm, view, *id, visible);
        }
    }
    json!({"cid": 0, "title": side_name(side), "type": "Identity"})
}

fn card_json(vm: &Vm, view: &View, id: ObjectId, visible: bool) -> Value {
    let Some(o) = vm.st.objects.get(&id) else {
        return json!({"cid": id.0, "facedown": true});
    };
    let adv = o.counters.get(&CounterKind::Advancement).copied().unwrap_or(0);
    if !visible {
        // jnet's private-card shape: presence, and nothing about identity.
        return json!({"cid": id.0, "facedown": true, "rezzed": false, "advance-counter": adv});
    }
    let p = &o.printed;
    let mut m = Map::new();
    m.insert("cid".into(), json!(id.0));
    m.insert("title".into(), json!(p.name));
    if let Some(c) = crate::carddata::by_title(p.name) {
        m.insert("code".into(), json!(c.code));
    }
    if let Some(t) = oracle_text(p.name) {
        m.insert("text".into(), json!(t));
    }
    m.insert("type".into(), json!(type_name(p.card_type)));
    m.insert("subtypes".into(), json!(p.subtypes));
    m.insert("cost".into(), json!(p.cost));
    m.insert("rezzed".into(), json!(o.faceup));
    m.insert("facedown".into(), json!(false));
    m.insert("advance-counter".into(), json!(adv));
    let mut counters = Map::new();
    for (k, n) in &o.counters {
        if *n == 0 || *k == CounterKind::Advancement {
            continue;
        }
        let key = match k {
            CounterKind::Credit => "credit",
            CounterKind::Power => "power",
            CounterKind::Virus => "virus",
            CounterKind::Agenda => "agenda",
            CounterKind::BadPublicity => "bad-publicity",
            CounterKind::Advancement => continue,
        };
        counters.insert(key.into(), json!(n));
    }
    if !counters.is_empty() {
        m.insert("counter".into(), Value::Object(counters));
    }
    if let Some(s) = vm.effective_strength(id) {
        m.insert("strength".into(), json!(s));
    }
    if p.card_type == CardType::Ice && o.faceup {
        let broken = vm
            .st
            .encounter
            .as_ref()
            .filter(|e| e.ice == id)
            .map(|e| e.broken.clone())
            .unwrap_or_default();
        let subs: Vec<Value> = vm
            .current_subs(id)
            .into_iter()
            .map(|(k, d)| json!({"label": d.label, "broken": broken.contains(&k)}))
            .collect();
        m.insert("subroutines".into(), Value::Array(subs));
    }
    if let Some(tc) = p.trash_cost {
        m.insert("trash-cost".into(), json!(tc));
    }
    if let Some(ap) = p.agenda_points {
        m.insert("agendapoints".into(), json!(ap));
        m.insert(
            "advancementcost".into(),
            json!(p.advancement_requirement.unwrap_or(0)),
        );
    }
    let abilities: Vec<Value> = p.abilities.iter().map(|a| json!({"label": a.label})).collect();
    if !abilities.is_empty() {
        m.insert("abilities".into(), Value::Array(abilities));
    }
    let _ = view; // the visibility decision was already made from it
    Value::Object(m)
}

// Keep the unused-import lint honest about the one type we only name in
// signatures above.
#[allow(dead_code)]
fn _ability_ref_is_used(a: AbilityRef) -> usize {
    a.index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_lists_match_the_printed_decks() {
        let a: u32 = ANDROMEDA_LIST.iter().map(|(_, n)| n).sum();
        let g: u32 = GAUNTLET_LIST.iter().map(|(_, n)| n).sum();
        assert_eq!(a, 46, "Andromeda is 45 cards + identity");
        assert_eq!(g, 50, "Gauntlet is 49 cards + identity");
    }

    #[test]
    fn readiness_covers_every_card_of_both_decks() {
        let r = readiness();
        assert!(r.problems.is_empty(), "deck list vs card layer: {:?}", r.problems);
        let distinct: usize = r.decks.iter().map(|d| d.distinct).sum();
        assert_eq!(r.total, distinct);
        assert_eq!(r.complete + r.missing.len(), r.total);
        for m in &r.missing {
            assert!(!m.unimplemented.is_empty(), "{} is incomplete for no stated reason", m.title);
        }
    }

    /// SYS-D-12's gate, from the other side: with every printed sentence of
    /// both decks expressed, the eternal setup is a game and not a refusal.
    #[test]
    fn the_two_eternal_decks_are_playable() {
        let r = readiness();
        assert!(r.ready, "{}/{} complete; missing {:?}", r.complete, r.total, r.missing);
        let setup = eternal_setup(1).expect("a complete pair of decks is a game");
        assert!(setup.corp_identity.is_some(), "Nebula Talent Management sits down");
        assert!(setup.runner_identity.is_some(), "Andromeda sits down");
        assert_eq!(setup.corp_deck.len(), 49, "the printed Gauntlet list, by copies");
        assert_eq!(setup.runner_deck.len(), 45, "the printed Andromeda list, by copies");
    }

    #[test]
    fn server_keys_round_trip() {
        for s in [ServerId::Hq, ServerId::Rnd, ServerId::Archives, ServerId::Remote(3)] {
            assert_eq!(server_from_key(&server_key(s)), Some(s));
        }
    }

    /// A game stepped far enough that the opening hands exist, with every
    /// decision answered by the engine's own neutral policy — the fixture for
    /// asking "what would this prompt look like".
    fn dealt_game() -> CrGame {
        let setup = eternal_setup(7).expect("a complete pair of decks is a game");
        let mut g = new_game(setup, [SeatState::bot(), SeatState::human("tester", None)], 0);
        for _ in 0..20_000 {
            if g.vm.cards_in_zone(Zone::Hand(Side::Runner)).len() >= 2 {
                break;
            }
            match g.vm.step() {
                Yield::Decision(_, spec) => {
                    let a = default_answer(&spec);
                    g.vm.answer(a);
                }
                Yield::GameOver(_) => break,
                Yield::Progressed => {}
            }
        }
        assert!(
            g.vm.cards_in_zone(Zone::Hand(Side::Runner)).len() >= 2,
            "the opening hand is dealt"
        );
        g
    }

    /// The reported bug, in one assertion. "Choose 1 card (CR 1.15.2)" over
    /// two copies of Boomerang is two buttons both reading "Boomerang" and no
    /// way to tell them apart, unless the CARD travels with the choice —
    /// which it did not, because only window options and actions were mapped
    /// back to a card and a target announcement is answered with neither.
    #[test]
    fn a_target_announcement_offers_cards_not_titles() {
        let mut g = dealt_game();
        let hand = g.vm.cards_in_zone(Zone::Hand(Side::Runner));
        let want: Vec<ObjectId> = hand[..2].to_vec();
        let spec = DecisionSpec::ChooseTargets {
            candidates: want.clone(),
            count: 1,
            up_to: false,
            min: 1,
            distinct_names: false,
        };
        g.pending = Some(present(&g.vm, Side::Runner, &spec));
        let view = g.vm.view_of(Side::Runner);
        let out = prompt_json(&g, &view, Side::Runner);

        let choices = out["choices"].as_array().expect("choices").clone();
        assert_eq!(choices.len(), 2, "one per candidate: {out:#?}");
        for (ch, id) in choices.iter().zip(&want) {
            assert_eq!(ch["cid"].as_u64(), Some(id.0 as u64), "the choice names its card");
            assert!(
                ch["card"]["title"].is_string(),
                "…and carries the card itself, or the client can only print the label: {ch:#?}"
            );
        }
        // The other half of the same question: the board lights the same
        // cards, from the same list, so the two can never disagree.
        let cards = out["select-cards"].as_array().expect("select-cards");
        let lit: Vec<u64> = cards.iter().filter_map(|c| c["cid"].as_u64()).collect();
        let acts: Vec<u64> = g
            .pending
            .as_ref()
            .unwrap()
            .actions
            .iter()
            .filter(|a| a["command"] == json!("select"))
            .filter_map(|a| a["cid"].as_u64())
            .collect();
        let ids: Vec<u64> = want.iter().map(|c| c.0 as u64).collect();
        assert_eq!(lit, ids, "the prompt shows every candidate");
        assert_eq!(acts, ids, "and the board offers exactly those to tap");
    }

    /// CR 1.15.2e: "choose UP TO n". Picking is by tapping, so the per-card
    /// buttons go — but the answer "none of them" is a real answer and has to
    /// survive (THE LAW §6), and so does the way to stop after one.
    #[test]
    fn up_to_two_can_still_be_answered_with_none_or_stopped_early() {
        let mut g = dealt_game();
        let hand = g.vm.cards_in_zone(Zone::Hand(Side::Runner));
        let want: Vec<ObjectId> = hand[..2].to_vec();
        let spec = DecisionSpec::ChooseTargets {
            candidates: want.clone(),
            count: 2,
            up_to: true,
            min: 0,
            distinct_names: false,
        };
        g.pending = Some(present(&g.vm, Side::Runner, &spec));
        let view = g.vm.view_of(Side::Runner);
        let out = prompt_json(&g, &view, Side::Runner);
        let choices = out["choices"].as_array().expect("choices");
        assert_eq!(choices.len(), 1, "the None button, and nothing else: {out:#?}");
        assert_eq!(choices[0]["value"], json!("None"));
        assert_eq!(out["select-done"], json!(true), "0 picks already meets a floor of 0");

        // The same with a floor of 1 (10.12.3a): stopping is offered only
        // once the floor is met, and one pick of two is a whole answer —
        // which the count-driven path alone cannot express at all.
        let spec = DecisionSpec::ChooseTargets {
            candidates: want.clone(),
            count: 2,
            up_to: true,
            min: 1,
            distinct_names: false,
        };
        g.pending = Some(present(&g.vm, Side::Runner, &spec));
        let view = g.vm.view_of(Side::Runner);
        assert_eq!(
            prompt_json(&g, &view, Side::Runner)["select-done"],
            json!(false),
            "nothing chosen yet, so there is nothing to be done with"
        );
        assert!(
            apply_command(&mut g, Side::Runner, &json!({"command":"select-done","args":{}}))
                .is_err(),
            "and the command refuses too, not just the button"
        );
        let cmd = json!({"command":"select","args":{"cid": want[0].0}});
        assert_eq!(apply_command(&mut g, Side::Runner, &cmd), Ok(false), "one of two picked");
        assert_eq!(g.picked, vec![want[0]]);
        let view = g.vm.view_of(Side::Runner);
        let out = prompt_json(&g, &view, Side::Runner);
        assert_eq!(out["select-done"], json!(true), "the floor is met: stopping is legal");
        assert_eq!(out["select-picked"], json!([want[0].0]), "and the sheet says which");
    }

    /// The dead end, in one assertion. CR 1.15.2b clamps an announcement to
    /// the eligible targets available, so a "choose 1 installed piece of ice"
    /// with no ice on the table asks for ZERO cards: nothing to draw, nothing
    /// to tap, no "None" (that button is offered only for an explicit "up
    /// to"), and a `select-done` that tested `up_to` and so said no. The
    /// sheet said "No card qualifies — there is nothing to choose" and then
    /// gave the player nothing to click, which stopped the game.
    ///
    /// The kernel no longer asks this question at all. The server must still
    /// be able to answer one, because a prompt that cannot be answered is not
    /// a bad prompt, it is a hung game — and the button and the command have
    /// to agree, or the button is a lie.
    #[test]
    fn an_announcement_of_nothing_can_still_be_answered() {
        let mut g = dealt_game();
        let spec = DecisionSpec::ChooseTargets {
            candidates: Vec::new(),
            count: 0,
            up_to: false,
            min: 0,
            distinct_names: false,
        };
        g.pending = Some(present(&g.vm, Side::Runner, &spec));
        let view = g.vm.view_of(Side::Runner);
        let out = prompt_json(&g, &view, Side::Runner);
        assert_eq!(
            out["select-cards"].as_array().map(|a| a.len()),
            Some(0),
            "there is nothing to show: {out:#?}"
        );
        assert!(
            out["choices"].as_array().map(|a| a.is_empty()).unwrap_or(false),
            "…and no per-candidate button either, which is what made it a dead end: {out:#?}"
        );
        assert_eq!(
            out["select-done"],
            json!(true),
            "so the empty set has to BE the answer, and the sheet has to say so: {out:#?}"
        );
        // The button the client draws and the command the server honours read
        // the SAME predicate, so asserting it asserts both — a button whose
        // command then refuses it is worse than no button at all. (Committing
        // for real would need a VM actually waiting on this decision, and the
        // kernel no longer produces one, which is the whole point.)
        let sel = g.pending.as_ref().unwrap().select.as_ref().expect("a selection");
        assert!(
            sel.complete_at(0),
            "…and `select-done` is accepted with nothing picked, not rejected \
             as 'you have not chosen enough cards yet'"
        );
    }

    /// A long list is a search box only when what it lists is the CARD POOL.
    /// A trace with fifteen credits to spend is also more than a dozen
    /// options, and it is not a search for anything.
    #[test]
    fn only_a_card_name_prompt_is_a_search() {
        use jinteki_cr::instr::NameSpace;
        let mut g = dealt_game();
        let cases = [
            (DecisionSpec::NameValue { of: NameSpace::CardName, excluding: None }, true),
            (DecisionSpec::NameValue { of: NameSpace::Number, excluding: None }, false),
            (DecisionSpec::TraceSpend { max: 15, strength_so_far: 0, corp_side: false }, false),
            (DecisionSpec::DeclareX { max: 20 }, false),
        ];
        for (spec, want) in cases {
            g.pending = Some(present(&g.vm, Side::Runner, &spec));
            let view = g.vm.view_of(Side::Runner);
            assert_eq!(
                prompt_json(&g, &view, Side::Runner)["picker"],
                json!(want),
                "{spec:?}"
            );
        }
    }

    /// THE LAW §3, the half that lets a prompt keep quiet. A reaction window
    /// on a card in the rig drew that card AGAIN, large, in a sheet on top of
    /// the board the card was already sitting on and already outlined green.
    /// The sheet may only fall silent where the board really is showing it —
    /// and the same `on_screen` decides for both prompt shapes, so the two
    /// cannot disagree about what "on screen" means.
    #[test]
    fn a_window_whose_options_are_already_on_the_board_does_not_redraw_them() {
        let mut g = dealt_game();
        // The rig is where an installed Runner card lives, and it is drawn.
        let hand = g.vm.cards_in_zone(Zone::Hand(Side::Runner));
        let on_board = hand[0];
        {
            let o = g.vm.st.objects.get_mut(&on_board).expect("a card");
            o.zone = Zone::Rig;
            o.faceup = true;
        }
        let spec = DecisionSpec::ReactionWindow {
            options: vec![WindowOption::TriggerPaid {
                ability: AbilityRef { obj: on_board, index: 0 },
                label: "use it",
            }],
            can_pass: true,
        };
        g.pending = Some(present(&g.vm, Side::Runner, &spec));
        let view = g.vm.view_of(Side::Runner);
        let out = prompt_json(&g, &view, Side::Runner);
        assert_eq!(
            out["choices-onboard"],
            json!(true),
            "the rig is drawn and the Runner sees it: {out:#?}"
        );
        // The card still travels — the sheet's labels and the board's outline
        // are drawn from one list, exactly as the select path does it.
        let ch = &out["choices"].as_array().expect("choices")[0];
        assert_eq!(ch["cid"].as_u64(), Some(on_board.0 as u64));

        // …and the same option on a card in the STACK is not on the board at
        // all, so the sheet must draw it or the question is unaskable.
        let deep = g.vm.cards_in_zone(Zone::Deck(Side::Runner))[0];
        let spec = DecisionSpec::ReactionWindow {
            options: vec![WindowOption::TriggerPaid {
                ability: AbilityRef { obj: deep, index: 0 },
                label: "use it",
            }],
            can_pass: true,
        };
        g.pending = Some(present(&g.vm, Side::Runner, &spec));
        let view = g.vm.view_of(Side::Runner);
        assert_eq!(
            prompt_json(&g, &view, Side::Runner)["choices-onboard"],
            json!(false),
            "nothing in the stack is drawn anywhere"
        );
    }

    /// "On screen" is one notion, and it is not "may they see it" nor "is it
    /// installed". A facedown card in a remote is DRAWN and unreadable; a
    /// scored agenda is READABLE and drawn nowhere (the score area is an AP
    /// chip that opens a list). Either mistake produces the same bug: a
    /// prompt that shows no cards over a board that lights none.
    #[test]
    fn on_screen_means_drawn_as_a_card_and_readable() {
        let g = dealt_game();
        let view = g.vm.view_of(Side::Runner);
        let mine = g.vm.cards_in_zone(Zone::Hand(Side::Runner))[0];
        assert!(on_screen(&g.vm, &view, Side::Runner, mine), "your own hand is drawn");
        let theirs = g.vm.cards_in_zone(Zone::Hand(Side::Corp))[0];
        assert!(!on_screen(&g.vm, &view, Side::Runner, theirs), "HQ is a count");
        let deep = g.vm.cards_in_zone(Zone::Deck(Side::Runner))[0];
        assert!(!on_screen(&g.vm, &view, Side::Runner, deep), "the stack is a count");

        let mut g = g;
        // Installed facedown in a remote: drawn, but as a blank rectangle.
        {
            let o = g.vm.st.objects.get_mut(&theirs).expect("a card");
            o.zone = Zone::Root(ServerId::Remote(1));
            o.faceup = false;
        }
        let view = g.vm.view_of(Side::Runner);
        assert!(
            !on_screen(&g.vm, &view, Side::Runner, theirs),
            "an unrezzed card's FACE is not on screen — the sheet is its only copy"
        );
        {
            g.vm.st.objects.get_mut(&theirs).expect("a card").faceup = true;
        }
        let view = g.vm.view_of(Side::Runner);
        assert!(on_screen(&g.vm, &view, Side::Runner, theirs), "rezzed, it is");

        // A scored agenda is open information and is drawn nowhere.
        {
            let o = g.vm.st.objects.get_mut(&theirs).expect("a card");
            o.zone = Zone::ScoreArea(Side::Corp);
            o.faceup = true;
        }
        let view = g.vm.view_of(Side::Runner);
        assert!(
            !on_screen(&g.vm, &view, Side::Runner, theirs),
            "the score area is an AP chip, not a row of cards"
        );
    }

    /// PRIORITY, as the identities show it. `pending` is only Some while a
    /// PERSON owes the decision, so a state pushed while the bot is being
    /// driven would say nobody is deciding — which is most of the frames the
    /// watching player sees.
    #[test]
    fn the_state_says_whose_decision_it_is() {
        let mut g = dealt_game();
        assert_eq!(
            state_json(&g, Side::Runner)["priority"],
            Value::Null,
            "nobody has been asked yet"
        );
        g.asked = Some(Side::Runner);
        for viewer in [Side::Corp, Side::Runner] {
            assert_eq!(
                state_json(&g, viewer)["priority"],
                json!("runner"),
                "both seats see the same clock"
            );
        }
        g.asked = Some(Side::Corp);
        assert_eq!(state_json(&g, Side::Runner)["priority"], json!("corp"));
        // An action window is a decision like any other: the commonest case
        // of all must not be the one that says nothing.
        g.pending = Some(present(&g.vm, Side::Corp, &DecisionSpec::TakeAction { options: vec![] }));
        assert_eq!(state_json(&g, Side::Corp)["priority"], json!("corp"));
        assert_eq!(
            state_json(&g, Side::Corp)["corp"]["prompt-state"],
            Value::Null,
            "…and it still drives the board rather than a sheet"
        );
        g.conceded = Some(Side::Runner);
        assert_eq!(
            state_json(&g, Side::Runner)["priority"],
            Value::Null,
            "a finished game is waiting on nobody"
        );
    }

    /// CR 7.1.2 as a CARD, not a log line. The sharp case is the one the
    /// player hit: an access that asks the Runner NOTHING. `vm.st.accessed`
    /// is cleared the moment the access ends and `drive` only pushes a state
    /// when the machine stops, so reading the live field at push time shows
    /// nothing at all — which is exactly the bug. The snapshot is taken when
    /// the entitlement is live and outlives it.
    #[test]
    fn an_accessed_card_reaches_the_runner_even_with_no_decision_attached() {
        let mut g = dealt_game();
        let card = g.vm.cards_in_zone(Zone::Deck(Side::Corp))[0];
        let title = g.vm.st.objects[&card].printed.name;
        // The access, and its end, in one batch — the shape a decisionless
        // access has by the time anybody could look at the state.
        g.vm.st.accessed = Some(card);
        g.vm.changes.log.push(jinteki_cr::change::GameChange::CardAccessed { obj: card });
        g.narrate();
        g.vm.st.accessed = None;

        assert!(g.vm.st.accessed.is_none(), "the access is over, as it would be");
        let out = state_json(&g, Side::Runner);
        let seen = out["accessed"].as_array().expect("the Runner's reveals");
        assert_eq!(seen.len(), 1, "one access, one card: {out:#?}");
        assert_eq!(seen[0]["card"]["title"], json!(title), "the CARD, not a sentence");
        assert_eq!(seen[0]["from"], json!("R&D"), "and which server it came out of");
        assert!(seen[0]["seq"].as_u64().is_some(), "so the client can dismiss it once");
        // 4.2.2: R&D is hidden from the Corp as much as from the Runner, and
        // the reveal is the Runner's entitlement in any case.
        assert!(
            state_json(&g, Side::Corp).get("accessed").is_none(),
            "the Corp is not handed the Runner's access reader"
        );

        // A deep dig does not become the whole game: the queue is bounded.
        for _ in 0..ACCESS_REVEALS * 2 {
            g.vm.changes.log.push(jinteki_cr::change::GameChange::CardAccessed { obj: card });
            g.narrate();
        }
        let out = state_json(&g, Side::Runner);
        assert_eq!(out["accessed"].as_array().expect("reveals").len(), ACCESS_REVEALS);
    }

    /// 5.5.4c is irreversible, so it is STAGED and then confirmed — MTG
    /// Arena's discard, for MTG Arena's reason. The bug this forecloses is
    /// the expensive one: a single tap on a 16px strip of a card, taken with
    /// no clicks left, that bins a card the player never meant to touch.
    ///
    /// Driven to a REAL discard decision rather than a fabricated one: the
    /// point of the change is that the commit path still commits, and a
    /// stubbed `pending` would test the flags and nothing else.
    #[test]
    fn a_discard_is_staged_and_only_a_confirmation_throws_the_cards_away() {
        let setup = eternal_setup(7).expect("a complete pair of decks is a game");
        let mut g = new_game(setup, [SeatState::bot(), SeatState::human("tester", None)], 0);
        let mut hand = Vec::new();
        for _ in 0..200_000 {
            match g.vm.step() {
                Yield::Decision(side, spec) => {
                    if let (Side::Runner, DecisionSpec::DiscardCards { count, hand: h }) =
                        (side, &spec)
                    {
                        assert!(*count >= 2, "5.5.4c with at least two to lose: {count}");
                        hand = h.clone();
                        g.pending = Some(present(&g.vm, Side::Runner, &spec));
                        break;
                    }
                    g.vm.answer(default_answer(&spec));
                }
                Yield::GameOver(_) => break,
                Yield::Progressed => {}
            }
        }
        assert!(!hand.is_empty(), "the Runner reached a discard phase");
        let count = match &g.pending.as_ref().unwrap().spec {
            DecisionSpec::DiscardCards { count, .. } => *count,
            _ => unreachable!(),
        };
        let before = g.vm.cards_in_zone(Zone::Hand(Side::Runner)).len();
        let sel = |g: &mut CrGame, c: ObjectId| {
            apply_command(g, Side::Runner, &json!({"command":"select","args":{"cid": c.0}}))
        };
        for c in hand.iter().take(count as usize) {
            assert_eq!(sel(&mut g, *c), Ok(false), "marked, not spent");
        }
        // The whole set is marked, and STILL nothing has happened.
        assert!(g.pending.is_some(), "the decision is still on the table");
        assert_eq!(
            g.vm.cards_in_zone(Zone::Hand(Side::Runner)).len(),
            before,
            "nothing has left the grip"
        );
        let view = g.vm.view_of(Side::Runner);
        let out = prompt_json(&g, &view, Side::Runner);
        assert_eq!(out["select-kind"], json!("discard"));
        assert_eq!(out["select-confirm"], json!(true), "the sheet may be confirmed");
        assert_eq!(
            out["select-picked"].as_array().map(|a| a.len()),
            Some(count as usize),
            "and says which cards are in white"
        );

        // Tapping a marked card puts it back, and the confirmation withdraws.
        assert_eq!(sel(&mut g, hand[0]), Ok(false), "unmarked");
        let view = g.vm.view_of(Side::Runner);
        assert_eq!(prompt_json(&g, &view, Side::Runner)["select-confirm"], json!(false));
        assert!(
            apply_command(&mut g, Side::Runner, &json!({"command":"select-confirm","args":{}}))
                .is_err(),
            "a half-answer cannot be confirmed"
        );
        // …and "start over" is a real way back out.
        assert_eq!(
            apply_command(&mut g, Side::Runner, &json!({"command":"select-clear","args":{}})),
            Ok(false)
        );
        assert!(g.picked.is_empty(), "nothing staged");

        for c in hand.iter().take(count as usize) {
            assert_eq!(sel(&mut g, *c), Ok(false));
        }
        assert_eq!(
            apply_command(&mut g, Side::Runner, &json!({"command":"select-confirm","args":{}})),
            Ok(true),
            "and now it happens"
        );
        assert!(g.pending.is_none(), "the decision was answered");
        g.vm.step();
        assert_eq!(
            g.vm.cards_in_zone(Zone::Hand(Side::Runner)).len(),
            before - count as usize,
            "the cards are gone, and only now"
        );
    }

    /// Every choice that is about a card carries the card. The audit that
    /// caught the bug, kept as a standing invariant: a `cid` without a `card`
    /// is a client that can only draw a button.
    #[test]
    fn a_choice_that_names_a_card_always_carries_it() {
        let mut g = dealt_game();
        let hand = g.vm.cards_in_zone(Zone::Hand(Side::Runner));
        let specs = [
            DecisionSpec::DiscardCards { count: 1, hand: hand.clone() },
            DecisionSpec::PaymentCards {
                candidates: hand[..2].to_vec(),
                count: 1,
                label: "trash",
            },
            DecisionSpec::ChooseCandidate { candidates: hand[..2].to_vec() },
            DecisionSpec::MinimalSet { sets: vec![vec![hand[0]], vec![hand[1]]] },
        ];
        for spec in specs {
            g.pending = Some(present(&g.vm, Side::Runner, &spec));
            let view = g.vm.view_of(Side::Runner);
            let out = prompt_json(&g, &view, Side::Runner);
            for ch in out["choices"].as_array().expect("choices") {
                if ch["cid"].is_null() {
                    continue;
                }
                assert!(
                    ch["card"].is_object(),
                    "{spec:?}: a choice that names a card must carry it: {ch:#?}"
                );
            }
        }
    }
}
