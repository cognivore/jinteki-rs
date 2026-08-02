use serde::{Deserialize, Serialize};

pub type Cid = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Corp,
    Runner,
}

impl Side {
    pub fn opponent(self) -> Side {
        match self {
            Side::Corp => Side::Runner,
            Side::Runner => Side::Corp,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Corp => "corp",
            Side::Runner => "runner",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerId {
    Hq,
    Rd,
    Archives,
    Remote(u32),
}

impl ServerId {
    pub fn key(&self) -> String {
        match self {
            ServerId::Hq => "hq".into(),
            ServerId::Rd => "rd".into(),
            ServerId::Archives => "archives".into(),
            ServerId::Remote(n) => format!("remote{n}"),
        }
    }
    /// jnet display name, as used in run targets and log lines.
    pub fn display(&self) -> String {
        match self {
            ServerId::Hq => "HQ".into(),
            ServerId::Rd => "R&D".into(),
            ServerId::Archives => "Archives".into(),
            ServerId::Remote(n) => format!("Server {n}"),
        }
    }
    pub fn from_key(k: &str) -> Option<ServerId> {
        match k {
            "hq" | "HQ" => Some(ServerId::Hq),
            "rd" | "R&D" => Some(ServerId::Rd),
            "archives" | "Archives" => Some(ServerId::Archives),
            _ => {
                let k = k.strip_prefix("remote").or_else(|| k.strip_prefix("Server "))?;
                k.trim().parse().ok().map(ServerId::Remote)
            }
        }
    }
    pub fn is_central(&self) -> bool {
        !matches!(self, ServerId::Remote(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardType {
    Identity,
    Agenda,
    Asset,
    Upgrade,
    Ice,
    Operation,
    Event,
    Program,
    Hardware,
    Resource,
}

impl CardType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CardType::Identity => "Identity",
            CardType::Agenda => "Agenda",
            CardType::Asset => "Asset",
            CardType::Upgrade => "Upgrade",
            CardType::Ice => "ICE",
            CardType::Operation => "Operation",
            CardType::Event => "Event",
            CardType::Program => "Program",
            CardType::Hardware => "Hardware",
            CardType::Resource => "Resource",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IceSubtype {
    Barrier,
    CodeGate,
    Sentry,
}

impl IceSubtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            IceSubtype::Barrier => "Barrier",
            IceSubtype::CodeGate => "Code Gate",
            IceSubtype::Sentry => "Sentry",
        }
    }
}

/// Where a card instance currently lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Deck,
    Hand,
    Discard,
    Scored(Side),
    /// Installed in a server (corp cards): in content row or ice column.
    InServer { server: ServerId, ice: bool },
    /// Installed in the runner rig.
    Rig,
    /// Identity zone.
    Identity,
    /// Removed from game (unused by pool, kept for completeness).
    Rfg,
}

/// Subroutine effects present in the playable pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubEffect {
    EndTheRun,
    RunnerLosesClick,
    NetDamage(u32),
    CorpGainCredits(u32),
    TrashProgram,
}

impl SubEffect {
    pub fn label(&self) -> String {
        match self {
            SubEffect::EndTheRun => "End the run".into(),
            SubEffect::RunnerLosesClick => "The Runner loses [Click]".into(),
            SubEffect::NetDamage(n) => format!("Do {n} net damage"),
            SubEffect::CorpGainCredits(n) => format!("Gain {n} [Credits]"),
            SubEffect::TrashProgram => "Trash 1 installed program".into(),
        }
    }
}

/// Declarative on-play effects (operations and events).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnPlay {
    GainCredits(u32),
    Draw(u32),
    /// Run event. `target: None` = prompt to choose any server (Dirty Laundry).
    RunEvent {
        target: Option<ServerId>,
        access_bonus: u32,
        /// Credits gained when the run ends successfully (Dirty Laundry).
        success_credits: u32,
    },
}

/// Declarative when-scored effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnScore {
    GainCredits(u32),
    GainCreditsAndBadPub(u32),
    /// "You may rez 1 piece of ice, ignoring all costs." (Priority Requisition)
    OptionalRezIceFree,
    /// Superconducting Hub: draw up to N; +2 max hand size handled statically.
    DrawUpTo(u32),
}

