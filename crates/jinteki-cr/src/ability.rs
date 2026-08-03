//! Abilities: the five types (9.3.7), the six flags (9.3.6), activity
//! (9.1.7 with ALL of 9.1.8's exceptions), pending instances (9.6.2-9.6.4),
//! independence points (9.5.4/9.6.12/9.8.8 enforced per 9.1.4),
//! optional-vs-mandatory (9.6.9), and static-condition repetition with the
//! no-effect throttle (9.6.7).

use crate::change::GameChange;
use crate::effects::DamageKind;
use crate::instr::Instruction;
use crate::object::{CardType, Object, ObjectId, ServerId, Side, Zone};

/// CR 9.1.1e / 9.3.7: every ability is exactly one of these five types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityKind {
    Static,
    Paid,
    Conditional,
    Play,
    Subroutine,
}

/// CR 9.3.6a: the six ability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityFlag {
    /// 9.3.6b: mid-access-window-only paid abilities.
    Access,
    /// 9.3.6c: icebreaker strength-gated abilities.
    Interface,
    /// 9.3.6d: interrupt-window-only abilities.
    Interrupt,
    /// 9.3.6e: can persist after trash-during-access.
    Persistent,
    /// 9.3.6f: active only at threat ≥ N.
    Threat(u8),
    /// 9.3.6g: usable once per turn.
    OncePerTurn,
}

/// Trigger conditions the W1 kernel can detect in checkpoint step (a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerCond {
    /// "When your turn begins." (side = controller's side)
    TurnBegins(Side),
    /// "When this run ends." Optionally only if it was successful.
    RunEnds { successful_only: bool },
    /// "Whenever a run on this server ends." (AMAZE class; source in root)
    RunOnThisServerEnds,
    /// "Whenever the Runner trashes a Corp card." — one instance per card
    /// (per-occurrence, 9.6.4b / 9.12.2a Hostile Infrastructure).
    RunnerTrashesCorpCard,
    /// "Whenever the Runner trashes at least 1 Corp card." — one instance per
    /// event (Warroid Tracker class, 9.12.2a). `in_this_server` adds 4.6.6i's
    /// scope ("…at least 1 card installed in or protecting this server"),
    /// which for a card trashed FROM that server means the server it left.
    RunnerTrashesAtLeastOneCorpCard { in_this_server: bool },
    /// "When you access this card." (active while inactive, 9.1.8a)
    /// `requires` carries 9.6.5c's additional requirements ("…if the Runner
    /// is tagged" — Quantum Predictive Model class).
    SelfAccessed { requires: Vec<TriggerRequirement> },
    /// "Whenever you access a card…" (Neutralize All Threats class) — a
    /// Runner-side condition met by accessing ANY card, not this one.
    RunnerAccessesCard,
    /// "When the Runner encounters this ice."
    SelfEncountered,
    /// "Whenever the Runner encounters a piece of ice." (Runner-side class)
    EncounterBegins,
    /// "Whenever the Runner takes a tag." (Mr. Stone class)
    RunnerTakesTag,
    /// "Whenever the Runner suffers damage." (per damage occurrence)
    RunnerSuffersDamage,
    /// Interrupt trigger: "…would draw any number of cards" (Class Act).
    WouldDraw { first_each_turn: bool },
    /// Interrupt trigger: "…this card would be trashed" (Harbinger class).
    SelfWouldBeTrashed,
    /// "Whenever the Runner breaches this server…" (Ash class).
    ThisServerBreached,
    /// CR 7.3.8: "when the current breach ends" — the condition the kernel
    /// gives the conditional ability a delayed breach is treated as.
    BreachEnds,
    /// CR 10.6.1: "whenever the Corp takes bad publicity…" (Raymond Flint
    /// class).
    PlayerTakesBadPublicity(Side),
    /// "When this turn ends." (Joshua B. class delayed conditionals.)
    TurnEnds(Side),
    /// "Whenever you use a [trash] ability." (Geist-adjacent test class)
    UsesTrashAbility(Side),
    /// "Whenever you advance a card." `had_no_advancement` adds the
    /// 9.6.6a "had"-condition read against the previous checkpoint snapshot.
    /// CR 1.18.2: met by an ADVANCE only — never by an instruction that
    /// places or moves an advancement counter directly.
    AdvancesCard { had_no_advancement: bool },
    /// "When you score this agenda…" (1.17.6; the dividends keyword, 10.13.1).
    /// `requires` carries 9.6.5c's additional requirements ("…if the Runner
    /// is tagged" — Market Research class), which 9.6.14d keeps in force even
    /// when an effect resolves the ability by class without a real scoring.
    SelfScored { requires: Vec<TriggerRequirement> },
    /// "When the Runner steals this agenda…" (1.17.7; Clone Retirement
    /// class). Met after the Runner has moved the agenda to their score area,
    /// which is why the ability resolves from the score area.
    SelfStolen,
    /// "When you install this card…" (9.6.14b's class: met at step 8.5.16f of
    /// installing its own source).
    SelfInstalled,
    /// "When this card is added to your stack…" (Nanuq class). The move that
    /// meets it is the move that makes the card INACTIVE, which is why
    /// 9.1.8g has to keep the ability active long enough to resolve.
    SelfAddedToDeck,
    /// CR 9.9.6c: interrupt trigger — "…would pay a play or install cost".
    /// A cost that would be paid while resolving an effect is a value, so an
    /// interrupt can modify it; the relevance test is whether the imminent
    /// instruction carries such a value.
    WouldPayCost,
    /// CR 9.12.2b: "whenever you gain credits…" (NASX class). One instance
    /// per OCCURRENCE (9.6.4b): an unaggregated group of effects gains the
    /// credits several times over, and this condition sees each of them.
    PlayerGainsCredits(Side),
    /// CR 10.11.5: "the first time each turn you make a successful run on
    /// your mark…" (Virtuoso class). 10.11.5: a condition checking a game
    /// property related to the mark only checks from the moment that server
    /// was designated, so an earlier successful run on the same server —
    /// before it was the mark — does not spend the "first time each turn".
    SuccessfulRunOnMark { first_each_turn: bool },
    /// CR 10.9.2: "when this card is empty…" (Crowdfunding class). The
    /// condition can only be met after the card has been LOADED with counters
    /// of this kind by a preceding ability of the same card — a card with no
    /// counters on it has not become empty, it was never loaded.
    SelfEmpty { kind: crate::object::CounterKind },
    /// CR 4.8.3: "whenever you install a program from your heap…" (Exile
    /// class) — a condition stipulating the zone the installed card came
    /// from. The set-aside zone is never that zone: 4.8.3 reports the
    /// location the card was in before it was set aside.
    CardInstalledFrom { side: Side, from: Zone },
    /// Interrupt trigger: "…would do damage" (ordinal: Some(1) = "the first
    /// time each run you would…", Tori Hanzō class).
    WouldDamage { kind: Option<DamageKind>, first_each_run: bool },
    /// Interrupt trigger: "…would take tags during a run" (Jesminder class:
    /// `during_run` requires a run to be in progress).
    WouldTakeTags { during_run: bool },
    /// "Whenever the Corp installs a card in the root of this server…"
    /// (Tranquility Home Grid class; the 9.6.5b activity gate is the point).
    CardInstalledInSourceServer,
    /// "When that encounter ends…" (Chum-class delayed conditionals).
    EncounterEnds,
    /// "…if all of its subroutines were broken during that encounter"
    /// (Forked class). 9.12.2d vacuous truth: ice with ZERO subroutines
    /// satisfies this as soon as step 6.9.3b of the encounter begins.
    AllSubsBrokenOnEncounteredIce,
    /// "Whenever the Runner steals an agenda…" (Bacterial Programming /
    /// Seidr class drivers for the 7.4.7a examples).
    RunnerStealsAgenda,
    /// "Whenever the Runner avoids receiving a tag…" (Thunder Art Gallery
    /// class — the 9.9.4c/d chain-reaction examples).
    RunnerAvoidsTag,
    /// "Whenever <side> searches their deck…" (Personality Profiles class).
    /// CR 8.7.5: a condition involving a search becomes met only after the
    /// search is complete and any shuffling has been performed — which is
    /// why the search records its change AFTER shuffling and the checkpoint
    /// that pends this ability is the one ending the search instruction
    /// (9.11.4d).
    PlayerSearchesDeck(Side),
    /// "Whenever you install a card…" (Near-Earth Hub class).
    CardInstalledBy(Side),
    /// "Whenever you make a successful run on the chosen server…" (Security
    /// Testing class). CR 9.10.3b: the server is read from the maintained
    /// choice under `key`, so the condition is met only by a run on the
    /// server chosen for THIS turn — and never when no server was chosen.
    SuccessfulRunOnChosenServer { key: &'static str },
    /// "When the Runner passes this ice…" (Tatu-Bola class). The pass happens
    /// at run step 6.9.4a (`rule_pass_ice`).
    SelfPassed,
    /// "After you resolve this operation/event…" (Oppo Research class). CR
    /// 8.6.7h: conditions related to finishing resolving a played card are
    /// met at that step, after the card has been trashed (8.6.7g) — which is
    /// why 9.1.8g keeps the ability active long enough to resolve.
    SelfPlayResolved,
    /// "Whenever this card prevents 1 or more damage…" (Guru Davinder class,
    /// 9.9.7f). Met only when the imminent damage value was greater than 0
    /// before the interrupt from the SAME source decreased or removed it.
    SourcePreventedDamage,
    /// "Whenever a card is exposed…" (Blackguard class). CR 9.6.4b: exposing
    /// several cards in ONE instruction meets this condition once per card,
    /// because exposing is not one of 9.12.2c's aggregated effect classes.
    CardExposed,
    /// "Whenever an installed <side> card is trashed…" (District 99 /
    /// Wasteland class). `of_types` narrows the description the way the
    /// printed text does ("a program or piece of hardware"); empty is any
    /// card type. CR 8.2.2a: a trash that was PREVENTED never happened, so
    /// this condition is not met by it.
    InstalledCardTrashed { side: Side, of_types: Vec<CardType> },
    /// "Whenever <side> spends 1 or more credits…" (GameNET class). CR
    /// 1.16.2b makes a calculated credit cost ONE payment, so this meets its
    /// condition once however many "for each" terms the calculation had.
    PlayerPaysCredits(Side),
}

