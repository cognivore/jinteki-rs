//! The card-authoring vocabulary: printed text in, kernel data out.
//!
//! A card is written here as Rust, but nothing about it is programming. You
//! copy the printed text into `.text(…)`, then make one call per printed
//! sentence. The compiler is the proof-reader: a sentence the vocabulary
//! cannot say will not compile, so it cannot be quietly approximated.
//!
//! `docs/cards/EDSL.md` is the guide, and it is the only thing a card
//! designer should have to read. Everything below is shaped to be found by
//! autocomplete: type `card(` and the types lead you; type `gain(` and the
//! side is asked for; type `.paid` and every kind of paid ability appears.
//!
//! Two rules run through all of it (DESIGN.md SYS-D-6, SYS-D-11):
//!
//! 1. **Every helper denotes into the kernel's own vocabulary** — the CR's
//!    §9.11 instruction taxonomy, §9.6's trigger conditions, §9.4's
//!    declarations, §1.16's costs. Nothing here has state access of its own,
//!    and nothing here is card-shaped (ARCHITECTURE §12). If the kernel
//!    cannot express a sentence, no helper is added: the card records the
//!    sentence with `.unimplemented(…)` and the gap goes on the list in
//!    `docs/vm/WAVES.md`.
//! 2. **The printed text is data, not a comment.** SYS-D-10 checks behaviour
//!    against the text, so the text has to be readable by the thing doing the
//!    checking. The doc comment above each card carries it too, for whoever
//!    is reading the file; `tests/decks.rs` asserts the two agree.
//!
//! # A whole card
//!
//! `docs/cards/EDSL.md` is the guide; this is what it teaches, compiled. The
//! examples below are doctests, so the vocabulary the guide names is checked
//! to exist every time the suite runs — the textual DSL's guide promised
//! verbs that had never been implemented and nothing caught it for a wave.
//! (The guide itself would be doctested directly, but `nix/package.nix`
//! keeps `docs/cards` out of the build closure.)
//!
//! ```
//! use jinteki_cards::edsl::*;
//!
//! /// Daily Casts — Resource. Install 3.
//! /// "When you install this resource, load 8[credit] onto it. When it is
//! ///  empty, trash it.
//! ///  When your turn begins, take 2[credit] from this resource."
//! fn daily_casts() -> Card {
//!     card("Daily Casts")
//!         .runner()
//!         .resource()
//!         .cost(3)
//!         .text("When you install this resource, load 8[credit] onto it. When it is empty, trash it.")
//!         .text("When your turn begins, take 2[credit] from this resource.")
//!         .when(installed(), [load(CounterKind::Credit, 8)])
//!         .when(empty_of(CounterKind::Credit), [trash_self()])
//!         .unimplemented("When your turn begins, take 2[credit] from this resource.")
//!         .build()
//! }
//! assert_eq!(daily_casts().printed.abilities.len(), 2);
//! assert!(!daily_casts().is_complete());
//! ```
//!
//! The sentences a card writes as one but that are really two or three:
//!
//! ```
//! use jinteki_cards::edsl::*;
//! // "Gain 4[credit] and draw 3 cards."
//! let _ = combined([gain(Corp, 4), draw(Corp, 3)]);
//! // "You may trash this card to gain 3[credit] and draw 3 cards."
//! let _ = may_pay(trash_this_card(), combined([gain(Corp, 3), draw(Corp, 3)]));
//! // "End the run unless the Runner pays 3[credit]."
//! let _ = unless_pays(Runner, credits(3), end_the_run());
//! // "Trace[4]. If successful, give the Runner 4 tags."
//! let _ = trace(4, [give_tags(4)]);
//! // "…they must take 1 tag or end the run."
//! let _ = choose_one([
//!     ("take 1 tag", vec![give_tags(1)]),
//!     ("end the run", vec![end_the_run()]),
//! ]);
//! // "2 installed Runner cards" — a description, picked from when it resolves.
//! let _ = add_to_hand(choose(2, &[installed_runner_card()]));
//! // "…+1 strength for each tag the Runner has", printed value included.
//! let _ = strength_is(plus(amount(0), times(1, per_runner_tag())));
//! // "[click][click][click], [trash]:"
//! let _ = clicks(3).plus_cost(trash_this_card());
//! ```

use jinteki_cr::ability::{AbilityDef, AbilityFlag, Condition, TimingRestriction};
pub use jinteki_cr::effects::{DamageKind, EffectClass};
use jinteki_cr::object::PrintedCard;

// Re-exported so a deck file needs exactly one `use` line. These are the
// kernel's own types: a designer who outgrows the helpers below can reach
// for them directly and is still inside the public vocabulary.
pub use jinteki_cr::ability::{Cost, StaticDecl, TriggerCond, TriggerRequirement, TurnScope};
pub use jinteki_cr::lingering::{ReplacementTransform, WantedDuration};
pub use jinteki_cr::instr::{
    InstallDest, InstallFilter, Instruction, LingeringSpec, Quantity, RunServerSet, SubroutineSpec,
    TargetFilter, TargetSpec, TrashDestination,
};
pub use jinteki_cr::object::Side::{Corp, Runner};
pub use jinteki_cr::object::{CardType, CounterKind, ServerId, Side};

// ---------------------------------------------------------------------------
// The card
// ---------------------------------------------------------------------------

/// One finished card: what the kernel plays, what the card says, and what
/// could not be said about it.
#[derive(Debug, Clone)]
pub struct Card {
    pub printed: PrintedCard,
    /// The printed text, verbatim (SYS-D-10).
    pub oracle_text: String,
    /// Printed sentences with no expression in the vocabulary yet (SYS-D-9),
    /// quoted from the card. Data, so the manifest test can count them.
    pub unimplemented: Vec<&'static str>,
}

impl Card {
    /// A card is *complete* when every printed sentence is expressed. Only
    /// complete cards are playable in a strict game (SYS-D-12).
    pub fn is_complete(&self) -> bool {
        self.unimplemented.is_empty()
    }
    pub fn name(&self) -> &'static str {
        self.printed.name
    }
}

/// Start a card. Everything else hangs off this.
pub fn card(name: &'static str) -> CardBuilder {
    CardBuilder {
        printed: PrintedCard::vanilla(name, Side::Corp, CardType::Operation),
        text: Vec::new(),
        unimplemented: Vec::new(),
        side_set: false,
        type_set: false,
    }
}

/// The card under construction. Every method returns the builder, so a card
/// is one expression ending in `.build()`.
pub struct CardBuilder {
    printed: PrintedCard,
    text: Vec<&'static str>,
    unimplemented: Vec<&'static str>,
    side_set: bool,
    type_set: bool,
}

