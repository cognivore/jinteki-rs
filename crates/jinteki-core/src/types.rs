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

// ═══════════════════════════════════════════════════════════════════════════
// Ability IR: triggers × effect sequences.
//
// Cards register `TriggeredAbility` rows; the engine dispatches every game
// event through one pipeline (`ir::fire_event`), gathering the registrations
// of active cards in a deterministic order (active player's cards first,
// mirroring the reference's `gather-events`). Suspended decisions live in
// the prompt queue as `PromptContext` data, never as host continuations.
// ═══════════════════════════════════════════════════════════════════════════

/// Kinds of counters a card can host (mirrors the reference's counter map).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CounterKind {
    Credit,
    Power,
    Virus,
    Agenda,
}

impl CounterKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CounterKind::Credit => "credit",
            CounterKind::Power => "power",
            CounterKind::Virus => "virus",
            CounterKind::Agenda => "agenda",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Net,
    Meat,
    Brain,
}

impl DamageKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DamageKind::Net => "net",
            DamageKind::Meat => "meat",
            DamageKind::Brain => "core",
        }
    }
}

/// Dynamic effect magnitudes, resolved against the source card at fire time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    Fixed(u32),
    /// n × hosted advancement counters (Junebug's 2×, Ghost Branch's 1×).
    PerAdvancement(u32),
    /// Cards in the runner's grip (Psychic Field).
    RunnerHandSize,
}

/// Server filters for run-related triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerFilter {
    Any,
    Central,
    Hq,
    Rd,
    Archives,
}

impl ServerFilter {
    pub fn matches(&self, server: ServerId) -> bool {
        match self {
            ServerFilter::Any => true,
            ServerFilter::Central => server.is_central(),
            ServerFilter::Hq => server == ServerId::Hq,
            ServerFilter::Rd => server == ServerId::Rd,
            ServerFilter::Archives => server == ServerId::Archives,
        }
    }
}

/// Preconditions checked when an ability would trigger (the reference's `:req`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Always,
    /// Hosted advancement counters > 0 (advanceable ambushes).
    AdvancementPositive,
    /// The runner made a successful run during their last turn (SEA Source).
    RunnerSuccessfulRunLastTurn,
    /// The ending run was successful (Dirty Laundry's run-ends hook).
    RunSuccessful,
}

/// What starts an ability. `...Self` triggers fire only for the card the
/// event happened to; the rest are global and fire for every active
/// registration that matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    TurnBegins(Side),
    TurnEnds(Side),
    SuccessfulRun(ServerFilter),
    RunEnds,
    /// Fired when the breach of a server begins (access-count window).
    BreachServer(ServerFilter),
    AgendaScored,
    AgendaStolen,
    /// This agenda was scored.
    OnScoreSelf,
    /// The runner accessed this card. `installed_only` restricts to installed
    /// cards (advanceable ambushes); otherwise anywhere except Archives
    /// (Snare!'s "anywhere except in Archives").
    OnAccessSelf { installed_only: bool },
    OnExposeSelf,
    OnRezSelf,
    OnInstallSelf,
    /// The runner encounters this ice (Data Raven).
    OnEncounterSelf,
    /// This operation/event was played (its printed effect).
    OnPlaySelf,
    /// The controller played an operation with this subtype (BABW).
    PlayOperationWithSubtype(&'static str),
}

/// One selectable branch inside `Effect::Choose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOption {
    pub label: &'static str,
    pub effects: &'static [Effect],
}

/// Composable effect steps. Sequences run left to right through one queue;
/// prompt-bearing steps suspend the queue as `PromptContext` data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    GainCredits(Side, u32),
    LoseCredits(Side, u32),
    Draw(Side, u32),
    Damage(DamageKind, Amount),
    /// Give the runner tags.
    GainTags(Amount),
    /// Remove runner tags.
    LoseTags(u32),
    GainBadPub(u32),
    /// Place counters on the source card.
    PlaceCounters(CounterKind, u32),
    /// Take up to n hosted credits onto the controller's pool; trash the
    /// source when it empties (Adonis Campaign).
    TakeCreditsFromSelf(u32),
    TrashSelf,
    EndTheRun,
    /// Runner selects an unrezzed installed corp card to expose (Infiltration).
    ExposeSelect,
    /// Corp selects a piece of ice to rez ignoring all costs (Priority Req).
    RezIceIgnoringCosts,
    /// Access additional cards from this breach (Legwork, The Maker's Eye).
    AccessBonus(u32),
    /// Modify the currently encountered ice's strength until the encounter
    /// ends (Datasucker).
    ModIceStrengthThisEncounter(i32),
    /// "You may pay N to ..." — the source's controller decides; the cost is
    /// paid on Yes (Snare!'s pay-4, Junebug's pay-1; cost 0 = plain yes/no).
    Optional {
        prompt: &'static str,
        cost: u32,
        yes: &'static [Effect],
        no: &'static [Effect],
    },
    /// A button choice between effect branches.
    Choose {
        who: Side,
        options: &'static [ChoiceOption],
    },
    /// Trace: corp reveals base, openly boosts with credits; runner then
    /// boosts link with credits; corp strength > runner strength = success.
    Trace {
        base: u32,
        on_success: &'static [Effect],
        on_fail: &'static [Effect],
    },
    /// Psi game: both players secretly bid 0-2 credits, pay them, then the
    /// equal/differ branch runs.
    Psi {
        on_equal: &'static [Effect],
        on_differ: &'static [Effect],
    },
}

