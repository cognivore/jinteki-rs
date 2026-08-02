//! Game state: zones, card instances, prompts, run/breach state, logging.
//!
//! Ordering conventions (mirroring the reference):
//! - `deck[0]` is the TOP of the deck.
//! - `Server.ices[0]` is the INNERMOST ice; installing appends to the outside.
//! - `run.position == ices.len()` at initiation; the approached ice is
//!   `ices[position - 1]`; position 0 means approaching the server itself.

use crate::carddb;
use crate::types::*;
use rand::seq::SliceRandom;
use rand::Rng as _;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone)]
pub struct CardInstance {
    pub cid: Cid,
    pub def: usize,
    pub zone: Zone,
    pub rezzed: bool,
    /// Face-up / seen (agendas in score areas, cards in archives, accessed cards).
    pub faceup: bool,
    pub advancement: u32,
    /// Hosted credit counters (Armitage, Regolith).
    pub credits: u32,
    /// Strength pumps active this encounter / this run (breakers).
    pub pump_encounter: i32,
    pub pump_run: i32,
    /// Broken flags per subroutine, live during an encounter.
    pub broken: Vec<bool>,
}

impl CardInstance {
    pub fn def(&self) -> &'static CardDef {
        carddb::def_at(self.def)
    }
    pub fn title(&self) -> &'static str {
        self.def().title
    }
    pub fn is_agenda(&self) -> bool {
        self.def().kind == CardType::Agenda
    }
    pub fn is_ice(&self) -> bool {
        self.def().kind == CardType::Ice
    }
    pub fn is_program(&self) -> bool {
        self.def().kind == CardType::Program
    }
}

#[derive(Debug, Clone, Default)]
pub struct Server {
    pub content: Vec<Cid>,
    pub ices: Vec<Cid>,
}

#[derive(Debug, Clone, Default)]
pub struct Rig {
    pub programs: Vec<Cid>,
    pub hardware: Vec<Cid>,
    pub resources: Vec<Cid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunPhase {
    ApproachIce,
    EncounterIce,
    Movement,
    ApproachServer,
    Success,
}
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct RunState {
    pub server: ServerId,
    pub position: usize,
    pub phase: RunPhase,
    pub successful: bool,
    /// Credits gained if the run ends successful (Dirty Laundry).
    pub success_credits: u32,
    /// Extra accesses granted for this run's breach (Legwork, Maker's Eye).
    pub access_bonus: u32,
    /// Bad-publicity pseudo-credits available this run (spent before real credits).
    pub run_credits: u32,
    /// Cid of the run event that made this run, if any.
    pub source: Option<Cid>,
}

#[derive(Debug, Clone)]
pub struct BreachState {
    pub server: ServerId,
    /// Cards remaining to access, front = next.
    pub queue: Vec<Cid>,
}

/// What answering the current prompt means. Defunctionalized continuations:
/// every suspended engine state is DATA (DESIGN.md TBC-4 resolved this way).
#[derive(Debug, Clone, PartialEq)]
pub enum PromptContext {
    Mulligan,
    /// Corp decides whether to rez the approached ice (pays cost on Yes).
    RezApproached { ice: Cid },
    /// Accessed agenda: mandatory steal (choices: ["Steal"]).
    AccessSteal { cid: Cid },
    /// Accessed card with a payable trash cost: ["Pay N [Credits] to trash", "No action"].
    AccessTrashOrNo { cid: Cid, trash_cost: u32 },
    /// Accessed card, nothing to do: ["No action"].
    AccessNoAction { cid: Cid },
    /// Dirty Laundry: choose which server to run.
    ChooseRunServer { success_credits: u32, access_bonus: u32 },
    /// Priority Requisition: select an unrezzed installed ice to rez for free.
    PriorityReqRez,
    /// Superconducting Hub: "Draw 2 cards?" Yes/No.
    HubDraw,
    /// Rototurret subroutine: corp selects an installed program to trash;
    /// afterwards subroutine firing resumes at `resume_index`.
    RototurretTrash { ice: Cid, resume_index: usize },
    /// Discard down to max hand size at end of turn: select a card from hand.
    DiscardDown,
    /// Choose which subroutine to break (breaker ability): one choice per
    /// breakable unbroken subroutine plus "Done".
    BreakChooseSub { breaker: Cid, ice: Cid },
}

#[derive(Debug, Clone)]
pub struct PromptChoice {
    pub uuid: String,
    pub label: String,
}

/// What a select-prompt will accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectKind {
    UnrezzedInstalledIce,
    InstalledRunnerProgram,
    OwnHandCard(Side),
}