impl CardBuilder {
    // ---- who owns it ---------------------------------------------------
    pub fn corp(mut self) -> Self {
        self.printed.side = Side::Corp;
        self.side_set = true;
        self
    }
    pub fn runner(mut self) -> Self {
        self.printed.side = Side::Runner;
        self.side_set = true;
        self
    }

    // ---- what it is ----------------------------------------------------
    // One method per printed card type. The two that carry numbers on the
    // card itself take them here, so the card reads the way it is printed:
    // an agenda is "3/2", ice has a strength.
    fn typed(mut self, t: CardType) -> Self {
        self.printed.card_type = t;
        self.type_set = true;
        self
    }
    pub fn event(self) -> Self {
        self.typed(CardType::Event)
    }
    pub fn operation(self) -> Self {
        self.typed(CardType::Operation)
    }
    pub fn hardware(self) -> Self {
        self.typed(CardType::Hardware)
    }
    pub fn program(self) -> Self {
        self.typed(CardType::Program)
    }
    pub fn resource(self) -> Self {
        self.typed(CardType::Resource)
    }
    pub fn asset(self) -> Self {
        self.typed(CardType::Asset)
    }
    pub fn upgrade(self) -> Self {
        self.typed(CardType::Upgrade)
    }
    pub fn identity(self) -> Self {
        self.typed(CardType::Identity)
    }
    /// "ICE: Barrier. Rez 3, strength 1."
    pub fn ice(mut self, strength: i32) -> Self {
        self.printed.strength = Some(strength);
        self.typed(CardType::Ice)
    }
    /// "Agenda: Initiative. 3/2." — advancement requirement and points.
    pub fn agenda(mut self, advancement: u32, points: i32) -> Self {
        self.printed.advancement_requirement = Some(advancement);
        self.printed.agenda_points = Some(points);
        // CR 2.3: an agenda has no play/install/rez cost at all.
        self.printed.cost = None;
        self.typed(CardType::Agenda)
    }