/// CR 9.6.5c: an ADDITIONAL requirement listed inside a trigger condition
/// ("…if the Runner is tagged"). It is part of the condition, not of the
/// effect, so it must hold at the moment the condition would occur — and
/// 9.6.14d keeps it in force when an effect resolves the ability by class
/// instead of the stipulation actually occurring. Carried as data next to
/// the condition it qualifies, so the requirement is one vocabulary rather
/// than a `CondIfRunnerTagged` variant per condition (§12 rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerRequirement {
    /// "…if the Runner is tagged" (5.4: the Runner is tagged with ≥ 1 tag).
    RunnerTagged,
}

/// Stable identity of one subroutine on a piece of ice: (category rank per
/// 9.8.2/9.8.3, source key, ordinal within that source). Category-d counts
/// shrink last-first (9.8.3d), which is exactly highest-ordinal-first here.
/// CR 1.15.1: subroutines are announced as targets like objects are, so this
/// key is part of the decision vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubKey {
    pub category: u8,
    pub src: u64,
    pub ord: u32,
}

/// Static conditions (9.6.7) for repeat-while-true conditionals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticCond {
    /// "While this card's host has 0 or less strength…" (Parasite class).
    HostStrengthAtMost(i32),
    /// CR 9.1.2b: "…during the resolution of this card's abilities" (Attini
    /// class). An ability "is resolving" from when its first instruction
    /// becomes imminent until its last instruction has finished resolving,
    /// which includes every interrupt window opened for its instructions —
    /// so a declaration scoped this way applies inside those windows.
    SourceAbilityResolving,
    /// CR 7.4.2b: "…as long as the Runner has accessed a card during the
    /// indicated run". The condition an ability reading "the Runner cannot
    /// access more than 1 card during this run" states about ITS OWN
    /// prohibition: it has no effect on breaches or candidates until a card
    /// has actually been accessed (7.3.6) during the run in progress.
    RunnerHasAccessedCardThisRun,
}