#[derive(Debug, Clone)]
pub struct Prompt {
    pub side: Side,
    pub msg: String,
    pub choices: Vec<PromptChoice>,
    pub select: Option<SelectKind>,
    pub context: PromptContext,
    /// jnet prompt-type string for the client ("mulligan", "select", "other").
    pub prompt_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// Mulligan prompts outstanding.
    Setup,
    /// Waiting for `active` to send start-turn.
    AwaitingStart,
    /// Active player is taking actions.
    Acting,
    GameOver,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub user: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub rng: ChaCha8Rng,
    pub seed: u64,
    pub cards: Vec<CardInstance>,
    pub identity: [Cid; 2],
    pub deck: [Vec<Cid>; 2],
    pub hand: [Vec<Cid>; 2],
    pub discard: [Vec<Cid>; 2],
    pub scored: [Vec<Cid>; 2],
    pub servers: Vec<(ServerId, Server)>,
    pub next_remote: u32,
    pub rig: Rig,
    pub credits: [i64; 2],
    pub clicks: [i32; 2],
    pub turn: u32,
    pub active: Side,
    pub turn_state: TurnState,
    pub keep: [Option<bool>; 2],
    pub mulliganed: [bool; 2],
    pub bad_pub: u32,
    pub tags: u32,
    pub run: Option<RunState>,
    pub breach: Option<BreachState>,
    pub prompts: Vec<Prompt>,
    pub log: Vec<LogEntry>,
    pub winner: Option<Side>,
    pub reason: Option<String>,
    pub hq_success_this_turn: bool,
    uuid_counter: u64,
}

fn idx(side: Side) -> usize {
    match side {
        Side::Corp => 0,
        Side::Runner => 1,
    }
}

impl GameState {
    pub fn new(seed: u64) -> GameState {
        GameState::new_with_decks(
            seed,
            carddb::CORP_ID,
            &carddb::corp_deck(),
            carddb::RUNNER_ID,
            &carddb::runner_deck(),
        )
    }

    pub fn new_with_decks(
        seed: u64,
        corp_id: &str,
        corp_deck: &[&str],
        runner_id: &str,
        runner_deck: &[&str],
    ) -> GameState {
        let mut st = GameState {
            rng: ChaCha8Rng::seed_from_u64(seed),
            seed,
            cards: Vec::new(),
            identity: [0, 0],
            deck: [Vec::new(), Vec::new()],
            hand: [Vec::new(), Vec::new()],
            discard: [Vec::new(), Vec::new()],
            scored: [Vec::new(), Vec::new()],
            servers: vec![
                (ServerId::Hq, Server::default()),
                (ServerId::Rd, Server::default()),
                (ServerId::Archives, Server::default()),
            ],
            next_remote: 1,
            rig: Rig::default(),
            credits: [5, 5],
            clicks: [0, 0],
            turn: 0,
            active: Side::Corp,
            turn_state: TurnState::Setup,
            keep: [None, None],
            mulliganed: [false, false],
            bad_pub: 0,
            tags: 0,
            run: None,
            breach: None,
            prompts: Vec::new(),
            log: Vec::new(),
            winner: None,
            reason: None,
            hq_success_this_turn: false,
            uuid_counter: 0,
        };
        let must = |r: Result<Cid, String>| {
            r.unwrap_or_else(|e| panic!("cannot start game: {e}"))
        };
        st.identity[0] = must(st.spawn(corp_id, Zone::Identity));
        st.identity[1] = must(st.spawn(runner_id, Zone::Identity));
        for t in corp_deck {
            let cid = must(st.spawn(t, Zone::Deck));
            st.deck[0].push(cid);
        }
        for t in runner_deck {
            let cid = must(st.spawn(t, Zone::Deck));
            st.deck[1].push(cid);
        }
        st.shuffle_deck(Side::Corp);
        st.shuffle_deck(Side::Runner);
        st.draw_n(Side::Corp, 5);
        st.draw_n(Side::Runner, 5);
        for side in [Side::Corp, Side::Runner] {
            st.push_prompt(Prompt {
                side,
                msg: "Keep hand?".into(),
                choices: vec![],
                select: None,
                context: PromptContext::Mulligan,
                prompt_type: "mulligan",
            });
            let p = st.prompts.len() - 1;
            st.add_choice(p, "Keep");
            st.add_choice(p, "Mulligan");
        }
        st.system_log("Game started.".into());
        st
    }