    // ---- the numbers under the name -------------------------------------
    pub fn subtypes(mut self, s: &[&'static str]) -> Self {
        self.printed.subtypes = s.to_vec();
        self
    }
    /// Play, install or rez cost (2.3).
    pub fn cost(mut self, n: u32) -> Self {
        self.printed.cost = Some(n);
        self
    }
    /// Icebreaker strength (2.7); ice takes its strength in `.ice(n)`.
    pub fn strength(mut self, n: i32) -> Self {
        self.printed.strength = Some(n);
        self
    }
    pub fn trash_cost(mut self, n: u32) -> Self {
        self.printed.trash_cost = Some(n);
        self
    }
    pub fn memory(mut self, n: u32) -> Self {
        self.printed.memory_cost = Some(n);
        self
    }
    /// The ◆ in front of the name (2.2).
    pub fn unique(mut self) -> Self {
        self.printed.unique = true;
        self
    }
    pub fn console(mut self) -> Self {
        self.printed.console = true;
        self
    }
    /// Printed base link (1.20). The kernel reads link as the sum of active
    /// declarations, so a printed number is a declaration of the identity.
    pub fn link(mut self, n: i32) -> Self {
        self.printed
            .abilities
            .push(AbilityDef::static_ability(vec![StaticDecl::LinkBonus(n)]).labeled("base link"));
        self
    }
    /// "N[recurring]" (1.10.5).
    pub fn recurring_credits(mut self, n: u32) -> Self {
        self.printed.recurring_credits = Some(n);
        self
    }

    // ---- the printed text ------------------------------------------------
    /// One printed line of the card's text box, copied exactly. Call it once
    /// per line; the lines are joined in order. **Required** — a card with no
    /// text fails the deck test, because behaviour is checked against it.
    /// "You draw a starting hand of N cards." (Andromeda class; 1.6.6.)
    pub fn starting_hand(mut self, n: u32) -> Self {
        self.printed.starting_hand_size = Some(n);
        self
    }
    /// The identity's back face (rule_identity_double_sided; Nebula class).
    /// Build the back exactly like a card — its own printed text and
    /// abilities — and "flip this identity" swaps which face applies.
    pub fn flip_face(mut self, back: Card) -> Self {
        self.printed.flip_face = Some(Box::new(back.printed));
        self
    }
    pub fn text(mut self, line: &'static str) -> Self {
        self.text.push(line);
        self
    }

    /// A printed sentence the vocabulary cannot say yet, quoted from the
    /// card. The card still exists and still does everything it *can* do, but
    /// it is counted honestly as partial (SYS-D-9/D-12). Never approximate a
    /// sentence: record it here instead, and put the missing kernel
    /// capability on the gap list in `docs/vm/WAVES.md`.
    pub fn unimplemented(mut self, printed_sentence: &'static str) -> Self {
        self.unimplemented.push(printed_sentence);
        self
    }

    // ---- 1.16.10 printed additional costs ---------------------------------
    /// "As an additional cost to play this operation, …"
    pub fn additional_play_cost(mut self, c: Cost) -> Self {
        self.printed.additional_play_cost = Some(c);
        self
    }
    /// "As an additional cost to steal **this** agenda, …" (printed on the
    /// agenda). A card that taxes *every* steal declares
    /// [`StaticDecl::AdditionalStealCost`] in `.declares(…)` instead.
    pub fn additional_steal_cost(mut self, c: Cost) -> Self {
        self.printed.additional_steal_cost = Some(c);
        self
    }

    // ---- the abilities ----------------------------------------------------
    /// The ability of an event or operation, resolved as it is played (9.7).
    pub fn play(self, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(AbilityDef::play(instrs.into_iter().collect()))
    }
    /// One `[subroutine]` line (9.8). Write one call per printed subroutine,
    /// in printed order.
    pub fn subroutine(self, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(AbilityDef::subroutine(instrs.into_iter().collect()))
    }
    /// "1[credit]: …" — a paid ability (9.5). The cost is everything printed
    /// before the colon.
    pub fn paid(self, cost: Cost, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(AbilityDef::paid(cost, instrs.into_iter().collect()))
    }
    /// "Interface → 1[credit]: …" — a paid ability gated by strength (9.3.6c)
    /// and usable only during an encounter (9.5.6a). Naming a subtype makes
    /// it usable only against ice of that kind (9.5.6c), which is exactly
    /// what "break 1 **sentry** subroutine" means.
    pub fn paid_interface(
        self,
        cost: Cost,
        ice_subtype: Option<&'static str>,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_flag(AbilityFlag::Interface)
                .with_timing(TimingRestriction::EncounterOnly { required_subtype: ice_subtype }),
        )
    }
    /// "Access → 1[credit]: …" — usable only in the mid-access window
    /// (9.3.6b).
    pub fn paid_access(self, cost: Cost, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect()).with_flag(AbilityFlag::Access),
        )
    }
    /// A paid ability usable only during an encounter (9.5.6a) — a break
    /// ability that is not an interface ability, or a Runner card that acts
    /// on the ice being encountered.
    pub fn paid_during_encounter(
        self,
        cost: Cost,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_timing(TimingRestriction::EncounterOnly { required_subtype: None }),
        )
    }
    /// "When <trigger>, …" — a conditional ability (9.6). Mandatory: its
    /// controller must resolve it.
    pub fn when(self, cond: TriggerCond, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(AbilityDef::conditional(cond, instrs.into_iter().collect(), false))
    }
    /// "The first time each turn <trigger>, …" — the once-per-turn flag
    /// (9.3.6g) carrying the card's "first time each turn".
    pub fn when_first_each_turn(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), false)
                .with_flag(AbilityFlag::OncePerTurn),
        )
    }
    /// "When <trigger>, you may …" — the same, declinable (9.6.9). Use this
    /// only where the "may" is the WHOLE ability; a "may" inside one sentence
    /// is [`may`] or [`may_pay`], which put the choice where the card does.
    pub fn may_when(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(AbilityDef::conditional(cond, instrs.into_iter().collect(), true))
    }
    /// "[interrupt] → …" (9.3.6d/9.9.1).
    pub fn interrupt(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), true)
                .with_flag(AbilityFlag::Interrupt),
        )
    }
    /// "[interrupt] → <cost>: …" — a PAID ability carrying the interrupt
    /// flag (9.3.6d; the Decoy class): it joins open interrupt windows
    /// freely (9.9.4d), offered where its effect is relevant (9.9.7f).
    pub fn interrupt_paid(
        self,
        cost: Cost,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_flag(AbilityFlag::Interrupt),
        )
    }
    /// The sentences that are permanently true rather than things that
    /// happen — a static ability's declarations (9.4). They never resolve;
    /// the engine reads them continuously.
    pub fn declares(self, decls: impl IntoIterator<Item = StaticDecl>) -> Self {
        self.ability(AbilityDef::static_ability(decls.into_iter().collect()))
    }
    /// "Threat N → …" (9.3.6f): declarations active only once a player has N
    /// agenda points.
    pub fn declares_at_threat(
        self,
        n: u8,
        decls: impl IntoIterator<Item = StaticDecl>,
    ) -> Self {
        self.ability(
            AbilityDef::static_ability(decls.into_iter().collect())
                .with_flag(AbilityFlag::Threat(n)),
        )
    }

    /// The escape hatch: attach an ability you built yourself. Everything
    /// above is a shorthand for this, so nothing is hidden — reach for it
    /// when a card wants a combination the shorthands do not name (a
    /// once-per-turn conditional, a static condition, a subroutine with a
    /// timing restriction).
    pub fn ability(mut self, def: AbilityDef) -> Self {
        let label = self.auto_label(&def);
        self.printed.abilities.push(def.labeled(label));
        self
    }

    /// Rename the ability added last. Labels are what tests and logs pick
    /// abilities out by, so name one whenever a card has two of a kind.
    pub fn named(mut self, label: &'static str) -> Self {
        let name = self.printed.name;
        if let Some(a) = self.printed.abilities.last_mut() {
            a.label = leak(&format!("{}: {label}", name.to_lowercase()));
        }
        self
    }

    /// A label that names the card and the kind of ability, with an ordinal
    /// when the card has more than one of that kind.
    fn auto_label(&self, def: &AbilityDef) -> &'static str {
        let kind = match (&def.condition, &def.cost, def.kind) {
            (_, _, jinteki_cr::ability::AbilityKind::Play) => "play".to_string(),
            (_, _, jinteki_cr::ability::AbilityKind::Subroutine) => "[sub]".to_string(),
            (_, _, jinteki_cr::ability::AbilityKind::Static) => "static".to_string(),
            (Some(Condition::Trigger(t)), _, _) => format!("when {}", trigger_word(t)),
            (_, Some(_), _) => "paid".to_string(),
            _ => "ability".to_string(),
        };
        let n = self
            .printed
            .abilities
            .iter()
            .filter(|a| a.label.rsplit(": ").next().map(|s| s.starts_with(&kind)).unwrap_or(false))
            .count();
        let suffix = if n > 0 { format!(" {}", n + 1) } else { String::new() };
        leak(&format!("{}: {kind}{suffix}", self.printed.name.to_lowercase()))
    }

    /// Finish the card.
    pub fn build(self) -> Card {
        assert!(self.side_set, "{}: say whether it is a .corp() or a .runner() card", self.printed.name);
        assert!(self.type_set, "{}: say what type of card it is, e.g. .event()", self.printed.name);
        assert!(
            !self.text.is_empty(),
            "{}: copy the printed text into .text(…) — behaviour is checked against it (SYS-D-10)",
            self.printed.name
        );
        Card {
            oracle_text: self.text.join("\n"),
            printed: self.printed,
            unimplemented: self.unimplemented,
        }
    }
}

/// A short word for a trigger, for auto-generated labels.
fn trigger_word(t: &TriggerCond) -> &'static str {
    match t {
        TriggerCond::TurnBegins(_) => "turn begins",
        TriggerCond::SelfScored { .. } => "scored",
        TriggerCond::SelfStolen => "stolen",
        TriggerCond::SelfInstalled => "installed",
        TriggerCond::SelfEmpty { .. } => "empty",
        TriggerCond::SelfEncountered => "encountered",
        TriggerCond::SelfAccessed { .. } => "accessed",
        TriggerCond::SelfPassed => "passed",
        TriggerCond::SelfPlayResolved => "resolved",
        TriggerCond::RunEnds { successful_only: true } => "successful run ends",
        TriggerCond::RunEnds { .. } => "run ends",
        _ => "trigger",
    }
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

// ===========================================================================
// The sentences — grouped by what the card says
// ===========================================================================

// ---- credits --------------------------------------------------------------

/// "Gain N[credit]."
pub fn gain(side: Side, n: i64) -> Instruction {
    Instruction::GainCredits(side, Quantity::c(n))
}
/// "Gain N[credit] for each …" — a calculated amount (9.12.2).
pub fn gain_q(side: Side, n: Quantity) -> Instruction {
    Instruction::GainCredits(side, n)
}
/// "Lose N[credit]." / "The Runner loses N[credit]."
pub fn lose(side: Side, n: u32) -> Instruction {
    Instruction::LoseCredits(side, Quantity::c(n as i64))
}

// ---- cards ----------------------------------------------------------------