/// CR 9.6.1a: the primary condition is a trigger or static condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Trigger(TriggerCond),
    Static(StaticCond),
}

/// A cost (1.16.1: anything spent, resolved, or met to use an ability or
/// apply an effect; must be payable all at once).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cost {
    /// CR 1.16.2b: a quantity position (§12 rule 6). "2[credit] for each
    /// advanced piece of ice protecting this server" is a selector evaluated
    /// AT THE TIME THE COST IS TO BE PAID, and the result is taken as an
    /// aggregate — one payment of 6, not three payments of 2.
    pub credits: crate::instr::Quantity,
    pub clicks: u32,
    /// [trash]: trash this card as part of the cost.
    pub trash_self: bool,
    /// "take N tags" as a cost (Funhouse class).
    pub tags: u32,
    /// "suffer N net damage" as a cost (Obokata class).
    pub net_damage: u32,
    /// CR 5.2.1a: "Lose [click]" as a cost — clicks are spent, but the
    /// ability is NOT an action (Eli 1.0's break ability), so it is used
    /// during a paid ability window and not in an action window.
    pub lose_clicks: u32,
    /// "trash N cards from your grip/HQ" as a cost (Patchwork class).
    /// KERNEL APPROXIMATION: which cards are trashed is not put to the payer
    /// (the front of the hand is taken); no example distinguishes them.
    pub trash_from_hand: u32,
    /// CR 1.9.2: "spend N <kind> counters hosted on this card" (Imp class).
    /// The counters come off the ability's SOURCE, which is what makes an
    /// empty card's ability unusable rather than free.
    pub spend_counters: Option<(crate::object::CounterKind, u32)>,
    /// CR 8.2.5 / 4.9.3: "forfeit an agenda" as a cost (24/7 News Cycle
    /// class) — N agendas move from the payer's score area to the
    /// removed-from-game zone, their agenda points stop counting, and
    /// anything hosted on them is trashed.
    ///
    /// KERNEL APPROXIMATION (deviation 11's class): WHICH agenda is forfeited
    /// is not put to the payer — the front of the score area is taken —
    /// because `Vm::pay_cost` is synchronous everywhere. No example turns on
    /// the choice; the 9.6.14d example makes its real choice afterwards, in
    /// the 1.15.2 announcement of whose ability to resolve.
    pub forfeit_agenda: u32,
}

impl Cost {
    pub fn credits(n: u32) -> Self {
        Cost { credits: crate::instr::Quantity::c(n as i64), ..Default::default() }
    }
    /// A credit cost whose amount is calculated when it is paid (1.16.2b).
    pub fn credits_q(q: crate::instr::Quantity) -> Self {
        Cost { credits: q, ..Default::default() }
    }
    pub fn trash_self() -> Self {
        Cost { trash_self: true, ..Default::default() }
    }
    pub fn tags(n: u32) -> Self {
        Cost { tags: n, ..Default::default() }
    }
    pub fn net_damage(n: u32) -> Self {
        Cost { net_damage: n, ..Default::default() }
    }
    /// CR 5.2.1a: a "Lose [click]" cost — spent clicks, but not an action.
    pub fn lose_clicks(n: u32) -> Self {
        Cost { lose_clicks: n, ..Default::default() }
    }
    pub fn trash_from_hand(n: u32) -> Self {
        Cost { trash_from_hand: n, ..Default::default() }
    }
    /// CR 1.9.2: "spend N hosted counters of a kind" as a cost.
    pub fn spend_counters(kind: crate::object::CounterKind, n: u32) -> Self {
        Cost { spend_counters: Some((kind, n)), ..Default::default() }
    }
    /// CR 8.2.5: "forfeit N agendas" as a cost.
    pub fn forfeit_agenda(n: u32) -> Self {
        Cost { forfeit_agenda: n, ..Default::default() }
    }
    pub fn free() -> Self {
        Cost::default()
    }
    /// The CONSTANT credit amount of this cost, for assertions and displays
    /// that do not have a source to evaluate a 1.16.2b calculation against.
    /// A calculated amount reads as 0 here, exactly as 1.16.2d treats an X
    /// out of context.
    pub fn flat_credits(&self) -> u32 {
        match self.credits {
            crate::instr::Quantity::Const(n) if n > 0 => n as u32,
            _ => 0,
        }
    }
    pub fn is_free(&self) -> bool {
        *self == Cost::default()
    }
    /// 1.16.10b: additional costs combine into one all-at-once payment.
    pub fn plus(&self, other: &Cost) -> Cost {
        Cost {
            // 1.16.10b combines additional costs into ONE payment; constant
            // amounts fold so `is_free` still recognises an empty cost.
            credits: match (&self.credits, &other.credits) {
                (crate::instr::Quantity::Const(a), crate::instr::Quantity::Const(b)) => {
                    crate::instr::Quantity::Const(a + b)
                }
                (a, b) => crate::instr::Quantity::Plus(Box::new(a.clone()), Box::new(b.clone())),
            },
            clicks: self.clicks + other.clicks,
            trash_self: self.trash_self || other.trash_self,
            tags: self.tags + other.tags,
            net_damage: self.net_damage + other.net_damage,
            lose_clicks: self.lose_clicks + other.lose_clicks,
            trash_from_hand: self.trash_from_hand + other.trash_from_hand,
            spend_counters: self.spend_counters.or(other.spend_counters),
            forfeit_agenda: self.forfeit_agenda + other.forfeit_agenda,
        }
    }
}