    /// Create a card instance. Titles with a behavior row use it; any other
    /// title known to the printed database gets a synthesized vanilla
    /// definition (printed stats, no behavior). Unknown titles are an error.
    fn spawn(&mut self, title: &str, zone: Zone) -> Result<Cid, String> {
        let def = carddb::def_index_or_synth(title)?;
        let cid = self.cards.len() as Cid;
        self.cards.push(CardInstance {
            cid,
            def,
            zone,
            rezzed: false,
            faceup: false,
            advancement: 0,
            credits: 0,
            pump_encounter: 0,
            pump_run: 0,
            broken: Vec::new(),
        });
        Ok(cid)
    }

    // ── accessors ──────────────────────────────────────────────────────────
    pub fn card(&self, cid: Cid) -> &CardInstance {
        &self.cards[cid as usize]
    }
    pub fn card_mut(&mut self, cid: Cid) -> &mut CardInstance {
        &mut self.cards[cid as usize]
    }
    pub fn credits(&self, side: Side) -> i64 {
        self.credits[idx(side)]
    }
    pub fn clicks(&self, side: Side) -> i32 {
        self.clicks[idx(side)]
    }
    pub fn deck(&self, side: Side) -> &Vec<Cid> {
        &self.deck[idx(side)]
    }
    pub fn hand(&self, side: Side) -> &Vec<Cid> {
        &self.hand[idx(side)]
    }
    pub fn discard(&self, side: Side) -> &Vec<Cid> {
        &self.discard[idx(side)]
    }
    pub fn scored(&self, side: Side) -> &Vec<Cid> {
        &self.scored[idx(side)]
    }
    pub fn identity(&self, side: Side) -> Cid {
        self.identity[idx(side)]
    }
    pub fn server(&self, id: ServerId) -> Option<&Server> {
        self.servers.iter().find(|(s, _)| *s == id).map(|(_, v)| v)
    }
    pub fn server_mut(&mut self, id: ServerId) -> Option<&mut Server> {
        self.servers
            .iter_mut()
            .find(|(s, _)| *s == id)
            .map(|(_, v)| v)
    }