/// "Draw N cards."
pub fn draw(side: Side, n: u32) -> Instruction {
    Instruction::Draw(side, n)
}
/// "Add <cards> to your grip/HQ."
pub fn add_to_hand(cards: TargetSpec) -> Instruction {
    Instruction::AddCardsToHand { cards }
}
/// "Add <cards> to <side>'s score area." (1.17.3e/f / 10.1.3.) A card added
/// this way is NOT scored or stolen, so nothing a "when you score"/"when the
/// Runner steals" condition could meet is recorded. `as_agenda` is 10.1.3's
/// conversion — `Some(n)` turns a non-agenda into an agenda worth n points,
/// `None` adds a card that is already an agenda and keeps its own value.
pub fn add_to_score_area(cards: TargetSpec, to: Side, as_agenda: Option<i32>) -> Instruction {
    Instruction::AddToScoreArea { cards, to, as_agenda }
}
/// "Add <cards> to the top / bottom of <a deck>."
pub fn add_to_deck(card: TargetSpec, top: bool) -> Instruction {
    Instruction::MoveToDeck { card, top }
}
/// "Search your stack for <criteria>." (8.7; the stack is reshuffled after.)
pub fn search_stack(criteria: &[TargetFilter], count: i64) -> Instruction {
    Instruction::Search {
        zone: jinteki_cr::object::Zone::Deck(Side::Runner),
        criteria: criteria.to_vec(),
        count: Quantity::c(count),
        may_fail: true,
    }
}

// ---- damage ---------------------------------------------------------------

/// "Do N net damage." — `by` is who is responsible (10.4.1).
pub fn net_damage(by: Side, n: i64) -> Instruction {
    damage(by, DamageKind::Net, n)
}
/// "Do N meat damage."
pub fn meat_damage(by: Side, n: i64) -> Instruction {
    damage(by, DamageKind::Meat, n)
}
/// "Do N core damage."
pub fn core_damage(by: Side, n: i64) -> Instruction {
    damage(by, DamageKind::Core, n)
}
fn damage(by: Side, kind: DamageKind, n: i64) -> Instruction {
    Instruction::Damage { kind, amount: Quantity::c(n), responsible: by }
}
/// "Prevent all meat damage." (9.9.7b — an interrupt effect.)
pub fn prevent_all_meat_damage() -> Instruction {
    Instruction::PreventAllDamage { kind: DamageKind::Meat }
}
/// "Prevent all net damage."
pub fn prevent_all_net_damage() -> Instruction {
    Instruction::PreventAllDamage { kind: DamageKind::Net }
}

// ---- tags -----------------------------------------------------------------

/// "Give the Runner N tags." / "Take N tags."
pub fn give_tags(n: u32) -> Instruction {
    Instruction::GainTags(n)
}
/// "Remove N tags."
pub fn remove_tags(n: i64) -> Instruction {
    Instruction::RemoveTags(Quantity::c(n))
}

// ---- the run --------------------------------------------------------------

/// "End the run."
pub fn end_the_run() -> Instruction {
    Instruction::EndTheRun
}
/// "Run <server>."
pub fn run(server: ServerId) -> Instruction {
    Instruction::run(server)
}
/// "Run <server>. If successful, …" (6.7.4.)
pub fn run_then_if_successful(
    server: ServerId,
    if_successful: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::InitiateRun {
        // 6.9.1a: `server` is the announcement position, `None` where the
        // effect leaves the choice to the Runner (jinteki-cr, W16b).
        server: Some(server),
        allowed: RunServerSet::These(vec![server]),
        if_successful: if_successful.into_iter().collect(),
    }
}
/// "Run any server. If successful, …" — the effect names no server, so the
/// Runner announces the attacked one at step 6.9.1a from everything 6.7.4a
/// allows (minus any server 6.3.2a forbids initiating a run on).
pub fn run_any_server(if_successful: impl IntoIterator<Item = Instruction>) -> Instruction {
    Instruction::run_any_server(if_successful.into_iter().collect())
}
/// "Bypass the ice you are encountering." (6.5.8.)
pub fn bypass_encountered_ice() -> Instruction {
    Instruction::BypassEncounteredIce
}
/// "The Runner encounters <ice>." — a forced encounter (6.5.9a).
pub fn force_encounter(ice: TargetSpec) -> Instruction {
    Instruction::ForceEncounter { ice }
}

// ---- this card ------------------------------------------------------------

/// "Trash this card."
pub fn trash_self() -> Instruction {
    Instruction::TrashSelf
}
/// "Remove this card from the game."
pub fn remove_self_from_game() -> Instruction {
    Instruction::RemoveSelfFromGame
}
/// "Trash <targets>."
pub fn trash(targets: TargetSpec) -> Instruction {
    Instruction::TrashCards(targets)
}
/// "Purge virus counters." (10.1.2.)
pub fn purge_virus_counters() -> Instruction {
    Instruction::PurgeVirusCounters
}
/// "Your action phase ends." (5.6.2b — the Terminal keyword.)
pub fn end_action_phase(side: Side) -> Instruction {
    Instruction::EndActionPhase(side)
}
/// "Host <cards> on <host>." (1.13.1.)
pub fn host(cards: TargetSpec, host: TargetSpec) -> Instruction {
    Instruction::HostCards { cards, host }
}

// ---- counters -------------------------------------------------------------

/// "Load N[credit] onto this card." (10.9.1 — a placement that also marks the
/// kind LOADED, which is what a "when it is empty" ability is linked to.)
pub fn load(kind: CounterKind, n: i64) -> Instruction {
    Instruction::LoadCounters {
        target: TargetSpec::SelfSource,
        kind,
        amount: Quantity::c(n),
    }
}
/// "Place N <kind> counters on this card." (1.18.2 — placing, not loading,
/// and never advancing.)
pub fn place(kind: CounterKind, n: i64) -> Instruction {
    place_on(TargetSpec::SelfSource, kind, n)
}
/// "Place N <kind> counters on <target>."
pub fn place_on(target: TargetSpec, kind: CounterKind, n: i64) -> Instruction {
    Instruction::PlaceCounters { target, kind, amount: Quantity::c(n) }
}
/// "Take N[credit] from this card." (1.10.3a — hosted credits move into a
/// credit pool, which is a GAIN. A card with fewer gives what it has.)
pub fn take_hosted_credits(from: TargetSpec, n: i64, to: Side) -> Instruction {
    Instruction::TakeHostedCredits { from, amount: Quantity::c(n), to }
}
/// "Remove N hosted <kind> counters." (1.9.2 — they return to the bank. This
/// is the mandatory-effect counterpart of `hosted_counters(…)` as a COST.)
pub fn remove_counters(kind: CounterKind, n: i64) -> Instruction {
    Instruction::RemoveCounters {
        target: TargetSpec::SelfSource,
        kind,
        amount: Quantity::c(n),
        up_to: false,
    }
}
/// "Reveal <cards>." (1.21.3 — shown to all players, then returned to their
/// previous state. 1.21.3a: NOT turning a card faceup.)
pub fn reveal(cards: TargetSpec) -> Instruction {
    Instruction::RevealCards { cards }
}
/// "Advance <a card>." (1.18.1 — an advance, so "whenever you advance"
/// conditions are met.)
pub fn advance(target: TargetSpec) -> Instruction {
    Instruction::AdvanceCard { target }
}