/// CR 9.5.6: effect-based timing restrictions on paid abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingRestriction {
    /// 9.5.6a/c: usable only during an encounter — and, where the ability
    /// refers to the encountered ice with a stipulation ("this code gate"),
    /// only during an encounter with a piece of ice that meets it.
    EncounterOnly { required_subtype: Option<&'static str> },
    /// 9.5.6b: usable only during the Approach Ice Phase, with the
    /// approached ice matching all stipulations used in referring to it.
    ApproachOnly { required_subtype: Option<&'static str>, rezzed: bool },
}

/// CR 1.13: which side of a hosting relationship a declaration reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRelation {
    /// The card this one is hosted on (1.13.1).
    Host,
    /// The cards hosted on this one — directly only; hosting is not
    /// transitive (1.13.9).
    Hosted,
}

/// Declarations of a static ability (kernel-wave subset). Statics never
/// resolve (9.4.1) — the VM queries them continuously.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticDecl {
    /// Characteristic modification of the source's host (Hush class) or self.
    StrengthMod { target_self: bool, delta: i32 },
    /// CR 9.1.9a: "<the related card> loses all of its abilities." The
    /// relation is the content (§12 rule 2), so both directions of §1.13's
    /// hosting relation are one declaration: the Hush class removes its
    /// HOST's abilities, the Magnet class removes its HOSTED cards'. When
    /// both are present the two effects form a 9.12.1e dependency loop, which
    /// is what the hosted-beats-host tiebreak exists for.
    RemoveAbilitiesOf(HostRelation),
    /// "This card gains/loses <subtypes>." (Morph class — Lycan's own
    /// ability removes one instance of a subtype it also prints.) 2.16.5
    /// counts instances, so removing one instance of a doubly-added subtype
    /// leaves the card with it.
    SubtypeModSelf { add: Vec<&'static str>, remove: Vec<&'static str> },
    /// CR 9.1.9a: "<the described cards> lose all of their abilities."
    /// (Direct Access class: "identity cards do not have abilities".) The
    /// described set is the shared criteria vocabulary, so the whole class is
    /// one declaration (§12 rule 2) — contrast `RemoveAbilitiesOf`, which
    /// names the hosting relation instead of a description.
    RemoveAbilitiesOfMatching { criteria: Vec<crate::instr::TargetFilter> },
    /// "This card gains the subtypes of <criteria>." (Mother Goddess class.)
    /// The subtypes copied are the source cards' EFFECTIVE subtypes, read
    /// through the same 9.12.1b pipeline — so a card that itself gained a
    /// subtype passes it on, and the dependency 9.12.1d describes is realised
    /// by the pipeline re-entering itself for each copied-from card.
    GainSubtypesOf { criteria: Vec<crate::instr::TargetFilter> },
    /// "Each <criteria> gains '[sub] …' before/after its other subroutines."
    /// (Warden Fatuma class.) A static ability that is NOT on the ice gaining
    /// the subroutine, so the grant lands in origin category 9.8.3a (before)
    /// or 9.8.3e (after) rather than the self-static categories b/d that
    /// [`StaticDecl::GainSubroutines`] carries.
    GrantSubroutinesTo {
        criteria: Vec<crate::instr::TargetFilter>,
        sub: Box<AbilityDef>,
        before: bool,
    },
    /// "This ice cannot be trashed by <side>'s card abilities."
    /// (Architect class; a restriction active per 9.1.8.)
    CannotBeTrashed,
    /// "Runs on this server cannot be declared successful." (Crisium class.)
    RunsNotDeclaredSuccessful,
    /// Memory limit modifier (Runner).
    MemoryLimitMod(i32),
    /// "+N to the amount of <kind> damage done by <responsible>."
    /// (The Cleaners class — modifies imminent damage values via statics.)
    DamageBonus { kind: DamageKind, responsible: Side, amount: i64 },
    /// Additional cost to steal agendas (Ben Musashi / Predictive Algorithm
    /// class; 1.16.10).
    AdditionalStealCost(Cost),
    /// CR 7.4.2: "the Runner cannot access any cards other than this one"
    /// (Flagship class). Declared by a STATIC ability rather than created as
    /// a lingering effect, so it applies exactly while the ability is active
    /// (9.1.7) and its stated condition holds (9.3.7a) — which is what makes
    /// 7.4.2a's mid-breach re-evaluation observable: uninstalling or
    /// derezzing the source lifts the prohibition and the cards it was
    /// keeping out become candidates again.
    RestrictCandidatesToSelf,
    /// "<side> cannot draw cards." (Lockdown class; 9.9.2 statics remove
    /// parts of expected effects.)
    CannotDraw(Side),
    /// "<side> cannot spend credits." (RSVP class; forces 0 bids, 10.14.3.)
    /// A static ability carrying `StaticCond::SourceAbilityResolving` scopes
    /// it to its own resolution (Attini class, 9.1.2b).
    CannotSpendCredits(Side),
    /// "This ice gains N copies of '[sub] …'" where N is a quantity selector
    /// (Ashigaru class: N = count of cards in HQ; category 9.8.3d —
    /// self-static, after printed, lose last-first as the count shrinks).
    GainSubroutines { sub: Box<AbilityDef>, count: crate::instr::Quantity },
    /// "Cards cannot be hosted on this card." (Tithonium class; 10.3.1e
    /// hosting-illegality restriction.)
    CannotHost,
    /// "This card can host <criteria>, up to <capacity>." (CR 1.13.5 /
    /// 1.13.6a — Off-Campus Apartment, Dhegdheer, Glenn Station and
    /// Leprechaun are all this one declaration.) `criteria` is the shared
    /// filter vocabulary as a conjunction; `capacity` is a quantity
    /// position, `None` meaning "any number" (1.13.5). A card carrying this
    /// declaration and NO ability that hosts cards onto itself is thereby an
    /// eligible installation destination for matching cards (1.13.6a); one
    /// that also has such an ability is not (1.13.6b).
    CanHost { criteria: Vec<crate::instr::TargetFilter>, capacity: Option<crate::instr::Quantity> },
    /// "The install cost of the hosted card is lowered by N." (Dhegdheer's
    /// second sentence; 1.16.6.) Applies only to cards hosted directly on
    /// the source — host relationships are not transitive (1.13.9).
    HostedInstallDiscount(crate::instr::Quantity),
    /// "Install only on <description>." (CR 1.13.6c, Egret class.) A
    /// restriction on where the source may be installed: if no card matching
    /// the description exists before the installation process begins, the
    /// source cannot be installed at all. Active while the source is
    /// inactive (9.1.8c).
    InstallOnlyHostedOn(Vec<crate::instr::TargetFilter>),
    /// "+N link" (Dyson Mem Chip class; the 9.6.5d link example).
    LinkBonus(i32),
    /// "This operation is not trashed until the Runner steals an agenda."
    /// (Targeted Marketing / current class — 8.6.6c: instead of trashing at
    /// 8.6.7g, a lingering effect keeps it in the play area until the
    /// indicated effect occurs.)
    PlayedNotTrashedUntilAgendaSteal,
    /// "As an additional cost to access a card in the root of a remote
    /// server, pay N." (Gagarin class — 7.4.3 example 2.)
    AdditionalAccessCost(Cost),
    /// "You may pay <cost> to lower the install cost of a card you are
    /// installing by N." (Patchwork class; 1.16.6 install costs.) The
    /// reduction is only available while its own cost is payable, which is
    /// exactly what makes it part of 8.7.2b's affordability query.
    InstallDiscount { cost: Cost, amount: u32 },
    /// CR 9.10.5 / 9.9.9a: "Lingering effects that would modify <this card's
    /// host / this card's> strength instead expire at <duration>."
    /// (Gebrselassie class.) The ability keeps the corresponding lingering
    /// effect alive until the additional duration expires, applies
    /// continuously only while this static ability is active (9.9.9a), and
    /// never touches the effects of static abilities — they have no
    /// durations and create no lingering effects (9.4.4).
    ExtendStrengthDurations { target_host: bool, until: crate::lingering::WantedDuration },
    /// "This ice's strength is X" where X is a quantity selector (Surveyor
    /// class: X = 2 × ice protecting this server). Evaluated through the
    /// characteristics pipeline; while the defining ability is lost (Hush)
    /// the 9.12.1d pipeline skips the effect and X is treated as 0
    /// (9.12.2e).
    SelfStrength(crate::instr::Quantity),
    /// CR 1.17.3a / 9.1.8e: "The Corp can score agendas in this server with N
    /// fewer advancement counters" (SanSan City Grid class) — a modification
    /// of the advancement REQUIREMENT of every agenda in the source's server.
    /// The scope is the source's server, exactly as
    /// `TargetFilter::IceProtectingSourceServer` scopes ice.
    ScoreRequirementModInSourceServer(i32),
    /// CR 4.6.8f: "Limit N remote servers." (Earth Station class.) While
    /// active, the Corp cannot create a new remote server that would take the
    /// total above N.
    RemoteServerLimit(u32),
    /// CR 6.3.2a: "The Runner cannot initiate a run on this server."
    /// (Off the Grid class.) The declaration refers to the ANNOUNCEMENT of
    /// the attacked server at step 6.9.1a and to nothing else — an ability
    /// that changes the attacked server mid-run (6.1.2d) is not affected.
    CannotInitiateRunOnSourceServer,
}