/// Click abilities on installed cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickAbility {
    /// "[Click]: take N credits from this card" (Armitage, Regolith).
    TakeCredits(u32),
}

/// Static, continuously-recomputed modifiers granted while active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticMod {
    MemoryUnits(i32),
    /// While in a score area (Superconducting Hub).
    MaxHandSize(i32),
}

/// Identity abilities in the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityAbility {
    /// Weyland BABW: gain 1 credit whenever you play a transaction.
    GainOnTransaction,
    /// Gabriel Santiago: gain 2 credits on first successful HQ run each turn.
    GabrielHq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakerDef {
    pub breaks: IceSubtype,
    pub break_cost: u32,
    /// (cost, amount) — None for fixed-strength breakers (Mimic).
    pub pump: Option<(u32, u32)>,
    /// Pump duration: true = lasts the whole run (Gordian Blade),
    /// false = this encounter only.
    pub pump_for_run: bool,
    pub base_strength: i32,
}

/// A card definition: printed data + declarative behavior.
/// This struct is the target the future designer DSL compiles into.
#[derive(Debug, Clone, Copy)]
pub struct CardDef {
    pub title: &'static str,
    pub side: Side,
    pub kind: CardType,
    /// Play/install/rez cost as printed.
    pub cost: u32,
    pub subtypes: &'static [&'static str],
    pub ice_subtype: Option<IceSubtype>,
    pub strength: Option<i32>,
    pub trash_cost: Option<u32>,
    pub mu_cost: u32,
    pub advancement_requirement: Option<u32>,
    pub agenda_points: Option<u32>,
    /// Starting counters placed when installed (Armitage 12, Regolith 15).
    pub start_credits: u32,
    pub advanceable: bool,
    pub subroutines: &'static [SubEffect],
    pub on_play: Option<OnPlay>,
    pub on_score: Option<OnScore>,
    /// Corp drip at start of corp turn while rezzed (PAD Campaign).
    pub drip_corp_turn: u32,
    pub click_ability: Option<ClickAbility>,
    pub statics: &'static [StaticMod],
    pub breaker: Option<BreakerDef>,
    pub identity_ability: Option<IdentityAbility>,
}

impl CardDef {
    pub const fn blank(title: &'static str, side: Side, kind: CardType) -> CardDef {
        CardDef {
            title,
            side,
            kind,
            cost: 0,
            subtypes: &[],
            ice_subtype: None,
            strength: None,
            trash_cost: None,
            mu_cost: 0,
            advancement_requirement: None,
            agenda_points: None,
            start_credits: 0,
            advanceable: false,
            subroutines: &[],
            on_play: None,
            on_score: None,
            drip_corp_turn: 0,
            click_ability: None,
            statics: &[],
            breaker: None,
            identity_ability: None,
        }
    }
    pub fn is_transaction(&self) -> bool {
        self.subtypes.contains(&"Transaction")
    }
}

/// Commands mirror jinteki.net's `:game/action` command strings (ICD §B.5 subset).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Keep,
    Mulligan,
    StartTurn,
    EndTurn,
    Credit,
    Draw,
    Play { cid: Cid },
    /// Install from hand; corp needs a server ("New remote" allowed).
    InstallCorp { cid: Cid, server: String },
    InstallRunner { cid: Cid },
    Advance { cid: Cid },
    Score { cid: Cid },
    Rez { cid: Cid },
    Run { server: ServerId },
    /// Paid ability on an installed card (breakers, Armitage/Regolith).
    Ability { cid: Cid, index: usize },
    /// Answer the open prompt by choice uuid.
    Choice { uuid: String },
    /// Answer the open select-prompt by clicking a card.
    Select { cid: Cid },
    Continue,
    JackOut,
    RemoveTag,
    /// Runner pays trash cost during access (also reachable as a Choice).
    TrashAccessed { cid: Cid },
    Concede,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    NotYourTurn,
    NoClicks,
    CantAfford,
    InvalidCard,
    InvalidCommand(String),
    PromptOpen,
    NoPrompt,
    BadChoice,
    GameOver,
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for EngineError {}