// ---- strength and subroutines ---------------------------------------------

/// "+N strength" / "−N strength" on this card. With no stated duration, an
/// icebreaker modifying its own strength keeps it for the rest of the
/// encounter (3.9.5b).
pub fn pump(n: i32) -> Instruction {
    Instruction::ModifyStrength {
        target: TargetSpec::SelfSource,
        amount: Quantity::c(n as i64),
        duration: None,
    }
}
/// "Break N subroutines."
pub fn break_subroutines(n: i64) -> Instruction {
    Instruction::BreakSubroutines {
        subs: SubroutineSpec::Chosen { count: Quantity::c(n), up_to: false },
    }
}
/// "Break up to N subroutines."
pub fn break_up_to(n: i64) -> Instruction {
    Instruction::BreakSubroutines {
        subs: SubroutineSpec::Chosen { count: Quantity::c(n), up_to: true },
    }
}
/// "Break all subroutines." (9.8.6a — targets nothing.)
pub fn break_all_subroutines() -> Instruction {
    Instruction::BreakSubroutines { subs: SubroutineSpec::All }
}

// ---- installing and playing -----------------------------------------------

/// "Install up to N cards from HQ (one at a time)." (8.5.5.)
pub fn install_cards_from_hand(
    count: u32,
    from_hand_of: Side,
    filter: InstallFilter,
    dest: InstallDest,
) -> Instruction {
    Instruction::InstallCards {
        count,
        from_hand_of,
        filter,
        dest,
        and_rez: false,
        and_rez_if_able: false,
        ignore_costs: false,
    }
}
/// "Install <a card>." (8.5.)
pub fn install(card: TargetSpec, dest: InstallDest) -> Instruction {
    Instruction::InstallCard {
        card,
        dest,
        and_rez: false,
        ignore_costs: false,
        reveal_check: None,
        reduce_total: Quantity::c(0),
    }
}
/// "You may play N operations from HQ." (8.6.3 — chosen one at a time, and
/// "up to" is built in, so the printed "you may" is already here.)
pub fn play_cards_from_hand(count: u32, from_hand_of: Side) -> Instruction {
    Instruction::PlayCards { count, from_hand_of, ignore_costs: false }
}
/// "Rez <a card>." (8.1.2.)
pub fn rez(target: TargetSpec) -> Instruction {
    Instruction::RezCard { target, ignore_costs: false }
}
/// "Resolve the \"when scored\" ability of <an agenda>." (9.6.14c/d.)
pub fn resolve_when_scored_ability_of(source: TargetSpec) -> Instruction {
    Instruction::ResolveAbilityOf {
        source,
        which: jinteki_cr::ability::AbilityClass::WhenScored,
    }
}

// ---- the sentences built out of other sentences ---------------------------