    pub fn username(&self, side: Side) -> &'static str {
        match side {
            Side::Corp => "Corp",
            Side::Runner => "Runner",
        }
    }

    // ── economy ────────────────────────────────────────────────────────────
    pub fn gain_credits(&mut self, side: Side, n: i64) {
        self.credits[idx(side)] += n;
    }

    /// Total spendable credits, including bad-pub run credits for the runner
    /// during a run.
    pub fn spendable(&self, side: Side) -> i64 {
        let mut c = self.credits[idx(side)];
        if side == Side::Runner {
            if let Some(run) = &self.run {
                c += run.run_credits as i64;
            }
        }
        c
    }

    /// Pay `n` credits, drawing on bad-pub run credits first for the runner.
    /// Returns false (and changes nothing) if unaffordable.
    pub fn pay_credits(&mut self, side: Side, n: i64) -> bool {
        if self.spendable(side) < n {
            return false;
        }
        let mut left = n;
        if side == Side::Runner {
            if let Some(run) = &mut self.run {
                let from_bp = left.min(run.run_credits as i64);
                run.run_credits -= from_bp as u32;
                left -= from_bp;
            }
        }
        self.credits[idx(side)] -= left;
        true
    }

    pub fn spend_click(&mut self, side: Side, n: i32) -> bool {
        if self.clicks[idx(side)] < n {
            return false;
        }
        self.clicks[idx(side)] -= n;
        true
    }

    // ── derived values ─────────────────────────────────────────────────────
    pub fn mu_limit(&self) -> i32 {
        let mut mu = 4;
        for &cid in &self.rig.hardware {
            for m in self.card(cid).def().statics {
                if let StaticMod::MemoryUnits(n) = m {
                    mu += n;
                }
            }
        }
        mu
    }
    pub fn mu_used(&self) -> i32 {
        self.rig
            .programs
            .iter()
            .map(|&cid| self.card(cid).def().mu_cost as i32)
            .sum()
    }
    pub fn max_hand_size(&self, side: Side) -> i32 {
        let mut n = 5;
        for &cid in self.scored(side) {
            for m in self.card(cid).def().statics {
                if let StaticMod::MaxHandSize(k) = m {
                    n += k;
                }
            }
        }
        n
    }
    pub fn agenda_points(&self, side: Side) -> u32 {
        self.scored(side)
            .iter()
            .filter_map(|&cid| self.card(cid).def().agenda_points)
            .sum()
    }
    pub fn ice_strength(&self, cid: Cid) -> i32 {
        let c = self.card(cid);
        let mut s = c.def().strength.unwrap_or(0);
        if c.def().advanceable {
            s += c.advancement as i32;
        }
        s
    }
    pub fn breaker_strength(&self, cid: Cid) -> i32 {
        let c = self.card(cid);
        c.def().strength.unwrap_or(0) + c.pump_encounter + c.pump_run
    }

    // ── zone movement ──────────────────────────────────────────────────────
    fn remove_everywhere(&mut self, cid: Cid) {
        for l in self.deck.iter_mut().chain(self.hand.iter_mut()) {
            l.retain(|&c| c != cid);
        }
        for l in self.discard.iter_mut().chain(self.scored.iter_mut()) {
            l.retain(|&c| c != cid);
        }
        for (_, srv) in self.servers.iter_mut() {
            srv.content.retain(|&c| c != cid);
            srv.ices.retain(|&c| c != cid);
        }
        self.rig.programs.retain(|&c| c != cid);
        self.rig.hardware.retain(|&c| c != cid);
        self.rig.resources.retain(|&c| c != cid);
    }

    /// Move a card to its owner's discard. Corp cards land face-up when seen.
    pub fn trash(&mut self, cid: Cid, faceup: bool) {
        self.remove_everywhere(cid);
        let side = self.card(cid).def().side;
        let c = self.card_mut(cid);
        c.zone = Zone::Discard;
        c.rezzed = false;
        c.faceup = faceup || side == Side::Runner;
        c.advancement = 0;
        c.credits = 0;
        c.pump_encounter = 0;
        c.pump_run = 0;
        c.broken.clear();
        self.discard[idx(side)].push(cid);
        self.prune_empty_remotes();
    }

    pub fn to_hand(&mut self, cid: Cid) {
        self.remove_everywhere(cid);
        let side = self.card(cid).def().side;
        self.card_mut(cid).zone = Zone::Hand;
        self.hand[idx(side)].push(cid);
    }

    pub fn to_score_area(&mut self, cid: Cid, side: Side) {
        self.remove_everywhere(cid);
        let c = self.card_mut(cid);
        c.zone = Zone::Scored(side);
        c.faceup = true;
        c.rezzed = false;
        c.advancement = 0;
        self.scored[idx(side)].push(cid);
        self.prune_empty_remotes();
    }

    /// Remove remotes that have neither content nor ice (jnet clears empties).
    pub fn prune_empty_remotes(&mut self) {
        let protected: Vec<ServerId> = self
            .run
            .iter()
            .map(|r| r.server)
            .chain(self.breach.iter().map(|b| b.server))
            .collect();
        self.servers.retain(|(id, srv)| {
            !matches!(id, ServerId::Remote(_))
                || !srv.content.is_empty()
                || !srv.ices.is_empty()
                || protected.contains(id)
        });
    }

    // ── deck ops (all randomness flows through the seeded RNG) ─────────────
    pub fn shuffle_deck(&mut self, side: Side) {
        let mut d = std::mem::take(&mut self.deck[idx(side)]);
        d.shuffle(&mut self.rng);
        self.deck[idx(side)] = d;
    }

    /// Return the whole hand to the deck and shuffle (mulligan).
    pub fn shuffle_hand_into_deck(&mut self, side: Side) {
        let hand = std::mem::take(&mut self.hand[idx(side)]);
        for cid in hand {
            self.card_mut(cid).zone = Zone::Deck;
            self.deck[idx(side)].push(cid);
        }
        self.shuffle_deck(side);
    }

    /// Draw n cards. Returns the number actually drawn.
    pub fn draw_n(&mut self, side: Side, n: usize) -> usize {
        let mut drawn = 0;
        for _ in 0..n {
            if self.deck[idx(side)].is_empty() {
                break;
            }
            let cid = self.deck[idx(side)].remove(0);
            self.card_mut(cid).zone = Zone::Hand;
            self.hand[idx(side)].push(cid);
            drawn += 1;
        }
        drawn
    }

    /// Pick `n` distinct random cards from a list (HQ access, net damage).
    pub fn pick_random(&mut self, mut pool: Vec<Cid>, n: usize) -> Vec<Cid> {
        pool.shuffle(&mut self.rng);
        pool.truncate(n);
        pool
    }

    pub fn rand_index(&mut self, len: usize) -> usize {
        self.rng.random_range(0..len)
    }

    // ── prompts ────────────────────────────────────────────────────────────
    pub fn next_uuid(&mut self) -> String {
        self.uuid_counter += 1;
        format!("u{}", self.uuid_counter)
    }

    pub fn push_prompt(&mut self, p: Prompt) {
        self.prompts.push(p);
    }

    pub fn add_choice(&mut self, prompt_index: usize, label: &str) {
        let uuid = self.next_uuid();
        self.prompts[prompt_index].choices.push(PromptChoice {
            uuid,
            label: label.to_string(),
        });
    }

    /// Convenience: build a button prompt in one call.
    pub fn prompt_buttons(
        &mut self,
        side: Side,
        msg: String,
        labels: &[&str],
        context: PromptContext,
    ) {
        let mut p = Prompt {
            side,
            msg,
            choices: Vec::new(),
            select: None,
            context,
            prompt_type: "other",
        };
        for l in labels {
            let uuid = self.next_uuid();
            p.choices.push(PromptChoice {
                uuid,
                label: l.to_string(),
            });
        }
        self.push_prompt(p);
    }

    pub fn prompt_select(
        &mut self,
        side: Side,
        msg: String,
        select: SelectKind,
        context: PromptContext,
    ) {
        self.push_prompt(Prompt {
            side,
            msg,
            choices: Vec::new(),
            select: Some(select),
            context,
            prompt_type: "select",
        });
    }

    /// The first open prompt for a side (its jnet prompt-state).
    pub fn current_prompt(&self, side: Side) -> Option<&Prompt> {
        self.prompts.iter().find(|p| p.side == side)
    }

    pub fn pop_prompt(&mut self, side: Side) -> Option<Prompt> {
        let i = self.prompts.iter().position(|p| p.side == side)?;
        Some(self.prompts.remove(i))
    }

    pub fn any_prompt_open(&self) -> bool {
        !self.prompts.is_empty()
    }

    // ── logging ────────────────────────────────────────────────────────────
    pub fn system_log(&mut self, text: String) {
        self.log.push(LogEntry {
            user: "__system__".into(),
            text,
        });
    }
    pub fn side_log(&mut self, side: Side, text: String) {
        let user = self.username(side).to_string();
        self.log.push(LogEntry {
            user: "__system__".into(),
            text: format!("{user} {text}"),
        });
    }

    // ── win/lose ───────────────────────────────────────────────────────────
    pub fn declare_winner(&mut self, side: Side, reason: &str) {
        if self.winner.is_some() {
            return;
        }
        self.winner = Some(side);
        self.reason = Some(reason.to_string());
        self.turn_state = TurnState::GameOver;
        self.prompts.clear();
        let name = self.username(side).to_string();
        self.system_log(format!("{name} wins the game ({reason})."));
    }

    pub fn check_agenda_win(&mut self) {
        for side in [Side::Corp, Side::Runner] {
            if self.agenda_points(side) >= 7 {
                self.declare_winner(side, "Agenda");
                return;
            }
        }
    }

    pub fn game_over(&self) -> bool {
        self.turn_state == TurnState::GameOver
    }

    // ── installed-card queries ─────────────────────────────────────────────
    pub fn all_installed_ice(&self) -> Vec<Cid> {
        self.servers
            .iter()
            .flat_map(|(_, s)| s.ices.iter().copied())
            .collect()
    }
    pub fn installed_programs(&self) -> Vec<Cid> {
        self.rig.programs.clone()
    }
    /// The server a piece of installed ice protects.
    pub fn ice_server(&self, cid: Cid) -> Option<ServerId> {
        self.servers
            .iter()
            .find(|(_, s)| s.ices.contains(&cid))
            .map(|(id, _)| *id)
    }
    pub fn content_server(&self, cid: Cid) -> Option<ServerId> {
        self.servers
            .iter()
            .find(|(_, s)| s.content.contains(&cid))
            .map(|(id, _)| *id)
    }
}
