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
    /// "When your turn begins." (`side` = whose turn), with 9.6.5c's
    /// additional requirements riding along exactly as they do on
    /// [`TriggerCond::DiscardPhaseEnds`] — "when your turn begins, **if this
    /// card is in Archives and the Runner did not initiate any runs during
    /// their last turn**, …" (Subliminal Messaging). The zone stipulation is
    /// also what makes the ability active where it sits (9.1.8b's first
    /// sentence), so it has to be part of the CONDITION and not of the
    /// instructions.
    TurnBegins { side: Side, requires: Vec<TriggerRequirement> },
    /// "When this run ends." Optionally only if it was successful.
    ///
    /// `on` is the server the sentence names — "when a run **on HQ or R&D**
    /// ends" (Zahya Sadeghi) — as content on the one condition (§12 rule 2),
    /// the same list [`TriggerCond::MakesSuccessfulRun`] carries. An empty
    /// list is a sentence naming no server, which is every run.
    RunEnds { successful_only: bool, on: Vec<ServerId> },
    /// CR 6.9.1: "when a run **on R&D** begins" (Captain Padma Isbister) —
    /// the Run Initiation Phase, whose first step announces the attacked
    /// server, so the server is known when the condition is met. `on` is the
    /// same stipulation [`TriggerCond::RunEnds`] carries, and empty is a
    /// sentence naming no server.
    ///
    /// Deliberately NOT [`TriggerCond::ServerApproached`]: 6.9.2 approaches a
    /// server after every piece of ice protecting it has been passed, which
    /// is a later step and does not happen at all if the run ends first.
    RunBegins { on: Vec<ServerId> },
    /// "Whenever a run on this server ends." (AMAZE class; source in root)
    RunOnThisServerEnds,
    /// "Whenever the Runner trashes a Corp card." — one instance per card
    /// (per-occurrence, 9.6.4b / 9.12.2a Hostile Infrastructure). `requires`
    /// is 9.6.5c's additional requirement listed inside the condition, which
    /// is where a draft identity's leading "if you have more … installed than
    /// any other faction" goes (Wyvern): asked when the trash occurs, and not
    /// again while the ability resolves.
    RunnerTrashesCorpCard { requires: Vec<TriggerRequirement> },
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
    /// "When your action phase ends…" (Nebula class). The §11 turn tables
    /// record the phase boundary as a change (5.6.2 / 5.7.2), and the
    /// stipulation rides as requirements exactly as on DiscardPhaseEnds.
    ActionPhaseEnds { side: Side, requires: Vec<TriggerRequirement> },
    /// "The first time each turn you play an operation…" (Gemilang class,
    /// with the once-per-turn flag carrying the "first time"): a card of one
    /// of these types was played by `by`. Empty `of_types` = any card.
    ///
    /// Every stipulation the sentence can make about the play is content on
    /// this one atom (§12 rule 2): `by` is `None` when the sentence names no
    /// player ("another current operation or event is played", 3.5.1b), the
    /// type and subtype lists are the 2.15/2.16 stipulations (each empty
    /// meaning no stipulation, exactly as on
    /// [`TriggerCond::RunnerAccessesCard`]), and `other_than_source` is the
    /// word "another" — the same reading
    /// [`crate::instr::TargetFilter::OtherThanSource`] gives "other".
    ///
    /// A card has exactly one type (2.15) and any number of subtypes (2.16),
    /// so `of_types` is read as "any of these" and `of_subtypes` as "all of
    /// these" — which is how the printed phrase "another current operation or
    /// event" reads in one condition.
    /// `also_installed` is CR 8.5.1's half of the same sentence: some cards
    /// name BOTH ways a card can be used out of a hand — "whenever the Runner
    /// **plays or installs** a copy of that card" (Targeted Marketing), "the
    /// Runner **plays or installs** a card that has the type you last named"
    /// (Azmari EdTech). That is ONE trigger condition, not two abilities,
    /// because the sentence is one and its 9.3.6g "first time each turn" has
    /// to be spent by whichever of the two happens first.
    ///
    /// `matching_choice` is 9.10.3's back-reference: the played or installed
    /// card must match the value the source is maintaining under that key,
    /// exactly as [`crate::instr::TargetFilter::MatchesMaintainedChoice`]
    /// reads it for a description. `None` is a sentence that names no such
    /// value.
    ///
    /// `criteria` is anything else the sentence says about the card, in the
    /// shared filter vocabulary (§12 rule 5) — "you play **a copy of
    /// Subliminal Messaging**" is [`crate::instr::TargetFilter::HasName`],
    /// which 10.1.5 reads as every card with that name rather than as a
    /// self-reference.
    ///
    /// The printed ordinal "**the first time each turn** you play …" is
    /// [`AbilityDef::ordinal`], not a field here: it is the same
    /// 9.6.5c stipulation for every condition, so it lives once beside the
    /// condition instead of once per variant (§12 rule 2).
    CardPlayed {
        by: Option<Side>,
        of_types: Vec<CardType>,
        of_subtypes: Vec<&'static str>,
        criteria: Vec<crate::instr::TargetFilter>,
        other_than_source: bool,
        also_installed: bool,
        matching_choice: Option<&'static str>,
    },
    /// `of_types` is the sentence's card-type stipulation ("whenever you
    /// access an agenda", Film Critic); empty means no stipulation, exactly
    /// as it does on [`TriggerCond::CorpRezzesCard`].
    RunnerAccessesCard { of_types: Vec<crate::object::CardType> },
    /// "When the Runner encounters this ice."
    SelfEncountered,
    /// "Whenever the Runner encounters a piece of ice." (Runner-side class),
    /// with the sentence's stipulations as content (§12 rule 2):
    /// `of_subtypes` is 2.16's subtype stipulation — "whenever you encounter a
    /// **barrier**" (Paperclip), read through the 9.12.1b pipeline like every
    /// other subtype query — and `requires` carries 9.6.5c requirements,
    /// including the zone statement ("install this program **from your heap**")
    /// that 9.1.8b reads to keep the ability active where it can act.
    EncounterBegins { of_subtypes: Vec<&'static str>, requires: Vec<TriggerRequirement> },
    /// CR 6.9.4g: "Whenever the Runner approaches a server." (Formicary class
    /// — the last step of the Movement Phase, so the reaction window that
    /// follows it is not one a phase BEGINNING opened, which is what 6.8.2c
    /// is about.)
    ServerApproached,
    /// "Whenever the Runner takes a tag." (Mr. Stone class)
    RunnerTakesTag,
    /// "Whenever the Runner suffers damage." (per damage occurrence)
    ///
    /// `kind` is the sentence's stipulation about WHICH damage — "whenever the
    /// Runner takes at least 1 net damage" names one kind, and `None` is a
    /// sentence naming none, exactly as [`TriggerCond::WouldDamage`] already
    /// carries it on the interrupt side. Content on one condition, not a
    /// condition per kind (§12 rule 1).
    ///
    /// `trashed_a_card` is the other stipulation the same occurrence can
    /// carry: "whenever the Runner **trashes a card** for brain damage"
    /// (Chronos Protocol: Haas-Bioroid). 10.4.2a/b make the trash the damage
    /// procedure ITSELF — "for each point of damage suffered, the player
    /// responsible for the damage trashes 1 randomly-chosen card from the
    /// grip" — so the trash and the damage are one occurrence and not two,
    /// which is why this is a stipulation about that occurrence rather than a
    /// condition of its own. (The printed "the Runner trashes" is the older
    /// wording of the same procedure; 10.4.2c says the same of "brain
    /// damage". Only the Runner's cards are ever trashed for damage, so the
    /// two sentences describe one thing.)
    ///
    /// The stipulation is what the sentence says, and it says something the
    /// unqualified one does not: damage against an empty grip suffers the
    /// damage and trashes nothing. No board can show the difference today —
    /// 1.7.2b flatlines the Runner for exactly that damage — so the field
    /// changes no game the kernel can currently play. It is written because
    /// the card writes it, not because a case is waiting on it. 1.15.4's cards
    /// are the trashed ones either way ([`crate::change::cards_named_by`]).
    ///
    /// `responsible` is 10.4.1's third stipulation about the same occurrence:
    /// "whenever **you do** damage" (AU Co.) is the does-branch — the Corp
    /// and the source are responsible — while a sentence that directs the
    /// Runner to "suffer" damage put the responsibility on the Runner, so a
    /// Corp identity's "you do damage" is NOT met by it. `None` is a sentence
    /// naming nobody, which is every condition that existed before this
    /// field. Read off the record, not the board: the source's printed
    /// wording decided it at the moment the damage resolved.
    RunnerSuffersDamage {
        kind: Option<DamageKind>,
        trashed_a_card: bool,
        responsible: Option<Side>,
    },
    /// Interrupt trigger: "…would draw any number of cards" (Class Act).
    /// `by` is the sentence's stipulation about WHOSE draw — "the first time
    /// each turn YOU would draw" is the ability's controller, and `None` is a
    /// sentence naming no player, exactly as on
    /// [`TriggerCond::CardPlayed`]. Without it a Runner card would interrupt
    /// the Corp's mandatory draw, and 9.9.5a's "first time each turn" would
    /// be spent by it.
    /// The printed ordinal is [`AbilityDef::ordinal`], which is where every
    /// condition's is (§12 rule 2).
    WouldDraw { by: Option<Side> },
    /// CR 8.4.2: "abilities with trigger conditions related to cards being
    /// drawn can act on them" — met once per card drawn, while the drawn
    /// cards are still set aside (8.4.2a), which is what lets a Daily-Business
    /// -Show-class ability move one of them before it reaches the hand.
    PlayerDrawsCards(Side),
    /// Interrupt trigger: "…this card would be trashed" (Harbinger class).
    SelfWouldBeTrashed,
    /// CR 10.4.2 / 9.1.8b: "when this card is trashed by damage" (I've Had
    /// Worse class). The condition can ONLY ever be met by the card moving
    /// from the grip to the heap, which is why 9.1.8b keeps the ability
    /// active THERE — and why a replacement that sends the card anywhere else
    /// leaves it inactive.
    SelfTrashedByDamage,
    /// "Whenever the Runner breaches this server…" (Ash class).
    ThisServerBreached,
    /// "Whenever you breach <this server>…" (Cupellation class — the server
    /// is named by the sentence), with 9.6.5c requirements riding along.
    ///
    /// `servers` is the list the sentence names — one for Cupellation's
    /// "breach R&D", two for Mercury's "breach **HQ or R&D**" — because a
    /// printed "or" between two servers is one description and must not
    /// become two abilities: an ordinal or a 9.3.6g flag stated once would
    /// then be spent twice.
    BreachesServer { servers: Vec<ServerId>, requires: Vec<TriggerRequirement> },
    /// CR 7.3.8: "when the current breach ends" — the condition the kernel
    /// gives the conditional ability a delayed breach is treated as.
    BreachEnds,
    /// CR 10.6.1: "whenever the Corp takes bad publicity…" (Raymond Flint
    /// class).
    PlayerTakesBadPublicity(Side),
    /// CR 8.1.2: "Whenever you rez a piece of ice…" (Lt. Todachine class) —
    /// the rez of a card matching what the sentence says about it.
    ///
    /// `of_types` is 2.15's stipulation ("a piece of ice") and `of_subtypes`
    /// is 2.16's ("an **advertisement**", Spark Agency), read the way every
    /// other condition reads the pair: a card has one type and any number of
    /// subtypes, so the types are "any of these" and the subtypes "all of
    /// these", and either list empty is a sentence making no such
    /// stipulation.
    ///
    /// `criteria` is what the sentence says about the card in the shared
    /// description vocabulary, for the stipulations the two lists cannot
    /// reach — "a piece of **AP** OR **destroyer** ice" (Thunderbolt
    /// Armaments) is a disjunction, and `of_subtypes` is a conjunction.
    /// `requires` is 9.6.5c's state stipulation ("…**during a run**").
    CorpRezzesCard {
        of_types: Vec<CardType>,
        of_subtypes: Vec<&'static str>,
        criteria: Vec<crate::instr::TargetFilter>,
        requires: Vec<TriggerRequirement>,
    },
    /// CR 10.1.2: "When the Corp purges virus counters…" (Clot class). The
    /// condition is met by the PURGE, not by any counter coming off, so it is
    /// met even when there was nothing to remove.
    CorpPurgesVirusCounters,
    /// "When this turn ends." (Joshua B. class delayed conditionals.)
    /// `requires` is 9.6.5c's additional stipulation — "…**if** you have more
    /// [haas-bioroid] cards rezzed than any other faction, when the Runner's
    /// turn ends" (Strategic Innovations).
    TurnEnds { side: Side, requires: Vec<TriggerRequirement> },
    /// CR 5.5.4 / 5.1.4b: "When a discard phase ends…" (Breaking News, The
    /// Class Act, Citadel Sanctuary). 5.1.4b is explicit that conditions
    /// related to a turn OR DISCARD PHASE ending are met at the same step —
    /// the formal end of that player's turn (5.6.3d / 5.7.2d) — so this is a
    /// distinct sentence met by the same occurrence, not a distinct moment.
    ///
    /// `side` is the sentence's stipulation about WHOSE discard phase, as
    /// content (§12 rule 2): `None` is a sentence naming no player ("when a
    /// discard phase ends" — Breaking News, The Class Act), `Some(s)` names
    /// one ("when YOUR discard phase ends" — Citadel Sanctuary). Both decks
    /// print both wordings, which is the whole reason the stipulation is a
    /// field rather than two conditions.
    DiscardPhaseEnds { side: Option<Side>, requires: Vec<TriggerRequirement> },
    /// CR 5.7.4 / 5.6.4: "Whenever you **discard cards to reach your maximum
    /// hand size**…" (Magdalene Keino-Chemutai). The DISCARD, not the phase
    /// around it: [`TriggerCond::DiscardPhaseEnds`] is 5.1.4b's formal end of
    /// the turn, which happens whether or not a single card was discarded and
    /// names none of them, while this condition is met by the discard itself
    /// and NAMES the cards it moved — which is what lets the sentence after it
    /// say "from among those cards" (1.15.4, read through
    /// [`crate::instr::TargetFilter::AmongTriggeringCards`]).
    ///
    /// `side` is the sentence's stipulation about WHOSE discard, exactly as it
    /// is on [`TriggerCond::PlayerDrawsCards`]. `to_hand_size` is the other
    /// stipulation, as content (§12 rule 2): `true` is the printed "to reach
    /// your maximum hand size", which is 5.7.4's step and nothing else, and
    /// `false` is a sentence naming any discard — a card ability's
    /// ([`crate::instr::Instruction::CorpDiscards`]) as much as the step's.
    ///
    /// Per-EVENT (9.12.2a via [`trigger_per_event`]): the printed noun is
    /// plural and the discard moves every card at once, so a player discarding
    /// three cards meets this once and the ability sees all three.
    PlayerDiscardsCards { side: Side, to_hand_size: bool },
    /// "Whenever you use a [trash] ability." (Geist-adjacent test class)
    /// "Whenever you use a [trash] ability…" (Geist class). `basic` is the
    /// sentence's stipulation about WHICH trash ability: `Some(false)` is the
    /// printed [trash] symbol (1.19.4's trigger cost), `Some(true)` is
    /// 7.1.5's basic trash ability, and `None` a sentence naming neither.
    /// Without it the two are indistinguishable and a Geist-class card fires
    /// on the Runner paying an accessed card's trash cost, which it does not
    /// print.
    UsesTrashAbility { side: Side, basic: Option<bool> },
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
    /// "…if <this card> is uninstalled" (DJ Fenris class; the reminder text
    /// of every Trojan says the same thing about its host). The move that
    /// meets it is the one that makes the card INACTIVE, so 9.1.8g is what
    /// keeps the ability active long enough to resolve — and an ability
    /// carried by a lingering effect (9.6.13) does not even need that, since
    /// 9.10.1 makes the effect independent of its source.
    SelfUninstalled,
    /// "When this card is added to your stack…" (Nanuq class). The move that
    /// meets it is the move that makes the card INACTIVE, which is why
    /// 9.1.8g has to keep the ability active long enough to resolve.
    SelfAddedToDeck,
    /// CR 9.9.6c: interrupt trigger — "…would pay a play or install cost".
    /// A cost that would be paid while resolving an effect is a value, so an
    /// interrupt can modify it; the relevance test is whether the imminent
    /// instruction carries such a value.
    WouldPayCost,
    /// CR 5.2.5b: "the first time each turn you take N DIFFERENT actions…"
    /// (MirrorMorph class). Met when the player takes an action and every
    /// action they have taken this turn — `count` of them — is different from
    /// every other, by 5.2.5a/b's identity: the same basic action, or the
    /// same ability of the same card.
    DifferentActionsThisTurn { side: Side, count: usize },
    /// CR 5.2.5b, the other half: "the first time you perform the SAME action
    /// three times in a row each turn" (The Collective). Met when the player
    /// takes an action and the last `count` actions taken this turn are all
    /// the same action by 5.2.5a/b's identity — the same basic action, or the
    /// same ability of the same card.
    ///
    /// "In a row" is what makes this a different condition rather than a
    /// polarity on [`TriggerCond::DifferentActionsThisTurn`]: that one asks
    /// about EVERY action of the turn, this one about the run of actions
    /// ending now, so a different action in between starts the count again.
    SameActionInARow { side: Side, count: usize },
    /// CR 1.16.4d: "the first time each turn you spend N [click] on the same
    /// action…" (Jeeves class). The clicks counted are all of the clicks
    /// spent to TAKE the action, including those of an additional cost paid
    /// several steps into the action's resolution.
    ClicksSpentOnAction { side: Side, count: u32 },
    /// CR 6.3.4: "whenever the Runner spends [click] during a run…"
    /// (Heinlein Grid class). The additional [click] an ability charges to
    /// MAKE a run is spent before the run formally begins, so it is not spent
    /// during the run and this condition is not met by it.
    /// `also_lost` is the printed "loses **or** spends [click]" (Seidr
    /// Laboratories): 5.2.1 distinguishes a click SPENT from one LOST, and a
    /// sentence naming both is one condition reaching both occurrences —
    /// exactly the shape [`TriggerCond::CardPlayed`]'s `also_installed` gives
    /// "plays or installs".
    PlayerSpendsClick { side: Side, during_run: bool, also_lost: bool },
    /// CR 9.12.2b: "whenever you gain credits…" (NASX class). One instance
    /// per OCCURRENCE (9.6.4b): an unaggregated group of effects gains the
    /// credits several times over, and this condition sees each of them.
    /// `criteria` is what the sentence says about the SOURCE the credits
    /// came through — "you gain credits through an ability on **an agenda or
    /// operation**" (The Zwicky Group) — asked in the shared filter
    /// vocabulary (§12 rule 5), exactly as [`TriggerCond::CardPlayed`] asks
    /// about the card played. An empty list is a sentence making no such
    /// stipulation ("whenever you gain credits", NASX), which the basic
    /// credit action meets as well as any card does.
    PlayerGainsCredits { side: Side, criteria: Vec<crate::instr::TargetFilter> },
    /// CR 10.11.5: "the first time each turn you make a successful run on
    /// your mark…" (Virtuoso class). 10.11.5: a condition checking a game
    /// property related to the mark only checks from the moment that server
    /// was designated, so an earlier successful run on the same server —
    /// before it was the mark — does not spend the "first time each turn".
    SuccessfulRunOnMark { first_each_turn: bool },
    /// CR 6.7.2: "whenever a run on this server is successful" (Ash class).
    /// Met when the run is DECLARED successful (6.9.5a), so the ability
    /// resolves in the reaction window that step's checkpoint opens — before
    /// the breach step where 6.7.4c puts the Runner's decision.
    SuccessfulRunOnServer,
    /// "Whenever you make a successful run" (Desperado class): the run is
    /// declared successful (6.8.4). `on` stipulates the servers the sentence
    /// names ("…a successful run on HQ or R&D" — Gemilang class); None = any.
    ///
    /// `after_subroutine_resolved` is "…a run becomes successful **after a
    /// subroutine resolved during that run**" (Ryō "Phoenix" Ōno) — a
    /// stipulation about the OCCURRENCE itself, read off
    /// [`crate::change::GameChange::RunDeclaredSuccessful`]'s recorded fact
    /// rather than off the state, because it is inside what a printed ordinal
    /// counts: 9.6.5c re-asks this whole condition of every earlier change in
    /// the turn, and a successful run with no subroutine resolved was never
    /// one of "the times", so it must not spend the ordinal — which only the
    /// record can still answer once that run's history window has closed.
    ///
    /// `requires` is 9.6.5c's stipulation about the STATE at the moment the
    /// run becomes successful ("…if you have at least N link" would be one).
    /// A requirement gates this occurrence firing and is deliberately OUTSIDE
    /// the ordinal's count — the Jamie "Bzzz" Micken parse, where a leading
    /// "if" clause does not change which occurrences are "the times".
    MakesSuccessfulRun {
        on: Option<Vec<crate::object::ServerId>>,
        after_subroutine_resolved: bool,
        requires: Vec<TriggerRequirement>,
    },
    /// CR 10.9.2: "when this card is empty…" (Crowdfunding class). The
    /// condition can only be met after the card has been LOADED with counters
    /// of this kind by a preceding ability of the same card — a card with no
    /// counters on it has not become empty, it was never loaded.
    SelfEmpty { kind: crate::object::CounterKind },
    /// CR 4.8.3: "whenever you install a program from your heap…" (Exile
    /// class) — a condition stipulating the zone the installed card came
    /// from. The set-aside zone is never that zone: 4.8.3 reports the
    /// location the card was in before it was set aside.
    ///
    /// `of_types` is the sentence's OTHER stipulation, about the installed
    /// card itself — "a **program** from your heap" — carried as content on
    /// the same atom (§12 rule 2), exactly as it is on
    /// [`TriggerCond::CorpRezzesCard`] and
    /// [`TriggerCond::RunnerAccessesCard`]. Empty is a sentence that names no
    /// type, and a card has exactly one type (2.15), so the list reads as
    /// "any of these".
    CardInstalledFrom { side: Side, from: Zone, of_types: Vec<CardType> },
    /// Interrupt trigger: "…would do damage" (ordinal: Some(1) = "the first
    /// time each run you would…", Tori Hanzō class).
    /// The printed ordinal — "the first time each RUN you would suffer net
    /// damage" — is [`AbilityDef::ordinal`], carrying its own span.
    WouldDamage { kind: Option<DamageKind> },
    /// CR 9.9.9c: interrupt trigger — "when the Runner would steal this
    /// agenda" (Project Vacheron class). Met by the expected effect of the
    /// access step that adds the agenda to the Runner's score area.
    WouldStealSelfAgenda,
    /// Interrupt trigger: "…would take tags during a run" (Jesminder class:
    /// `during_run` requires a run to be in progress).
    WouldTakeTags { during_run: bool },
    /// Interrupt trigger: "…if that run **would** be declared successful, …"
    /// — met by the expected effect of the Success Phase's own instruction
    /// (6.9.5a), which is the last moment at which the attacked server can
    /// still be changed and have the declaration follow it.
    ///
    /// It says nothing about WHICH run: an ability with this condition is
    /// carried by the run it belongs to
    /// ([`crate::instr::Instruction::InitiateRun::if_would_be_successful`]),
    /// exactly as 6.7.4's "If successful" one is, so there is nothing left
    /// for the condition itself to stipulate.
    WouldDeclareRunSuccessful,
    /// "Whenever the Corp installs a card in the root of this server…"
    /// (Tranquility Home Grid class; the 9.6.5b activity gate is the point).
    CardInstalledInSourceServer,
    /// "When you install that card, …" (Topan class) — met by an install an
    /// ability of THIS card performed, and by nothing else. The sentence
    /// follows an install instruction of the same printed ability, and its
    /// occurrence happens while that instruction is resolving — so a delayed
    /// conditional created after it would wait for the NEXT install (9.6.13),
    /// and a condition saying merely "you install a card" would be met by
    /// every install the player makes. What it must say instead is WHOSE
    /// ability made the install, which is
    /// [`crate::change::GameChange::CardInstalled::by_ability_of`]'s moment
    /// fact — compared against the source the way every other "this card"
    /// condition is.
    CardInstalledByAbilityOfSource,
    /// "When that encounter ends…" (Chum-class delayed conditionals).
    ///
    /// `criteria` is what the sentence says about the ice's own
    /// characteristics, asked in the shared description vocabulary — those
    /// travel with the card, so a later read still answers for the moment. An
    /// empty list is a sentence making no such stipulation.
    ///
    /// `with_advanced_ice` is "an encounter with an **advanced** piece of ice
    /// ends" (Weyland Consortium: Builder of Nations) — 1.18.2, but a fact
    /// about the MOMENT rather than a characteristic: the counters can move
    /// or leave with the ice before 9.6.5c's ordinal re-asks this condition
    /// of the occurrence, so it is read off
    /// [`crate::change::GameChange::EncounterEnded`]'s recorded fact and not
    /// off the board.
    EncounterEnds { criteria: Vec<crate::instr::TargetFilter>, with_advanced_ice: bool },
    /// "…if all of its subroutines were broken during that encounter"
    /// (Forked class). 9.12.2d vacuous truth: ice with ZERO subroutines
    /// satisfies this as soon as step 6.9.3b of the encounter begins.
    AllSubsBrokenOnEncounteredIce,
    /// CR 6.5.7a: "When the Runner fully breaks THIS ice…" (Paper Wall
    /// class) — the same occurrence as
    /// [`TriggerCond::AllSubsBrokenOnEncounteredIce`], scoped to the source
    /// the way every other "this card" condition is. 6.5.7c's vacuous case
    /// (ice with no subroutines) meets it too, and 6.5.7d means it is never
    /// retracted.
    SelfFullyBroken,
    /// "Whenever the Runner breaks a printed subroutine on this ice…"
    /// (Gold Farmer class.) Met once per subroutine, unlike
    /// [`TriggerCond::SelfFullyBroken`] which is met once per encounter.
    /// `printed_only` is the origin stipulation as content (§12 rule 2).
    SubroutineBrokenOnSelf { printed_only: bool },
    /// "Whenever the Runner steals an agenda…" (Bacterial Programming /
    /// Seidr class drivers for the 7.4.7a examples). `requires` is 9.6.5c's
    /// additional stipulation — "…**if** you have more [nbn] cards rezzed
    /// than any other faction" (Information Dynamics).
    RunnerStealsAgenda { requires: Vec<TriggerRequirement> },
    /// CR 1.17.6: "Whenever the Corp scores an agenda…" (Fan Site class) —
    /// the scoring twin of [`TriggerCond::RunnerStealsAgenda`], met "after
    /// the Corp moves the agenda from its current zone to their score area".
    /// 1.17.3e/f: a card ADDED to a score area is not scored, so it cannot
    /// meet this. `requires` is 9.6.5c's additional stipulation, exactly as
    /// on the steal twin.
    ///
    /// `criteria` is what the sentence says about the AGENDA — "an agenda
    /// **that you did not install or advance this turn**" (Issuaq Adaptics) —
    /// in the shared filter vocabulary (§12 rule 5), asked of the card the
    /// occurrence names. That is a different question from `requires`, which
    /// asks about the game state and knows nothing of which agenda it was.
    CorpScoresAgenda {
        requires: Vec<TriggerRequirement>,
        criteria: Vec<crate::instr::TargetFilter>,
    },
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
    /// "Whenever you install a card…" (Near-Earth Hub class), with whatever
    /// the sentence says about the card INSTALLED.
    ///
    /// `of_types` and `of_subtypes` are the 2.15/2.16 stipulations, read the
    /// way [`TriggerCond::CardPlayed`] reads its pair: a card has exactly one
    /// type and any number of subtypes, so the types are "any of these" and
    /// the subtypes "all of these", and either list empty is a sentence
    /// making no such stipulation. Noise's "whenever you install a **virus**
    /// program" is one condition with both; Engineering the Future's
    /// "whenever you install a card" is the same condition with neither.
    ///
    /// This is deliberately NOT [`TriggerCond::CardInstalledFrom`]: that
    /// condition insists on a zone, which a sentence saying only "whenever
    /// you install a virus program" does not name. Nor is it `CardPlayed`
    /// with `also_installed`, which is the sentence that names BOTH ways a
    /// card leaves a hand (8.5.1 / 8.6.1) and would fire on a play as well.
    ///
    /// `requires` is 9.6.5c's additional stipulation — "…**if** you have more
    /// [shaper] cards installed than any other faction, when you install a
    /// card" (Jamie "Bzzz" Micken).
    ///
    /// `into_remote_server` is CR 4.6.6b + 4.6.8: "…you install a card **in
    /// the root of or protecting a remote server**" (A Teia). The sentence
    /// stipulates nothing about the card and everything about WHERE it went,
    /// which is why it is not one of `of_types`/`of_subtypes` and not a
    /// description of the object either — 4.6.6b makes a server the cards in
    /// its root TOGETHER WITH the ice protecting it, so "in the root of or
    /// protecting" is one location and not two, and 4.6.8 is what makes
    /// "remote" a distinction it can draw. It is answered from
    /// [`crate::change::GameChange::CardInstalled::to`] rather than of the
    /// card's zone now, because 9.6.5c's ordinal scan asks this same condition
    /// of EARLIER changes and the card can have moved since. `false` is a
    /// sentence making no such stipulation.
    CardInstalledBy {
        side: Side,
        of_types: Vec<CardType>,
        of_subtypes: Vec<&'static str>,
        into_remote_server: bool,
        requires: Vec<TriggerRequirement>,
    },
    /// "Whenever you make a successful run on the chosen server…" (Security
    /// Testing class). CR 9.10.3b: the server is read from the maintained
    /// choice under `key`, so the condition is met only by a run on the
    /// server chosen for THIS turn — and never when no server was chosen.
    SuccessfulRunOnChosenServer { key: &'static str },
    /// "…passes a piece of ice." The pass happens at run step 6.9.4a
    /// (`rule_pass_ice`), and every stipulation a printed sentence makes about
    /// it is content on this one atom (§12 rule 2). All three `false` is a
    /// sentence making none of them — the plain "the first time you pass a
    /// piece of ice each turn" (Khan), which each of the stipulated readings
    /// says something more than.
    ///
    /// - `this_ice` — the sentence is printed on the ice and speaks about it
    ///   ("when the Runner passes THIS ice", Tatu-Bola class);
    /// - `fully_broken` — CR 6.1.3f's "a piece of ice you fully broke during
    ///   that encounter" (Inversificator class). The scope is the encounter
    ///   the pass DIRECTLY follows (6.1.3e), so breaking the same ice earlier
    ///   in the run does not satisfy it;
    /// - `subs_resolved` — CR 9.8.9's "if any of its subroutines resolved
    ///   during that encounter" (Persephone class). A subroutine resolved
    ///   through a 9.8.9 replacement still counts, because "the replaced
    ///   subroutine is treated as having the same source as the original
    ///   imminent subroutine".
    ///
    /// The two encounter-scoped stipulations each require the pass to follow
    /// an encounter at all; a pass with no encounter before it (a bypass)
    /// meets neither, and meets the plain sentence.
    ///
    /// `criteria` is what the sentence says about the ice's own
    /// characteristics — "a piece of **bioroid** ice" — asked in the shared
    /// description vocabulary, the way [`TriggerCond::CardPlayed`] asks about
    /// the card played; those travel with the card, so a later read still
    /// answers for the moment. An empty list is a sentence making no such
    /// stipulation.
    ///
    /// `rezzed` is "the Runner passes a **rezzed** piece of bioroid ice"
    /// (Haas-Bioroid: Architects of Tomorrow) — 8.1's status, but a fact
    /// about the MOMENT rather than a characteristic: a rez after the pass
    /// would change what 9.6.5c's ordinal sees when it re-asks this condition
    /// of the occurrence, so it is read off
    /// [`crate::change::GameChange::IcePassed`]'s recorded fact and not off
    /// the board.
    IcePassed {
        this_ice: bool,
        fully_broken: bool,
        subs_resolved: bool,
        rezzed: bool,
        criteria: Vec<crate::instr::TargetFilter>,
    },
    /// "After you resolve this operation/event…" (Oppo Research class). CR
    /// 8.6.7h: conditions related to finishing resolving a played card are
    /// met at that step, after the card has been trashed (8.6.7g) — which is
    /// why 9.1.8g keeps the ability active long enough to resolve.
    SelfPlayResolved,
    /// "Whenever **you** finish resolving **an operation**…" (Nuvem SA) — the
    /// same 8.6.7h moment [`TriggerCond::SelfPlayResolved`] reads, said by a
    /// card OTHER than the one being played. The self-view fixes the card by
    /// identity (1.15.4) and can say nothing else about it; this one names the
    /// player and DESCRIBES the card, which is why it is a second atom rather
    /// than content on the first.
    ///
    /// `by` is the player who played the card (8.6.2), read off the record
    /// rather than off the card: step (g) has already trashed the operation by
    /// the time this condition is scanned, and 1.14.1's owner is a different
    /// question. `criteria` is what the sentence says about the card, in the
    /// shared filter vocabulary (§12 rule 5); an empty list is a sentence
    /// making no such stipulation.
    CardPlayResolved { by: Side, criteria: Vec<crate::instr::TargetFilter> },
    /// "Whenever you finish resolving **an action on an expendable card**…"
    /// (Nuvem SA) — CR 5.2.2d's moment, when "conditions related to an action
    /// finishing" are met, which 5.2.2a puts at the end of the action step
    /// that ran the action.
    ///
    /// Deliberately NOT `GameChange::AbilityUsed`: 9.1.6 records that an
    /// ability was USED, which happens as the action is taken and its optional
    /// effects are carried out, and the sentence names the other end of it.
    ///
    /// `criteria` describes the CARD the action was an ability of — 5.2.4's
    /// "an ability that is an action" is a card ability, so the card is what a
    /// description can reach — asked in the shared filter vocabulary (§12
    /// rule 5). 5.2.6/5.2.7's basic actions are the game's and not any card's
    /// (9.1.3), so they match an empty list and nothing else.
    ActionCompleted { side: Side, criteria: Vec<crate::instr::TargetFilter> },
    /// "Whenever this card prevents 1 or more damage…" (Guru Davinder class,
    /// 9.9.7f). Met only when the imminent damage value was greater than 0
    /// before the interrupt from the SAME source decreased or removed it.
    SourcePreventedDamage,
    /// "Whenever a card is exposed…" (Blackguard class). CR 9.6.4b: exposing
    /// several cards in ONE instruction meets this condition once per card,
    /// because exposing is not one of 9.12.2c's aggregated effect classes.
    CardExposed,
    /// "Whenever an installed <side> card is trashed…" (District 99 /
    /// Wasteland class), "whenever **you** trash a piece of hardware **(from
    /// any location)**" (Hiram "0mission" Svensson). CR 8.2.2a: a trash that
    /// was PREVENTED never happened, so this condition is not met by it.
    ///
    /// Every stipulation a printed sentence can make about a trash is content
    /// on this one atom (§12 rule 2), and `None`/empty/`false` is a sentence
    /// making none of it:
    /// - `owner` — 1.14.1, whose card it was ("a card **you own**");
    /// - `by` — 1.14.5, who did the trashing ("**you** trash");
    /// - `of_types` — the description's card type ("a program or piece of
    ///   hardware");
    /// - `installed_only` — where it was trashed from. The printed "(from any
    ///   location)" is the parenthesis a card writes precisely because the
    ///   usual reading is the installed one;
    /// - `while_accessed` — CR 7.1.2's "a card you are accessing" (René
    ///   "Loup" Arcemont). It is not a zone: the card may be trashed out of
    ///   HQ, R&D, Archives or a server root, and what the sentence names is
    ///   the ACCESS it was being trashed inside of. `installed_only` is
    ///   therefore left `false` beside it, or the sentence would quietly say
    ///   something narrower than it prints.
    /// - `from_zone` — the ONE zone the sentence names ("a card **from R&D**",
    ///   Nuvem SA), read from the zone the card left. It is not a narrowing of
    ///   `installed_only`: 8.1's play area is many zones and a sentence saying
    ///   "installed" names all of them, while this one names a single zone and
    ///   says nothing about whether it is in play.
    /// - `requires` — 9.6.5c's state stipulation, which for this condition is
    ///   how a printed span narrower than the turn is said ("the first time
    ///   you trash a card from R&D during each of **your** turns"): the
    ///   ordinal counts within whatever turn is being played, so the sentence
    ///   also has to say whose turn it is.
    /// - `at_least_one` — 9.12.2a's plural noun ("you trash **1 or more
    ///   cards** from HQ", AU Co.), read by [`trigger_per_event`]: the
    ///   sentence speaks of the cards of one event together, so it is met
    ///   ONCE per occurrence group however many cards that event trashed —
    ///   the same reading [`TriggerCond::RunnerTrashesAtLeastOneCorpCard`]
    ///   has always had, carried here as content because the singular
    ///   sentence ("you trash **a card** from R&D", Nuvem SA) is met once per
    ///   card (9.6.4b) and only the noun's number tells them apart.
    CardTrashed {
        owner: Option<Side>,
        by: Option<Side>,
        of_types: Vec<CardType>,
        installed_only: bool,
        while_accessed: bool,
        from_zone: Option<Zone>,
        at_least_one: bool,
        requires: Vec<TriggerRequirement>,
    },
    /// "Whenever <side> spends 1 or more credits…" (GameNET class). CR
    /// 1.16.2b makes a calculated credit cost ONE payment, so this meets its
    /// condition once however many "for each" terms the calculation had.
    ///
    /// Every stipulation the printed sentence makes about the spending is
    /// content on this one atom (§12 rule 2), and the plain sentence is all
    /// three of them empty:
    /// - `also_lost` is the printed "spend **or lose**" (GameNET). 1.10.3b's
    ///   forced loss and 1.10.3c's payment are different movements of the
    ///   same credits, and a sentence naming both is one condition reaching
    ///   both occurrences — exactly the shape
    ///   [`TriggerCond::PlayerSpendsClick`]'s `also_lost` gives the clicks.
    /// - `caused_by` is what the sentence says CAUSED it — "whenever **a Corp
    ///   card ability** causes the Runner to spend or lose" — asked of the
    ///   source recorded on the change (9.1.4) in the shared filter
    ///   vocabulary (§12 rule 5). An empty list is a sentence naming no
    ///   cause, which a basic action's own cost meets as well as a card does.
    /// - `requires` is 9.6.5c's state stipulation ("…**during a run**").
    PlayerPaysCredits {
        side: Side,
        also_lost: bool,
        caused_by: Vec<crate::instr::TargetFilter>,
        requires: Vec<TriggerRequirement>,
    },
    /// CR 8.2.5: "whenever you forfeit an agenda…" (Jemison Astronautics).
    /// `by` is the sentence's stipulation about WHO forfeited — the printed
    /// "you" — carried as content the way every other condition carries it.
    ///
    /// Met by the forfeit itself (1.16.10b records it during the payment), so
    /// the agenda has already gone to the removed-from-game zone by the time
    /// the ability resolves. 1.15.4 still names it, which is what lets the
    /// next sentence read its printed agenda point value.
    AgendaForfeited { by: Side },
    /// CR 4.6.8d: "the first time each turn you create a remote server…"
    /// (Near-Earth Hub). Met by step 8.5.16e of the installation that puts
    /// the first card in a remote's root or protecting it — the moment the
    /// server comes into existence, which is before the "when installed"
    /// conditions of step (f).
    ///
    /// There is no central-server twin: 4.6.5's centrals exist for the whole
    /// game and are never created, so "a remote server" is the only thing
    /// this condition could name.
    RemoteServerCreated { by: Side },
    /// CR 1.21.3: "the first time each turn you reveal a card…" (Hyoubu
    /// Institute). `by` is the sentence's "you" — the player who revealed it,
    /// which 9.1.1a makes the ability's controller and not the card's owner,
    /// so revealing a card out of the opponent's hand meets it.
    ///
    /// 1.21.5 keeps this distinct from looking, exposing and accessing: a
    /// card LOOKED at is not revealed, and neither is one accessed.
    CardRevealed { by: Side },
    /// CR 10.5.1: "the first time each turn **a tag is removed**…" (Synapse
    /// Global). Met once per tag returned to the bank — an effect removing
    /// two tags meets it twice, exactly as
    /// [`TriggerCond::PlayerDrawsCards`] is met once per card drawn.
    ///
    /// The sentence names no player, and that is the whole of it: 10.5.4's
    /// basic action is the Runner removing their own tag, a card ability may
    /// have either player remove one, and this condition is met by all of
    /// them. It is the counterpart of [`TriggerCond::RunnerTakesTag`], which
    /// says the same thing about the tag arriving.
    TagRemoved,
    /// CR 10.14.6c: "Whenever you and the Runner **reveal secretly spent
    /// credits**, …" (Nisei Division). The reveal step of the construction
    /// 10.14.6 builds, which is one moment for both players — the sentence
    /// names them both and the reveal happens to both at once, so the
    /// condition is met once per psi game and says nothing about the amounts.
    ///
    /// Not the spending: 10.14.4a spends the credits immediately AFTER the
    /// reveal, and those are `CreditsLost` occurrences a
    /// [`TriggerCond::PlayerPaysCredits`] would read instead.
    SecretlySpentCreditsRevealed,
    /// CR 9.6.1a + 9.6.5c: "the first time each turn the Runner **steals or
    /// trashes** a Corp card" (Epiphany Analytica) — ONE condition describing
    /// two kinds of occurrence, met by either.
    ///
    /// The printed "or" between whole occurrences has no other way to be said.
    /// 9.6.1a gives an ability ONE primary condition, so a sentence with two
    /// of them is ordinarily written as two abilities (Leela Patel class) —
    /// which is right until the sentence also prints an ordinal, because
    /// [`AbilityDef::ordinal`] belongs to one ability and the pair would each
    /// spend their own and fire twice. One condition, one ordinal, one
    /// ability; the alternatives are content on it (§12 rule 2), exactly as
    /// [`crate::instr::TargetFilter::AnyOf`] is for a description.
    ///
    /// `requires` is 9.6.5c's additional stipulation about the game state,
    /// stated about the WHOLE condition because that is where a printed "if"
    /// clause sits. An alternative that carries requirements of its own is not
    /// readable here — a requirement is checked once, against the state, and
    /// there is nowhere to say which alternative it came with — so the
    /// alternatives are asserted to carry none.
    AnyOf { alternatives: Vec<TriggerCond>, requires: Vec<TriggerRequirement> },
}

impl TriggerCond {
    /// "When your turn begins, …" with no further stipulation — the plain
    /// sentence most cards print.
    pub fn turn_begins(side: Side) -> Self {
        TriggerCond::TurnBegins { side, requires: Vec::new() }
    }
    /// "Whenever the Runner encounters a piece of ice, …" — no stipulation
    /// about which ice, and none about the source.
    pub fn encounter_begins() -> Self {
        TriggerCond::EncounterBegins { of_subtypes: Vec::new(), requires: Vec::new() }
    }
}

/// Which turn a history question looks at. CR 10.2.1 makes the game history
/// open information, and 1.12.6's "this turn" and "their last turn" are the
/// two windows cards actually name — the window is content (§12 rule 2), not
/// a requirement of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnScope {
    /// Since the CURRENT turn began — the last `TurnBegan` of either side,
    /// which is how every other "this turn" requirement reads it.
    ThisTurn,
    /// The most recently COMPLETED turn of the side in question.
    LastCompletedTurn,
}