/// Several effects in ONE printed sentence: "Gain 4[credit] **and** draw 3
/// cards." (9.11.4a.)
///
/// Use this only where the effects would AGGREGATE (9.12.2c) — same class,
/// one occurrence. Effects of different classes in one sentence are two
/// instructions resolved in order: pass them as two list items instead. The
/// kernel resolves a `Combined` by walking its effect ATOMS, and an effect
/// with no value-carrying atom (removing counters, say) is dropped silently
/// if it is put here.
pub fn combined(instrs: impl IntoIterator<Item = Instruction>) -> Instruction {
    Instruction::Combined(instrs.into_iter().collect())
}
/// "You may …" — an optional part its controller may decline (9.6.9c).
pub fn may(instr: Instruction) -> Instruction {
    Instruction::DeclineableChoice(Box::new(instr))
}
/// "You may pay <cost> to …" (1.16.11a): paying is the choice, and the
/// paid-for branch is what follows.
pub fn may_pay(cost: Cost, instr: Instruction) -> Instruction {
    Instruction::NestedCostThen { cost, effect: Box::new(instr), payer: None }
}
/// "… **unless** the Runner pays <cost>." (1.16.11b): paying suppresses the
/// effect; declining makes it happen.
pub fn unless_pays(payer: Side, cost: Cost, instr: Instruction) -> Instruction {
    Instruction::NestedCostUnless { cost, effect: Box::new(instr), payer: Some(payer) }
}
/// "Resolve 1 of the following." (9.11.4g.) Each option is labelled with the
/// printed words of that option.
pub fn choose_one(
    options: impl IntoIterator<Item = (&'static str, Vec<Instruction>)>,
) -> Instruction {
    Instruction::ChooseOne { options: options.into_iter().collect() }
}
/// "Trace[N]. If successful, …" (10.8.) One sentence, one instruction.
pub fn trace(base: i64, if_successful: impl IntoIterator<Item = Instruction>) -> Instruction {
    Instruction::Trace {
        base: Quantity::c(base),
        if_successful: if_successful.into_iter().collect(),
        if_unsuccessful: Vec::new(),
        determined_min: None,
    }
}
/// "Trace[N]. If successful, …; if unsuccessful, …"
pub fn trace_both(
    base: i64,
    if_successful: impl IntoIterator<Item = Instruction>,
    if_unsuccessful: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::Trace {
        base: Quantity::c(base),
        if_successful: if_successful.into_iter().collect(),
        if_unsuccessful: if_unsuccessful.into_iter().collect(),
        determined_min: None,
    }
}
/// "Instead of breaching <the server>, …" (9.9.2b): a replacement created
/// ahead of the breach and applied where the breach would have happened
/// (step 6.9.5b). `optional` is the printed "you may" — 6.7.4c puts that
/// decision with the Runner. The replacing instructions resolve through the
/// ordinary pipeline, which is what keeps their damage and tags preventable.
pub fn instead_of_breaching(
    optional: bool,
    instrs: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::CreateLingeringEffect {
        payload: LingeringSpec::Replacement {
            applies_to: EffectClass::Breach,
            with: ReplacementTransform::SuppressAndResolve(instrs.into_iter().collect()),
            optional,
        },
        duration: WantedDuration::ThisRun,
    }
}

/// "<The named player> does …" (1.14.5): the player who carries the effect
/// out and makes its choices.
pub fn performed_by(side: Side, instr: Instruction) -> Instruction {
    Instruction::PerformedBy { side, instr: Box::new(instr) }
}

// ---- what an instruction acts on (1.15.2) ---------------------------------

/// "this card" — the ability's own source.
pub fn this_card() -> TargetSpec {
    TargetSpec::SelfSource
}
/// "the card you are accessing" (7.2).
pub fn accessed_card() -> TargetSpec {
    TargetSpec::AccessedCard
}
/// "the ice you are encountering."
pub fn encountered_ice() -> TargetSpec {
    TargetSpec::EncounteredIce
}
/// "N <description>" — announced by the ability's controller (1.15.2). The
/// criteria are a conjunction: `&[installed_runner_card(), program()]` is
/// "an installed program".
pub fn choose(count: i64, criteria: &[TargetFilter]) -> TargetSpec {
    TargetSpec::Choose { count: Quantity::c(count), criteria: criteria.to_vec(), up_to: false }
}

// The filter atoms, named the way a card describes cards.
pub fn installed_corp_card() -> TargetFilter {
    TargetFilter::InstalledCorpCard
}
pub fn installed_runner_card() -> TargetFilter {
    TargetFilter::InstalledRunnerCard
}
pub fn rezzed() -> TargetFilter {
    TargetFilter::Rezzed
}
pub fn of_type(t: CardType) -> TargetFilter {
    TargetFilter::CardTypeIs(t)
}
pub fn with_subtype(s: &'static str) -> TargetFilter {
    TargetFilter::HasSubtype(s)
}
pub fn named_card(n: &'static str) -> TargetFilter {
    TargetFilter::HasName(n)
}
pub fn in_archives() -> TargetFilter {
    TargetFilter::InDiscardOf(Corp)
}
pub fn in_heap() -> TargetFilter {
    TargetFilter::InDiscardOf(Runner)
}
pub fn in_hand_of(side: Side) -> TargetFilter {
    TargetFilter::CardsInHandOf(side)
}
pub fn in_score_area_of(side: Side) -> TargetFilter {
    TargetFilter::InScoreAreaOf(side)
}
pub fn other_than_this_card() -> TargetFilter {
    TargetFilter::OtherThanSource
}
/// "cards hosted on this card" (1.13.2) — installed or not.
pub fn hosted_on_this_card() -> TargetFilter {
    TargetFilter::HostedOnSource
}
/// "a card you did NOT install this turn" (1.12.6) — a question about the
/// game history, which is open information to both players. Pass `true` for
/// the other polarity, "installed during this turn".
pub fn installed_this_turn(yes: bool) -> TargetFilter {
    TargetFilter::InstalledThisTurn(yes)
}
/// "a card you can advance" (1.18.3) — the PERMISSION as a criterion, read
/// from the same place the basic advance action reads it, so a card the
/// action would refuse cannot be described here either. (The declaration
/// that GRANTS the permission to its own card is `can_be_advanced()`.)
pub fn advanceable() -> TargetFilter {
    TargetFilter::CanBeAdvanced
}

// ---- what an ability costs (1.16) -----------------------------------------

/// No cost at all.
pub fn free() -> Cost {
    Cost::free()
}
/// "N[credit]:"
pub fn credits(n: u32) -> Cost {
    Cost::credits(n)
}
/// "[click]:" — N of them.
pub fn clicks(n: u32) -> Cost {
    Cost { clicks: n, ..Cost::default() }
}
/// "[trash]:" — trash this card to use it.
pub fn trash_this_card() -> Cost {
    Cost::trash_self()
}
/// "Hosted power counter:" — spend N counters of a kind from this card
/// (1.9.2), which is what makes an empty card's ability unusable.
pub fn hosted_counters(kind: CounterKind, n: u32) -> Cost {
    Cost::spend_counters(kind, n)
}
/// "Forfeit an agenda" as a cost (8.2.5).
pub fn forfeit_agenda(n: u32) -> Cost {
    Cost::forfeit_agenda(n)
}
/// "Suffer N net damage" as a cost (Obokata class).
pub fn suffer_net_damage(n: u32) -> Cost {
    Cost::net_damage(n)
}
/// "Take N tags" as a cost (Funhouse class).
pub fn take_tags(n: u32) -> Cost {
    Cost::tags(n)
}

/// Two costs paid as one, all at once (1.16.10b): `clicks(3).plus(trash_this_card())`.
pub trait CostExt {
    fn plus_cost(self, other: Cost) -> Cost;
}
impl CostExt for Cost {
    fn plus_cost(self, other: Cost) -> Cost {
        self.plus(&other)
    }
}

// ---- when it happens (9.6) ------------------------------------------------

/// "When your turn begins, …"
pub fn turn_begins(side: Side) -> TriggerCond {
    TriggerCond::TurnBegins(side)
}
/// "When you score this agenda, …"
pub fn scored() -> TriggerCond {
    TriggerCond::SelfScored { requires: Vec::new() }
}
/// "When the Runner steals this agenda, …"
pub fn stolen() -> TriggerCond {
    TriggerCond::SelfStolen
}
/// "When you install this card, …"
pub fn installed() -> TriggerCond {
    TriggerCond::SelfInstalled
}
/// "When it is empty, …" (10.9.2 — only after the card was LOADED.)
pub fn empty_of(kind: CounterKind) -> TriggerCond {
    TriggerCond::SelfEmpty { kind }
}
/// "When the Runner encounters this ice, …"
pub fn encountered() -> TriggerCond {
    TriggerCond::SelfEncountered
}
/// "When the Runner accesses this card, …"
pub fn accessed() -> TriggerCond {
    TriggerCond::SelfAccessed { requires: Vec::new() }
}
/// "When the Runner passes this ice, …"
pub fn passed() -> TriggerCond {
    TriggerCond::SelfPassed
}
/// "When this run ends, …"
pub fn run_ends() -> TriggerCond {
    TriggerCond::RunEnds { successful_only: false }
}
/// "After you resolve this operation, …" (8.6.7h.)
pub fn after_this_resolves() -> TriggerCond {
    TriggerCond::SelfPlayResolved
}
/// "Whenever you make a successful run" — any server (6.8.4).
pub fn makes_successful_run() -> TriggerCond {
    TriggerCond::MakesSuccessfulRun { on: None }
}
/// "…makes a successful run on <these servers>" (Gemilang class).
pub fn makes_successful_run_on(servers: &[ServerId]) -> TriggerCond {
    TriggerCond::MakesSuccessfulRun { on: Some(servers.to_vec()) }
}
/// "When your action phase ends, if <requirements>…" (Nebula class; 5.6.2.)
pub fn action_phase_ends_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::ActionPhaseEnds { side, requires: reqs.to_vec() }
}
/// "…you play an operation" (Gemilang class; the trigger half — pair with
/// [`CardBuilder::when_first_each_turn`] for the printed "first time each
/// turn").
pub fn plays_a(by: Side, of: CardType) -> TriggerCond {
    TriggerCond::CardPlayed { by, of_types: vec![of] }
}
/// "…if you played an operation this turn" (Nebula class; history 1.12.6).
pub fn played_operation_this_turn(side: Side) -> TriggerRequirement {
    TriggerRequirement::PlayedOperationThisTurn(side)
}
/// "Gain [click]." (1.11.3a.)
pub fn gain_clicks(side: Side, n: u32) -> Instruction {
    Instruction::GainClicks(side, Quantity::c(n as i64))
}
/// "<side> loses N credits" with a calculated amount (1.10.3b; W17c made the
/// position a quantity).
pub fn loses_credits(side: Side, amount: Quantity) -> Instruction {
    Instruction::LoseCredits(side, amount)
}
/// "…if the Runner has at least N tags" (BOOM! class; RunnerTagsAtLeast(1)
/// IS "tagged").
pub fn runner_tags_at_least(n: u32) -> TriggerRequirement {
    TriggerRequirement::RunnerTagsAtLeast(n)
}
/// "When a discard phase ends, if <requirements>…" (5.5.4 / Breaking News class.)
pub fn discard_phase_ends_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::DiscardPhaseEnds { side, requires: reqs.to_vec() }
}
/// "…if you scored this agenda this turn" (Breaking News class).
pub fn self_scored_this_turn() -> TriggerRequirement {
    TriggerRequirement::SelfScoredThisTurn
}
/// "…if you installed this resource this turn" (The Class Act class).
pub fn self_installed_this_turn() -> TriggerRequirement {
    TriggerRequirement::SelfInstalledThisTurn
}
/// "Trace[N]. If unsuccessful, …" (10.8.6.)
pub fn trace_if_unsuccessful(base: i64, if_unsuccessful: impl IntoIterator<Item = Instruction>) -> Instruction {
    Instruction::Trace {
        base: Quantity::c(base),
        if_successful: Vec::new(),
        if_unsuccessful: if_unsuccessful.into_iter().collect(),
        determined_min: None,
    }
}
/// "Shuffle up to N cards from <side>'s discard into their deck" (Jackson
/// class; the targets are announced, "up to" makes the floor zero).
pub fn shuffle_from_discard_into_deck(side: Side, up_to: i64) -> Instruction {
    Instruction::ShuffleCardsIntoDeck {
        targets: TargetSpec::Choose {
            count: Quantity::c(up_to),
            criteria: vec![TargetFilter::InDiscardOf(side)],
            up_to: true,
        },
        to: side,
    }
}
/// "Remove 1 card in the heap from the game." (Bloo Moose class; §4.9.)
pub fn remove_from_heap_from_game(count: i64) -> Instruction {
    Instruction::RemoveCardsFromGame {
        targets: TargetSpec::Choose {
            count: Quantity::c(count),
            criteria: vec![TargetFilter::InDiscardOf(Side::Runner)],
            up_to: false,
        },
    }
}
/// "Remove <this card> from the game:" as a trigger cost (Jackson class).
pub fn remove_self_cost() -> Cost {
    Cost { remove_self_from_game: true, ..Default::default() }
}
/// "[trash], trash all cards from your grip:" (Citadel Sanctuary class).
pub fn trash_self_and_grip() -> Cost {
    Cost { trash_self: true, trash_all_from_hand: true, ..Default::default() }
}
/// "Prevent all <kind> damage." (9.9.7b.)
pub fn prevent_all_damage(kind: DamageKind) -> Instruction {
    Instruction::PreventAllDamage { kind }
}
/// "…would suffer <kind> damage" as an interrupt condition (9.9.4).
pub fn would_damage(kind: DamageKind) -> TriggerCond {
    TriggerCond::WouldDamage { kind: Some(kind), first_each_run: false }
}
/// "Host the <accessed> card on this program/resource." (Cupellation and
/// Film Critic class; the accessed card is no longer being accessed.)
pub fn host_accessed_on_self() -> Instruction {
    Instruction::HostCards { cards: TargetSpec::AccessedCard, host: TargetSpec::SelfSource }
}
/// "…access 2 additional cards." (Cupellation class; 7.3.5.)
pub fn additional_accesses(n: i64) -> Instruction {
    Instruction::AdditionalAccesses(Quantity::c(n))
}
/// "Whenever you breach <server>, if <requirements>…"
pub fn breaches_server_if(server: ServerId, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::BreachesServer { server, requires: reqs.to_vec() }
}
/// "…if this program has a hosted Corp card" (Cupellation class).
pub fn source_hosts_corp_card() -> TriggerRequirement {
    TriggerRequirement::SourceHostsCorpCard
}
/// "…this ice in R&D" / "…anywhere except in Archives" — zone stipulations
/// on a trigger (9.6.5c class; Archangel).
pub fn source_in_rnd() -> TriggerRequirement {
    TriggerRequirement::SourceInDeck
}
pub fn source_not_in_archives() -> TriggerRequirement {
    TriggerRequirement::SourceNotInDiscard
}
/// "…they encounter it." (6.5.9a; Archangel class.)
pub fn force_encounter_self() -> Instruction {
    Instruction::ForceEncounter { ice: TargetSpec::SelfSource }
}
/// "Reveal <this card>." (1.21.3.)
pub fn reveal_self() -> Instruction {
    Instruction::RevealCards { cards: TargetSpec::SelfSource }
}
/// "Add 1 installed Runner card to the grip." (Archangel class; 8.1.)
pub fn add_installed_runner_card_to_grip() -> Instruction {
    Instruction::AddCardsToHand {
        cards: TargetSpec::Choose {
            count: Quantity::c(1),
            criteria: vec![TargetFilter::InstalledRunnerCard],
            up_to: false,
        },
    }
}
/// "…access 1 card in the root of another server. If that card is an agenda,
/// you cannot steal or trash it during this access." (Pinhole class.)
pub fn access_one_root_of_another_server_restricted() -> Instruction {
    Instruction::AccessCards {
        cards: TargetSpec::Choose {
            count: Quantity::c(1),
            criteria: vec![TargetFilter::InRootOfServerOtherThanAttacked],
            up_to: false,
        },
        restricted: true,
    }
}
/// "…flip this identity." (rule_identity_double_sided.)
pub fn flip_identity(side: Side) -> Instruction {
    Instruction::FlipIdentity(side)
}
/// "all credits in their credit pool" (Closed Accounts class; 1.10.)
pub fn credits_in_pool_of(side: Side) -> Quantity {
    Quantity::CreditsInPoolOf(side)
}
/// "Whenever the Corp scores an agenda…" (1.17.6.)
pub fn corp_scores_agenda() -> TriggerCond {
    TriggerCond::CorpScoresAgenda
}
/// "Whenever the Runner steals an agenda…" (1.17.7.)
pub fn runner_steals_agenda() -> TriggerCond {
    TriggerCond::RunnerStealsAgenda
}
/// "When a discard phase ends, …" (5.5.4). CR 5.1.4b puts it at the same step
/// as the turn formally ending, so it is that occurrence read as a different
/// sentence.
pub fn discard_phase_ends(side: Side) -> TriggerCond {
    TriggerCond::DiscardPhaseEnds { side, requires: Vec::new() }
}