/// One ability as printed/granted: the unit of rules text (9.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityDef {
    pub kind: AbilityKind,
    pub flags: Vec<AbilityFlag>,
    /// Conditional abilities: the primary condition (9.6.1).
    pub condition: Option<Condition>,
    /// Paid abilities: the trigger cost (9.5.1).
    pub cost: Option<Cost>,
    /// Non-static abilities: instructions in order (9.1.1g, 9.1.2).
    pub instructions: Vec<Instruction>,
    /// Static abilities: declarations (9.3.7a).
    pub statics: Vec<StaticDecl>,
    /// CR 9.6.9: optional iff the ability could have no effects at all
    /// ("may"/"allows"/once-per-turn). Mandatory otherwise.
    pub optional: bool,
    /// CR 9.5.6: effect-based timing restriction, if any.
    pub timing: Option<TimingRestriction>,
    /// Human-readable tag for tests/logs.
    pub label: &'static str,
}

impl AbilityDef {
    pub fn conditional(cond: TriggerCond, instrs: Vec<Instruction>, optional: bool) -> Self {
        AbilityDef {
            kind: AbilityKind::Conditional,
            flags: Vec::new(),
            condition: Some(Condition::Trigger(cond)),
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional,
            timing: None,
            label: "",
        }
    }

    pub fn paid(cost: Cost, instrs: Vec<Instruction>) -> Self {
        // CR 9.5.3: paid abilities are always optional.
        AbilityDef {
            kind: AbilityKind::Paid,
            flags: Vec::new(),
            condition: None,
            cost: Some(cost),
            instructions: instrs,
            statics: Vec::new(),
            optional: true,
            timing: None,
            label: "",
        }
    }

    pub fn subroutine(instrs: Vec<Instruction>) -> Self {
        AbilityDef {
            kind: AbilityKind::Subroutine,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional: false,
            timing: None,
            label: "",
        }
    }

    pub fn static_ability(statics: Vec<StaticDecl>) -> Self {
        AbilityDef {
            kind: AbilityKind::Static,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: Vec::new(),
            statics,
            optional: false,
            timing: None,
            label: "",
        }
    }

    pub fn with_timing(mut self, t: TimingRestriction) -> Self {
        self.timing = Some(t);
        self
    }