/// A trigger → effect-sequence registration on a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggeredAbility {
    pub trigger: Trigger,
    pub condition: Condition,
    pub once_per_turn: bool,
    pub effects: &'static [Effect],
}

impl TriggeredAbility {
    pub const fn when(trigger: Trigger, effects: &'static [Effect]) -> TriggeredAbility {
        TriggeredAbility {
            trigger,
            condition: Condition::Always,
            once_per_turn: false,
            effects,
        }
    }
}

/// When a hosted-counter paid ability may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityTiming {
    /// The controller's action window, or any time during a run.
    Anytime,
    /// Only while a run is in progress (Nisei MK II).
    DuringRun,
    /// Only while encountering a rezzed piece of ice (Datasucker).
    DuringEncounter,
}

/// A paid ability whose cost is hosted counters (Data Raven's power counter,
/// Nisei MK II's agenda counter, Datasucker's virus counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterAbility {
    pub label: &'static str,
    pub cost: (CounterKind, u32),
    pub timing: AbilityTiming,
    pub effects: &'static [Effect],
}

/// Subroutine effects present in the playable pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubEffect {
    EndTheRun,
    RunnerLosesClick,
    NetDamage(u32),
    CorpGainCredits(u32),
    TrashProgram,
    /// A general IR effect sequence (traces, psi games, ...).
    Ability {
        label: &'static str,
        effects: &'static [Effect],
    },
}

impl SubEffect {
    pub fn label(&self) -> String {
        match self {
            SubEffect::EndTheRun => "End the run".into(),
            SubEffect::RunnerLosesClick => "The Runner loses [Click]".into(),
            SubEffect::NetDamage(n) => format!("Do {n} net damage"),
            SubEffect::CorpGainCredits(n) => format!("Gain {n} [Credits]"),
            SubEffect::TrashProgram => "Trash 1 installed program".into(),
            SubEffect::Ability { label, .. } => (*label).into(),
        }
    }
}

/// Run events: play = initiate a run. `target: None` prompts for any server
/// (Dirty Laundry). Success/breach effects live in `triggered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunEventDef {
    pub target: Option<ServerId>,
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
    pub advanceable: bool,
    pub subroutines: &'static [SubEffect],
    /// Playing this event initiates a run.
    pub run_event: Option<RunEventDef>,
    /// Play legality gate for operations/events (SEA Source's "play only if").
    pub play_condition: Option<Condition>,
    /// Event-driven abilities (the IR registrations).
    pub triggered: &'static [TriggeredAbility],
    pub click_ability: Option<ClickAbility>,
    /// Paid abilities costing hosted counters.
    pub counter_abilities: &'static [CounterAbility],
    pub statics: &'static [StaticMod],
    pub breaker: Option<BreakerDef>,
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
            advanceable: false,
            subroutines: &[],
            run_event: None,
            play_condition: None,
            triggered: &[],
            click_ability: None,
            counter_abilities: &[],
            statics: &[],
            breaker: None,
        }
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
    /// Paid ability on an installed card (breakers, Armitage/Regolith,
    /// hosted-counter abilities).
    Ability { cid: Cid, index: usize },
    /// Answer the open prompt by choice uuid.
    Choice { uuid: String },
    /// Answer the open select-prompt by clicking a card.
    Select { cid: Cid },
    Continue,
    JackOut,
    RemoveTag,
    /// Corp basic action: [click] + 2 credits, trash 1 resource if tagged.
    TrashResource,
    /// Corp basic action: [click][click][click], purge virus counters.
    Purge,
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