// ---- states a card can require (9.6.5c / 9.1.8c) --------------------------

/// "…the Runner is tagged" (5.4).
pub fn runner_is_tagged() -> TriggerRequirement {
    TriggerRequirement::RunnerTagsAtLeast(1)
}
/// "…the Runner made a run during their last turn."
pub fn runner_made_a_run_last_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        successful_only: false,
        scope: TurnScope::LastCompletedTurn,
    }
}
/// "…the Runner made a successful run during their last turn."
pub fn runner_made_a_successful_run_last_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        successful_only: true,
        scope: TurnScope::LastCompletedTurn,
    }
}
/// "…you made a successful run this turn" (Mutual Favor class) — the same
/// question asked of the CURRENT turn.
pub fn made_a_successful_run_this_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun { successful_only: true, scope: TurnScope::ThisTurn }
}
/// "…you have at least N link" (1.20).
pub fn link_at_least(n: u32) -> TriggerRequirement {
    TriggerRequirement::RunnerLinkAtLeast(n)
}
/// "…there is an installed AI program" (IP Block class) — at least `n` cards
/// on the board match the description. The criteria are the same ones a
/// target announcement uses, so 1.15.2c applies: without a criterion naming
/// a zone this asks about INSTALLED cards, which is what "there is" means.
pub fn board_has(criteria: &[TargetFilter], n: u32) -> TriggerRequirement {
    TriggerRequirement::BoardHasMatching { criteria: criteria.to_vec(), at_least: n }
}
/// "If <state>, <effect>." (9.6.5d — the requirement is in the instructions.)
pub fn if_met(
    requires: &[TriggerRequirement],
    then: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::IfMet {
        requires: requires.to_vec(),
        then: then.into_iter().collect(),
        otherwise: Vec::new(),
    }
}
/// "If <state>, <effect>. If you do not, <other effect>." — the printed
/// two-branch form (Mutual Favor class).
pub fn if_met_else(
    requires: &[TriggerRequirement],
    then: impl IntoIterator<Item = Instruction>,
    otherwise: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::IfMet {
        requires: requires.to_vec(),
        then: then.into_iter().collect(),
        otherwise: otherwise.into_iter().collect(),
    }
}