/// CR 1.16.4a: which of a card's INHERENT costs a modification reaches —
/// "the costs to install, rez, or play a card, as printed on the card". The
/// site is content on [`StaticDecl::InherentCostMod`] (§12 rule 2), not a
/// declaration per site: "costs 1[credit] less to install" and "costs
/// 1[credit] more to rez" are the same sentence about a different number.
///
/// 1.16.4a's third member, the play cost, has no variant because no card in
/// the decks states a modification of one; it is added when one does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InherentCost {
    /// CR 8.5.11: the install cost, paid at step 8.5.16d.
    Install,
    /// CR 8.1.2d: the rez cost, paid as part of the rez procedure.
    Rez,
}

/// Which agendas a modification of the advancement requirement reaches. The
/// reach is content (§12 rule 2), not a declaration of its own: SanSan City
/// Grid and The Source say the same thing about a different set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReqScope {
    /// "…agendas in this server" — the source's own server, exactly as
    /// `TargetFilter::IceProtectingSourceServer` scopes ice. A source that is
    /// not itself in a server reaches nothing.
    SourceServer,
    /// "…all agendas" (The Source) — every agenda in the game, wherever it
    /// sits, so an agenda still in HQ already has the raised requirement.
    AllAgendas,
}

/// CR 9.6.5c: an ADDITIONAL requirement listed inside a trigger condition
/// ("…if the Runner is tagged"). It is part of the condition, not of the
/// effect, so it must hold at the moment the condition would occur — and
/// 9.6.14d keeps it in force when an effect resolves the ability by class
/// instead of the stipulation actually occurring. Carried as data next to
/// the condition it qualifies, so the requirement is one vocabulary rather
/// than a `CondIfRunnerTagged` variant per condition (§12 rule 2).
// NOT `Copy`: `BoardHasMatching` carries criteria, so a requirement is as
// big as the description it holds. Requirements are read by reference
// everywhere (`state_requirement_holds_for(&self, req: &_)`), so this costs
// nothing at the call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerRequirement {
    /// "…if you have at least N link" (Underworld Contact class; 1.20).
    RunnerLinkAtLeast(u32),
    /// "…if there is an installed AI program" (IP Block class) — a question
    /// about the BOARD rather than about a player or the history: at least
    /// `at_least` cards match the criteria. The criteria are the shared
    /// filter vocabulary, so the card described is content (§12 rule 2).
    BoardHasMatching { criteria: Vec<crate::instr::TargetFilter>, at_least: u32 },
    /// "…if there are **no more** hosted cards" (Asmund Pudlat) — the same
    /// question with the threshold at the other end. A separate atom rather
    /// than a signed one for the same reason `RunnerTagsAtLeast` is: the
    /// direction is part of what the sentence says, and "at most 0" is the
    /// only way to say "none" with a count.
    BoardHasAtMostMatching { criteria: Vec<crate::instr::TargetFilter>, at_most: u32 },
    /// "…if you revealed **2 or more** cards that share a type" (Slot
    /// Machine) — a calculated amount (9.12.2) compared against a threshold.
    /// The amount is the shared quantity language (§12 rule 6) and the
    /// threshold is content (§12 rule 2), so a card asking the same question
    /// about a different number is the same atom.
    QuantityAtLeast { amount: crate::instr::Quantity, at_least: i64 },
    /// "…if you did **not** break any subroutines during that run" (Mercury)
    /// — the same comparison with the threshold at the other end, and a
    /// separate atom for the reason `BoardHasAtMostMatching` is: the
    /// direction is part of what the sentence says, and "at most 0" is the
    /// only way a count says "none".
    QuantityAtMost { amount: crate::instr::Quantity, at_most: i64 },
    /// "…if you have **the same number of** cards in your grip **as** the
    /// Corp has in HQ" (Lat: Ethical Freelancer) — two calculated amounts
    /// (9.12.2) compared against each other rather than against a printed
    /// number. A third atom beside `QuantityAtLeast` and `QuantityAtMost` for
    /// the reason those two are separate: the direction is part of what the
    /// sentence says, and equality is the direction this one says.
    ///
    /// Both sides are the shared quantity language (§12 rule 6), so nothing
    /// here knows about grips or HQs; a card comparing any two amounts it can
    /// name is this same atom.
    QuantitiesEqual { left: crate::instr::Quantity, right: crate::instr::Quantity },
    /// "…**during a run**" (Mercury) — CR 6.1.1: a run is in progress. Not a
    /// question about the run's server or its success, only that there is
    /// one; a breach can happen without a run (7.2), which is exactly the
    /// case this stipulation excludes.
    RunInProgress,
    /// "…if the Runner is tagged" (10.5: the Runner is tagged while they have
    /// 1 or more tags) and "…if the Runner has at least 2 tags" (BOOM!) — one
    /// predicate, the threshold as content (§12 rule 2). `RunnerTagsAtLeast(1)`
    /// IS "tagged".
    RunnerTagsAtLeast(u32),
    /// "…if the Runner made a run during their last turn" (Neural EMP), with
    /// `successful_only` "…made a successful run during their last turn"
    /// (SEA Source, Hard-Hitting News), and with `scope` "…if you made a
    /// successful run this turn" (Mutual Favor). The game history is public
    /// information (10.2.1), so this is read from the change log, exactly as
    /// 1.12.6's "ice you passed during this run" is. The success stipulation,
    /// the window and the POLARITY are all content, not separate atoms (§12
    /// rule 2): `made: false` is "…the Runner **did not initiate any runs**
    /// during their last turn" (Subliminal Messaging), the same question with
    /// the answer the sentence wants.
    ///
    /// `on` is the SERVER stipulation — "…whenever the Runner runs on a
    /// central server" (Jinteki: Replicating Perfection) — carried as content
    /// exactly as [`TriggerCond::RunEnds`] and [`TriggerCond::RunBegins`]
    /// carry theirs. An empty list is a sentence naming no server, which is
    /// every run.
    RunnerMadeRun {
        made: bool,
        successful_only: bool,
        scope: TurnScope,
        on: Vec<ServerId>,
    },
    /// "…if this is **not the first time** they have approached ice this run"
    /// (Mti Mwekundu). 1.12.6's "during this run" read the way `RunnerMadeRun`
    /// reads a turn's history — the change log from where the run began, which
    /// 10.2.1 makes open information — and counting 6.9.2's approaches, which
    /// [`crate::change::GameChange::IceApproached`] already records.
    ///
    /// The threshold is content (§12 rule 2): 1 is "they have approached ice
    /// this run already", which is what "not the first time" says about the
    /// approach an ability is about to cause, since the run's own step has not
    /// reached it yet. Outside a run nothing has been approached and the count
    /// is 0 — which is also 6.1.5b's case, a server with no ice protecting it.
    IceApproachesThisRunAtLeast(u32),
    /// "…if you played an operation this turn" (Nebula class) — the game
    /// history since the current turn began (1.12.6, 10.2.1).
    PlayedOperationThisTurn(Side),
    /// "…during each of **your** turns" (Nuvem SA) — CR 9.2.1: the named
    /// player is the ACTIVE player, the one whose turn is being played.
    ///
    /// A question about the present state and not about the history, which is
    /// what separates it from `PlayedOperationThisTurn` and every other
    /// "…this turn" requirement: those ask what happened inside the turn, this
    /// one asks whose turn it is. It is how a printed span narrower than
    /// [`OrdinalScope::Turn`] is said — the ordinal counts within whichever
    /// turn is being played, so a sentence scoped to one player's turns has to
    /// state that separately.
    ActiveTurnIs(Side),
    /// "…if you scored this agenda this turn" (Breaking News class) — the
    /// SOURCE was scored since the turn began (1.12.6 history).
    SelfScoredThisTurn,
    /// "…if you installed this resource this turn" (The Class Act class).
    SelfInstalledThisTurn,
    /// Zone stipulations on a trigger (9.6.5c class): "…accesses this ice IN
    /// R&D" (Archangel).
    SourceInDeck,
    /// "…anywhere except in Archives" (Archangel).
    SourceNotInDiscard,
    /// CR 5.2.2a/b: "…if you have **not finished an action** yet this turn"
    /// (Petty Cash). An action is finished when the game may advance past
    /// the action step that ran it (5.2.2a), which is where the change log
    /// records it; the threshold is content (§12 rule 2), so this one atom
    /// says "not yet" with 0 and any other count with a number.
    ActionsFinishedThisTurn { side: Side, at_most: u32 },
    /// CR 8.6.7a: "…if you played this operation **from anywhere except
    /// HQ**" (Petty Cash) — the zone the card was in when it was placed into
    /// the play area, asked of the play IN PROGRESS. Both the zone and the
    /// polarity are content (§12 rule 2): `is: false` is the printed
    /// "anywhere except". A source that is not being played meets neither
    /// reading.
    SourcePlayedFrom { from: Zone, is: bool },
    /// "…**if this card is in Archives**" (Subliminal Messaging) — the
    /// positive twin, and the stipulation 9.1.8b's first sentence reads: an
    /// ability stating that it is active in a particular zone is active in
    /// that zone, so this requirement both gates the condition and keeps the
    /// ability alive in the discard pile where it is the only thing that
    /// could ever meet it.
    SourceInDiscard,
    /// "…if this program has a hosted Corp card" (Cupellation class).
    SourceHostsCorpCard,
    /// "…if this program **can interface with the barrier you are
    /// encountering**" (Paperclip). CR 3.9.5g is the strength half — an
    /// interface ability is usable only while the icebreaker's strength is
    /// greater than or equal to the encountered ice's — and 3.9.5h is the
    /// subtype half, which is content here (`None` stipulates no subtype).
    ///
    /// Deliberately a REQUIREMENT rather than 9.3.6d's interface flag: the
    /// flag is checked when the ability is offered, and this sentence asks
    /// after "+X strength" has already resolved. Written as the flag, the
    /// card could never pump itself up to a barrier it did not already match.
    CanInterfaceWithEncounteredIce { required_subtype: Option<&'static str> },
    /// CR 6.9.2b: "…**after an approach during which that ice was rezzed**"
    /// (Nasir Meidan). A 9.6.5c requirement listed inside an encounter
    /// condition, asked of the approach the encounter directly follows: the
    /// Approach Ice Phase is where the Corp may rez the ice it opened over,
    /// so the question is whether THIS ice was rezzed between the approach
    /// and the encounter.
    ///
    /// It is answered from the change log, which 10.2.1 makes open
    /// information, and not from the ice's faceup state: an ice rezzed on an
    /// EARLIER approach this run is faceup now and was not rezzed during the
    /// approach that just ended, and the sentence must leave it alone.
    EncounteredIceRezzedDuringApproach,
    /// CR 1.17.1: "…if the Runner has 3 or more agenda points" (Complete
    /// Image). The score of the named player as 9.12.1a computes it, so an
    /// agenda whose point value an active ability is modifying counts as
    /// modified. Not the threat level (1.17.1a), which is the HIGHER of the
    /// two scores and says nothing about whose.
    AgendaPointsAtLeast { side: Side, points: i32 },
    /// CR 1.17.1: "…if the Corp has MORE scored agenda points than you" (Iain
    /// Stirling). A comparison between the two score areas, not a threshold
    /// against a printed number — which is what
    /// [`TriggerRequirement::AgendaPointsAtLeast`] is and why it cannot say
    /// this. `side` is the player the sentence puts AHEAD.
    AgendaPointsAhead { side: Side },
    /// CR 1.15.4: "…**if the exposed card** has the named card type"
    /// (Falsified Credentials), "…add it to HQ **if it** has the named
    /// subtype" (Wari). A question about a target this ability already
    /// announced — `nth` is 0-based over the ability's announcements, the
    /// same index [`crate::instr::TargetSpec::EarlierTarget`] uses — asked
    /// with the shared filter vocabulary, so what is asked about it is
    /// content (§12 rule 2). Never met when the ability announced no such
    /// target (1.15.3).
    EarlierTargetMatches { nth: usize, criteria: Vec<crate::instr::TargetFilter> },
    /// CR 2.13: "…if you have more **[criminal]** cards installed than any
    /// other faction" / "…more **[nbn]** cards rezzed than any other faction"
    /// — the clause every draft-format identity opens with. A comparison
    /// across the FACTION PARTITION of a described set of cards, which is why
    /// [`TriggerRequirement::BoardHasMatching`] cannot say it: that one
    /// measures one description against a printed number, and this sentence
    /// prints no number at all — it measures one faction's share of the
    /// described cards against every other faction's.
    ///
    /// `criteria` is the described set in the shared filter vocabulary (§12
    /// rule 5), so WHICH cards are partitioned is content exactly as it is
    /// for `BoardHasMatching`: "cards installed" is the play area, "cards
    /// rezzed" adds 8.1.2's faceup stipulation, and a sentence about some
    /// other set is this same atom.
    ///
    /// 2.13.3 gives every card a faction and 2.13.2's neutral is one of them,
    /// so a neutral card joins the neutral group rather than no group; a card
    /// printing no faction at all is in no group and is not counted.
    ///
    /// "MORE than any other" is strict — a tie with any other faction does
    /// not meet it, and neither does an empty board.
    LargestFactionGroupIs { faction: &'static str, criteria: Vec<crate::instr::TargetFilter> },
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
    /// CR 4.5 / 9.3.7a: "…while this card is in <side>'s score area" (Merger,
    /// Global Food Initiative). A stated condition on a static ability whose
    /// source is in a score area — 9.1.8a keeps such a card active in BOTH
    /// score areas, so the side is the whole content of the restriction.
    SourceInScoreAreaOf(Side),
    /// CR 9.3.7a: "**While the Runner is tagged**, …" (Harishchandra Ent.) —
    /// a stated condition that is a question about the GAME STATE rather than
    /// about the source. The question is asked in the shared requirement
    /// vocabulary (§12 rule 5), the same one 9.6.5c's `requires` uses on a
    /// trigger condition, so a static ability and a conditional one say
    /// "while the Runner is tagged" with the same words.
    StateRequirement(Vec<TriggerRequirement>),
}

/// WHOSE the declaration speaks about, when a card can say either. "Your
/// maximum hand size" and "each player's maximum hand size" are one sentence
/// pattern with a scope, exactly as [`ReqScope`] is for an advancement
/// requirement — content on the declaration, never a declaration per scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclSubject {
    /// "**Your** maximum hand size…" — the controller of the source (9.1.1a).
    Controller,
    /// "**Each player's** maximum hand size…" — both players.
    EachPlayer,
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
    /// CR 10.5.1 / 1.16.1: "[click], **remove 1 tag**:" as a cost (Synapse
    /// Global). The mirror of `tags`, and a separate component for the reason
    /// `RemoveTags` is a separate instruction from `GainTags`: the tags move
    /// the other way, so what makes the cost unpayable is the Runner having
    /// too FEW of them (1.16.1b), not an interrupt that would stop them
    /// arriving.
    ///
    /// The tags are always the Runner's (10.5.1), whoever is paying — a Corp
    /// card printing this cost spends the Runner's tags, which is what makes
    /// the ability cost the Corp something real.
    pub remove_tags: u32,
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
    /// "Remove <this card> from the game:" as a trigger cost (Jackson class;
    /// 1.16.1 — the payment moves the source to the removed-from-game zone).
    pub remove_self_from_game: bool,
    /// "…trash all cards from your grip:" as a trigger cost (Citadel
    /// Sanctuary class) — however many that is, including zero (1.16.2b's
    /// calculated cost is payable at any value).
    pub trash_all_from_hand: bool,
    /// CR 1.9.2: "spend N <kind> counters hosted on this card" (Imp class).
    /// The counters come off the ability's SOURCE, which is what makes an
    /// empty card's ability unusable rather than free.
    pub spend_counters: Option<(crate::object::CounterKind, u32)>,
    /// CR 8.2.5 / 4.9.3: "forfeit an agenda" as a cost (24/7 News Cycle
    /// class) — N agendas move from the payer's score area to the
    /// removed-from-game zone, their agenda points stop counting, and
    /// anything hosted on them is trashed.
    ///
    /// Which agenda is forfeited is the payer's choice, made while the
    /// payment gathers its choices (W13a); it is only elided where the score
    /// area holds exactly as many agendas as the cost forfeits.
    pub forfeit_agenda: u32,
    /// CR 1.16.10: "trash 1 of your other installed cards" as a cost — N
    /// cards matching criteria, CHOSEN by the payer. 1.16.1c filters the
    /// choice: a card whose being spent would leave a restriction on the
    /// effect being paid for unmet is not offered.
    pub trash_matching: Option<(u32, Vec<crate::instr::TargetFilter>)>,
    /// CR 1.16.2c: this cost contains the variable X, and the payer announces
    /// a value for it BEFORE paying. The quantity is the restriction the
    /// ability states on that value ("X must be equal to or less than the
    /// number of tags the Runner has"); the announced value is read back by
    /// [`crate::instr::Quantity::AnnouncedX`]. 1.16.2d: outside a payment,
    /// `AnnouncedX` is 0.
    pub x_restriction: Option<crate::instr::Quantity>,
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
    /// CR 10.5.1: "remove N tags" as a cost.
    pub fn remove_tags(n: u32) -> Self {
        Cost { remove_tags: n, ..Default::default() }
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
    /// CR 1.16.10: "trash N of your installed cards matching …" as a cost.
    pub fn trash_matching(n: u32, criteria: Vec<crate::instr::TargetFilter>) -> Self {
        Cost { trash_matching: Some((n, criteria)), ..Default::default() }
    }
    /// CR 1.16.2c: a cost of X, with the restriction the ability states on
    /// the value the payer may announce.
    pub fn x(restriction: crate::instr::Quantity) -> Self {
        Cost {
            credits: crate::instr::Quantity::AnnouncedX,
            x_restriction: Some(restriction),
            ..Default::default()
        }
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
            remove_tags: self.remove_tags + other.remove_tags,
            net_damage: self.net_damage + other.net_damage,
            lose_clicks: self.lose_clicks + other.lose_clicks,
            trash_from_hand: self.trash_from_hand + other.trash_from_hand,
            remove_self_from_game: self.remove_self_from_game || other.remove_self_from_game,
            trash_all_from_hand: self.trash_all_from_hand || other.trash_all_from_hand,
            spend_counters: self.spend_counters.or(other.spend_counters),
            forfeit_agenda: self.forfeit_agenda + other.forfeit_agenda,
            trash_matching: self.trash_matching.clone().or_else(|| other.trash_matching.clone()),
            x_restriction: self.x_restriction.clone().or_else(|| other.x_restriction.clone()),
        }
    }
}