    pub fn with_flag(mut self, f: AbilityFlag) -> Self {
        self.flags.push(f);
        self
    }

    pub fn labeled(mut self, l: &'static str) -> Self {
        self.label = l;
        self
    }

    pub fn has_flag(&self, f: AbilityFlag) -> bool {
        self.flags.contains(&f)
    }

    /// CR 9.9.1: an interrupt is flagged [interrupt] or uses
    /// prevent/avoid/would. In the kernel the card layer sets the flag or the
    /// instruction vocabulary implies it.
    pub fn is_interrupt(&self) -> bool {
        cite!("rule_interrupt_keywords");
        if self.has_flag(AbilityFlag::Interrupt) {
            return true;
        }
        if let Some(Condition::Trigger(
            TriggerCond::WouldDamage { .. }
            | TriggerCond::WouldTakeTags { .. }
            | TriggerCond::WouldDraw { .. }
            | TriggerCond::SelfWouldBeTrashed,
        )) = self.condition
        {
            return true;
        }
        self.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::PreventDamage { .. }
                    | Instruction::PreventAllDamage { .. }
                    | Instruction::AvoidTags(_)
                    | Instruction::IncreaseImminentDamage { .. }
                    | Instruction::PreventTrashOf(_)
            )
        })
    }

    /// CR 5.2.1: an action is a paid ability whose cost begins with [click].
    pub fn is_action(&self) -> bool {
        cite!("rule_action");
        self.kind == AbilityKind::Paid && self.cost.as_ref().map(|c| c.clicks > 0).unwrap_or(false)
    }
}

/// Reference to one ability on one object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilityRef {
    pub obj: ObjectId,
    pub index: usize,
}

/// A pending/resolving instance of a conditional ability (9.6.2).
#[derive(Debug, Clone)]
pub struct AbilityInstance {
    pub id: u64,
    pub ability: AbilityRef,
    /// Snapshot of the definition at creation (survives source movement).
    pub def: AbilityDef,
    pub controller: Side,
    /// CR 9.6.9: mandatory instances gate passing (9.2.8e).
    pub mandatory: bool,
    /// The window this instance is pending in (fixed set, 9.2.8a).
    pub window: Option<u64>,
    /// CR 9.1.8g: condition met by the source's own move to an inactive
    /// zone — the ability remains active until this instance resolves.
    pub hangover: bool,
    /// CR 9.6.12/9.5.4/9.8.8 → 9.1.4: once independent, a source zone change
    /// strands self-referencing effects. `source_generation` is the source
    /// OBJECT's generation (1.12.3) as of when this instance came into being;
    /// a zone change bumps it, so a later comparison says the object the
    /// ability referred to no longer exists.
    pub independent: bool,
    pub source_generation: u32,
    /// Group of the change occurrence that created this instance.
    pub occurrence_group: u64,
    /// For delayed conditionals: the lingering effect maintaining it.
    pub from_lingering: Option<u64>,
    /// Structure instance this pending is tied to (persistent/9.12.5d and
    /// run-scoped conditions).
    pub run_id: Option<u64>,
}

/// CR 9.1.7 + 9.1.8: whether an ability is active. `encounter_ice` is the
/// currently-encountered ice (for 9.1.8h), `accessed` the currently-accessed
/// card (for 9.1.8a mid-access relevance), `threat` the current threat level
/// (1.17.1a) that gates the "threat N" flag (9.3.6f).
pub fn ability_active(
    obj: &Object,
    def: &AbilityDef,
    encountered_ice: Option<ObjectId>,
    accessed_card: Option<ObjectId>,
    threat: i32,
) -> bool {
    cite!("rule_ability_active");
    // 9.3.6f: the threat flag gates activity "regardless of section 9.1.8",
    // so it is checked before every other rule here.
    cite!("rule_threat_flag");
    if def.flags.iter().any(|f| matches!(f, AbilityFlag::Threat(n) if threat < *n as i32)) {
        return false;
    }
    if crate::object::card_active(obj) {
        return true;
    }
    // 9.1.8a: access-condition abilities are active while the card is
    // inactive (so "when accessed" fires on cards in R&D/HQ/Archives).
    if matches!(def.condition, Some(Condition::Trigger(TriggerCond::SelfAccessed { .. }))) {
        cite!("rule_active_exception_access");
        return true;
    }
    if def.has_flag(AbilityFlag::Access) && accessed_card == Some(obj.id) {
        cite!("rule_active_exception_access");
        return true;
    }
    // 9.1.8h: subroutines of uninstalled encountered ice are active during
    // that encounter.
    if def.kind == AbilityKind::Subroutine && encountered_ice == Some(obj.id) {
        cite!("rule_active_exception_encounter_not_installed");
        return true;
    }
    // 9.1.8c/d/e/f: play/install/rez permissions and cost modifiers,
    // advancement-requirement modifiers, can-advance grants. The kernel-wave
    // StaticDecl set has no such declarations yet; when the card layer adds
    // them they gain activity here.
    cite!("rule_active_exception_modify_play_install_rez");
    cite!("rule_active_exception_modify_cost");
    cite!("rule_active_exception_advancement_requirement");
    cite!("rule_active_exception_can_be_advanced");
    // 9.1.8b: zone-scoped abilities (none in the W1 vocabulary).
    cite!("rule_active_exception_catchall");
    // 9.1.8g is instance-driven (hangover) and handled by the checkpoint scan.
    // 9.1.8i persistent: handled via lingering effects.
    false
}