// ---- what is permanently true (9.4) ---------------------------------------

/// "+N[mu]" (1.19.)
pub fn plus_memory(n: i32) -> StaticDecl {
    StaticDecl::MemoryLimitMod(n)
}
/// "+N link" (1.20.)
pub fn plus_link(n: i32) -> StaticDecl {
    StaticDecl::LinkBonus(n)
}
/// "This card gets +N / −N strength." A constant modification.
pub fn strength_mod(n: i32) -> StaticDecl {
    StaticDecl::StrengthMod { target_self: true, delta: n }
}
/// "This card has +1 strength for each …" — the card's strength IS this
/// expression, printed value included (the Ice Wall reading, 9.12.1b).
pub fn strength_is(q: Quantity) -> StaticDecl {
    StaticDecl::SelfStrength(q)
}
/// "This card can host <criteria>, up to N." (1.13.5.)
pub fn can_host(criteria: &[TargetFilter], capacity: Option<i64>) -> StaticDecl {
    StaticDecl::CanHost {
        criteria: criteria.to_vec(),
        capacity: capacity.map(Quantity::c),
    }
}
/// "Runs against this server cannot be declared successful." (6.9.5a.)
pub fn runs_not_declared_successful() -> StaticDecl {
    StaticDecl::RunsNotDeclaredSuccessful
}
/// "Remove this card from the game instead of trashing it." (9.9.8b.)
pub fn removed_from_game_instead_of_trashed() -> StaticDecl {
    StaticDecl::ReplaceTrashDestination {
        criteria: vec![TargetFilter::IsSource],
        to: TrashDestination::RemovedFromGame,
    }
}
/// "This card is not trashed until another current is played or an agenda is
/// stolen." (8.6.6c.)
pub fn not_trashed_until_an_agenda_is_stolen() -> StaticDecl {
    StaticDecl::PlayedNotTrashedUntilAgendaSteal
}
/// "As an additional cost to steal **an** agenda, pay <cost>." — reaching
/// every agenda, for as long as this card is active (1.16.10).
pub fn additional_cost_to_steal_any_agenda(c: Cost) -> StaticDecl {
    StaticDecl::AdditionalStealCost(c)
}
/// "Play only if <state>." (9.1.8c.) Every requirement must hold or the card
/// is not a legal play at all: the basic play action does not offer it, and
/// an effect that would play a card cannot choose it.
pub fn play_only_if(reqs: &[TriggerRequirement]) -> StaticDecl {
    StaticDecl::PlayOnlyIf(reqs.to_vec())
}
/// "You can advance this card." (1.18.3.)
pub fn can_be_advanced() -> StaticDecl {
    StaticDecl::CanBeAdvancedSelf
}

// ---- amounts that count things (9.12.2) -----------------------------------

/// A printed number.
pub fn amount(n: i64) -> Quantity {
    Quantity::c(n)
}
/// "for each tag the Runner has" (10.7).
pub fn per_runner_tag() -> Quantity {
    Quantity::RunnerTags
}
/// "for each <kind> counter hosted on this card" (9.12.2).
pub fn per_hosted_counter(kind: CounterKind) -> Quantity {
    Quantity::CountersOnSource(kind)
}
/// "for each <description>" — the count of matching cards (9.12.2a).
pub fn per_card(f: TargetFilter) -> Quantity {
    Quantity::Count(f)
}
/// "N for each …" — scale a quantity.
pub fn times(n: i64, q: Quantity) -> Quantity {
    Quantity::Times(n, Box::new(q))
}
/// "N plus 1 for each …" — a printed base plus a count.
pub fn plus(a: Quantity, b: Quantity) -> Quantity {
    Quantity::Plus(Box::new(a), Box::new(b))
}
/// "…for each credit lost" — the credits this ability has ACTUALLY caused
/// `side` to lose, which 1.10.3b caps at what their pool held. That is what
/// makes "lose up to 5" and "for each credit lost" agree.
pub fn per_credit_lost_by(side: Side) -> Quantity {
    Quantity::CreditsLostThisAbility(side)
}