/// CR 9.5.6 + 9.3.3c: restrictions on WHEN and WHERE a paid ability can be
/// used. 9.5.6's are effect-based (an ability that could break subroutines is
/// an encounter ability whatever else it says); 9.3.3c's are stated ("Play
/// this operation **from Archives**").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingRestriction {
    /// 9.5.6a/c: usable only during an encounter — and, where the ability
    /// refers to the encountered ice with a stipulation ("this code gate"),
    /// only during an encounter with a piece of ice that meets it.
    ///
    /// `required_choice` is 9.10.3's back-reference as the same kind of
    /// stipulation: "use this hardware only during encounters with THAT ICE"
    /// (Boomerang) names the ice its source is maintaining a choice of, so
    /// the ability is usable only while the ice being encountered is that
    /// one. Both stipulations are content on this one atom (§12 rule 2), and
    /// `None` in each is a sentence that makes no such stipulation.
    EncounterOnly {
        required_subtype: Option<&'static str>,
        required_choice: Option<&'static str>,
    },
    /// 9.5.6b: usable only during the Approach Ice Phase, with the
    /// approached ice matching all stipulations used in referring to it.
    ApproachOnly { required_subtype: Option<&'static str>, rezzed: bool },
    /// 9.3.3c: "Limits on when, WHERE, or how often an ability can be used
    /// are restrictions." An ability stating the zone it works from — "Play
    /// this operation **from Archives**" (Petty Cash) — can only be used
    /// while its source is in that zone. Distinct from 9.1.8b's activity
    /// question: 9.1.8c already keeps a source-playing ability active while
    /// its source is inactive, and this is what stops the same ability being
    /// used from a hand.
    SourceInZone(Zone),
    /// CR 9.3.3c + 6.1: "**Use this ability only during a run.**" (Arissana
    /// Rocha Nahu.) A limit on WHEN, stated about the run structure itself
    /// rather than about one of its phases — the sibling of
    /// [`TimingRestriction::EncounterOnly`] and
    /// [`TimingRestriction::ApproachOnly`], which name a phase inside the run
    /// and are therefore narrower than this. 6.1.1: a run is in progress from
    /// its initiation until the Run Ends Phase completes, so the whole of that
    /// span qualifies and nothing outside it does.
    RunOnly,
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
    /// CR 9.9.8b / 8.2.2: a static ability stipulating a REPLACEMENT of where
    /// a trashed card goes — "instead of adding it to the heap, remove it
    /// from the game" (Skorpios class), "if this card would be trashed,
    /// instead turn it facedown" (Harbinger class). 8.2.2 is the point: the
    /// modified effect "is still an occurrence of that movement and can still
    /// meet trigger conditions relating to that type of movement", so the
    /// trash is still recorded and only its destination changes.
    ReplaceTrashDestination {
        criteria: Vec<crate::instr::TargetFilter>,
        to: crate::instr::TrashDestination,
    },
    /// CR 9.8.9 / 9.9.8b: while this static ability is active, an imminent
    /// subroutine is replaced by the stated one (Tsakhia "Bankhar" Gantulga
    /// class). "The replaced subroutine is treated as having the same source
    /// as the original imminent subroutine", so it still resolves FROM the
    /// ice — which is what a Persephone-class condition asks about.
    ReplaceSubroutineResolution { instead: Vec<crate::instr::Instruction> },
    /// Characteristic modification of the source's host (Hush class) or self.
    StrengthMod { target_self: bool, delta: i32 },
    /// CR 2.5 through the 9.12.1a pipeline: "**all <description>** has +N
    /// strength." (Stronger Together class.) The same characteristic
    /// modification `StrengthMod` states about one card the source is related
    /// to, stated instead about the cards a DESCRIPTION reaches — so the
    /// description is the content (§12 rule 2) and the whole class is one
    /// declaration, the way `RemoveAbilitiesOfMatching` is one beside
    /// `RemoveAbilitiesOf`.
    ///
    /// The criteria are read shallowly (deviation 47's class): this
    /// declaration is gathered inside `char_effects`, which is the pipeline a
    /// deep `HasSubtype` would ask, so a subtype criterion here reads PRINTED
    /// subtypes and a piece of ice that merely gained "bioroid" is not
    /// described by it.
    StrengthModMatching { criteria: Vec<crate::instr::TargetFilter>, delta: i32 },
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
    /// CR 9.1.9b: "<this card> gains the text of <the described cards>."
    /// (DJ Fenris class.) The abilities gained are the described cards'
    /// EFFECTIVE abilities, read through the same 9.12.1d/e pipeline as
    /// `GainSubtypesOf`'s subtypes — so a card that itself gained an ability
    /// passes it on, and a card that lost one passes on nothing. The gaining
    /// object is the SOURCE of every ability it gains (9.1.1b), so "this
    /// card" inside a gained ability means the gainer.
    GainAbilitiesOf { criteria: Vec<crate::instr::TargetFilter> },
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
    /// CR 5.7.3: maximum hand size modifier. "Your maximum hand size is
    /// increased by 1" (NBN: The World is Yours*) and "each player's maximum
    /// hand size is reduced by 1" (Cybernetics Division) are the SAME
    /// declaration: the amount carries the polarity and `whose` carries the
    /// scope, so neither is a variant of its own (§12 rule 1). "Your" is read
    /// against the source's controller, so a Runner card saying it means the
    /// Runner's.
    MaxHandSizeMod { whose: DeclSubject, amount: i32 },
    /// CR 9.12.1a: "your maximum hand size **is equal to** the number of
    /// credits in your credit pool" (Cerebral Imaging) — the effect that SETS
    /// the value, applied before every effect that increases or lowers it.
    /// It carries a 9.12.2 quantity rather than a printed number, which is
    /// the whole reason it is not [`StaticDecl::MaxHandSizeMod`] with a
    /// different sign, and `whose` is the same scope word.
    ///
    /// Two of these at once is 9.12.1a's own case: both are applied, so the
    /// last one read wins and neither is a modification of the other.
    MaxHandSizeIs { whose: DeclSubject, to: crate::instr::Quantity },
    /// "+N to the amount of <kind> damage done by <responsible>."
    /// (The Cleaners class — modifies imminent damage values via statics.)
    DamageBonus { kind: DamageKind, responsible: Side, amount: i64 },
    /// Additional cost to steal agendas (Ben Musashi / Predictive Algorithm
    /// class; 1.16.10).
    AdditionalStealCost(Cost),
    /// CR 1.16.2e: "You can [instead] as you [use this card] to pay for
    /// N[credit] of its cost." An alternate payment does NOT change the value
    /// of the cost — it gives the payer one more OPTION when deciding how to
    /// pay it, covering `covers` credits of whatever cost is being paid FOR
    /// THIS SOURCE in exchange for `instead`.
    AlternatePaymentForSelf { label: &'static str, covers: u32, instead: Cost },
    /// CR 1.16.10 / 6.3.4: "The Runner must pay [cost] as an additional cost
    /// to make a run." (Service Outage / Enhanced Login Protocol class.) It
    /// is an additional cost to the basic run ACTION, paid to initiate the
    /// run — 6.3.4: the run formally begins only after the attacked server is
    /// announced and any costs are paid, so nothing paid here is paid
    /// "during a run".
    ///
    /// `on` is what the printed sentence says about the server: "…to make a
    /// run" reaches every server ([`crate::instr::RunServerSet::Any`]), "…to
    /// run HQ" names one (Earth Station: SEA Headquarters), and "…to run **a
    /// remote server**" names 4.6.8's class, a set the game state computes
    /// (Earth Station: Ascending to Orbit). One stipulation on this one
    /// declaration (§12 rule 2), read at 6.9.1a where the attacked server is
    /// already announced — which is why the cost can be asked per server at
    /// all.
    AdditionalRunActionCost { cost: Cost, on: crate::instr::RunServerSet },
    /// CR 9.12.3a/e: "You must make a run with your first [click] each turn."
    /// (Always Be Running class.) A requirement on the action window, not an
    /// effect: while it holds, the only actions offered are runs. 9.12.3e:
    /// declining the additional cost of a run SATISFIES the requirement, so
    /// the "must" cannot force the player to pay it.
    MustRunWithFirstClick(Side),
    /// CR 10.4.3a: a declaration modifying the damage procedure so that the
    /// named player SELECTS up to `count` of the cards trashed, instead of
    /// their being chosen at random. The cards are still trashed
    /// simultaneously (10.4.3); only the selection is sequential.
    ///
    /// CR 9.12.1c: when both players' effects make this declaration, the
    /// choice can only be made once, so the ACTIVE player makes it — and the
    /// rest of each ability still resolves.
    ///
    /// Every stipulation the printed sentence makes about WHICH damage the
    /// declaration is stated about, and about how it is offered, is content on
    /// this one atom (§12 rule 2). The plain sentence ("you choose the cards
    /// you trash to damage" — Titanium Ribs) is all three of them empty:
    /// - `kinds` is the damage type the sentence names ("the first **net**
    ///   damage"). 10.4.2 lists three of them and an empty list names none, so
    ///   it reaches every kind, which is what a sentence saying plain "damage"
    ///   means.
    /// - `first_each_turn` is the printed ordinal, read from the change log
    ///   (10.2.1 open information) exactly as
    ///   [`StaticDecl::InherentCostMod`]'s is: the declaration reaches this
    ///   damage only while no earlier `DamageSuffered` this turn matched
    ///   `kinds`. It is NOT 9.3.6g's once-per-turn flag — 9.4.1 says a static
    ///   ability never resolves, so it never spends one.
    /// - `optional` is the printed "**you may** look at the Runner's grip and
    ///   select the card that is trashed" (Chronos Protocol: Selective
    ///   Mind-mapping). The "may" governs the looking as well as the
    ///   selecting, so it is asked BEFORE the candidates are named: offering
    ///   the grip as candidates and letting the player pick none would show
    ///   them a grip the card only lets them see when they use the ability.
    SelectsDamageTrashes {
        by: Side,
        count: crate::instr::Quantity,
        kinds: Vec<crate::effects::DamageKind>,
        first_each_turn: bool,
        optional: bool,
    },
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
    /// "You cannot install <description>." (Apex; CR 1.2.2.) A prohibition
    /// stated by the declaring player ABOUT THEIR OWN installs — "you" — so
    /// it describes cards and says nothing about which ability would install
    /// them: 1.2.2's "cannot" takes precedence over anything that directs one,
    /// including 5.2.7d's basic action.
    ///
    /// Deliberately not a variant of [`StaticDecl::InstallOnlyHostedOn`],
    /// which is 1.13.6c's restriction printed on the card being installed and
    /// describes DESTINATIONS. This one is printed on a third card and
    /// describes the installee, so it is the shared filter vocabulary
    /// (§12 rule 5) and nothing else.
    CannotInstallMatching { criteria: Vec<crate::instr::TargetFilter> },
    /// "+N link" (Dyson Mem Chip class; the 9.6.5d link example).
    LinkBonus(i32),
    /// "This card is not trashed until another current is played or an agenda
    /// is <scored|stolen>." (The current class — 8.6.6c: instead of trashing
    /// at 8.6.7g, a lingering effect keeps the card in the play area until
    /// the indicated effect occurs.)
    ///
    /// CR 3.5.1b and 3.7.1b print the two halves of the same sentence with
    /// one word different — a current OPERATION lasts until the Runner
    /// steals an agenda, a current EVENT until the Corp scores one — so the
    /// indicated effect is content on this one declaration (§12 rule 2)
    /// rather than a declaration per side. The occurrences are stated in the
    /// vocabulary that already names occurrences, so "another current
    /// operation or event is played" needs nothing of its own.
    PlayedNotTrashedUntil { until: Vec<TriggerCond> },
    /// "As an additional cost to access a card in the root of a remote
    /// server, pay N." (Gagarin class — 7.4.3 example 2.)
    AdditionalAccessCost(Cost),
    /// "You may pay <cost> to lower the install cost of a card you are
    /// installing by N." (Patchwork class; 1.16.6 install costs.) The
    /// reduction is only available while its own cost is payable, which is
    /// exactly what makes it part of 8.7.2b's affordability query.
    InstallDiscount { cost: Cost, amount: u32 },
    /// CR 1.16.2a / 1.16.4a: "<the described cards> cost N more/less to
    /// <install|rez>." (Kate "Mac" McCaffrey, Az McCaffrey, Reina Roja.) An
    /// AUTOMATIC modification of an inherent cost, applied of its own accord
    /// wherever that cost is calculated — contrast
    /// [`StaticDecl::InstallDiscount`], which is Patchwork's: a reduction the
    /// installer may PAY for and must therefore choose to use.
    ///
    /// `which` is 1.16.4a's cost site. The three inherent costs are one list
    /// in the rules and a sentence modifying one is written exactly like a
    /// sentence modifying another, so the site is content on this one
    /// declaration (§12 rule 2) rather than a declaration each.
    ///
    /// `criteria` is what the sentence says about the cards, in the shared
    /// filter vocabulary (§12 rule 5); `amount` carries the polarity, and
    /// 1.16.2a lowers before flooring at 0.
    ///
    /// `first_each_turn` is the printed ordinal — "**the first** piece of ice
    /// the Corp rezzes **each turn**", "**the first** program or piece of
    /// hardware you install **each turn**". It describes the occurrence by
    /// its position in the turn, exactly as 9.6.5c's ordinal does for a
    /// trigger condition, and is read from the change log (10.2.1 makes the
    /// history open information): the modification applies while no earlier
    /// matching install or rez has happened this turn. One stipulation on one
    /// declaration, never a declaration per ordinal (§12 rule 1).
    ///
    /// The sentence's "**you** install" needs no field of its own: 2.15
    /// partitions the card types by side, so a description naming programs,
    /// hardware and resources names Runner cards only, and only the Corp ever
    /// rezzes anything at all (8.1.4f).
    InherentCostMod {
        which: InherentCost,
        criteria: Vec<crate::instr::TargetFilter>,
        amount: i32,
        first_each_turn: bool,
    },
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
    ScoreRequirementMod { scope: ReqScope, amount: i32 },
    /// CR 4.6.8f: "Limit N remote servers." (Earth Station class.) While
    /// active, the Corp cannot create a new remote server that would take the
    /// total above N.
    RemoteServerLimit(u32),
    /// CR 6.3.2a: "The Runner cannot initiate a run on this server."
    /// (Off the Grid class.) The declaration refers to the ANNOUNCEMENT of
    /// the attacked server at step 6.9.1a and to nothing else — an ability
    /// that changes the attacked server mid-run (6.1.2d) is not affected.
    CannotInitiateRunOnSourceServer,
    /// CR 1.18.3: "You can advance this ice" / "this card can be advanced"
    /// (Ice Wall class). Agendas can always be advanced; every other card can
    /// be advanced only while an ability says so. 9.1.8f makes this class of
    /// static ability active even while the card is INACTIVE — an unrezzed
    /// Ice Wall can still be advanced, which is the whole point of the rule.
    CanBeAdvancedSelf,
    /// CR 9.1.8c: "Play only if <state>." — a static ability that modifies
    /// WHEN OR IF its source card can be played, so it is active while the
    /// card is inactive (in HQ or the grip, the only place it could ever
    /// matter). Every requirement must hold or the card is not a legal play:
    /// the basic play action does not offer it (5.2.6e/5.2.7e) and an effect
    /// that would play a card cannot choose it (8.6.3).
    PlayOnlyIf(Vec<TriggerRequirement>),
    /// CR 2.5 / 9.12.1a: "This agenda is worth N more agenda points."
    /// (Project Beale's second sentence; Merger's and Global Food
    /// Initiative's whole text, with a `StaticCond::SourceInScoreAreaOf`
    /// stating when.) The amount is a quantity position (§12 rule 6), so
    /// "1 more for each hosted agenda counter" is the same declaration as a
    /// flat "1 more", and "1 fewer" is a negative quantity
    /// (`Quantity::Minus`).
    SelfAgendaPointsMod(crate::instr::Quantity),
    /// CR 1.7.2a / 1.17.2: "Each player needs 1 fewer agenda point to win the
    /// game." (Harmony Medtech.) "For each hosted power counter, you need 1
    /// less agenda point to win the game." (Issuaq Adaptics.)
    ///
    /// 1.17.2 states the win as a comparison — "if at any time a player's
    /// score is greater than or equal to 7, they win the game at the next
    /// checkpoint" — and this declaration modifies the number on the other
    /// side of it. Not the score: a player who needs 6 has not gained a point,
    /// and every ability that reads a score (1.17.1a's threat level, an
    /// "agenda points at least" requirement) still reads the real one.
    ///
    /// `whose` is the same scope word [`StaticDecl::MaxHandSizeMod`] carries —
    /// "each player" against "you" — and `amount` is a quantity position (§12
    /// rule 6) carrying the polarity, so "1 fewer" is a flat negative and "1
    /// less for each hosted power counter" is the same declaration with a
    /// calculated one. Nothing floors the result: the requirement is the
    /// printed 7 as these declarations leave it, and a card that took it to
    /// zero would say exactly that.
    AgendaPointsToWinMod { whose: DeclSubject, amount: crate::instr::Quantity },
    /// CR 1.17.3 / 9.12.4: "The Corp cannot score <the described agendas>."
    /// (Clot's first sentence, scoped by "during the same turn they installed
    /// that agenda".) A prohibition on the (S) OPTION rather than on an
    /// ability — 9.1.9's restrictions reach abilities, and scoring is not one
    /// (1.17.3c) — so it is applied where the option is offered, in the paid
    /// windows of the Corp's turn (9.2.7d). The described set is the shared
    /// criteria vocabulary (§12 rule 2).
    CannotScoreMatching { criteria: Vec<crate::instr::TargetFilter> },
    /// CR 4.3.2 / 10.2.2: "they play with the grip revealed" (Harishchandra
    /// Ent.) — the named player's HAND stops being hidden information, so
    /// 4.3.2's "not at any of the cards in their opponent's hands" no longer
    /// applies to it. Whose hand is content (§12 rule 2); nothing else about
    /// the zone changes, and the cards are not revealed one at a time, so no
    /// [`crate::change::GameChange::CardRevealed`] is ever recorded by it.
    HandRevealed { whose: Side },
    /// CR 4.4.2: "**You must maintain the order of your heap.**" (Wyvern.)
    /// 4.4.2 is that discard piles are NOT ordered — "a player may freely
    /// arrange the cards in their discard pile in any order at any time" — so
    /// this sentence is a declaration that takes that freedom away and makes
    /// one player's discard pile an ordered zone. Whose is content (§12 rule
    /// 2); a Corp card printing it about Archives is the same declaration.
    ///
    /// Its reader is [`crate::instr::TargetSpec::TopOfDiscard`]: "the top card
    /// of your heap" describes nothing at all while 4.4.2 stands, because an
    /// unordered pile has no top, and this is what gives it one.
    DiscardPileIsOrdered { whose: Side },
    /// CR 1.17.7: "the Runner cannot steal more than one agenda each turn"
    /// (Haarpsichord Studios). The count is taken from the turn's history
    /// (1.12.6, 10.2.1) and the threshold is content (§12 rule 2), so a card
    /// saying "more than two" is this same declaration.
    ///
    /// 1.2.2: "cannot" is absolute. Once the limit is reached the agenda is
    /// simply not stolen — the access carries on and nothing is put to the
    /// Runner, exactly as a Pinhole-class access restriction leaves it.
    StealsPerTurnAtMost(u32),
    /// CR 7.1.5a: "the trash cost of each card is increased by 1 for each
    /// facedown card in Archives" (Industrial Genomics) — a modification of
    /// the trash cost of the cards a description names.
    ///
    /// `criteria` is that description in the shared filter vocabulary (§12
    /// rule 5); an empty list is the printed "each card", which stipulates
    /// nothing. `amount` is a calculated quantity (9.12.2), so it is
    /// re-evaluated every time the cost is read — a card entering Archives
    /// raises every trash cost on the board at once.
    TrashCostMod { criteria: Vec<crate::instr::TargetFilter>, amount: crate::instr::Quantity },
    /// CR 6.3.2a: "the Runner cannot run on remote servers" (Jinteki:
    /// Replicating Perfection) — a prohibition on ANNOUNCING one of the named
    /// servers as the attacked server.
    ///
    /// The general form of [`StaticDecl::CannotInitiateRunOnSourceServer`]:
    /// that one is an Off-the-Grid-class sentence about the source's own
    /// server, this one is a sentence that names the servers itself. Like the
    /// other, it reaches no further than the announcement — a run already in
    /// progress can still be moved onto such a server (6.1.2d).
    CannotInitiateRunOn(crate::instr::RunServerSet),
}

/// A **citation anchor**: CR §1.16's cost taxonomy and §9.6's conditional
/// ability model, both of which the kernel carries as data.
///
/// 1.16.2: the contents of a cost depend on the game state, which is why
/// [`Cost::credits`] is a `Quantity`; 1.16.2d: outside a payment a cost of X
/// is treated as 0. 1.16.3: a checkpoint occurs after a cost is paid. 1.16.4:
/// the six main types of cost, of which 1.16.4a's install/rez/play costs are
/// inherent properties of cards ([`PrintedCard::cost`]) and 1.16.4b says
/// their presence does not make an ability optional. 1.16.5a: an ability may
/// direct a player to ignore a whole type of cost. 1.16.6a/c, 1.16.7, 1.16.8,
/// 1.16.9, 1.16.11: install, play, rez, trigger and nested costs.
///
/// 9.6.1/a: a conditional ability is triggered at a specific point, and its
/// primary condition is a trigger or static condition. 9.6.4/a: it can have
/// several instances, and meeting the condition again while one is pending
/// makes another. 9.6.5: the trigger condition describes an occurrence;
/// 9.6.5e: "If successful" is one with its own rules (§6.7). 9.6.7a: static
/// conditions are checked at every checkpoint. 9.6.8: a player triggers a
/// pending ability while they have priority in a reaction window. 9.6.9/a/b/c:
/// optional versus mandatory, and the optional PARTS a mandatory ability may
/// still have.
pub fn cost_and_conditional_model() {
    cite!("rule_modified_costs");
    cite!("rule_cost_x_out_of_context");
    cite!("rule_cost_checkpoint");
    cite!("rule_types_of_costs");
    cite!("rule_inherent_cost");
    cite!("rule_inherent_cost_in_ability");
    cite!("rule_ignore_general_cost");
    cite!("rule_install_cost_on_card");
    cite!("rule_no_install_cost");
    cite!("rule_play_cost");
    cite!("rule_rez_cost");
    cite!("rule_trigger_cost");
    cite!("rule_nested_cost");
    cite!("rule_conditional_ability");
    cite!("rule_primary_condition");
    cite!("rule_trigger_condition_multiple_instances");
    cite!("rule_condition_met_with_pending_instances");
    cite!("rule_trigger_description");
    cite!("rule_condition_if_successful");
    cite!("rule_conditional_ability_check_to_become_pending");
    cite!("rule_trigger_conditional_ability");
    cite!("rule_optional_conditional_ability");
    cite!("rule_pass_with_optional_conditional_abilities_pending");
    cite!("rule_cannot_pass_with_mandatory_conditional_abilities_pending");
    cite!("rule_mandatory_conditional_ability_with_optional_effects");
}

/// A **citation anchor**: CR 9.3 classifies every unit of an ability's text,
/// and that classification IS this module's type structure.
///
/// 9.3.1: text is classified into conditions, restrictions, instructions,
/// declarations and ability flags. 9.3.2/a/b/c: a condition is a cost
/// condition ([`Cost`] on a paid ability), a trigger condition
/// ([`TriggerCond`]) or a static condition ([`StaticCond`]) — which is exactly
/// [`Condition`]. 9.3.4/a/b/c/d: an instruction resolves at a specific time,
/// originates from an ability or a game rule, announces its targets BEFORE
/// becoming imminent (1.15.2), is atomic once it begins, and its steps run in
/// the order written. 9.3.5: a declaration applies continuously — that is
/// [`StaticDecl`]. 9.3.6/a: there are six ability flags and [`AbilityFlag`]
/// has six variants; 9.3.6b-e are the four with timing consequences, all
/// implemented. 9.3.7/a-e: the five ability types are identified by the text
/// they are made of, which is [`AbilityKind`].
pub fn text_classification_model() {
    cite!("rule_text_classification");
    cite!("rule_condition");
    cite!("rule_cost_condition");
    cite!("rule_trigger_condition");
    cite!("rule_static_condition");
    cite!("rule_instruction");
    cite!("rule_instruction_source");
    cite!("rule_instruction_target");
    cite!("rule_instruction_atomic");
    cite!("rule_resolve_instruction_in_order");
    cite!("rule_declaration");
    cite!("rule_ability_flag");
    cite!("rule_ability_flag_types");
    cite!("rule_access_flag");
    cite!("rule_interface_flag");
    cite!("rule_interrupt_flag");
    cite!("rule_persistent_flag");
    cite!("rule_ability_classification");
    cite!("rule_static_abilities_link");
    cite!("rule_paid_abilities_link");
    cite!("rule_conditional_abilities_link");
    cite!("rule_play_abilities_link");
    cite!("rule_subroutines_link");
}

/// A **citation anchor**: CR 1.14 ownership and control, which the kernel
/// keeps on the objects themselves.
///
/// 1.14.1: the OWNER is the player who provided the card ([`Object::owner`]).
/// 1.14.2a: the controller of a card in the play area is whoever installed or
/// placed it; 1.14.2b: each player controls the agendas in their own score
/// area (which is why a score-area swap changes control); 1.14.2c: cards
/// elsewhere are controlled by their owner; 1.14.2d/e: a player controls the
/// credits in their pool, the Corp its bad publicity and the Runner its tags;
/// 1.14.2f: a hosted counter's controller is the host's controller (1.13.3).
/// 1.14.3: a player can only pay costs with objects they control, which is
/// what every payment path reads. 1.14.4: the controller of an ability is the
/// player responsible for it — `AbilityFrame::controller`.
pub fn ownership_and_control_model() {
    cite!("rule_owner");
    cite!("rule_controller_card_play_area");
    cite!("rule_controller_agenda");
    cite!("rule_controller_default_owner");
    cite!("rule_controller_credits");
    cite!("rule_controller_bad_publicity_tag");
    cite!("rule_controller_hosted_counter");
    cite!("rule_pay_costs_controlled_objects");
    cite!("rule_controller_ability");
    cite!("rule_trigger_condition_effect_by_player");
}

/// A **citation anchor**: these rules are realised structurally — by the shape
/// of [`AbilityDef`] and [`AbilityKind`], not at one call site — so this is
/// where the traceability registry records them.
///
/// CR 9.1.1: "an ability is an independent unit of text on a card or counter,
/// a basic action, or the basic trash ability", and 9.1.1a "all rules text on
/// a card or counter is part of an ability" — which is why `PrintedCard` has
/// no free-form text field at all. 9.1.1e categorises every ability as static
/// (§9.4), paid (§9.5), conditional (§9.6), a play ability (§9.7) or a
/// subroutine (§9.8), and that is exactly [`AbilityKind`]; 9.1.1b/c put the
/// basic actions and the basic trash ability in the same vocabulary; 9.1.1f
/// is the `[interrupt]` flag; 9.1.1g is the instruction list.
///
/// 9.1.9c: abilities on an object have no inherent order except play
/// abilities and subroutines — which is why subroutines are ordered by §9.8's
/// categories and everything else is read as a set.
pub fn ability_model() {
    cite!("rule_ability");
    cite!("rule_all_text_is_an_ability");
    cite!("rule_basic_action_link");
    cite!("rule_trash_ability_link");
    cite!("rule_lingering_effects_link");
    cite!("rule_ability_categories");
    cite!("rule_interrupt_link");
    cite!("rule_instruction_link");
    cite!("rule_abilities_no_inherent_order");
    cite!("rule_gaining_losing_abilities");
    cite!("rule_determine_actual_abilities");
}

/// A **citation anchor** (see [`ability_model`]).
///
/// CR 9.1.3: the SOURCE of an ability is the card, counter, or game rule that
/// originated it. 9.1.3a a card is the source of its printed abilities;
/// 9.1.3b a granted ability's source is the object it was granted to; 9.1.3c
/// an ability maintained by a lingering effect has the object that created
/// that effect as its source — which is what [`AbilityRef`] and
/// `LingeringEffect::source` record.
pub fn ability_source_model() {
    cite!("rule_source");
    cite!("rule_source_printed_abilities");
    cite!("rule_source_granted_abilities");
    cite!("rule_source_lingering_effect");
    cite!("rule_effect");
    cite!("rule_effect_beyond_resolution");
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
    /// CR 9.6.5c: the printed "**the first time each turn** <the
    /// condition>" — a stipulation about the OCCURRENCE, which is part of
    /// the condition and is therefore checked when the condition would be
    /// met, not when the ability is used.
    ///
    /// The SPAN the ordinal counts over is content on the ordinal (§12
    /// rule 2), not a field on any one condition: "the first time each turn"
    /// and "the first time each run" are the same stipulation about the same
    /// occurrence with a different span, so every condition says both or
    /// neither. `None` is a sentence printing no ordinal at all.
    ///
    /// Deliberately NOT [`AbilityFlag::OncePerTurn`]. 9.3.6g's flag is spent
    /// by USING the ability, 9.1.6 says players never use an entirely
    /// mandatory ability, and 1.12.2's Vaporframe Fabricator example makes
    /// the flag per OBJECT — so a mandatory "first time each turn" ability
    /// written as the flag would either never expend at all or come back
    /// fresh when its card was reinstalled the same turn. The occurrence is
    /// counted from the game history instead, which 10.2.1 makes open
    /// information.
    pub ordinal: Option<OrdinalScope>,
    /// Human-readable tag for tests/logs.
    pub label: &'static str,
}

/// CR 9.6.5c: the span an ordinal stipulation counts over — "the first time
/// **each turn**" against "the first time **each run**". Content on
/// [`AbilityDef::ordinal`], because the two sentences differ in nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdinalScope {
    /// "…each turn" — counted from the current turn's beginning (10.2.1's
    /// open history; 1.12.6).
    Turn,
    /// "…each run" — counted from the run in progress beginning. Outside a
    /// run there is no span at all, so nothing can be the first time in it.
    Run,
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
            ordinal: None,
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
            ordinal: None,
            label: "",
        }
    }

    /// CR 9.7.1: a PLAY ability — the ability of an operation or event that
    /// resolves as the card is played (step 8.6.7f).
    pub fn play(instrs: Vec<Instruction>) -> Self {
        cite!("rule_play_ability");
        AbilityDef {
            kind: AbilityKind::Play,
            flags: Vec::new(),
            condition: None,
            cost: None,
            instructions: instrs,
            statics: Vec::new(),
            optional: false,
            timing: None,
            ordinal: None,
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
            ordinal: None,
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
            ordinal: None,
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

    /// CR 9.6.5c: "**the first time each turn** <the condition>" — the
    /// printed ordinal, stipulated on the condition rather than on the use
    /// (9.1.6/9.3.6g; see [`AbilityDef::ordinal`]).
    pub fn first_time_each_turn(mut self) -> Self {
        cite!("rule_condition_requirements_part_of_condition");
        self.ordinal = Some(OrdinalScope::Turn);
        self
    }

    /// CR 9.6.5c again, with the other span: "**the first time each run**
    /// <the condition>" (Jesminder Sareen). The same stipulation about the
    /// same occurrence — only what it is counted over differs.
    pub fn first_time_each_run(mut self) -> Self {
        cite!("rule_condition_requirements_part_of_condition");
        self.ordinal = Some(OrdinalScope::Run);
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
            | TriggerCond::WouldDeclareRunSuccessful
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
    /// CR 1.15.4: the card the OCCURRENCE that met this condition named —
    /// what a printed "…place 1 agenda counter on **it**" (Titan
    /// Transnational) or "…you may expose **that card**" (419) points at.
    /// It is not a target: nothing was announced, and the card is fixed by
    /// the condition rather than chosen. `None` for a condition that names no
    /// card (a turn beginning, a tag taken) and for an instance created by
    /// something other than the checkpoint scan.
    pub triggering_card: Option<ObjectId>,
    /// CR 1.15.4 in the PLURAL: every card the occurrence that met this
    /// condition named — what a printed "…from among **those cards**"
    /// (Magdalene Keino-Chemutai) points at.
    ///
    /// A per-occurrence condition (9.6.4b) names at most one card, so this
    /// holds the same card [`AbilityInstance::triggering_card`] does or
    /// nothing at all. It is a per-EVENT condition ([`trigger_per_event`])
    /// that makes the difference: one instance stands for the whole occurrence
    /// GROUP — every card of one draw (8.4.2), every card of one discard
    /// (5.7.4) — and the sentence after such a condition speaks of all of
    /// them. `triggering_card` stays the FIRST of them, so a singular
    /// back-reference reads the same thing it always did.
    pub triggering_cards: Vec<ObjectId>,
    /// CR 1.15.4 + 9.6.13: the targets the ability that CREATED this delayed
    /// conditional had already announced — "install 1 program … **when that
    /// run ends, trash that program**" (Arissana Rocha Nahu), "…**trash that
    /// program**" (Howler). The delayed ability resolves long after the frame
    /// that made it has gone, so the reference is bound when the ability is
    /// created and travels with it; the frame it eventually resolves in
    /// starts with these as its own announced targets, which is what lets
    /// every "the target this ability already announced" word — the
    /// [`crate::instr::TargetSpec::EarlierTarget`] a later instruction reads,
    /// the [`TriggerRequirement::EarlierTargetMatches`] a conditional one
    /// asks — say the same thing inside a delayed ability as outside one.
    ///
    /// Empty for every instance not created that way.
    pub bound_targets: Vec<ObjectId>,
    /// CR 1.15.4 + 9.6.13 for the cards the creating ability INSTALLED rather
    /// than announced — "search your stack for a non-virus program and install
    /// it … if **that program** is still installed when your turn ends"
    /// (Kabonesa Wu). 8.7.4's find is not 1.15.2's announcement, so such a card
    /// was never a target and cannot travel in
    /// [`AbilityInstance::bound_targets`]; it travels here, and the frame the
    /// delayed ability resolves in starts with these as its own
    /// [`crate::frames::AbilityFrame::installed_cards`], so
    /// [`crate::instr::TargetFilter::InstalledByThisAbility`] says the same
    /// thing inside the delay as outside it.
    ///
    /// Empty for every instance not created that way.
    pub bound_installs: Vec<ObjectId>,
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
    // 9.1.8d: "abilities that modify the cost to install, rez, or play their
    // source card are active even while that card is inactive" — a 1.16.2e
    // alternate payment for the source's own cost is exactly that, and it is
    // the whole point of the class (the ice is unrezzed when it is rezzed).
    cite!("rule_active_exception_modify_cost");
    if def
        .statics
        .iter()
        .any(|d| matches!(d, StaticDecl::AlternatePaymentForSelf { .. }))
    {
        return true;
    }
    // 9.1.8c: "abilities that modify when or if their source card can be
    // played, installed, or rezzed are active even while that card is
    // inactive". A Formicary-class ability whose effect IS rezzing its own
    // source is exactly that — it modifies WHEN the card can be rezzed — and
    // the ability would be unusable on any other reading, since the source is
    // unrezzed by construction until it resolves. The instruction list is the
    // kernel's only representation of what an ability does, so the scan reads
    // it (a shallow scan: wrappers are not looked inside).
    cite!("rule_active_exception_modify_play_install_rez");
    // The other half of 9.1.8c: a static ability DECLARING when its source
    // may be played is active while the source is inactive, which is the only
    // state a playable card is ever in.
    if def.statics.iter().any(|d| matches!(d, StaticDecl::PlayOnlyIf(_))) {
        return true;
    }
    if def.instructions.iter().any(|i| {
        matches!(
            i,
            Instruction::RezCard { target: crate::instr::TargetSpec::SelfSource, .. }
                | Instruction::InstallCard { card: crate::instr::TargetSpec::SelfSource, .. }
                | Instruction::PlayCard { card: crate::instr::TargetSpec::SelfSource, .. }
        )
    }) {
        return true;
    }
    // 9.1.8f: "abilities that allow their source card to be advanced are
    // active while that card is installed" — an unrezzed Ice Wall can be
    // advanced, which is what the rule exists for. (9.1.8e's
    // advancement-requirement modifiers are the neighbouring case; the kernel
    // states those over a SERVER, not over the source card, so they are
    // active by the ordinary rule.)
    cite!("rule_active_exception_advancement_requirement");
    cite!("rule_active_exception_can_be_advanced");
    if !obj.staged
        && matches!(obj.zone, crate::object::Zone::Root(_) | crate::object::Zone::Ice(_))
        && def.statics.iter().any(|d| matches!(d, StaticDecl::CanBeAdvancedSelf))
    {
        return true;
    }
    // 9.1.8b: "abilities that can only ever meet their conditions in a
    // particular zone are active in that zone. … When determining whether
    // these stipulations apply, refer only to the GAME RULES, not to any
    // other effects that may be changing them." A "when this card is trashed
    // by damage" condition can only be met by the card moving from the grip
    // to the heap (10.4.2), so the ability is active in the heap — and only
    // there: a replacement that sent the card elsewhere leaves it inactive,
    // because the rule reads the zone the card is actually in.
    cite!("rule_active_exception_catchall");
    if let Some(Condition::Trigger(t)) = &def.condition {
        if condition_only_met_in_zone(t, obj) == Some(obj.zone) {
            return true;
        }
    }
    // 9.1.8b's first sentence — "abilities STATING that they are active in a
    // particular zone are active in that zone" — is 4.5.4's "unless stated
    // otherwise" in person: an agenda in the Runner's score area is inactive,
    // and Merger's "…while it is in the Runner's score area" is the statement
    // that makes its one ability an exception.
    if let Some(Condition::Static(StaticCond::SourceInScoreAreaOf(side))) = &def.condition {
        if obj.zone == crate::object::Zone::ScoreArea(*side) {
            return true;
        }
    }
    // The same sentence, for a TRIGGER condition that states the zone in one
    // of its 9.6.5c requirements: "when your turn begins, **if this card is
    // in Archives**…" (Subliminal Messaging) is a statement that the ability
    // is active in the discard pile, and nowhere else — the requirement is
    // what the rule reads, so an ability whose requirement is not met by the
    // zone the card is actually in stays inactive.
    if let Some(Condition::Trigger(t)) = &def.condition {
        if trigger_requirements(t)
            .iter()
            .any(|r| requirement_states_zone(r, obj) == Some(obj.zone))
        {
            return true;
        }
    }
    // 9.1.8g is instance-driven (hangover) and handled by the checkpoint scan.
    // 9.1.8i persistent: handled via lingering effects.
    false
}

/// CR 9.1.8b, FIRST sentence: "abilities stating that they are active in a
/// particular zone are active in that zone." A 9.6.5c requirement naming a
/// zone is exactly such a statement — Subliminal Messaging's "if this card is
/// in Archives" says where its "when your turn begins" ability lives — so the
/// zone the requirement names is the zone it is active in. Only POSITIVE
/// statements count: "anywhere except in Archives" names no zone to be active
/// in.
fn requirement_states_zone(req: &TriggerRequirement, obj: &Object) -> Option<Zone> {
    match req {
        TriggerRequirement::SourceInDiscard => Some(Zone::Discard(obj.owner)),
        TriggerRequirement::SourceInDeck => Some(Zone::Deck(obj.owner)),
        _ => None,
    }
}

/// CR 9.1.8b, second sentence: "abilities that can only ever meet their
/// conditions in a particular zone are active in that zone. … When
/// determining whether these stipulations apply, refer only to the GAME
/// RULES, not to any other effects that may be changing how cards move
/// between zones."
///
/// So this reads the RULES that say where each condition's occurrence puts
/// the source, and nothing else — a replacement that sent the card somewhere
/// else leaves the ability inactive, because the zone the card is actually in
/// is what is compared.
fn condition_only_met_in_zone(cond: &TriggerCond, obj: &Object) -> Option<Zone> {
    match cond {
        // 10.4.2: damage trashes cards from the grip to the heap, so this
        // condition can only ever be met with the card in its owner's heap.
        TriggerCond::SelfTrashedByDamage => Some(Zone::Discard(obj.owner)),
        // 1.17.3: only the Runner steals, and stealing moves the agenda to
        // the Runner's score area (1.17.7) — where 4.5.4 would otherwise
        // leave it inactive. Clone Retirement's "when you steal this agenda"
        // is exactly the class the rule exists for.
        TriggerCond::SelfStolen => {
            cite!("rule_agenda_stolen");
            Some(Zone::ScoreArea(Side::Runner))
        }
        // 1.17.6: the scoring twin, for symmetry — the Corp's score area is
        // active anyway (4.5.4), so this changes nothing today.
        TriggerCond::SelfScored { .. } => {
            cite!("rule_agenda_scored");
            Some(Zone::ScoreArea(Side::Corp))
        }
        _ => None,
    }
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
    // The printed card type of an object named by the change, for a condition
    // that stipulates one (2.15).
    card_type_of: impl Fn(ObjectId) -> Option<crate::object::CardType>,
    // Whether an object named by the change has a subtype the condition
    // stipulates (2.16) — read through the 9.12.1b pipeline, so a subtype an
    // active effect granted counts.
    has_subtype: impl Fn(ObjectId, &'static str) -> bool,
    // CR 9.10.3: whether an object named by the change matches the value the
    // SOURCE is maintaining under a key the condition stipulates — the same
    // question `TargetFilter::MatchesMaintainedChoice` asks of a description,
    // asked here of an occurrence.
    matches_choice: impl Fn(ObjectId, &'static str) -> bool,
    // §12 rule 5: whether an object named by the change matches the criteria
    // the condition stipulates, in the shared filter vocabulary — the same
    // question a description asks, asked here of an occurrence ("you play a
    // copy of <name>").
    matches_criteria: impl Fn(ObjectId, &[crate::instr::TargetFilter]) -> bool,
) -> bool {
    trigger_matches_dyn(
        cond,
        change,
        source,
        server_of_source,
        &trashed_is_corp,
        &card_type_of,
        &has_subtype,
        &matches_choice,
        &matches_criteria,
    )
}

/// The body of [`trigger_matches`], with the five state readers taken as trait
/// OBJECTS. A disjunctive condition (9.6.5) asks the same question of each
/// alternative, so the function is recursive — and a recursion that passed the
/// readers by generic reference would instantiate a new copy of it per level.
fn trigger_matches_dyn(
    cond: &TriggerCond,
    change: &GameChange,
    source: &Object,
    server_of_source: Option<ServerId>,
    trashed_is_corp: &dyn Fn(ObjectId) -> bool,
    card_type_of: &dyn Fn(ObjectId) -> Option<crate::object::CardType>,
    has_subtype: &dyn Fn(ObjectId, &'static str) -> bool,
    matches_choice: &dyn Fn(ObjectId, &'static str) -> bool,
    matches_criteria: &dyn Fn(ObjectId, &[crate::instr::TargetFilter]) -> bool,
) -> bool {
    cite!("rule_trigger_condition_checked");
    // 9.6.5: one condition describing several kinds of occurrence is met by
    // any of them. Asked before the pairwise table, since the alternatives are
    // what the table then sees.
    if let TriggerCond::AnyOf { alternatives, .. } = cond {
        return alternatives.iter().any(|alt| {
            trigger_matches_dyn(
                alt,
                change,
                source,
                server_of_source,
                trashed_is_corp,
                card_type_of,
                has_subtype,
                matches_choice,
                matches_criteria,
            )
        });
    }
    match (cond, change) {
        // 9.6.5c: the requirements riding on the condition are checked by the
        // checkpoint scan (it has the state access); this arm matches the
        // occurrence itself.
        (TriggerCond::TurnBegins { side, .. }, GameChange::TurnBegan { side: s }) => side == s,
        (TriggerCond::RunEnds { on, .. }, GameChange::RunEnded { server, .. }) => {
            // The server the sentence names, where it names one — the same
            // reading `MakesSuccessfulRun` gives its list.
            on.is_empty() || on.contains(server)
        }
        // 6.9.1: the run begins at the Run Initiation Phase, whose first step
        // has already announced the attacked server.
        (TriggerCond::RunBegins { on }, GameChange::RunBegan { server }) => {
            cite!("rule_run_initiation_phase");
            on.is_empty() || on.contains(server)
        }
        (TriggerCond::DifferentActionsThisTurn { side, .. }, GameChange::ActionTaken { side: s, .. }) => {
            // 5.2.5b: the "all different" test is a game-state question the
            // checkpoint scan answers against the turn's action history.
            cite!("rule_defferent_actions");
            side == s
        }
        (TriggerCond::SameActionInARow { side, .. }, GameChange::ActionTaken { side: s, .. }) => {
            // 5.2.5a/b again, from the other end: the "same action" test is
            // the checkpoint scan's, which can read the turn's history.
            cite!("rule_same_actions");
            side == s
        }
        (TriggerCond::ClicksSpentOnAction { side, .. }, GameChange::ClickSpent { side: s }) => {
            cite!("rule_inherent_cost_aggregates");
            side == s
        }
        (TriggerCond::PlayerSpendsClick { side, .. }, GameChange::ClickSpent { side: s }) => {
            // 6.3.4: the "during a run" half is a game-state test, applied by
            // the checkpoint scan, which can see whether a run is in progress.
            cite!("rule_abilities_during_a_run");
            side == s
        }
        (
            TriggerCond::PlayerSpendsClick { side, also_lost, .. },
            GameChange::ClicksLost { side: s, .. },
        ) => {
            // 5.2.1: losing a click is not spending one, so only a sentence
            // that names both is met here.
            cite!("rule_abilities_during_a_run");
            *also_lost && side == s
        }
        (
            TriggerCond::SuccessfulRunOnServer,
            GameChange::RunDeclaredSuccessful { server, .. },
        ) => {
            cite!("rule_successful_run");
            server_of_source == Some(*server)
        }
        (
            TriggerCond::MakesSuccessfulRun { on, after_subroutine_resolved, .. },
            GameChange::RunDeclaredSuccessful { server, subroutine_resolved },
        ) => {
            cite!("rule_successful_run");
            on.as_ref().is_none_or(|set| set.contains(server))
                // 9.8.7 as the record kept it: a stipulation inside what the
                // ordinal counts, answered of the occurrence's own moment.
                && (!*after_subroutine_resolved || *subroutine_resolved)
        }
        (TriggerCond::ActionPhaseEnds { side, .. }, GameChange::ActionPhaseEnded { side: s }) => {
            cite!("rule_action_phase_duration");
            side == s
        }
        // 8.6.1 / 8.5.1: the two ways a card is used out of a hand. A sentence
        // naming only the first ("another current is played") matches only
        // `CardPlayed`; one naming both ("plays or installs a copy of that
        // card") matches either, through the same stipulations.
        (
            TriggerCond::CardPlayed {
                by,
                of_types,
                of_subtypes,
                criteria,
                other_than_source,
                also_installed,
                matching_choice,
                // The ordinal is a question about the HISTORY, which this
                // function does not see; the checkpoint scan applies it.
            },
            GameChange::CardPlayed { obj, side } | GameChange::CardInstalled { obj, side, .. },
        ) => {
            cite!("rule_play_ability");
            if matches!(change, GameChange::CardInstalled { .. }) && !*also_installed {
                return false;
            }
            if matches!(change, GameChange::CardInstalled { .. }) {
                cite!("rule_installing");
            }
            by.is_none_or(|b| b == *side)
                && (of_types.is_empty()
                    || card_type_of(*obj).is_some_and(|t| of_types.contains(&t)))
                && of_subtypes.iter().all(|s| has_subtype(*obj, s))
                && (!*other_than_source || *obj != source.id)
                // 9.10.3 / 2.1.4: "a copy of that card" — the card that was
                // played or installed is compared against what the source is
                // remembering.
                && matching_choice.is_none_or(|k| matches_choice(*obj, k))
                // 10.1.5: "a copy of <name>" is a description of the card
                // played, asked in the shared filter vocabulary.
                && matches_criteria(*obj, criteria)
        }
        (TriggerCond::RunOnThisServerEnds, GameChange::RunEnded { server, .. }) => {
            server_of_source == Some(*server)
        }
        (TriggerCond::RunnerTrashesCorpCard { .. }, GameChange::CardTrashed { by, obj, .. }) => {
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
        (TriggerCond::RunnerAccessesCard { of_types }, GameChange::CardAccessed { obj }) => {
            cite!("rule_accessing");
            cite!("rule_card_type_list");
            of_types.is_empty() || card_type_of(*obj).is_some_and(|t| of_types.contains(&t))
        }
        (TriggerCond::PlayerDrawsCards(side), GameChange::CardDrawn { side: s, .. }) => {
            cite!("rule_draw_procedure");
            side == s
        }
        (TriggerCond::SelfEncountered, GameChange::EncounterBegan { ice, .. }) => {
            *ice == source.id
        }
        (
            TriggerCond::EncounterBegins { of_subtypes, .. },
            GameChange::EncounterBegan { ice, .. },
        ) => {
            cite!("rule_subtypes_active");
            of_subtypes.iter().all(|s| has_subtype(*ice, s))
        }
        (TriggerCond::ServerApproached, GameChange::ServerApproached { .. }) => {
            cite!("step_approach_server");
            true
        }
        (
            TriggerCond::PlayerPaysCredits { side, caused_by, .. },
            GameChange::CostPaid { side: s, credits, source: cause, .. },
        ) => {
            cite!("rule_cost_quantities");
            cite!("rule_spend_credits");
            side == s
                && *credits > 0
                // §12 rule 5: what the sentence says about the cause, asked
                // of the ability's source the way a description asks it. A
                // payment no card caused meets no such stipulation.
                && (caused_by.is_empty()
                    || cause.is_some_and(|c| matches_criteria(c, caused_by)))
        }
        // 1.10.3b: the other half of "spend **or** lose" — the same condition,
        // met by the forced movement instead of the payment.
        (
            TriggerCond::PlayerPaysCredits { side, also_lost: true, caused_by, .. },
            GameChange::CreditsLost { side: s, amount, source: cause },
        ) => {
            cite!("rule_lose_credits");
            side == s
                && *amount > 0
                && (caused_by.is_empty()
                    || cause.is_some_and(|c| matches_criteria(c, caused_by)))
        }
        (
            TriggerCond::IcePassed { this_ice, fully_broken, subs_resolved, rezzed, criteria },
            GameChange::IcePassed {
                ice,
                after_encounter,
                fully_broken: fb,
                subs_resolved: sr,
                rezzed: rz,
            },
        ) => {
            cite!("rule_pass_ice");
            cite!("rule_run_phase_after");
            cite!("rule_pass_after_breaking");
            cite!("rule_replace_subroutine_resolution");
            (!*this_ice || *ice == source.id)
                && (!*fully_broken || (*after_encounter && *fb))
                && (!*subs_resolved || (*after_encounter && *sr))
                // 8.1's status as the record kept it: a fact of the moment,
                // inside what the ordinal counts.
                && (!*rezzed || *rz)
                // §12 rule 5: what the sentence says about the ice's own
                // characteristics, asked in the shared description vocabulary.
                && matches_criteria(*ice, criteria)
        }
        (TriggerCond::ThisServerBreached, GameChange::BreachBegan { server }) => {
            server_of_source == Some(*server)
        }
        (
            TriggerCond::BreachesServer { servers, .. },
            GameChange::BreachBegan { server: s },
        ) => {
            cite!("rule_breaching_servers");
            servers.contains(s)
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
        (
            TriggerCond::CorpRezzesCard { of_types, of_subtypes, criteria, .. },
            GameChange::CardRezzed { obj, card_type },
        ) => {
            cite!("rule_rez_in_paw");
            (of_types.is_empty() || of_types.contains(card_type))
                && of_subtypes.iter().all(|s| has_subtype(*obj, s))
                // §12 rule 5: the rest of what the sentence says about the
                // card, asked the way a description asks it.
                && matches_criteria(*obj, criteria)
        }
        (TriggerCond::CorpPurgesVirusCounters, GameChange::VirusCountersPurged) => {
            cite!("rule_purge");
            true
        }
        (TriggerCond::TurnEnds { side, .. }, GameChange::TurnEnded { side: s }) => side == s,
        // 5.1.4b: "Trigger conditions related to a turn or discard phase
        // ending are met at the timing step that indicates the formal end of
        // the turn." Same step, same occurrence, different sentence.
        (TriggerCond::DiscardPhaseEnds { side, .. }, GameChange::TurnEnded { side: s }) => {
            cite!("rule_turn_end_trigger_conditions");
            cite!("rule_discard_step");
            // A sentence naming no player is met by EITHER discard phase.
            side.is_none() || side.as_ref() == Some(s)
        }
        // 5.7.4: the discard step's own discard. `to_hand_size` on the record
        // is what the step wrote there; a sentence naming any discard is met
        // by either kind, one naming the step's is met only by it.
        (
            TriggerCond::PlayerDiscardsCards { side, to_hand_size },
            GameChange::CardDiscarded { side: s, to_hand_size: t, .. },
        ) => {
            cite!("rule_discard_step");
            side == s && (!*to_hand_size || *t)
        }
        (TriggerCond::RunnerTakesTag, GameChange::TagsTaken { .. }) => true,
        // 10.5.1: the tag counter went back to the bank. One record per tag,
        // so a sentence about "a tag" is met once for each of them.
        (TriggerCond::TagRemoved, GameChange::TagRemoved) => {
            cite!("rule_tag");
            true
        }
        // 10.4.1: a sentence naming a kind of damage is met only by that kind;
        // one naming none is met by any.
        (
            TriggerCond::RunnerSuffersDamage { kind, trashed_a_card, responsible },
            GameChange::DamageSuffered { kind: k, cards, responsible: resp, .. },
        ) => {
            // 10.4.2a/b: the trash IS the damage procedure, so a sentence
            // that speaks of the trash asks one more thing of the same
            // occurrence — and an empty grip answers it no.
            cite!("rule_meat_net_damage");
            cite!("rule_brain_damage");
            // 10.4.1: "you do damage" is the does-branch's responsibility,
            // a fact the record carries about the occurrence.
            cite!("rule_suffer_or_take_damage");
            (kind.is_none() || kind.as_ref() == Some(k))
                && (!*trashed_a_card || !cards.is_empty())
                && responsible.is_none_or(|r| *resp == r)
        }
        (
            TriggerCond::UsesTrashAbility { side, basic },
            GameChange::TrashAbilityUsed { side: s, basic: b, .. },
        ) => {
            let _ = basic.is_none_or(|w| w == *b);
            if !basic.is_none_or(|w| w == *b) {
                return false;
            }
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
        (TriggerCond::SelfUninstalled, GameChange::CardUninstalled { obj, .. }) => {
            cite!("rule_active_exception_conditional_move_to_inactive_zone");
            *obj == source.id
        }
        (
            TriggerCond::PlayerGainsCredits { side, criteria },
            GameChange::CreditsGained { side: s, source, .. },
        ) => {
            cite!("rule_calculated_quantity");
            if side != s {
                return false;
            }
            // 9.1.4: what the credits came THROUGH. A sentence stipulating a
            // source is not met by a gain that had none — the basic credit
            // action came through no card at all.
            criteria.is_empty()
                || source.is_some_and(|src| matches_criteria(src, criteria))
        }
        // 8.2.5: the forfeit moved the agenda out of the score area. The
        // sentence names who did it and nothing else.
        (TriggerCond::AgendaForfeited { by }, GameChange::AgendaForfeited { by: s, .. }) => {
            cite!("rule_forfeit_rfg");
            by == s
        }
        // 1.21.3: shown to all players by the named player.
        (TriggerCond::CardRevealed { by }, GameChange::CardRevealed { by: s, .. }) => {
            cite!("rule_reveal");
            by == s
        }
        // 4.6.8d: the server came into existence with the card that now sits
        // in it. The sentence names who created it and nothing else.
        (
            TriggerCond::RemoteServerCreated { by },
            GameChange::RemoteServerCreated { by: s, .. },
        ) => {
            cite!("rule_remote_server_existence");
            by == s
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
        // 8.5.16f: "when installed" conditions meet their trigger conditions
        // — this one is the installing ABILITY's own card saying "when you
        // install that card". The occurrence recorded which ability's
        // resolution performed the install, because the frame that did it is
        // gone by the time any condition is scanned; the comparison against
        // the source is all that is left to ask.
        (
            TriggerCond::CardInstalledByAbilityOfSource,
            GameChange::CardInstalled { by_ability_of, .. },
        ) => {
            cite!("rule_steps_installing_installed_condition");
            *by_ability_of == Some(source.id)
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
        (TriggerCond::SelfPlayResolved, GameChange::CardPlayResolved { obj, .. }) => {
            cite!("rule_steps_playing_after_resolve_condition");
            *obj == source.id
        }
        (
            TriggerCond::CardPlayResolved { by, criteria },
            GameChange::CardPlayResolved { obj, by: player },
        ) => {
            cite!("rule_steps_playing_after_resolve_condition");
            // 8.6.2's player, and §12 rule 5 for what the sentence says about
            // the card — asked here even though step (g) has already trashed
            // it, because the description reads the card's own
            // characteristics and those travel with it.
            *player == *by && matches_criteria(*obj, criteria)
        }
        (
            TriggerCond::ActionCompleted { side, criteria },
            GameChange::ActionCompleted { side: s, action },
        ) => {
            cite!("rule_action_completion");
            cite!("rule_finish_action_trigger_condition");
            *s == *side
                && match action {
                    // 5.2.4: an ability that is an action — the card it is on
                    // is what a description can reach.
                    crate::change::ActionIdentity::CardAbility(a) => {
                        matches_criteria(a.obj, criteria)
                    }
                    // 9.1.3: a basic action's source is a game rule, so there
                    // is no card for a description to be about, and only a
                    // sentence describing none reaches it.
                    crate::change::ActionIdentity::Basic(_) => criteria.is_empty(),
                }
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
            TriggerCond::CardTrashed {
                owner,
                by,
                installed_only,
                while_accessed,
                from_zone,
                ..
            },
            GameChange::CardTrashed { obj, was_zone, by: trasher, while_accessed: wa },
        ) => {
            // 8.2.2a: only a trash that actually happened records this change.
            // The `of_types` narrowing is applied by the checkpoint scan,
            // which can read the trashed card's type.
            cite!("rule_cancelled_movement");
            (!*installed_only || was_zone.is_installed())
                // The one zone the sentence names, which is the zone the card
                // left — the same field `installed_only` reads more broadly.
                && from_zone.is_none_or(|z| *was_zone == z)
                // 7.1.2: the card was the one being accessed at the time.
                && (!*while_accessed || *wa)
                // 1.14.1: whose card it was.
                && owner.is_none_or(|s| is_corp_card_side(trashed_is_corp(*obj)) == s)
                // 1.14.5: who did the trashing.
                && by.is_none_or(|s| *trasher == s)
        }
        (TriggerCond::SelfTrashedByDamage, GameChange::DamageSuffered { cards, .. }) => {
            cite!("rule_meat_net_damage");
            cards.contains(&source.id)
        }
        (
            TriggerCond::EncounterEnds { criteria, with_advanced_ice },
            GameChange::EncounterEnded { ice, ice_advanced, .. },
        ) => {
            cite!("step_encounter_complete");
            // 1.18.2 as the record kept it: a fact of the moment, inside what
            // the ordinal counts.
            (!*with_advanced_ice || *ice_advanced)
                // §12 rule 5: what the sentence says about the ice's own
                // characteristics, asked the way a description asks it.
                && matches_criteria(*ice, criteria)
        }
        (TriggerCond::AllSubsBrokenOnEncounteredIce, GameChange::AllSubsBroken { .. }) => {
            cite!("rule_vacuous_truth");
            true
        }
        // 6.5.7a: "the Runner fully breaks the encountered ice the first time
        // all subroutines on that ice are broken" — scoped to this card.
        (TriggerCond::SelfFullyBroken, GameChange::AllSubsBroken { ice }) => {
            cite!("rule_fully_break");
            cite!("rule_fully_break_no_subroutines");
            *ice == source.id
        }
        (
            TriggerCond::SubroutineBrokenOnSelf { printed_only },
            GameChange::SubroutineBroken { ice, printed },
        ) => {
            cite!("rule_break_subroutine");
            *ice == source.id && (!*printed_only || *printed)
        }
        (TriggerCond::RunnerStealsAgenda { .. }, GameChange::AgendaStolen { .. }) => true,
        (TriggerCond::CorpScoresAgenda { criteria, .. }, GameChange::AgendaScored { obj, .. }) => {
            cite!("rule_agenda_scored");
            // §12 rule 5: what the sentence says about the agenda, asked the
            // way a description asks it.
            matches_criteria(*obj, criteria)
        }
        (TriggerCond::RunnerAvoidsTag, GameChange::TagsAvoided { .. }) => true,
        (
            TriggerCond::SecretlySpentCreditsRevealed,
            GameChange::SecretlySpentCreditsRevealed { .. },
        ) => {
            cite!("rule_psi_bid_reveal");
            true
        }
        (TriggerCond::PlayerSearchesDeck(side), GameChange::ZoneSearched { by, zone }) => {
            cite!("rule_search_condition");
            by == side && *zone == Zone::Deck(*side)
        }
        (
            TriggerCond::CardInstalledFrom { side, from, of_types },
            GameChange::CardInstalled { side: s, from: f, obj, .. },
        ) => {
            // 4.8.3: `from` is the location the card is TREATED as having come
            // from, so an Exile-class "whenever you install a program from
            // your heap" is met by an install out of an 8.7.4 set-aside.
            cite!("rule_set_aside_zone_passthrough");
            // 2.15: the type stipulation, asked of the card the change names.
            side == s
                && from == f
                && (of_types.is_empty()
                    || card_type_of(*obj).is_some_and(|t| of_types.contains(&t)))
        }
        (
            TriggerCond::CardInstalledBy { side, of_types, of_subtypes, into_remote_server, .. },
            GameChange::CardInstalled { side: s, obj, to, .. },
        ) => {
            // 2.15/2.16: the stipulations, asked of the card the change names.
            side == s
                && (of_types.is_empty()
                    || card_type_of(*obj).is_some_and(|t| of_types.contains(&t)))
                && of_subtypes.iter().all(|s| has_subtype(*obj, s))
                // 4.6.6b + 4.6.8: where the install went, read from the record
                // so that a card which has since moved still answers for the
                // install it made.
                && (!*into_remote_server || {
                    cite!("rule_remote_server");
                    cite!("rule_server_root");
                    matches!(
                        to,
                        crate::object::Zone::Root(crate::object::ServerId::Remote(_))
                            | crate::object::Zone::Ice(crate::object::ServerId::Remote(_))
                    )
                })
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
        TriggerCond::SelfAccessed { requires }
        | TriggerCond::SelfScored { requires }
        | TriggerCond::ActionPhaseEnds { requires, .. }
        | TriggerCond::BreachesServer { requires, .. }
        | TriggerCond::MakesSuccessfulRun { requires, .. }
        | TriggerCond::TurnBegins { requires, .. }
        | TriggerCond::EncounterBegins { requires, .. }
        | TriggerCond::CorpRezzesCard { requires, .. }
        | TriggerCond::DiscardPhaseEnds { requires, .. }
        | TriggerCond::TurnEnds { requires, .. }
        | TriggerCond::RunnerStealsAgenda { requires }
        | TriggerCond::CorpScoresAgenda { requires, .. }
        | TriggerCond::CardInstalledBy { requires, .. }
        | TriggerCond::RunnerTrashesCorpCard { requires }
        | TriggerCond::CardTrashed { requires, .. }
        | TriggerCond::PlayerPaysCredits { requires, .. } => requires,
        // The disjunction's requirements are the whole condition's. An
        // alternative carrying its own would be silently dropped — a
        // requirement is checked once against the state and cannot be
        // attributed to one alternative — so it is an authoring error.
        TriggerCond::AnyOf { alternatives, requires } => {
            debug_assert!(
                alternatives.iter().all(|a| trigger_requirements(a).is_empty()),
                "9.6.5c: an alternative of a disjunctive trigger condition cannot \
                 carry requirements of its own"
            );
            requires
        }
        _ => &[],
    }
}

/// CR 9.6.4b vs 9.12.2a: is this trigger per-occurrence (each matching
/// change record pends an instance) or per-event (one instance per change
/// group)?
pub fn trigger_per_event(cond: &TriggerCond) -> bool {
    cite!("rule_act_on_multiple_cards");
    // 8.4.2: the cards of one draw are set aside — and so considered drawn —
    // together, so "whenever you draw 1 or more cards" is met ONCE per draw.
    // 5.7.4's discard is the same shape: one instruction moves every card a
    // player discards, and "whenever you discard cardS" is one occurrence
    // naming all of them.
    match cond {
        TriggerCond::RunnerTrashesAtLeastOneCorpCard { .. }
        | TriggerCond::PlayerDrawsCards(_)
        | TriggerCond::PlayerDiscardsCards { .. }
        | TriggerCond::CardTrashed { at_least_one: true, .. } => true,
        // A sentence met by either of two occurrences keeps each half's own
        // number: "you do damage or trash 1 or more cards from HQ" (AU Co.)
        // groups the trash half's cards while the damage half was one
        // aggregated occurrence to begin with (9.12.2c).
        TriggerCond::AnyOf { alternatives, .. } => alternatives.iter().any(trigger_per_event),
        _ => false,
    }
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