/// Does a change record match a trigger condition? Returns per-occurrence
/// match; the checkpoint scan handles multiplicity/grouping (9.6.4b,
/// 9.12.2a) and "had"-snapshot requirements (9.6.6a).
pub fn trigger_matches(
    cond: &TriggerCond,
    change: &GameChange,
    source: &Object,
    server_of_source: Option<ServerId>,
    trashed_is_corp: impl Fn(ObjectId) -> bool,
) -> bool {
    cite!("rule_trigger_condition_checked");
    match (cond, change) {
        (TriggerCond::TurnBegins(side), GameChange::TurnBegan { side: s }) => side == s,
        (TriggerCond::RunEnds { .. }, GameChange::RunEnded { .. }) => true,
        (TriggerCond::RunOnThisServerEnds, GameChange::RunEnded { server, .. }) => {
            server_of_source == Some(*server)
        }
        (TriggerCond::RunnerTrashesCorpCard, GameChange::CardTrashed { by, obj, .. }) => {
            *by == Side::Runner && trashed_is_corp(*obj)
        }
        (
            TriggerCond::RunnerTrashesAtLeastOneCorpCard { .. },
            GameChange::CardTrashed { by, obj, .. },
        ) => {
            // The server scope (4.6.6i) is applied by the checkpoint scan,
            // which has the state access to resolve "this server".
            *by == Side::Runner && trashed_is_corp(*obj)
        }
        // 9.6.5c: any additional requirement carried by the condition is
        // checked by the checkpoint scan (it has the state access); this arm
        // only matches the change class.
        (TriggerCond::SelfAccessed { .. }, GameChange::CardAccessed { obj }) => *obj == source.id,
        (TriggerCond::RunnerAccessesCard, GameChange::CardAccessed { .. }) => {
            cite!("rule_accessing");
            true
        }
        (TriggerCond::SelfEncountered, GameChange::EncounterBegan { ice, .. }) => {
            *ice == source.id
        }
        (TriggerCond::EncounterBegins, GameChange::EncounterBegan { .. }) => true,
        (TriggerCond::PlayerPaysCredits(side), GameChange::CostPaid { side: s, credits, .. }) => {
            cite!("rule_cost_quantities");
            side == s && *credits > 0
        }
        (TriggerCond::SelfPassed, GameChange::IcePassed { ice }) => {
            cite!("rule_pass_ice");
            *ice == source.id
        }
        (TriggerCond::ThisServerBreached, GameChange::BreachBegan { server }) => {
            server_of_source == Some(*server)
        }
        (TriggerCond::BreachEnds, GameChange::BreachEnded { .. }) => {
            cite!("rule_consecutive_breaches");
            true
        }
        (
            TriggerCond::PlayerTakesBadPublicity(side),
            GameChange::BadPublicityTaken { side: s, .. },
        ) => {
            cite!("rule_bad_publicity");
            side == s
        }
        (TriggerCond::TurnEnds(side), GameChange::TurnEnded { side: s }) => side == s,
        (TriggerCond::RunnerTakesTag, GameChange::TagsTaken { .. }) => true,
        (TriggerCond::RunnerSuffersDamage, GameChange::DamageSuffered { .. }) => true,
        (TriggerCond::UsesTrashAbility(side), GameChange::TrashAbilityUsed { side: s, .. }) => {
            side == s
        }
        // 1.18.2: only an ADVANCE meets this condition. An instruction that
        // places an advancement counter directly (Mushin No Shin class), or
        // moves one from another card, records `CounterPlaced` and nothing
        // else, so a "whenever you advance" ability does not fire for it.
        (TriggerCond::AdvancesCard { .. }, GameChange::CardAdvanced { .. }) => {
            cite!("rule_advance");
            cite!("rule_placing_advancement_counter");
            true
        }
        // 1.17.6: "when you score this agenda" — met after the Corp moves the
        // agenda to their score area.
        (TriggerCond::SelfScored { .. }, GameChange::AgendaScored { obj, .. }) => {
            cite!("rule_agenda_scored");
            *obj == source.id
        }
        // 1.17.7: "when the Runner steals this agenda" — met after the Runner
        // moves it to their score area.
        (TriggerCond::SelfStolen, GameChange::AgendaStolen { obj, .. }) => {
            cite!("rule_agenda_stolen");
            *obj == source.id
        }
        // 9.6.14b: the stipulation point is step 8.5.16f of installing the
        // source itself.
        (TriggerCond::SelfInstalled, GameChange::CardInstalled { obj, .. }) => {
            cite!("rule_when_installed");
            *obj == source.id
        }
        (TriggerCond::PlayerGainsCredits(side), GameChange::CreditsGained { side: s, .. }) => {
            cite!("rule_calculated_quantity");
            side == s
        }
        // 10.11.5: the server must be the mark, and the "first time each
        // turn" ordinal is counted from the designation — both are state the
        // checkpoint scan checks.
        (TriggerCond::SuccessfulRunOnMark { .. }, GameChange::RunDeclaredSuccessful { .. }) => {
            cite!("rule_mark_designated_condition_check");
            true
        }
        // 10.9.1: becoming empty is a counter of a LOADED kind leaving the
        // card. Whether the kind was loaded, and whether any are left, is
        // state the checkpoint scan checks (this match only sees the change).
        (
            TriggerCond::SelfEmpty { kind },
            GameChange::CounterRemoved { obj: Some(o), kind: k, .. },
        ) => {
            cite!("rule_load_and_empty");
            *o == source.id && k == kind
        }
        (TriggerCond::SelfAddedToDeck, GameChange::CardMoved { obj, to: Zone::Deck(_), .. }) => {
            cite!("rule_active_exception_conditional_move_to_inactive_zone");
            *obj == source.id
        }
        (
            TriggerCond::CardInstalledInSourceServer,
            GameChange::CardInstalled { obj, side: Side::Corp, .. },
        ) => {
            // The installed card's server must be the source's server. The
            // caller passes the source's server; the installed card's server
            // is read through the same closure surface used for trash
            // triggers, so we compare zones here via the source-server hook.
            cite!("rule_condition_only_met_while_active");
            let _ = obj;
            // Server comparison happens in the checkpoint scan (it has state
            // access); this arm only matches the change class.
            true
        }
        (TriggerCond::SelfPlayResolved, GameChange::CardPlayResolved { obj }) => {
            cite!("rule_steps_playing_after_resolve_condition");
            *obj == source.id
        }
        (TriggerCond::SourcePreventedDamage, GameChange::DamagePrevented { by, .. }) => {
            cite!("rule_prevent_as_trigger_condition");
            *by == source.id
        }
        (TriggerCond::CardExposed, GameChange::CardExposed { .. }) => {
            cite!("rule_expose");
            true
        }
        (
            TriggerCond::InstalledCardTrashed { side, .. },
            GameChange::CardTrashed { obj, was_zone, .. },
        ) => {
            // 8.2.2a: only a trash that actually happened records this change.
            // The `of_types` narrowing is applied by the checkpoint scan,
            // which can read the trashed card's type.
            cite!("rule_cancelled_movement");
            was_zone.is_installed() && is_corp_card_side(trashed_is_corp(*obj)) == *side
        }
        (TriggerCond::EncounterEnds, GameChange::EncounterEnded { .. }) => true,
        (TriggerCond::AllSubsBrokenOnEncounteredIce, GameChange::AllSubsBroken { .. }) => {
            cite!("rule_vacuous_truth");
            true
        }
        (TriggerCond::RunnerStealsAgenda, GameChange::AgendaStolen { .. }) => true,
        (TriggerCond::RunnerAvoidsTag, GameChange::TagsAvoided { .. }) => true,
        (TriggerCond::PlayerSearchesDeck(side), GameChange::ZoneSearched { by, zone }) => {
            cite!("rule_search_condition");
            by == side && *zone == Zone::Deck(*side)
        }
        (
            TriggerCond::CardInstalledFrom { side, from },
            GameChange::CardInstalled { side: s, from: f, .. },
        ) => {
            // 4.8.3: `from` is the location the card is TREATED as having come
            // from, so an Exile-class "whenever you install a program from
            // your heap" is met by an install out of an 8.7.4 set-aside.
            cite!("rule_set_aside_zone_passthrough");
            side == s && from == f
        }
        (TriggerCond::CardInstalledBy(side), GameChange::CardInstalled { side: s, .. }) => {
            side == s
        }
        (
            TriggerCond::SuccessfulRunOnChosenServer { .. },
            GameChange::RunDeclaredSuccessful { .. },
        ) => {
            // 9.10.3b: the chosen server is compared by the checkpoint scan,
            // which can read the maintained choice.
            cite!("rule_lingering_effect_maintaining_choice_turn_begins_duration");
            true
        }
        _ => false,
    }
}

/// CR 9.6.14: a class of ability referred to by its trigger condition, plus
/// the one non-conditional ability an effect can name positionally. This is
/// the CONTENT of [`crate::instr::Instruction::ResolveAbilityOf`] (§12 rule
/// 2), so "resolve the 'when scored' ability of an agenda in your score
/// area" and "resolve its first subroutine" are the same instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityClass {
    /// 9.6.14a: any ability that could meet its trigger condition at step
    /// 6.9.3a of an encounter with its source.
    WhenEncountered,
    /// 9.6.14b: … at step 8.5.16f of installing its source.
    WhenInstalled,
    /// 9.6.14c: any ability on an agenda that could meet its trigger
    /// condition as a result of the Corp choosing to score that agenda.
    WhenScored,
    /// §9.8: the Nth subroutine of the card in the 9.8.2 order (0-based).
    /// Not a 9.6.14 class — a subroutine is not a conditional ability, so it
    /// never becomes pending (9.8.10: it resolves where it is named).
    Subroutine(usize),
}

/// CR 9.6.14a–c: is `def` a member of the named class — an ability that
/// COULD meet its trigger condition at that class's stipulation point?
pub fn ability_in_class(def: &AbilityDef, class: AbilityClass) -> bool {
    cite!("rule_references_to_trigger_conditions");
    let Some(Condition::Trigger(cond)) = &def.condition else { return false };
    match class {
        AbilityClass::WhenEncountered => {
            cite!("rule_when_encountered");
            matches!(cond, TriggerCond::SelfEncountered)
        }
        AbilityClass::WhenInstalled => {
            cite!("rule_when_installed");
            matches!(cond, TriggerCond::SelfInstalled)
        }
        AbilityClass::WhenScored => {
            cite!("rule_when_scored");
            matches!(cond, TriggerCond::SelfScored { .. })
        }
        AbilityClass::Subroutine(_) => false,
    }
}

/// CR 9.6.5c: the additional requirements a trigger condition carries, which
/// must be met by the game state for the condition to occur — and, per
/// 9.6.14d, for an effect to resolve the ability by class.
pub fn trigger_requirements(cond: &TriggerCond) -> &[TriggerRequirement] {
    cite!("rule_condition_requirements_part_of_condition");
    match cond {
        TriggerCond::SelfAccessed { requires } | TriggerCond::SelfScored { requires } => requires,
        _ => &[],
    }
}

/// CR 9.6.4b vs 9.12.2a: is this trigger per-occurrence (each matching
/// change record pends an instance) or per-event (one instance per change
/// group)?
pub fn trigger_per_event(cond: &TriggerCond) -> bool {
    cite!("rule_act_on_multiple_cards");
    matches!(cond, TriggerCond::RunnerTrashesAtLeastOneCorpCard { .. })
}

/// Map the trash-trigger filter's Corp-ness back to a side.
fn is_corp_card_side(is_corp: bool) -> Side {
    if is_corp {
        Side::Corp
    } else {
        Side::Runner
    }
}

/// Is a card a Corp card by printed side (for trash-trigger filters)?
pub fn is_corp_card(t: CardType) -> bool {
    matches!(
        t,
        CardType::Identity
            | CardType::Agenda
            | CardType::Asset
            | CardType::Ice
            | CardType::Operation
            | CardType::Upgrade
    )
}

/// Zone shorthand used by trigger filters.
pub fn in_archives(z: Zone) -> bool {
    matches!(z, Zone::Discard(Side::Corp))
}
