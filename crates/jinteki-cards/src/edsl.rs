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
//! // "…look at the top X cards of your stack. Add 1 of those cards to the
//! //  bottom of your stack. X is equal to the number of cards you would draw
//! //  plus 1."
//! let _ = look_at(top_of_stack(plus(cards_you_would_draw(), amount(1))), Runner);
//! let _ = add_to_deck(choose(1, &[looked_at_by_this_ability()]), false);
//! ```
//!
//! CR 1.15.1b's naming, and the sentences that refer back to what was named:
//!
//! ```
//! use jinteki_cards::edsl::*;
//! // "Name a card." / "Name a card other than <this card>." (10.1.5)
//! let _ = name_a_card("marketing target");
//! let _ = name_a_card_other_than_this_one("reclamation order target");
//! // "Name a card type." — 2.15.2 lists ten, so this is a choice of options.
//! let _ = name_a_card_type("azmari type");
//! let _ = name_one_of_these_card_types(
//!     "embezzle type",
//!     &[CardType::Asset, CardType::Ice, CardType::Operation, CardType::Upgrade],
//! );
//! // "Name sentry, code gate or barrier." / "Name a number." (1.1.3)
//! let _ = name_one_of_these_subtypes("wari subtype", &["Sentry", "Code Gate", "Barrier"]);
//! let _ = name_a_number("rng key number", WantedDuration::ThisRun);
//! // A card that is TRASHED to name says how long the name lasts.
//! let _ = name_a_card_for("whistleblower name", WantedDuration::ThisRun);
//! let _ = name_one_of_these_subtypes_for("wari subtype", &["Sentry"], WantedDuration::ThisRun);
//! // "…of the named type" / "…all copies of that card in the heap"
//! let _ = choose(1, &[in_hand_of(Runner), named_by("salem type")]);
//! let _ = all_named_cards_in_discard_of(Runner, "ark lockdown target");
//! let _ = all_named_cards_in_hand_of(Runner, "salem's name");
//! let _ = any_number_of_named_cards_in_discard_of(Corp, "reclamation order target");
//! // "…whenever the Runner plays or installs a copy of that card"
//! let _ = plays_or_installs_named_by(Runner, "marketing target");
//! // "…gain 5[credit] if the exposed card has the named card type."
//! let _ = expose(choose(1, &[in_a_remote_server()]));
//! let _ = if_met(
//!     &[earlier_choice_matches(0, &[named_by("falsified type")])],
//!     [gain(Runner, 5)],
//! );
//! let _ = add_to_hand(earlier_choice(0));
//! let _ = add_to_hand(earlier_choices());
//! ```
//!
//! CR 2.1.5's "cards with different names", for a choice and for a search:
//!
//! ```
//! use jinteki_cards::edsl::*;
//! let _ = shuffle_into_deck(choose_up_to(5, &[in_heap(), with_different_names()]), Runner);
//! let _ = search_stack(&[with_any_subtype(&["Virus", "Weapon"]), with_different_names()], 2);
//! let _ = host_faceup(found_by_search(), this_card());
//! let _ = if_met(&[board_has_at_most(&[hosted_on_this_card()], 0)], [trash_self()]);
//! ```
//!
//! CR 1.15.4's back-reference to what a condition's occurrence named — one
//! card, and the whole set of them when the occurrence named more than one:
//!
//! ```
//! use jinteki_cards::edsl::*;
//! // "…another card of the same type" / "…another copy of that ice"
//! let _ = choose(1, &[in_hand_of(Runner), of_the_same_type_as_the_triggering_card()]);
//! let _ = choose(1, &[a_copy_of_the_triggering_card()]);
//! let _ = add_to_hand(the_triggering_card());
//! // "Whenever you discard cards to reach your maximum hand size, you may
//! //  install 1 program or piece of hardware from among those cards."
//! let _ = discards_cards_to_reach_maximum_hand_size(Runner);
//! let _ = install(
//!     choose(1, &[among_those_cards(), of_any_type(&[CardType::Program, CardType::Hardware])]),
//!     InstallDest::RunnerChoiceHostOrRig,
//! );
//! ```

use jinteki_cr::ability::{AbilityDef, AbilityFlag, Condition, TimingRestriction};
pub use jinteki_cr::effects::{DamageKind, EffectClass};
use jinteki_cr::object::PrintedCard;

// Re-exported so a deck file needs exactly one `use` line. These are the
// kernel's own types: a designer who outgrows the helpers below can reach
// for them directly and is still inside the public vocabulary.
pub use jinteki_cr::ability::{
    Cost, InherentCost, ReqScope, StaticDecl, TriggerCond, TriggerRequirement, TurnScope,
};
pub use jinteki_cr::lingering::{ProhibitedAction, ReplacementTransform, WantedDuration};
pub use jinteki_cr::instr::{
    InstallDest, InstallFilter, Instruction, LingeringSpec, Quantity, RunServerSet, SubroutineSpec,
    TargetFilter, TargetSpec, TrashDestination,
};
pub use jinteki_cr::object::Side::{Corp, Runner};
pub use jinteki_cr::object::{CardType, CounterKind, ServerId, Side, Zone};

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
        blank_text_box: false,
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
    blank_text_box: bool,
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
    /// The faction printed on the card (2.13): "Anarch", "Criminal",
    /// "Shaper", "Haas-Bioroid", "Jinteki", "NBN", "Weyland Consortium",
    /// "Adam", "Apex", "Sunny Lebeau" — or "Neutral" for 2.13.2's white
    /// background and no logo. Deckbuilding reads it (1.4.5), and so do the
    /// two cards that talk about a faction while the game is running.
    pub fn faction(mut self, f: &'static str) -> Self {
        self.printed.faction = Some(f);
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
    /// "You can spend these credits on anything." — CR 1.10.3c: hosted
    /// credits may only be spent as the card's ability allows, and this card
    /// allows everything.
    pub fn credits_spendable_on_anything(mut self) -> Self {
        self.printed.hosted_credits_spendable = Some(jinteki_cr::instr::CreditUse::AnyPayment);
        self
    }
    /// "Use these credits **to trash installed cards**." (Miss Bones; CR
    /// 1.10.3c.) The cards are described with the ordinary filter words, so
    /// an empty description is 1.15.2c's default — the installed cards — and
    /// that is exactly what the sentence says.
    pub fn credits_only_for_trashing(mut self, criteria: &[TargetFilter]) -> Self {
        self.printed.hosted_credits_spendable =
            Some(jinteki_cr::instr::CreditUse::TrashingCards(criteria.to_vec()));
        self
    }
    /// "Use this credit **to pay for using icebreakers**." (Ele "Smoke"
    /// Scovak; CR 1.10.3c + 9.1.6a.) The cards whose use the credits pay for
    /// are described with the ordinary filter words.
    pub fn credits_only_for_using(mut self, criteria: &[TargetFilter]) -> Self {
        self.printed.hosted_credits_spendable =
            Some(jinteki_cr::instr::CreditUse::UsingAbilitiesOf(criteria.to_vec()));
        self
    }
    /// "Use these credits **during trace attempts**." (NBN: Making News;
    /// CR 1.10.3c + 10.8.6c/d.) The restriction names a moment and no card.
    pub fn credits_only_during_trace_attempts(mut self) -> Self {
        self.printed.hosted_credits_spendable = Some(jinteki_cr::instr::CreditUse::TraceAttempts);
        self
    }
    /// "You can spend hosted credits **to use programs during runs**."
    /// (Trickster Taka; CR 1.10.3c + 9.1.6a + 6.1.1.) A description AND a
    /// moment at once: the cards whose use the credits pay for are the
    /// ordinary filter words, and "during runs" further restricts WHEN —
    /// [`Self::credits_only_for_using`] and
    /// [`Self::credits_only_during_trace_attempts`] each state one half, and
    /// this sentence states both.
    pub fn credits_only_for_using_during_runs(mut self, criteria: &[TargetFilter]) -> Self {
        self.printed.hosted_credits_spendable =
            Some(jinteki_cr::instr::CreditUse::UsingAbilitiesDuringRuns(criteria.to_vec()));
        self
    }
    /// "Use this credit **to advance ice**." (Weyland Consortium: Because We
    /// Built It; CR 1.10.3c + 1.18.1.) The cards that may be advanced with the
    /// credits are described with the ordinary filter words, so 1.15.2c's
    /// default applies and "ice" reaches the installed pieces of ice.
    pub fn credits_only_for_advancing(mut self, criteria: &[TargetFilter]) -> Self {
        self.printed.hosted_credits_spendable =
            Some(jinteki_cr::instr::CreditUse::AdvancingCards(criteria.to_vec()));
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
    /// "You start the game with N[credit]." (GRNDL class; 1.6.4.)
    pub fn starting_credits(mut self, n: u32) -> Self {
        self.printed.starting_credits = Some(n);
        self
    }
    /// "…and N bad publicity." / "The Corp starts the game with N bad
    /// publicity." (GRNDL and Valencia Estevez; 10.6.) Bad publicity is
    /// always the Corp's, so the fact says only how much — which is why one
    /// identity may print it about the other player.
    pub fn starting_bad_publicity(mut self, n: u32) -> Self {
        self.printed.starting_bad_publicity = Some(n);
        self
    }
    /// One back face of the identity (rule_identity_double_sided; Nebula
    /// class). Build the back exactly like a card — its own printed text and
    /// abilities — and "flip this identity" swaps which face applies. Call
    /// once per back, in face order: most double-siders print one, and
    /// Méliès U ships as three copies with a different back each, so its
    /// definition calls this three times and "secretly set your identity to
    /// any copy" chooses among them.
    /// "You start the game with N different <criteria> cards installed
    /// (these cards are not considered part of your deck)." (Adam; CR 1.5.3 +
    /// 1.6.2.) The fourth setup FACT of the `starting_*` family: the cards
    /// come from the extra pile the player brought along with their deck
    /// (`GameSetup::extra_cards`, 1.5.3a), exactly N differently-named
    /// matching ones begin the game installed (1.5.3b), and from the game's
    /// first moment they are ordinary installed cards (1.5.3d).
    pub fn starts_the_game_with_installed(
        mut self,
        count: u32,
        criteria: &[TargetFilter],
    ) -> Self {
        self.printed.starting_extra_installs =
            Some(jinteki_cr::object::StartingExtraInstalls {
                count,
                criteria: criteria.to_vec(),
                distinct_names: true,
            });
        self
    }
    /// The identity's back face (rule_identity_double_sided; Nebula class).
    /// Build the back exactly like a card — its own printed text and
    /// abilities — and "flip this identity" swaps which face applies.
    pub fn flip_face(mut self, back: Card) -> Self {
        self.printed.flip_faces.push(back.printed);
        self
    }
    pub fn text(mut self, line: &'static str) -> Self {
        self.text.push(line);
        self
    }

    /// The card's text box is BLANK — it prints no rules text at all (Sunny
    /// Lebeau, whose whole card is its link and its deckbuilding numbers).
    /// Say it explicitly rather than by omitting `.text(…)`: a card that
    /// simply forgot its printed text is a bug, and this is what tells the
    /// two apart.
    pub fn no_printed_text(mut self) -> Self {
        self.blank_text_box = true;
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
    /// "As an additional cost to rez this card, …" (Archer, Ibrahim Salem;
    /// 1.16.4c). Declinable during an "install and rez" effect (8.5.13d).
    pub fn additional_rez_cost(mut self, c: Cost) -> Self {
        self.printed.additional_rez_cost = Some(c);
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
    /// "Once per turn → [click], 1[credit]: …" — a paid ability carrying
    /// 9.3.6g's once-per-turn flag, which is spent by USING the ability and
    /// comes back when the turn ends.
    ///
    /// This is the flag, not [`CardBuilder::when_first_each_turn`]'s 9.6.5c
    /// stipulation: the printed words are different sentences and the rules
    /// treat them differently. A paid ability is used (9.1.6), so the flag has
    /// something to be spent by — which is exactly what a mandatory
    /// conditional does not.
    pub fn paid_once_per_turn(
        self,
        cost: Cost,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_flag(AbilityFlag::OncePerTurn),
        )
    }
    /// "Once per turn → <cost>: …" on an ability the card may use only
    /// during an encounter with ice of a named subtype (9.5.6c). Quetzal's
    /// "break 1 **barrier** subroutine" is this and NOT an interface ability:
    /// 9.3.6c gates an interface ability on the source's strength, and an
    /// identity has none, so the subtype is the whole restriction.
    pub fn paid_once_per_turn_during_encounters_with(
        self,
        cost: Cost,
        ice_subtype: &'static str,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_flag(AbilityFlag::OncePerTurn)
                .with_timing(TimingRestriction::EncounterOnly {
                    required_subtype: Some(ice_subtype),
                    required_choice: None,
                }),
        )
    }
    /// "Once per turn → 0[credit]: … **Use this ability only during a run.**"
    /// (Arissana Rocha Nahu.) The once-per-turn flag (9.3.6g) and 9.3.3c's
    /// limit on WHEN, which names the run structure itself and not one of its
    /// phases — so the ability is offered in every paid window from the run's
    /// initiation to the end of its Run Ends Phase (6.1.1).
    pub fn paid_once_per_turn_during_a_run(
        self,
        cost: Cost,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_flag(AbilityFlag::OncePerTurn)
                .with_timing(TimingRestriction::RunOnly),
        )
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
                .with_timing(TimingRestriction::EncounterOnly { required_subtype: ice_subtype, required_choice: None }),
        )
    }
    /// "Access → 1[credit]: …" — usable only in the mid-access window
    /// (9.3.6b).
    pub fn paid_access(self, cost: Cost, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect()).with_flag(AbilityFlag::Access),
        )
    }
    /// "Access, once per turn → <cost>: …" (Freedom Khumalo) — the
    /// mid-access flag (9.3.6b) and the once-per-turn flag (9.3.6g)
    /// together. The once-per-turn flag is spent by USE — 9.1.6a puts the
    /// use at the moment the trigger cost is paid — so an access where the
    /// ability is offered and declined leaves it usable at the next access
    /// the same turn.
    pub fn paid_access_once_per_turn(
        self,
        cost: Cost,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_flag(AbilityFlag::Access)
                .with_flag(AbilityFlag::OncePerTurn),
        )
    }
    /// "[click]: Play this operation **from Archives**." — a paid ability
    /// whose printed words state WHERE its source works from, which CR 9.3.3c
    /// makes a restriction ("limits on when, where, or how often an ability
    /// can be used"). It is not offered while the card is anywhere else — in
    /// hand, the card is played with the basic action like any other.
    pub fn paid_from_discard(
        self,
        cost: Cost,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        let zone = jinteki_cr::object::Zone::Discard(self.printed.side);
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect())
                .with_timing(TimingRestriction::SourceInZone(zone)),
        )
    }
    /// "Use this card only during encounters with **that ice**." (Boomerang;
    /// CR 9.3.3c makes it a restriction, 9.10.3 is what "that ice" means.)
    /// The ability is offered only while the ice being encountered is the one
    /// this card remembers under `key` — and never at all while it remembers
    /// nothing.
    pub fn paid_during_encounters_with(
        self,
        cost: Cost,
        key: &'static str,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::paid(cost, instrs.into_iter().collect()).with_timing(
                TimingRestriction::EncounterOnly {
                    required_subtype: None,
                    required_choice: Some(key),
                },
            ),
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
                .with_timing(TimingRestriction::EncounterOnly { required_subtype: None, required_choice: None }),
        )
    }
    /// "When <trigger>, …" — a conditional ability (9.6). Mandatory: its
    /// controller must resolve it.
    pub fn when(self, cond: TriggerCond, instrs: impl IntoIterator<Item = Instruction>) -> Self {
        self.ability(AbilityDef::conditional(cond, instrs.into_iter().collect(), false))
    }
    /// "The first time each turn <trigger>, …" — CR 9.6.5c's stipulation
    /// about the OCCURRENCE, checked when the condition would be met.
    ///
    /// NOT 9.3.6g's once-per-turn flag, which is a different sentence
    /// ("use this ability only once per turn") with different rules: the flag
    /// is spent by USING the ability, 9.1.6 says an entirely mandatory
    /// ability is never used, and 1.12.2 makes the flag per object — so a
    /// mandatory ability written with the flag would come back fresh when its
    /// card was reinstalled the same turn.
    pub fn when_first_each_turn(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), false)
                .first_time_each_turn(),
        )
    }

    /// "The first time each turn <trigger>, you may …" — the same ordinal on
    /// a declinable ability (9.6.9).
    pub fn may_when_first_each_turn(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), true)
                .first_time_each_turn(),
        )
    }
    /// "Once per turn → When <trigger>, you may …" — a conditional ability
    /// carrying 9.3.6g's once-per-turn flag.
    ///
    /// The flag and the ability's OPTIONALITY go together, and that is not a
    /// convenience: 9.1.6's second sentence says "players do not 'use'
    /// abilities that are entirely mandatory", so a mandatory conditional
    /// would never spend the flag at all. The optional part may sit inside
    /// the instructions instead of on the ability (Null: Whistleblower's
    /// "you may trash 1 card from your grip"), which 9.6.9d is — hence
    /// [`CardBuilder::when_once_per_turn`] beside this one.
    pub fn may_when_once_per_turn(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), true)
                .with_flag(AbilityFlag::OncePerTurn),
        )
    }
    /// "Once per turn → When <trigger>, <effect with an optional component>"
    /// — the same flag on an ability whose "may" is inside one sentence
    /// (9.6.9d), so it is the INSTRUCTION that is declinable and not the
    /// ability. Never use this for an entirely mandatory ability: 9.1.6 would
    /// never spend the flag, and the sentence the designer means is
    /// [`CardBuilder::when_first_each_turn`]'s 9.6.5c ordinal.
    pub fn when_once_per_turn(
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
    /// "[interrupt] → …" (9.3.6d/9.9.1) — mandatory, as the printed wording
    /// is when it carries no "you may".
    pub fn interrupt(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), false)
                .with_flag(AbilityFlag::Interrupt),
        )
    }
    /// "[interrupt] → you may …" — the same, declinable (9.6.9).
    pub fn may_interrupt(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), true)
                .with_flag(AbilityFlag::Interrupt),
        )
    }
    /// "[interrupt] → **The first time each turn** <trigger>, …" — 9.6.5c's
    /// ordinal on an interrupt, which 9.9.5a reads of the IMMINENCE: the
    /// ability is relevant only while the imminent instruction is the first
    /// of its class this turn.
    pub fn interrupt_first_each_turn(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), false)
                .with_flag(AbilityFlag::Interrupt)
                .first_time_each_turn(),
        )
    }
    /// "[interrupt] → **The first time each run** <trigger>, …" (Jesminder
    /// Sareen) — the same ordinal counted over the run instead of the turn.
    pub fn interrupt_first_each_run(
        self,
        cond: TriggerCond,
        instrs: impl IntoIterator<Item = Instruction>,
    ) -> Self {
        self.ability(
            AbilityDef::conditional(cond, instrs.into_iter().collect(), false)
                .with_flag(AbilityFlag::Interrupt)
                .first_time_each_run(),
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

    /// "**While <state>**, …" (9.3.7a): declarations that apply only while a
    /// stated condition about the GAME holds — "while the Runner is tagged,
    /// they play with the grip revealed" (Harishchandra Ent.). The state is
    /// asked in the same words a trigger condition's 9.6.5c requirements use.
    pub fn declares_while(
        self,
        reqs: &[TriggerRequirement],
        decls: impl IntoIterator<Item = StaticDecl>,
    ) -> Self {
        let mut def = AbilityDef::static_ability(decls.into_iter().collect());
        def.condition = Some(Condition::Static(jinteki_cr::ability::StaticCond::StateRequirement(
            reqs.to_vec(),
        )));
        self.ability(def)
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
            !self.text.is_empty() || self.blank_text_box,
            "{}: copy the printed text into .text(…) — behaviour is checked against it (SYS-D-10). \
             If the card's text box really is blank, say .no_printed_text().",
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
        TriggerCond::TurnBegins { .. } => "turn begins",
        TriggerCond::SelfScored { .. } => "scored",
        TriggerCond::SelfStolen => "stolen",
        TriggerCond::SelfInstalled => "installed",
        TriggerCond::SelfUninstalled => "uninstalled",
        TriggerCond::SelfEmpty { .. } => "empty",
        TriggerCond::SelfEncountered => "encountered",
        TriggerCond::SelfAccessed { .. } => "accessed",
        TriggerCond::IcePassed { this_ice: true, .. } => "passed",
        TriggerCond::IcePassed { .. } => "an ice was passed",
        TriggerCond::SelfPlayResolved => "resolved",
        TriggerCond::RunEnds { successful_only: true, .. } => "successful run ends",
        TriggerCond::RunEnds { .. } => "run ends",
        TriggerCond::RunBegins { .. } => "a run begins",
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
    Instruction::Draw(side, Quantity::c(n as i64))
}
/// "Draw until you have N cards in HQ." (NEXT Design; 8.4 + 9.12.2.) A
/// draw-up-to is a draw whose count is calculated when it resolves — the
/// printed target minus the hand as it then stands, floored at zero — so a
/// hand already at N draws nothing, and a deck shorter than the difference
/// gives what it has (`Vm::draw` takes what remains; 1.7.2c is about the
/// MANDATORY draw, which this is not).
pub fn draw_until_hand_has(side: Side, n: i64) -> Instruction {
    Instruction::Draw(
        side,
        Quantity::Minus(Box::new(Quantity::c(n)), Box::new(Quantity::CardsInHandOf(side))),
    )
}
/// "Add <cards> to your grip/HQ."
pub fn add_to_hand(cards: TargetSpec) -> Instruction {
    Instruction::AddCardsToHand { cards }
}
/// "…add <cards> to the heap." (Skorpios Defense Systems.) The heap is
/// 4.4.1's name for the Runner's discard pile, so the destination is fixed by
/// the word. An ADD, not a trash: where the cards were trashed on the way in,
/// the trash was recorded then (8.2.2), and this is the movement completing.
pub fn add_to_heap(cards: TargetSpec) -> Instruction {
    Instruction::AddCardsToHeap { cards }
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
/// "Look at <cards>." (1.21.2 — only `by` sees the front faces, and the cards
/// stay where they are.) CR 9.11.4e: looking ENDS an instruction, so whatever
/// the card says to do with the cards is the next sentence, written next.
pub fn look_at(cards: TargetSpec, by: Side) -> Instruction {
    Instruction::LookAtCards { cards, by }
}
/// "Search your stack for <criteria>." (8.7; the stack is reshuffled after.)
/// "Search your stack for up to N <description>." (8.7.) 8.7.2e lets the
/// search fail to find.
pub fn search_stack(criteria: &[TargetFilter], count: i64) -> Instruction {
    search_deck_of(Runner, criteria, count)
}
/// "Search R&D for <criteria>." (Editorial Division, The Foundry.) The same
/// 8.7 search, of the other player's deck — which is why the zone is content
/// on one instruction rather than two words.
pub fn search_rnd(criteria: &[TargetFilter], count: i64) -> Instruction {
    search_deck_of(Corp, criteria, count)
}
/// "Search <a player>'s deck for <criteria>." (8.7; 8.7.2e lets it fail.)
pub fn search_deck_of(side: Side, criteria: &[TargetFilter], count: i64) -> Instruction {
    Instruction::Search {
        zone: jinteki_cr::object::Zone::Deck(side),
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
    Instruction::GainTags { amount: n, avoidable: true }
}
/// "Give the Runner N tags **(cannot be avoided)**." (NBN: Controlling the
/// Message.) CR 9.3.3g makes the parenthesis a restriction and 9.4.5 makes it
/// ride the value, so nothing offered in the interrupt window can take these
/// tags away — the same words [`prevent_all_net_damage`] answers on the other
/// side.
pub fn give_tags_that_cannot_be_avoided(n: u32) -> Instruction {
    Instruction::GainTags { amount: n, avoidable: false }
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
/// "…you may **jack out**." (Nero Severn.) CR 6.1.5's process — the Runner
/// voluntarily ends the run — offered by an ability wherever the card says,
/// rather than at the one place the run's own structure opens it (6.1.5b).
/// Pair it with [`may`] for the printed "you may".
pub fn jack_out() -> Instruction {
    Instruction::JackOut
}
/// "The Runner **moves to that ice and approaches it**." (Mti Mwekundu;
/// 6.2.8a.) The Runner's position becomes that ice's, the server it protects
/// becomes the attacked server, and the run's timing step becomes the Approach
/// Ice Phase. With no such ice — nothing was installed — nothing happens.
pub fn move_runner_to_ice(ice: TargetSpec) -> Instruction {
    Instruction::MoveRunnerToIce { ice, encounter: false }
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
        if_would_be_successful: Vec::new(),
    }
}
/// "Run any server. If successful, …" — the effect names no server, so the
/// Runner announces the attacked one at step 6.9.1a from everything 6.7.4a
/// allows (minus any server 6.3.2a forbids initiating a run on).
pub fn run_any_server(if_successful: impl IntoIterator<Item = Instruction>) -> Instruction {
    Instruction::run_any_server(if_successful.into_iter().collect())
}
/// "Run <server>. If that run **would** be declared successful, …" (Omar
/// Keung.) 9.9.1's "would" makes the second sentence an INTERRUPT, relevant
/// to the imminence of 6.9.5a's declaration — the last moment at which the
/// attacked server can still be changed and have the declaration follow it.
///
/// It rides on the run for the same reason [`run_then_if_successful`]'s
/// clause does: the sentence says "that run", and the run this instruction
/// creates is what identifies it. `allowed` is 6.7.4a's set all the same,
/// because the FIRST sentence is what states it — but 6.7.4a's tie is stated
/// about "If successful" abilities only, and this clause is not one, so
/// nothing re-reads it.
pub fn run_then_if_would_be_successful(
    server: ServerId,
    if_would_be_successful: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::InitiateRun {
        server: Some(server),
        allowed: RunServerSet::These(vec![server]),
        if_successful: Vec::new(),
        if_would_be_successful: if_would_be_successful.into_iter().collect(),
    }
}
/// "The attacked server becomes <server>." (6.1.2d.) The run's timing step is
/// untouched: the Runner does not move, and so approaches and encounters
/// nothing on the way in.
pub fn change_attacked_server(server: ServerId) -> Instruction {
    Instruction::ChangeAttackedServer { server }
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
    Instruction::HostCards { cards, host, faceup: false }
}
/// "Host <cards> **faceup** on <host>." (1.13.1 + 1.21.1 — both players are
/// entitled to a faceup card's identity.)
pub fn host_faceup(cards: TargetSpec, host: TargetSpec) -> Instruction {
    Instruction::HostCards { cards, host, faceup: true }
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
/// "Place X <kind> counters on <target>." — the same sentence with a
/// CALCULATED amount (9.12.2), for the cards that print an X instead of a
/// number ("place X advancement counters on 1 installed card", Jemison
/// Astronautics).
pub fn place_on_q(target: TargetSpec, kind: CounterKind, amount: Quantity) -> Instruction {
    Instruction::PlaceCounters { target, kind, amount }
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
        distinct_servers: false,
    }
}
/// "You may install up to N pieces of ice, with no more than a single piece
/// of ice per server." (NEXT Design; 8.5.5 + 8.5.2a.) The "up to"/"you may"
/// is 8.5.5's one-at-a-time choice, declinable at every pick; the per-server
/// stipulation excludes a server this ability already installed to from the
/// 8.5.16b destination declaration. Costs are paid as ever — the sentence
/// says nothing about them, so 8.5.11's ice install cost stands (and under
/// this stipulation it is always 0[credit], each server taking its first
/// ice of the effect).
pub fn install_up_to_max_one_per_server(
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
        distinct_servers: true,
    }
}
/// "Set aside the top N cards of your stack facedown." said by a card whose
/// LATER ability refers to "cards set aside with this identity" (Ayla "Bios"
/// Rahim; 4.8.7 + 4.8.3) — the group is stamped with the source card, so it
/// outlives this resolution and [`set_aside_with_this_card`] can name it.
/// The "(You may look at those cards at any time.)" entitlement is the
/// facedown group's ordinary visibility: it belongs to its controller.
pub fn set_aside_top_of_deck_with_this_card(deck_of: Side, n: i64) -> Instruction {
    Instruction::SetAsideTopOfDeck {
        deck_of,
        count: Quantity::c(n),
        with_source: true,
    }
}
/// "…1 card set aside with this identity" (Ayla "Bios" Rahim) — a card whose
/// set-aside group was stamped with the selecting ability's source card.
pub fn set_aside_with_this_card() -> TargetFilter {
    TargetFilter::SetAsideWithSource
}
/// "…a card with a trash cost" (Neutralize All Threats; 2.6/7.1.5a).
pub fn has_trash_cost() -> TargetFilter {
    TargetFilter::HasTrashCost
}
/// "You must trash that card by paying its trash cost, if able." (Neutralize
/// All Threats; 9.12.3b — only the basic trash ability satisfies the
/// requirement, so nothing else can be forced.)
pub fn must_trash_accessed_by_paying_trash_cost() -> Instruction {
    Instruction::MustTrashAccessedCard {
        means: jinteki_cr::instr::TrashMeans::PayingTheTrashCost,
    }
}
/// "Install <a card>." (8.5.)
pub fn install(card: TargetSpec, dest: InstallDest) -> Instruction {
    Instruction::InstallCard {
        card,
        dest,
        and_rez: false,
        ignore_costs: false,
        ignore_credit_costs: false,
        reveal_check: None,
        reduce_total: Quantity::c(0),
        reduce_install: Quantity::c(0),
        facedown: false,
        distinct_servers: false,
    }
}
/// "Install <a card>, **ignoring all costs**." (Synapse Global; 1.16.5c —
/// every element of the cost is removed, including 8.5.11a's 1[credit] per
/// piece of ice already protecting the destination server and any 1.16.10
/// additional cost.)
pub fn install_ignoring_all_costs(card: TargetSpec, dest: InstallDest) -> Instruction {
    Instruction::InstallCard {
        card,
        dest,
        and_rez: false,
        ignore_costs: true,
        ignore_credit_costs: false,
        reveal_check: None,
        reduce_total: Quantity::c(0),
        reduce_install: Quantity::c(0),
        facedown: false,
        distinct_servers: false,
    }
}
/// "Install and rez the card you found, **ignoring credit costs**." (Ob
/// Superheavy Logistics; 8.5.15's install-and-rez of 8.7.4's found card.)
/// "Ignoring credit costs" selects cost components by KIND, cutting across
/// 1.16.4's inherent/additional split — every credit component (the inherent
/// install cost, the rez cost, the credit part of an additional cost)
/// becomes 0, while the non-credit parts of an additional cost (an
/// Archer-class forfeit) are still paid. That is a different axis from
/// [`install_ignoring_all_costs`]'s 1.16.5c, which the kernel reads as the
/// INHERENT costs only. The card states no destination, so the Corp declares
/// one at step 8.5.16b.
pub fn install_and_rez_found_ignoring_credit_costs() -> Instruction {
    Instruction::InstallCard {
        card: TargetSpec::FoundBySearch,
        dest: InstallDest::DeclaredByInstaller,
        and_rez: true,
        ignore_costs: false,
        ignore_credit_costs: true,
        reveal_check: None,
        reduce_total: Quantity::c(0),
        reduce_install: Quantity::c(0),
        facedown: false,
        distinct_servers: false,
    }
}
/// "Install <a card>, paying N[credit] less." (1.16.6 — a reduction of the
/// install cost alone, so it needs no rez cost to divide with.)
pub fn install_paying_less(card: TargetSpec, dest: InstallDest, less: i64) -> Instruction {
    Instruction::InstallCard {
        card,
        dest,
        and_rez: false,
        ignore_costs: false,
        ignore_credit_costs: false,
        reveal_check: None,
        reduce_total: Quantity::c(0),
        reduce_install: Quantity::c(less),
        facedown: false,
        distinct_servers: false,
    }
}
/// "Install 1 card from your grip **facedown**." (Apex; CR 4.6.4d / 8.1.4.)
/// The card goes into the rig with no characteristics at all (8.1.4a), which
/// is why it costs nothing to install (8.5.11a) and why the description
/// beside it need not name a card type: a facedown card has none.
pub fn install_facedown(card: TargetSpec, dest: InstallDest) -> Instruction {
    Instruction::InstallCard {
        card,
        dest,
        and_rez: false,
        ignore_costs: false,
        ignore_credit_costs: false,
        reveal_check: None,
        reduce_total: Quantity::c(0),
        reduce_install: Quantity::c(0),
        facedown: true,
        distinct_servers: false,
    }
}
/// "You may play N operations from HQ." (8.6.3 — chosen one at a time, and
/// "up to" is built in, so the printed "you may" is already here.)
pub fn play_cards_from_hand(count: u32, from_hand_of: Side) -> Instruction {
    Instruction::PlayCards { count, from_hand_of, ignore_costs: false }
}
/// "Play 1 **current** from HQ or Archives **(paying its play cost)**." (New
/// Angeles Sol; 8.6.3.) One card, described the way any other target is — so
/// where it is played FROM is a criterion on the description and not a
/// property of the instruction, which is what lets one sentence name two
/// zones at once. The parenthetical is 8.6.7b restated: an effect that plays
/// a card pays the play cost unless it says otherwise, and this one does not.
pub fn play_card(card: TargetSpec) -> Instruction {
    Instruction::PlayCard { card, ignore_costs: false, then_remove_from_game: false }
}
/// "Rez <a card>." (8.1.2.)
pub fn rez(target: TargetSpec) -> Instruction {
    Instruction::RezCard { target, ignore_costs: false, reduce: Quantity::c(0) }
}
/// "Rez <a card>, **paying N[credit] less**." (Haas-Bioroid: Architects of
/// Tomorrow; CR 1.16.2a.) The same rez, with one effect lowering its cost —
/// which is why it is content on the instruction and not an instruction of
/// its own. 1.16.2a floors the payment at zero.
pub fn rez_paying_less(target: TargetSpec, less: i64) -> Instruction {
    Instruction::RezCard { target, ignore_costs: false, reduce: Quantity::c(less) }
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
/// cards." (9.11.3/9.11.4a.)
///
/// One sentence is one instruction whatever the classes of its effects —
/// Snare!'s "do 3 net damage and give the Runner 1 tag" is the case this
/// exists for — so writing an "X and Y" sentence as two list items invents a
/// checkpoint, a reaction window and an interrupt window the card does not
/// print (see `docs/cards/EDSL.md`). A half that chooses its own targets
/// announces them with the sentence's other choices, before any of it
/// resolves (1.15.2), so a later half's back-reference ("its rez cost") reads
/// the card an earlier half chose. A §9.2.2e procedure half (an install, a
/// play, a trace) is 9.11.4b's own instruction and resolves after the merged
/// ones, as do the halves printed after it.
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
/// "the **non-agenda** card you are accessing" (Freedom Khumalo; 7.1.2) —
/// the accessed card with the sentence's stipulation. Nothing is chosen: the
/// access fixed the card, and the criteria only decide whether the
/// description reaches it. During an access the stipulation does not
/// describe, the ability is not offered at all.
pub fn accessed_card_matching(criteria: &[TargetFilter]) -> TargetSpec {
    TargetSpec::AccessedCardMatching(criteria.to_vec())
}
/// "it" / "that card" — the card the OCCURRENCE that met this ability's
/// condition named (1.15.4). Nothing is announced: the condition fixed the
/// card, exactly as an access fixes [`accessed_card`].
pub fn the_triggering_card() -> TargetSpec {
    TargetSpec::TriggeringCard
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
/// "the top <amount> cards of your stack" (4.2.1) — the cards themselves, in
/// deck order, not a description the controller picks from. The amount is a
/// quantity, so "the top X cards, where X is …" is one call.
pub fn top_of_stack(count: Quantity) -> TargetSpec {
    TargetSpec::TopOfDeck { side: Runner, count }
}
/// "the top <amount> cards of R&D" (4.2.1).
pub fn top_of_rnd(count: Quantity) -> TargetSpec {
    TargetSpec::TopOfDeck { side: Corp, count }
}
/// "the top <amount> cards of your heap" (Wyvern). CR 4.4.2 leaves a discard
/// pile unordered, so this names nothing at all unless the same card also
/// declares [`discard_pile_is_ordered`].
pub fn top_of_heap(count: Quantity) -> TargetSpec {
    TargetSpec::TopOfDiscard { side: Runner, count }
}
/// "…1 of those cards" — a card this ability has already looked at (1.21.2).
/// CR 1.12.3: a card that then moves to an unknown location becomes a new
/// object, and this description stops reaching it.
pub fn looked_at_by_this_ability() -> TargetFilter {
    TargetFilter::LookedAtByThisAbility
}
/// "…1 of them" — a card in this ability's own 4.8.7 set-aside group, still
/// in the set-aside zone (Skorpios Defense Systems).
pub fn set_aside_by_this_ability() -> TargetFilter {
    TargetFilter::SetAsideByThisAbility
}
/// "…all of those cards that are still set aside" (Skorpios Defense Systems)
/// — every card still in this ability's own 4.8.7 set-aside group, named
/// with no announcement, the way [`TargetSpec::FoundBySearch`] names a
/// search's finds. "Still" is the zone: a card an earlier half removed from
/// the game has left the set-aside zone and is not among them.
pub fn still_set_aside_by_this_ability() -> TargetSpec {
    TargetSpec::StillSetAsideByThisAbility
}
/// "…**that program**", said of the card an earlier instruction of the SAME
/// ability installed (Kabonesa Wu). CR 8.7.4's find is not 1.15.2's
/// announcement, so a card a search installed is no target of anything and
/// [`earlier_choice`] cannot reach it. Fixes the card by identity, so it says
/// nothing about where the card now is — "still installed" is
/// [`installed_runner_card`] written beside it.
pub fn installed_by_this_ability() -> TargetFilter {
    TargetFilter::InstalledByThisAbility
}
/// "…remove **it** from the game" — the same card pointed at rather than
/// described, so nothing is announced (the pointing twin of
/// [`installed_by_this_ability`]).
pub fn the_card_this_ability_installed() -> TargetSpec {
    TargetSpec::InstalledByThisAbility
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
/// "1 unrezzed card" — CR 8.1.2's other half: an installed facedown Corp
/// card. It names the play area on its own, exactly as [`rezzed`] does.
pub fn unrezzed() -> TargetFilter {
    TargetFilter::Unrezzed
}
pub fn of_type(t: CardType) -> TargetFilter {
    TargetFilter::CardTypeIs(t)
}
pub fn with_subtype(s: &'static str) -> TargetFilter {
    TargetFilter::HasSubtype(s)
}
/// "…**virus or weapon** cards" (2.16) — any one of these subtypes. The
/// criteria of a description are otherwise a conjunction, so a printed "or"
/// between subtypes is this one call.
pub fn with_any_subtype(list: &'static [&'static str]) -> TargetFilter {
    TargetFilter::HasAnySubtype(list)
}
/// "…cards **with different names**" (2.1.5) — no two of the cards chosen or
/// found may share a name. A property of the whole set, so it is written
/// alongside the other criteria and applies to the choice as a whole.
pub fn with_different_names() -> TargetFilter {
    TargetFilter::DistinctNames
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
/// "…another identity" — CR 1.5.4a's pile of additional identities, kept
/// outside the game. 1.5.4b is what makes this the right description: "when
/// an ability refers to an identity other than the Runner's current identity,
/// it refers to the cards provided this way".
pub fn in_identity_pile_of(side: Side) -> TargetFilter {
    TargetFilter::InIdentityPileOf(side)
}
/// "…from the same faction" (`same = true`, Rebirth) / "…that does not match
/// the faction of your identity" (`same = false`, DJ Fenris) — CR 2.13,
/// compared against the faction of that player's current identity. Write
/// whichever the card writes.
pub fn faction_matching_identity_of(side: Side, same: bool) -> TargetFilter {
    TargetFilter::FactionMatchesIdentityOf { side, same }
}
/// "…**(from any location)**" (Skorpios Defense Systems), and the bare
/// "cards" of a sentence that means every one of them wherever it sits
/// (Whizzard). CR 1.15.2c: writing NO zone means the installed cards, so a
/// card that means more than that has to say so — this is that word.
pub fn in_any_location() -> TargetFilter {
    TargetFilter::InAnyLocation
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
/// "the Corp's …" / "the Runner's …" — the player responsible for the object
/// (1.14.2), which reaches a card that is not installed, such as an identity.
pub fn controlled_by(side: Side) -> TargetFilter {
    TargetFilter::ControlledBy(side)
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
/// "an agenda that you did not **advance** this turn" (Issuaq Adaptics) —
/// 1.18.1's advance, asked of the same open game history
/// [`installed_this_turn`] reads. Pass `true` for the other polarity.
/// 1.18.2's bare placement of an advancement counter is not an advance, so a
/// card a Tennin-class ability put a counter on was never advanced.
pub fn advanced_this_turn(yes: bool) -> TargetFilter {
    TargetFilter::AdvancedThisTurn(yes)
}
/// "a card you can advance" (1.18.3) — the PERMISSION as a criterion, read
/// from the same place the basic advance action reads it, so a card the
/// action would refuse cannot be described here either. (The declaration
/// that GRANTS the permission to its own card is `can_be_advanced()`.)
pub fn advanceable() -> TargetFilter {
    TargetFilter::CanBeAdvanced
}
/// "…the piece of ice being encountered" (6.5.1) — the ice the encounter in
/// progress is with, and nothing outside an encounter. The description half
/// of [`TargetSpec::EncounteredIce`].
pub fn the_encountered_ice() -> TargetFilter {
    TargetFilter::IsEncounteredIce
}
/// "…the outermost piece of ice protecting its server" (6.2.1/6.2.2) — the
/// last ice in its own server's innermost-first sequence, whichever server
/// that is. "…protecting **any** server" (Acme Consulting) is this atom
/// alone; "…protecting **that** server" (AgInfusion) is this atom conjoined
/// with a criterion naming the server.
pub fn outermost_ice_of_its_server() -> TargetFilter {
    TargetFilter::OutermostIceOfItsServer
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
/// "**Any** X <kind> counters: … X must be equal to <quantity>." (Freedom
/// Khumalo; 1.16.2c + 1.10.3c.) Neither half of [`hosted_counters`]: the
/// amount is X, DETERMINED by the equality rather than chosen, and the
/// counters come from any of the payer's cards — which cards is the payer's
/// division, put to them the way a credit payment's division already is.
/// 1.16.1b: payable only if the payer's cards host at least exactly-X
/// counters between them; a determined X of 0 is a zero cost, paid by
/// announcing it (1.16.1d).
pub fn any_x_counters_equal_to(kind: CounterKind, q: Quantity) -> Cost {
    Cost::any_x_counters_equal_to(kind, q)
}
/// "Forfeit an agenda" as a cost (8.2.5).
/// "…trash 1 card from your grip" as a cost (Null: Whistleblower; 1.16.10).
/// The cards are the payer's to choose, and naming the grip is what lifts
/// 1.15.2c's installed-cards default.
pub fn trash_cards_from_hand_of(side: Side, n: u32) -> Cost {
    Cost::trash_matching(n, vec![in_hand_of(side)])
}
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
/// "[click], **remove 1 tag**:" as a cost (Synapse Global; 10.5.1). The tags
/// are the Runner's whoever pays, so a Corp card printing this spends
/// something it needed the Runner to have — and cannot use the ability at all
/// while the Runner has no tags (1.16.1b).
pub fn remove_a_tag(n: u32) -> Cost {
    Cost::remove_tags(n)
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
    TriggerCond::turn_begins(side)
}
/// "When your turn begins, **if <state>**, …" (Subliminal Messaging) — the
/// same sentence with 9.6.5c requirements. A requirement naming a ZONE is
/// also what keeps the ability active there (9.1.8b), which is why a card
/// that talks from its discard pile writes the zone here and not in an
/// `if_met(…)` instruction.
pub fn turn_begins_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::TurnBegins { side, requires: reqs.to_vec() }
}
/// "Whenever you encounter a **barrier**, …" (Paperclip) — the encounter with
/// the sentence's subtype stipulation (2.16) and its 9.6.5c requirements. A
/// requirement naming a zone is also what keeps the ability active there
/// (9.1.8b), which is how a program talks from the heap.
pub fn encounters_a(subtype: &'static str, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::EncounterBegins {
        of_subtypes: vec![subtype],
        requires: reqs.to_vec(),
    }
}
/// "…if this program **can interface with the barrier you are encountering**"
/// (Paperclip) — CR 3.9.5g's strength comparison and 3.9.5h's subtype, asked
/// as a question the INSTRUCTIONS check (9.6.5d) rather than as the interface
/// flag, because the card asks it after "+X strength" has resolved.
pub fn can_interface_with_the_encountered(subtype: &'static str) -> TriggerRequirement {
    TriggerRequirement::CanInterfaceWithEncounteredIce { required_subtype: Some(subtype) }
}
/// "X[credit]:" — 1.16.2c's variable cost, announced before it is paid. With
/// no printed restriction on X, 1.16.1c is the only bound: what the payer can
/// actually pay.
pub fn credits_x() -> Cost {
    Cost { credits: Quantity::AnnouncedX, ..Cost::free() }
}
/// "+X strength" (Paperclip, Corporate Troubleshooter) — the value announced
/// for this use of the ability (1.16.2c).
pub fn pump_x() -> Instruction {
    Instruction::ModifyStrength {
        target: TargetSpec::SelfSource,
        amount: Quantity::AnnouncedX,
        duration: None,
    }
}
/// "Break up to X subroutines."
pub fn break_up_to_x() -> Instruction {
    Instruction::BreakSubroutines {
        subs: SubroutineSpec::Chosen { count: Quantity::AnnouncedX, up_to: true },
    }
}
/// "…install this card from your heap." (Paperclip; 8.5.) The install is the
/// ordinary procedure, so its cost is the printed one.
pub fn install_this_card() -> Instruction {
    install(this_card(), InstallDest::DeclaredByInstaller)
}
/// "…if this card is in Archives" (Subliminal Messaging; 9.1.8b).
pub fn source_in_discard() -> TriggerRequirement {
    TriggerRequirement::SourceInDiscard
}
/// "Play only if you have **not finished an action yet this turn**." (Petty
/// Cash; CR 5.2.2a — an action is finished once the game may advance past the
/// action step that ran it.)
pub fn no_action_finished_yet_this_turn(side: Side) -> TriggerRequirement {
    TriggerRequirement::ActionsFinishedThisTurn { side, at_most: 0 }
}
/// "…if you played this operation **from anywhere except HQ**" (Petty Cash) —
/// a question about the play in progress (8.6.7a), with the zone and the
/// polarity as content.
pub fn played_from_anywhere_except(zone: Zone) -> TriggerRequirement {
    TriggerRequirement::SourcePlayedFrom { from: zone, is: false }
}
/// "**Play this operation.** After it resolves, remove it from the game."
/// (Petty Cash.) CR 8.6.6d names the pair as ONE construction — the played
/// card is not trashed at step 8.6.7g, and the nested conditional removes it
/// from the game instead — so it is one call. Written as two it could not
/// work at all: 9.1.4 stops an ability acting on a source that changed zones,
/// and playing the card moves it into the play area.
pub fn play_this_card_then_remove_it_from_the_game() -> Instruction {
    Instruction::PlayCard {
        card: TargetSpec::SelfSource,
        ignore_costs: false,
        then_remove_from_game: true,
    }
}
/// "…if the Runner did not initiate any runs during their last turn"
/// (Subliminal Messaging) — the negative of
/// [`runner_made_a_run_last_turn`], the polarity being content on the same
/// question (§12 rule 2).
pub fn runner_made_no_runs_last_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        made: false,
        successful_only: false,
        scope: TurnScope::LastCompletedTurn,
        on: Vec::new(),
    }
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
/// "When you install that card, …" (Topan) — "that card" being the one an
/// install instruction of this same card's ability performed, so the
/// condition is met by that install and by no other: not by the basic
/// action's, not by another card's, and not by a swap that made a card
/// installed without installing it.
pub fn installs_that_card() -> TriggerCond {
    TriggerCond::CardInstalledByAbilityOfSource
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
    TriggerCond::IcePassed {
        this_ice: true,
        fully_broken: false,
        subs_resolved: false,
        rezzed: false,
        criteria: Vec::new(),
    }
}
/// "…the Runner passes a **rezzed** piece of **bioroid** ice…" (Haas-Bioroid:
/// Architects of Tomorrow) — 6.9.4a's pass. "Rezzed" is a fact of the pass's
/// own moment, read off the record so a later rez does not rewrite what
/// 9.6.5c's ordinal counts; the rest of the sentence describes the ice in
/// the ordinary description words.
pub fn passes_rezzed_ice_matching(criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::IcePassed {
        this_ice: false,
        fully_broken: false,
        subs_resolved: false,
        rezzed: true,
        criteria: criteria.to_vec(),
    }
}
/// "…you pass a piece of ice…" — the pass with no stipulation at all: not
/// this card's, not one fully broken, not one whose subroutines resolved
/// (Khan). 6.9.4a's step and nothing else.
pub fn passes_any_ice() -> TriggerCond {
    TriggerCond::IcePassed {
        this_ice: false,
        fully_broken: false,
        subs_resolved: false,
        rezzed: false,
        criteria: Vec::new(),
    }
}
/// "When the Runner **approaches a server**, …" (Mti Mwekundu; 6.9.4g's step,
/// reached once every piece of ice protecting the attacked server has been
/// passed — or straight away when none is).
pub fn runner_approaches_a_server() -> TriggerCond {
    TriggerCond::ServerApproached
}
/// "When this run ends, …"
pub fn run_ends() -> TriggerCond {
    TriggerCond::RunEnds { successful_only: false, on: Vec::new() }
}
/// "When a run on HQ or R&D ends, …" — the same condition with the server
/// the sentence names (Zahya Sadeghi).
pub fn run_on_ends(servers: &[ServerId]) -> TriggerCond {
    TriggerCond::RunEnds { successful_only: false, on: servers.to_vec() }
}
/// "When a run on R&D begins, …" (Captain Padma Isbister; 6.9.1.)
pub fn run_begins_on(servers: &[ServerId]) -> TriggerCond {
    TriggerCond::RunBegins { on: servers.to_vec() }
}
/// "After you resolve this operation, …" (8.6.7h.)
pub fn after_this_resolves() -> TriggerCond {
    TriggerCond::SelfPlayResolved
}
/// "Whenever you make a successful run" — any server (6.8.4).
pub fn makes_successful_run() -> TriggerCond {
    TriggerCond::MakesSuccessfulRun {
        on: None,
        after_subroutine_resolved: false,
        requires: Vec::new(),
    }
}
/// "Whenever you make a successful run, **if <state>**, …" (Dewi
/// Subrotoputri's "if your [mu] is full") — 6.8.4's successful run carrying
/// 9.6.5c's additional requirement, read against the game state at the
/// occurrence.
pub fn makes_successful_run_if(reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::MakesSuccessfulRun {
        on: None,
        after_subroutine_resolved: false,
        requires: reqs.to_vec(),
    }
}
/// "…a run becomes successful **after a subroutine resolved during that
/// run**" (Ryō "Phoenix" Ōno) — a stipulation about the occurrence itself,
/// inside what a printed ordinal counts: a successful run with no subroutine
/// resolved was never one of "the times" and spends nothing. It is read off
/// the declaration's own record, which is what still answers once the run's
/// history window has closed.
pub fn makes_successful_run_after_subroutine_resolved() -> TriggerCond {
    TriggerCond::MakesSuccessfulRun {
        on: None,
        after_subroutine_resolved: true,
        requires: Vec::new(),
    }
}
/// "…makes a successful run on <these servers>" (Gemilang class).
pub fn makes_successful_run_on(servers: &[ServerId]) -> TriggerCond {
    TriggerCond::MakesSuccessfulRun {
        on: Some(servers.to_vec()),
        after_subroutine_resolved: false,
        requires: Vec::new(),
    }
}
/// "…you make a successful run on **a central server**" (Liza Talking
/// Thunder, Laramy Fisk). CR 4.6.5 names the central servers and no others —
/// HQ, R&D and Archives — so the sentence's stipulation IS that list, fixed
/// by the rule rather than by the board.
pub fn makes_successful_run_on_a_central_server() -> TriggerCond {
    makes_successful_run_on(&[ServerId::Hq, ServerId::Rnd, ServerId::Archives])
}
/// "…you make a successful run on **your mark**" (Virtuoso, Nyusha Sintashta;
/// CR 10.11.5).
///
/// The printed "the first time each turn" rides on THIS condition instead of
/// on [`CardBuilder::when_first_each_turn`], because 10.11.5 counts it
/// differently: a condition that checks a game property related to the mark
/// only checks from the moment that server was designated, so a successful run
/// on the same server EARLIER in the turn — before it was the mark — is not
/// one of the times this condition counts.
pub fn makes_successful_run_on_your_mark(first_each_turn: bool) -> TriggerCond {
    TriggerCond::SuccessfulRunOnMark { first_each_turn }
}
/// "…the Corp rezzes a piece of ice" (Los class; 8.1.2) — the rez of a card
/// of the type the sentence names. The condition names no player because only
/// the Corp rezzes cards (8.1.1), which is what lets a Runner card watch for
/// it in the same words a Corp card does (Lt. Todachine class). Pair it with
/// [`CardBuilder::when_first_each_turn`] for the printed "the first time each
/// turn".
pub fn corp_rezzes_a(of: CardType) -> TriggerCond {
    TriggerCond::CorpRezzesCard {
        of_types: vec![of],
        of_subtypes: Vec::new(),
        criteria: Vec::new(),
        requires: Vec::new(),
    }
}
/// "When your action phase ends, if <requirements>…" (Nebula class; 5.6.2.)
pub fn action_phase_ends_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::ActionPhaseEnds { side, requires: reqs.to_vec() }
}
/// "…you play an operation" (Gemilang class; the trigger half — pair with
/// [`CardBuilder::when_first_each_turn`] for the printed "first time each
/// turn").
pub fn plays_a(by: Side, of: CardType) -> TriggerCond {
    TriggerCond::CardPlayed {
        by: Some(by),
        of_types: vec![of],
        of_subtypes: Vec::new(),
        criteria: Vec::new(),
        other_than_source: false,
        also_installed: false,
        matching_choice: None,
    }
}
/// "…you play a **run** event" (Ken Tenma class) — the same trigger with the
/// sentence's subtype stipulation (2.16), which is read through the 9.12.1b
/// pipeline like every other subtype query.
pub fn plays_a_subtyped(by: Side, of: CardType, subtype: &'static str) -> TriggerCond {
    TriggerCond::CardPlayed {
        by: Some(by),
        of_types: vec![of],
        of_subtypes: vec![subtype],
        criteria: Vec::new(),
        other_than_source: false,
        also_installed: false,
        matching_choice: None,
    }
}
/// "The first time each turn you play **a copy of** <name>…" (Subliminal
/// Messaging). Two rules make this one condition: 10.1.5 — a card's own name
/// used with the word "copy" is not self-reference, so every card with that
/// name meets it. The sentence's "the first time each turn" is the ordinal
/// [`CardBuilder::when_first_each_turn`] carries — one 9.6.5c stipulation for
/// every condition, and deliberately not 9.3.6g's flag (that flag is per
/// object, so a second copy of the card would carry a fresh one, and 9.1.6
/// never counts a mandatory ability as "used").
pub fn plays_a_copy_of(by: Side, name: &'static str) -> TriggerCond {
    TriggerCond::CardPlayed {
        by: Some(by),
        of_types: Vec::new(),
        of_subtypes: Vec::new(),
        criteria: vec![TargetFilter::HasName(name)],
        other_than_source: false,
        also_installed: false,
        matching_choice: None,
    }
}
/// "…if you played an operation this turn" (Nebula class; history 1.12.6).
pub fn played_operation_this_turn(side: Side) -> TriggerRequirement {
    TriggerRequirement::PlayedOperationThisTurn(side)
}
/// "Gain [click]." (1.11.3a.)
pub fn gain_clicks(side: Side, n: u32) -> Instruction {
    Instruction::GainClicks(side, Quantity::c(n as i64))
}
/// "Identify your mark." (CR 10.11.2.) The parenthetical every card printing
/// this sentence carries — "if you don't have a mark, a random central server
/// becomes your mark for this turn" — is 1.4's reminder text: it restates
/// 10.11.2a and 10.11.3, which this instruction already is, so it needs no
/// second call.
pub fn identify_mark() -> Instruction {
    Instruction::IdentifyMark
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
/// "…if there are N or more <kind> counters hosted on this card" (Trickster
/// Taka's hosted credits): 9.12.2's calculated amount — the count
/// [`per_hosted_counter`] reads — compared against a printed threshold, as a
/// 9.6.5c requirement on the condition it rides.
pub fn hosted_counters_at_least(kind: CounterKind, n: i64) -> TriggerRequirement {
    TriggerRequirement::QuantityAtLeast {
        amount: Quantity::CountersOnSource(kind),
        at_least: n,
    }
}
/// "When a discard phase ends, if <requirements>…" (5.5.4 / Breaking News,
/// The Class Act.) The sentence names no player, so EITHER discard phase
/// meets it — for "when YOUR discard phase ends", write
/// [`your_discard_phase_ends_if`].
pub fn discard_phase_ends_if(reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::DiscardPhaseEnds { side: None, requires: reqs.to_vec() }
}
/// "When your discard phase ends, if <requirements>…" (5.5.4 / Citadel
/// Sanctuary), naming whose.
pub fn your_discard_phase_ends_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::DiscardPhaseEnds { side: Some(side), requires: reqs.to_vec() }
}
/// "Whenever you **discard cards to reach your maximum hand size**…"
/// (Magdalene Keino-Chemutai) — 5.7.4's discard itself, met once however many
/// cards it moved, and naming every one of them for
/// [`among_those_cards`]. Not [`your_discard_phase_ends_if`], which is the
/// formal end of the turn around it (5.1.4b) and happens whether or not a
/// card was discarded.
pub fn discards_cards_to_reach_maximum_hand_size(side: Side) -> TriggerCond {
    TriggerCond::PlayerDiscardsCards { side, to_hand_size: true }
}
/// "…you would draw any number of cards" (9.9.5a) — an [interrupt] trigger on
/// a draw of `by`'s. Naming the player is what keeps a Runner card off the
/// Corp's draws. The printed "the first time each turn" is the ability's
/// ordinal ([`CardBuilder::interrupt_first_each_turn`]), where every
/// condition's is.
pub fn would_draw(by: Side) -> TriggerCond {
    TriggerCond::WouldDraw { by: Some(by) }
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
/// "…the cards found by this ability's search" (8.7.4) — still set aside
/// facedown, and reachable by the instruction that follows the search.
pub fn found_by_search() -> TargetSpec {
    TargetSpec::FoundBySearch
}
/// "Shuffle <cards> into your stack." (8.7.3-adjacent; the cards move and the
/// deck is shuffled.)
pub fn shuffle_into_deck(targets: TargetSpec, to: Side) -> Instruction {
    Instruction::ShuffleCardsIntoDeck { targets, to }
}
/// "Choose up to N <description>." (1.15.2e: "up to" is what makes the floor
/// zero.)
pub fn choose_up_to(count: i64, criteria: &[TargetFilter]) -> TargetSpec {
    TargetSpec::Choose { count: Quantity::c(count), criteria: criteria.to_vec(), up_to: true }
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
    TriggerCond::WouldDamage { kind: Some(kind) }
}
/// "…you would take 1 or more tags" as an interrupt condition (9.9.4). The
/// sentence names no run — where a card's ordinal is counted over the run
/// (Jesminder Sareen), the SPAN is what says so, not the condition.
pub fn would_take_tags() -> TriggerCond {
    TriggerCond::WouldTakeTags { during_run: false }
}
/// "…you would take 1 or more tags **during a run**" — the same condition
/// with the run stated as part of it.
pub fn would_take_tags_during_a_run() -> TriggerCond {
    TriggerCond::WouldTakeTags { during_run: true }
}
/// "Prevent 1 tag." / "Avoid 1 tag." — CR 9.9.6a: a number of tags the
/// Runner would take is a VALUE, and this is the only thing that decreases
/// it. Both printed wordings are the same modification of the same value.
pub fn avoid_tags(n: u32) -> Instruction {
    Instruction::AvoidTags(n)
}
/// "Whenever the Runner suffers <kind> damage…" (10.4.1) — the kind is the
/// sentence's stipulation, and [`suffers_any_damage`] is the sentence that
/// makes none.
pub fn suffers_damage(kind: DamageKind) -> TriggerCond {
    TriggerCond::RunnerSuffersDamage { kind: Some(kind), trashed_a_card: false, responsible: None }
}
/// "Whenever the Runner suffers damage…" — any kind.
pub fn suffers_any_damage() -> TriggerCond {
    TriggerCond::RunnerSuffersDamage { kind: None, trashed_a_card: false, responsible: None }
}
/// "Whenever **you do** damage…" (AU Co.) — the same occurrence
/// [`suffers_any_damage`] names, with 10.4.1's stipulation about who was
/// RESPONSIBLE: a card that "does" damage makes its own side responsible,
/// while one that directs the Runner to "suffer" damage made the Runner
/// responsible — so a sentence on the second shape is not one of these.
pub fn does_damage(by: Side) -> TriggerCond {
    TriggerCond::RunnerSuffersDamage { kind: None, trashed_a_card: false, responsible: Some(by) }
}
/// "Whenever the Runner **trashes a card for** <kind> damage…" (Chronos
/// Protocol: Haas-Bioroid) — the same occurrence [`suffers_damage`] names,
/// with the sentence's extra stipulation that the damage procedure actually
/// trashed something. 10.4.2a/b make the trash the procedure itself, so this
/// is one condition and not two; against an empty grip the damage is suffered
/// and this sentence is not met. The cards trashed are what the sentence's
/// "that card" means (1.15.4).
pub fn trashes_a_card_for_damage(kind: DamageKind) -> TriggerCond {
    TriggerCond::RunnerSuffersDamage { kind: Some(kind), trashed_a_card: true, responsible: None }
}
/// "…sabotage N." (10.16: the Corp trashes N cards of their choice from HQ
/// and/or the top of R&D.)
pub fn sabotage(n: i64) -> Instruction {
    Instruction::Sabotage { count: Quantity::c(n) }
}
/// "Whenever you access a <type>…" (9.6: the Runner's side of 7.3.6's
/// access), with the sentence's card-type stipulation as content.
pub fn accesses_a(of: CardType) -> TriggerCond {
    TriggerCond::RunnerAccessesCard { of_types: vec![of], criteria: Vec::new() }
}
/// "Whenever the Runner accesses a **faceup installed** agenda…" (BANGUN:
/// When Disaster Strikes) — the same access occurrence with the sentence's
/// further words about the card as criteria, in the shared filter vocabulary
/// (§12 rule 5), asked of the card's state at the access itself.
pub fn accesses_a_matching(of: CardType, criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::RunnerAccessesCard { of_types: vec![of], criteria: criteria.to_vec() }
}
/// "…you access a card <matching the criteria>…" — the same access condition
/// with the sentence's other stipulations in the shared filter vocabulary
/// ("a card **with a trash cost**", Neutralize All Threats).
pub fn accesses_a_card_matching(criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::RunnerAccessesCard { of_types: Vec::new(), criteria: criteria.to_vec() }
}
/// "Before drawing your starting hand, …" (Ayla "Bios" Rahim; CR 1.6.1a) —
/// an identity ability resolved by the §1.6 setup procedure immediately
/// before the 1.6.6 starting-hand draw (and after the 1.6.5 shuffle). A
/// 1.6.6a mulligan redraw does not resolve it again.
pub fn before_drawing_starting_hand() -> TriggerCond {
    TriggerCond::BeforeDrawingStartingHand
}
/// "Before taking your first turn, …" (NEXT Design; CR 1.6.7a) — the Corp's
/// identity ability resolved immediately before the first turn, after both
/// mulligan decisions and "thus before the game starts".
pub fn before_taking_first_turn() -> TriggerCond {
    TriggerCond::BeforeTakingFirstTurn
}
/// "Whenever you install a card…" — 8.5's install, with no stipulation about
/// what was installed.
pub fn installs_a_card(side: Side) -> TriggerCond {
    TriggerCond::CardInstalledBy {
        side,
        of_types: Vec::new(),
        of_subtypes: Vec::new(),
        into_remote_server: false,
        requires: Vec::new(),
    }
}
/// "…you install a card **in the root of or protecting a remote server**…"
/// (A Teia) — the same install condition with 4.6.6b's one location word
/// narrowed by 4.6.8 to the remotes. Nothing is said about the card.
pub fn installs_a_card_in_a_remote_server(side: Side) -> TriggerCond {
    TriggerCond::CardInstalledBy {
        side,
        of_types: Vec::new(),
        of_subtypes: Vec::new(),
        into_remote_server: true,
        requires: Vec::new(),
    }
}
/// "If you have more [shaper] cards installed than any other faction, when
///  you install a card…" (Jamie "Bzzz" Micken) — the same install condition
/// carrying 9.6.5c's additional requirement.
pub fn installs_a_card_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::CardInstalledBy {
        side,
        of_types: Vec::new(),
        of_subtypes: Vec::new(),
        into_remote_server: false,
        requires: reqs.to_vec(),
    }
}
/// "Whenever you install a <subtype> <type>…" (Noise class) — 2.15's type and
/// 2.16's subtype, both stipulations on the one install condition.
pub fn installs_a_subtyped(side: Side, of: CardType, subtype: &'static str) -> TriggerCond {
    TriggerCond::CardInstalledBy {
        side,
        of_types: vec![of],
        of_subtypes: vec![subtype],
        into_remote_server: false,
        requires: Vec::new(),
    }
}
/// "Whenever you install a piece of hardware…" — 8.5's install with the
/// sentence's card-type stipulation and nothing else.
pub fn installs_a(side: Side, of: CardType) -> TriggerCond {
    TriggerCond::CardInstalledBy {
        side,
        of_types: vec![of],
        of_subtypes: Vec::new(),
        into_remote_server: false,
        requires: Vec::new(),
    }
}
/// "…you trash a card **from R&D**…" (Nuvem SA) — 8.2's trash, naming who did
/// it (1.14.5) and the ONE zone the card left, with 9.6.5c's state
/// stipulation beside it. Nothing is said about the card, so an agenda counts
/// as readily as an operation.
pub fn trashes_a_card_from(by: Side, zone: Zone, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::CardTrashed {
        owner: None,
        by: Some(by),
        of_types: Vec::new(),
        installed_only: false,
        while_accessed: false,
        from_zone: Some(zone),
        at_least_one: false,
        rezzed_only: false,
        except_during_install: false,
        requires: reqs.to_vec(),
    }
}
/// "…you trash **1 or more cards** from HQ…" (AU Co.) — the same trash
/// [`trashes_a_card_from`] names, with 9.12.2a's plural noun: the sentence
/// speaks of the cards of one event together, so it is met ONCE however many
/// cards that event trashed, where the singular sentence is met once per
/// card (9.6.4b).
pub fn trashes_at_least_one_card_from(by: Side, zone: Zone) -> TriggerCond {
    TriggerCond::CardTrashed {
        owner: None,
        by: Some(by),
        of_types: Vec::new(),
        installed_only: false,
        while_accessed: false,
        from_zone: Some(zone),
        at_least_one: true,
        rezzed_only: false,
        except_during_install: false,
        requires: Vec::new(),
    }
}
/// "Whenever **you finish resolving an operation**…" (Nuvem SA) — CR 8.6.7h's
/// step, said by a card OTHER than the one being played: it names the player
/// who played it (8.6.2) and describes the card in the ordinary words. Step
/// 8.6.7g has already trashed the card by then, which is why the player is
/// read off the occurrence and not off the card.
pub fn finishes_resolving_a_played_card(by: Side, criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::CardPlayResolved { by, criteria: criteria.to_vec() }
}
/// "…you finish resolving **an action on an expendable card**" (Nuvem SA) —
/// CR 5.2.2d's moment, which 5.2.2a puts at the end of the action step that
/// ran the action. The description is about the CARD the action was an
/// ability of (5.2.4); a basic action is the game's and not any card's
/// (9.1.3), so no description reaches one.
pub fn finishes_an_action_on(side: Side, criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::ActionCompleted { side, criteria: criteria.to_vec() }
}
/// "…during each of **your** turns" (Nuvem SA) — CR 9.2.1's active player.
/// A printed ordinal counts inside whichever turn is being played, so a
/// sentence whose span is one player's turns says this as well.
pub fn during_the_turn_of(side: Side) -> TriggerRequirement {
    TriggerRequirement::ActiveTurnIs(side)
}
/// "Whenever you trash a piece of hardware **(from any location)**…" — 8.2's
/// trash, naming the player who does it (1.14.5) and the type of card, and
/// deliberately NOT naming where it was trashed from.
pub fn trashes_a_from_anywhere(by: Side, of: CardType) -> TriggerCond {
    TriggerCond::CardTrashed {
        owner: None,
        by: Some(by),
        of_types: vec![of],
        installed_only: false,
        while_accessed: false,
        from_zone: None,
        at_least_one: false,
        rezzed_only: false,
        except_during_install: false,
        requires: Vec::new(),
    }
}
/// "…the Runner trashes an **installed** Corp card…" (NBN: Controlling the
/// Message) — 8.2's trash, naming whose card it was (1.14.1), who did the
/// trashing (1.14.5) and where it was trashed from. A card the Runner trashes
/// on access out of HQ or R&D was never installed, so it is not one of these.
pub fn runner_trashes_an_installed_corp_card() -> TriggerCond {
    TriggerCond::CardTrashed {
        owner: Some(Corp),
        by: Some(Runner),
        of_types: Vec::new(),
        installed_only: true,
        while_accessed: false,
        from_zone: None,
        at_least_one: false,
        rezzed_only: false,
        except_during_install: false,
        requires: Vec::new(),
    }
}
/// "Whenever a <side> card ability causes <the other player> to spend or lose
/// at least 1[credit] <during a run>…" (GameNET) — 1.10.3b's forced loss and
/// 1.10.3c's payment as one condition, with what CAUSED it described in the
/// ordinary words and 9.6.5c's state stipulation beside it.
pub fn spends_or_loses_credits(
    side: Side,
    caused_by: &[TargetFilter],
    reqs: &[TriggerRequirement],
) -> TriggerCond {
    TriggerCond::PlayerPaysCredits {
        side,
        also_lost: true,
        caused_by: caused_by.to_vec(),
        requires: reqs.to_vec(),
    }
}
/// "…**a tag is removed**…" (Synapse Global; 10.5.1) — met once per tag that
/// went back to the bank, by whichever player removed it and whether they did
/// it with 10.5.4's basic action or with a card.
pub fn a_tag_is_removed() -> TriggerCond {
    TriggerCond::TagRemoved
}
/// "…you trash a card you are accessing…" (René "Loup" Arcemont) — 8.2's
/// trash, naming the player who does it (1.14.5) and 7.1.2's access it
/// happens inside of. No zone is named, because the accessed card can be in
/// any of them.
pub fn trashes_the_card_being_accessed(by: Side) -> TriggerCond {
    TriggerCond::CardTrashed {
        owner: None,
        by: Some(by),
        of_types: Vec::new(),
        installed_only: false,
        while_accessed: true,
        from_zone: None,
        at_least_one: false,
        rezzed_only: false,
        except_during_install: false,
        requires: Vec::new(),
    }
}
/// "When you trash a **rezzed** card, **except during installation**…" (Ob
/// Superheavy Logistics) — 8.2's trash, naming the player who does it
/// (1.14.5) and two facts of the MOMENT of the trash: 8.1.2's rezzed (the
/// card was a faceup installed Corp card then, whatever 10.3.1a has done to
/// it since) and NOT 8.5.11a's like-card trash, the one the install
/// procedure itself performs and this sentence excludes. Both are read from
/// the record rather than from the state, which no longer says either.
pub fn trashes_a_rezzed_card_except_during_install(by: Side) -> TriggerCond {
    TriggerCond::CardTrashed {
        owner: None,
        by: Some(by),
        of_types: Vec::new(),
        installed_only: false,
        while_accessed: false,
        from_zone: None,
        at_least_one: false,
        rezzed_only: true,
        except_during_install: true,
        requires: Vec::new(),
    }
}
/// "…the Runner … trashes a Corp card" (Epiphany Analytica) — 8.2's trash,
/// naming whose card it was (1.14.1) and who did the trashing (1.14.5) and
/// nothing else, so a card trashed on access counts as readily as an installed
/// one.
pub fn runner_trashes_a_corp_card() -> TriggerCond {
    TriggerCond::RunnerTrashesCorpCard { requires: Vec::new() }
}
/// "Whenever you trash a Corp card, if <requirements>…" (Wyvern) — one
/// occurrence per card trashed (9.6.4b), with 9.6.5c's requirements listed
/// inside the condition, so they are asked when the trash happens.
pub fn runner_trashes_a_corp_card_if(reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::RunnerTrashesCorpCard { requires: reqs.to_vec() }
}
/// "The first time you perform the same action three times in a row each
/// turn…" (The Collective; 5.2.5a/b).
pub fn same_action_in_a_row(side: Side, count: usize) -> TriggerCond {
    TriggerCond::SameActionInARow { side, count }
}
/// "Whenever the Runner draws a card…" (8.4.2: met once per card drawn).
pub fn draws_a_card(side: Side) -> TriggerCond {
    TriggerCond::PlayerDrawsCards(side)
}
/// "…if the Runner did not make a successful run during their last turn"
/// (Tennin Institute class) — the negative of
/// [`runner_made_a_successful_run_last_turn`], and NOT the same sentence as
/// "made no runs at all".
pub fn runner_made_no_successful_run_last_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        made: false,
        successful_only: true,
        scope: jinteki_cr::ability::TurnScope::LastCompletedTurn,
        on: Vec::new(),
    }
}
/// "…an encounter **with an advanced piece of ice** ends" (Weyland
/// Consortium: Builder of Nations) — the encounter's end (6.5.10), with
/// 1.18.2's "advanced" read off the record of that moment rather than off
/// the board, so counters that move or leave with the ice afterwards do not
/// rewrite what 9.6.5c's ordinal counts.
pub fn encounter_with_advanced_ice_ends() -> TriggerCond {
    TriggerCond::EncounterEnds { criteria: Vec::new(), with_advanced_ice: true }
}
/// "Whenever you take 1 or more bad publicity…" (10.6.1.)
pub fn takes_bad_publicity(side: Side) -> TriggerCond {
    TriggerCond::PlayerTakesBadPublicity(side)
}
/// "Whenever the Runner takes a tag…" (Mr. Stone class.)
pub fn runner_takes_a_tag() -> TriggerCond {
    TriggerCond::RunnerTakesTag { had_no_tags: false }
}
/// "Whenever you take 1 or more tags, if you had no tags…" (Sebastião Souza
/// Pessoa.) The same occurrence — met per TAKING, not per tag — with 9.6.6a's
/// "had"-requirement about the moment before it, read off the occurrence's
/// record rather than off a pool that already counts these very tags.
pub fn runner_takes_tags_having_had_none() -> TriggerCond {
    TriggerCond::RunnerTakesTag { had_no_tags: true }
}
/// "Whenever you advance a card…" — 1.18.2's advance and nothing else, so a
/// counter merely PLACED never meets it. `had_no_advancement` is the printed
/// "if it had no advancement counters".
pub fn advances_a_card(had_no_advancement: bool) -> TriggerCond {
    TriggerCond::AdvancesCard { had_no_advancement }
}
/// "Your maximum hand size is increased by N." / "…is reduced by N." (5.7.3 —
/// the amount carries the polarity.)
pub fn max_hand_size_mod(n: i32) -> StaticDecl {
    StaticDecl::MaxHandSizeMod { whose: jinteki_cr::ability::DeclSubject::Controller, amount: n }
}
/// "**Each player's** maximum hand size is reduced by N." (Cybernetics
/// Division class — the same declaration, with the other scope.)
pub fn each_players_max_hand_size_mod(n: i32) -> StaticDecl {
    StaticDecl::MaxHandSizeMod { whose: jinteki_cr::ability::DeclSubject::EachPlayer, amount: n }
}
/// "Limit N remote servers." (A Teia; CR 4.6.8f.) A restriction (9.3.4) read
/// at step 8.5.16b: while it is active, a destination that would create a new
/// remote beyond the limit is not one the Corp may declare, so an install that
/// has no other destination identifies none at all (8.5.14).
pub fn remote_server_limit(n: u32) -> StaticDecl {
    StaticDecl::RemoteServerLimit(n)
}
/// "Your maximum hand size **is equal to** <amount>." (Cerebral Imaging.) CR
/// 9.12.1a applies an effect that SETS a value before every effect that
/// moves it, which is why this is a different declaration from
/// [`max_hand_size_mod`] and not the same one with a bigger number.
pub fn max_hand_size_is(q: Quantity) -> StaticDecl {
    StaticDecl::MaxHandSizeIs { whose: jinteki_cr::ability::DeclSubject::Controller, to: q }
}
/// "…another card **of the same type**" (Hayley Kaplan) — the same type as
/// the card the occurrence that met this ability's condition named (1.15.4).
pub fn of_the_same_type_as_the_triggering_card() -> TargetFilter {
    TargetFilter::SameCardTypeAsTriggeringCard
}
/// "…**another copy of that ice**" (The Foundry) — a card with the same NAME
/// as the one the occurrence that met this ability's condition named (1.15.4
/// + 2.1.4). It names no zone of its own, so whatever the sentence says about
/// where to look ("search R&D") is what says it.
pub fn a_copy_of_the_triggering_card() -> TargetFilter {
    TargetFilter::SameNameAsTriggeringCard
}
/// "…a card with a printed rez cost exactly N[credit] **less than the
/// trashed card's** printed rez cost" (Ob Superheavy Logistics) — a printed
/// number compared against the card the occurrence that met this ability's
/// condition named (1.15.4), the relational sibling of
/// [`a_copy_of_the_triggering_card`]. Both sides must HAVE a printed rez
/// cost — 8.1.2's assets, ice and upgrades — so an operation or an agenda
/// matches nothing. It names no zone of its own; "search R&D" does.
pub fn rez_cost_exactly_less_than_the_triggering_cards(n: i64) -> TargetFilter {
    TargetFilter::RezCostRelativeToTriggeringCard { delta: -n }
}
/// "…from among **those cards**" (Magdalene Keino-Chemutai) — one of the
/// cards the occurrence that met this ability's condition named (1.15.4 in
/// the plural). [`the_triggering_card`] is the same reference to one card;
/// this is what a condition met by a whole event — one draw, one discard —
/// leaves behind. It fixes the cards by identity, so it says where they are
/// without naming a zone.
pub fn among_those_cards() -> TargetFilter {
    TargetFilter::AmongTriggeringCards
}
/// "…a card **in the root of or protecting the attacked server**" (LEO
/// Construction) — 4.6.6b puts both halves of a server *in* it, and 6.1.2 is
/// which server is under attack. It reaches nothing outside a run, which is
/// what keeps a cost naming one of these cards unpayable then.
pub fn in_the_attacked_server() -> TargetFilter {
    TargetFilter::InAttackedServer
}
/// "**Trash 1 rezzed bioroid card in the root of or protecting the attacked
/// server:**" — trashing described cards as a trigger cost (1.16.10). The
/// cards are the payer's to choose, and the description says where they are.
pub fn trash_cards_matching(n: u32, criteria: &[TargetFilter]) -> Cost {
    Cost::trash_matching(n, criteria.to_vec())
}
/// "…a **non**-agenda card", "…a **non**-virus program" — any description
/// word, negated. It names no zone of its own, so 1.15.2c's play-area default
/// still applies unless another word beside it lifts it.
pub fn non(f: TargetFilter) -> TargetFilter {
    TargetFilter::Not(Box::leak(Box::new(f)))
}
/// "As an additional cost to access a card in the root of a remote server,
/// the Runner must pay <cost>." (Gagarin class; 1.16.10 / 7.4.3.)
pub fn additional_cost_to_access_a_card_in_a_remote_root(c: Cost) -> StaticDecl {
    StaticDecl::AdditionalAccessCost(c)
}
/// "As an additional cost to run <these servers>, the Runner must pay
/// <cost>." (Earth Station: SEA Headquarters; 1.16.10 / 6.3.4.) The named
/// servers are the sentence's stipulation on the one declaration — a
/// sentence naming none ("…to make a run", Service Outage class) is the same
/// declaration with [`jinteki_cr::instr::RunServerSet::Any`].
pub fn additional_cost_to_run(servers: &[ServerId], c: Cost) -> StaticDecl {
    StaticDecl::AdditionalRunActionCost {
        cost: c,
        on: jinteki_cr::instr::RunServerSet::These(servers.to_vec()),
    }
}
/// "As an additional cost to run **a remote server**, the Runner must pay
/// <cost>." (Earth Station: Ascending to Orbit.) 4.6.8's remotes are a class
/// the game state computes, not a list the card could print — which is why
/// this is [`jinteki_cr::instr::RunServerSet::AnyRemote`] and never an
/// enumeration.
pub fn additional_cost_to_run_a_remote_server(c: Cost) -> StaticDecl {
    StaticDecl::AdditionalRunActionCost { cost: c, on: jinteki_cr::instr::RunServerSet::AnyRemote }
}
/// "The Runner pays 1[credit] more when spending a [click] to remove a tag
/// **(not through a card ability)**." (SYNC, front face; 1.16.2 / 5.2.7g.)
/// The parenthetical is the declaration's whole scope: it names the BASIC
/// ACTION (5.2.5a's identity), so it reaches every taking of that action and
/// no card ability, however alike that ability's text.
pub fn remove_tag_basic_action_costs_more(n: i32) -> StaticDecl {
    StaticDecl::BasicActionCostMod {
        action: jinteki_cr::change::BasicAction::RemoveTag,
        amount: n,
    }
}
/// "You may pay 2[credit] fewer when spending a [click] to trash a resource
/// (not through a card ability)." (SYNC, back face; 1.16.2 / 5.2.6g.) The
/// same declaration about the other basic action, with the polarity the
/// sentence prints; 1.16.2a floors the modified cost at 0.
pub fn trash_resource_basic_action_costs_less(n: i32) -> StaticDecl {
    StaticDecl::BasicActionCostMod {
        action: jinteki_cr::change::BasicAction::TrashResource,
        amount: -n,
    }
}
/// "As an additional cost to trash a <matching> resource with the basic
/// action, … must <cost>." (Sebastião Souza Pessoa; 1.16.10 / 5.2.6g.) The
/// criteria describe the resource the action ANNOUNCES — which is why that
/// action announces before paying — and the cost is combined with the
/// regular 2[credit] into one payment by the action's taker, the Corp.
/// 1.16.1b: a resource whose combined cost is unpayable cannot be announced
/// at all.
pub fn additional_cost_to_basic_trash_matching(
    criteria: &[TargetFilter],
    c: Cost,
) -> StaticDecl {
    StaticDecl::AdditionalBasicActionCost {
        action: jinteki_cr::change::BasicAction::TrashResource,
        target_criteria: criteria.to_vec(),
        cost: c,
    }
}
/// "…1 resource **or** piece of hardware" (2.15) — the type LIST as one
/// description word, because a card has exactly one type and several
/// [`of_type`] words together would mean all of them.
/// "…an **icebreaker** or a **run** event" — the printed "or" between two
/// whole descriptions. Each inner list is one description, read as all of its
/// words together, exactly as descriptions written beside each other are.
///
/// Use [`of_any_type`] or [`with_any_subtype`] for the "or" between single
/// words of one kind ("a resource **or** piece of hardware"); this is for the
/// one that separates descriptions.
pub fn any_of(alternatives: &[&[TargetFilter]]) -> TargetFilter {
    let leaked: Vec<&'static [TargetFilter]> =
        alternatives.iter().map(|alt| &*Box::leak(alt.to_vec().into_boxed_slice())).collect();
    TargetFilter::AnyOf(Box::leak(leaked.into_boxed_slice()))
}
/// "…a card that already has [a power counter] on it" (the charge keyword's
/// reminder text; 1.9).
pub fn with_counters(kind: CounterKind, at_least: u32) -> TargetFilter {
    TargetFilter::HasCounters { kind, at_least }
}
pub fn of_any_type(list: &'static [CardType]) -> TargetFilter {
    TargetFilter::CardTypeIsAny(list)
}
/// "Whenever you use a [trash] ability…" (Geist class) — 1.19.4's printed
/// [trash] symbol, which is not 7.1.5's basic trash ability.
pub fn uses_a_trash_symbol_ability(side: Side) -> TriggerCond {
    TriggerCond::UsesTrashAbility { side, basic: Some(false) }
}
/// "…if <side> has MORE scored agenda points than the other player" (1.17.1).
/// Strictly ahead: a tie does not meet it.
pub fn agenda_points_ahead(side: Side) -> TriggerRequirement {
    TriggerRequirement::AgendaPointsAhead { side }
}
/// "If you have more **[criminal]** cards installed than any other faction, …"
/// / "…more **[nbn]** cards rezzed than any other faction, …" — the clause
/// every draft-format identity opens with (2.13). The described cards are the
/// ordinary filter words, so "installed" is `[installed_runner_card()]` and
/// "rezzed" is `[installed_corp_card(), rezzed()]`; they are grouped by
/// printed faction and the named group must be STRICTLY the largest.
pub fn more_cards_of_this_faction_than_any_other(
    faction: &'static str,
    criteria: &[TargetFilter],
) -> TriggerRequirement {
    TriggerRequirement::LargestFactionGroupIs { faction, criteria: criteria.to_vec() }
}
/// "…a facedown card" (1.13.2) — the plain question, in whatever zone the
/// rest of the description names.
pub fn facedown() -> TargetFilter {
    TargetFilter::Facedown
}
/// "Whenever you rez an <subtype>…" (Spark Agency class) — 8.1.2's rez with
/// 2.16's subtype stipulation and no stipulation about the card's type.
pub fn corp_rezzes_a_subtyped(subtype: &'static str) -> TriggerCond {
    TriggerCond::CorpRezzesCard {
        of_types: Vec::new(),
        of_subtypes: vec![subtype],
        criteria: Vec::new(),
        requires: Vec::new(),
    }
}
/// "Whenever you rez a piece of **AP** or **destroyer** ice **during a run**…"
/// (Thunderbolt Armaments) — 8.1.2's rez with a whole description of the card
/// rezzed and 9.6.5c's state stipulation. The description is the ordinary
/// filter vocabulary, so a printed "or" between subtypes is
/// [`with_any_subtype`] here exactly as it is in a target announcement.
pub fn corp_rezzes_matching(
    criteria: &[TargetFilter],
    reqs: &[TriggerRequirement],
) -> TriggerCond {
    TriggerCond::CorpRezzesCard {
        of_types: Vec::new(),
        of_subtypes: Vec::new(),
        criteria: criteria.to_vec(),
        requires: reqs.to_vec(),
    }
}
/// "Whenever <side> loses or spends [click] during a run…" (Seidr class;
/// 5.2.1 keeps a click SPENT and a click LOST apart, and this sentence names
/// both).
pub fn spends_or_loses_click_during_run(side: Side) -> TriggerCond {
    TriggerCond::PlayerSpendsClick { side, during_run: true, also_lost: true }
}
/// "…gets −N strength for the remainder of <duration>." — a modification of
/// a card the description names, rather than of this one.
pub fn modify_strength_of(target: TargetSpec, n: i32, duration: WantedDuration) -> Instruction {
    Instruction::ModifyStrength {
        target,
        amount: Quantity::c(n as i64),
        duration: Some(duration),
    }
}
/// "Host the <accessed> card on this program/resource." (Cupellation and
/// Film Critic class; the accessed card is no longer being accessed.)
pub fn host_accessed_on_self() -> Instruction {
    Instruction::HostCards {
        cards: TargetSpec::AccessedCard,
        host: TargetSpec::SelfSource,
        faceup: false,
    }
}
/// "…access 2 additional cards." (Cupellation class; 7.3.5.)
pub fn additional_accesses(n: i64) -> Instruction {
    Instruction::AdditionalAccesses(Quantity::c(n))
}
/// "Whenever you breach <server>, if <requirements>…" — one server, or the
/// several a printed "or" names ("breach HQ or R&D").
pub fn breaches_server_if(server: ServerId, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::BreachesServer { servers: vec![server], requires: reqs.to_vec() }
}
/// "Whenever you breach HQ or R&D, if <requirements>…" (Mercury.)
pub fn breaches_one_of_if(servers: &[ServerId], reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::BreachesServer { servers: servers.to_vec(), requires: reqs.to_vec() }
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
/// "…**secretly set** your identity to any copy of <this identity>."
/// (Méliès U class.) The controller seals a choice among the identity's
/// printed backs — the copies it ships as; the flip is the reveal.
pub fn secretly_set_identity_face(side: Side) -> Instruction {
    Instruction::SecretlySetFlipFace(side)
}
/// "When you flip this identity **to this side** during a run on <these
/// servers>…" (Méliès U's backs). Written on the BACK face, whose abilities
/// exist exactly while it is up — which is what "to this side" says.
pub fn flipped_to_this_side_during_a_run_on(servers: &[ServerId]) -> TriggerCond {
    TriggerCond::SelfFlippedTo { requires: vec![during_a_run_on(servers)] }
}
/// "Switch your identity with another identity …" (Rebirth class; CR 1.5.4).
/// The identity that leaves the play area goes back to the pile it came from
/// (1.5.4b); a double-sided one arrives front side faceup (1.5.4d).
pub fn switch_identity(side: Side, with: TargetSpec) -> Instruction {
    Instruction::SwitchIdentity { side, with }
}
/// "all credits in their credit pool" (Closed Accounts class; 1.10.)
pub fn credits_in_pool_of(side: Side) -> Quantity {
    Quantity::CreditsInPoolOf(side)
}
/// "Whenever the Corp scores an agenda…" (1.17.6.)
pub fn corp_scores_agenda() -> TriggerCond {
    TriggerCond::CorpScoresAgenda { requires: Vec::new(), criteria: Vec::new() }
}
/// "Whenever you score an agenda **that you did not install or advance this
///  turn**…" (Issuaq Adaptics) — the same condition with what the sentence
/// says about the AGENDA, in the ordinary description words.
pub fn corp_scores_an_agenda_matching(criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::CorpScoresAgenda { requires: Vec::new(), criteria: criteria.to_vec() }
}
/// "If you have more [nbn] cards rezzed than any other faction, whenever an
///  agenda is scored…" (Information Dynamics) — the same condition carrying
/// 9.6.5c's additional requirement.
pub fn corp_scores_agenda_if(reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::CorpScoresAgenda { requires: reqs.to_vec(), criteria: Vec::new() }
}
/// "…the Runner **steals or trashes** a Corp card" (Epiphany Analytica) — ONE
/// condition met by either occurrence, so a printed ordinal in front of it is
/// spent once and not once per half. Two abilities is the wrong shape for that
/// sentence, however right it is for a sentence with no ordinal (Leela Patel).
pub fn either_of(alternatives: &[TriggerCond]) -> TriggerCond {
    TriggerCond::AnyOf { alternatives: alternatives.to_vec(), requires: Vec::new() }
}
/// "Whenever you and the Runner **reveal secretly spent credits**, …" (Nisei
/// Division) — 10.14.6c's reveal step, one occurrence for both players.
pub fn secretly_spent_credits_are_revealed() -> TriggerCond {
    TriggerCond::SecretlySpentCreditsRevealed
}
/// "Whenever the Runner steals an agenda…" (1.17.7.)
pub fn runner_steals_agenda() -> TriggerCond {
    TriggerCond::RunnerStealsAgenda { requires: Vec::new() }
}
/// "…whenever an agenda is stolen" with 9.6.5c's additional requirement
/// (Information Dynamics).
pub fn runner_steals_agenda_if(reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::RunnerStealsAgenda { requires: reqs.to_vec() }
}
/// "When a discard phase ends, …" (5.5.4). CR 5.1.4b puts it at the same step
/// as the turn formally ending, so it is that occurrence read as a different
/// sentence. `side` is the sentence's stipulation about whose — `None` where
/// it names no player.
pub fn discard_phase_ends(side: Option<Side>) -> TriggerCond {
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
        made: true,
        successful_only: false,
        scope: TurnScope::LastCompletedTurn,
        on: Vec::new(),
    }
}
/// "…the Runner made a successful run during their last turn."
pub fn runner_made_a_successful_run_last_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        made: true,
        successful_only: true,
        scope: TurnScope::LastCompletedTurn,
        on: Vec::new(),
    }
}
/// "…you made a successful run this turn" (Mutual Favor class) — the same
/// question asked of the CURRENT turn.
pub fn made_a_successful_run_this_turn() -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        made: true,
        successful_only: true,
        scope: TurnScope::ThisTurn,
        on: Vec::new(),
    }
}
/// "…the Runner has not run on a central server this turn" — the same
/// history question with the server stipulation the sentence names (4.6.5's
/// three centrals) and the polarity it wants. Written for the "ignore this
/// ability until the end of the turn whenever the Runner runs on a central
/// server" clause, which is that sentence read from the other side.
pub fn runner_made_no_run_this_turn_on(servers: &[ServerId]) -> TriggerRequirement {
    TriggerRequirement::RunnerMadeRun {
        made: false,
        successful_only: false,
        scope: TurnScope::ThisTurn,
        on: servers.to_vec(),
    }
}
/// "…you have at least N link" (1.20).
pub fn link_at_least(n: u32) -> TriggerRequirement {
    TriggerRequirement::RunnerLinkAtLeast(n)
}
/// "…if the Runner has 3 or more agenda points" (Complete Image; 1.17.1).
pub fn agenda_points_at_least(side: Side, points: i32) -> TriggerRequirement {
    TriggerRequirement::AgendaPointsAtLeast { side, points }
}
/// "…a card in a remote server" (4.6.6 — the root and the ice protecting it).
pub fn in_a_remote_server() -> TargetFilter {
    TargetFilter::InRemoteServer
}
/// "Look at <side>'s grip." (1.21.2 — every card in that hand.)
pub fn look_at_whole_hand_of(hand_of: Side, by: Side) -> Instruction {
    look_at(all_matching(&[TargetFilter::CardsInHandOf(hand_of)]), by)
}
/// "When your turn ends, …" (5.6.3/5.7.2 — the formal end of the turn).
pub fn turn_ends(side: Side) -> TriggerCond {
    TriggerCond::TurnEnds { side, requires: Vec::new() }
}
/// "If you have more [haas-bioroid] cards rezzed than any other faction, when
///  the Runner's turn ends, …" (Strategic Innovations) — 5.6.3/5.7.2's formal
/// end of the turn carrying 9.6.5c's additional requirement.
pub fn turn_ends_if(side: Side, reqs: &[TriggerRequirement]) -> TriggerCond {
    TriggerCond::TurnEnds { side, requires: reqs.to_vec() }
}
/// "Remove <cards> from the game." (§4.9.)
pub fn remove_from_game(targets: TargetSpec) -> Instruction {
    Instruction::RemoveCardsFromGame { targets }
}
/// "Then, they shuffle their stack." (Chronos Protocol: Haas-Bioroid.) A
/// shuffle on its own, with no cards moving into the deck — CR 4.2.3's
/// "explicitly directed to manipulate the cards in a deck". Not 8.7.3's
/// post-search shuffle, which the search performs with no sentence asking.
pub fn shuffle_deck_of(side: Side) -> Instruction {
    Instruction::ShuffleDeck { side }
}
/// "Whenever the Runner breaks a printed subroutine on this ice, …"
/// (Gold Farmer class — met once per subroutine broken, not once per
/// encounter.)
pub fn printed_subroutine_broken() -> TriggerCond {
    TriggerCond::SubroutineBrokenOnSelf { printed_only: true }
}
/// "…there is an installed AI program" (IP Block class) — at least `n` cards
/// on the board match the description. The criteria are the same ones a
/// target announcement uses, so 1.15.2c applies: without a criterion naming
/// a zone this asks about INSTALLED cards, which is what "there is" means.
pub fn board_has(criteria: &[TargetFilter], n: u32) -> TriggerRequirement {
    TriggerRequirement::BoardHasMatching { criteria: criteria.to_vec(), at_least: n }
}
/// "…if there are no more <description>" — the same question with the
/// threshold at the other end (Asmund Pudlat; `n = 0` is "none").
pub fn board_has_at_most(criteria: &[TargetFilter], n: u32) -> TriggerRequirement {
    TriggerRequirement::BoardHasAtMostMatching { criteria: criteria.to_vec(), at_most: n }
}
/// "…if this is **not the first time** they have approached ice this run"
/// (Mti Mwekundu) — said the way the run's own history answers it: they have
/// approached ice this run already. 6.1.5b is the case that answers no, a
/// server with no ice protecting it.
pub fn approached_ice_this_run_already() -> TriggerRequirement {
    TriggerRequirement::IceApproachesThisRunAtLeast(1)
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

/// "**Each player** needs 1 fewer agenda point to win the game." (Harmony
/// Medtech.) 1.17.2 states the win as a comparison against a number, and this
/// modifies that number — not anyone's score, which every other ability goes
/// on reading unchanged.
pub fn each_player_needs_fewer_agenda_points_to_win(n: i64) -> StaticDecl {
    StaticDecl::AgendaPointsToWinMod {
        whose: jinteki_cr::ability::DeclSubject::EachPlayer,
        amount: Quantity::c(-n),
    }
}
/// "For each hosted power counter, **you** need 1 less agenda point to win the
/// game." (Issuaq Adaptics.) The same declaration about the controller alone,
/// with a calculated amount — re-read every time the comparison is made, so a
/// counter arriving lowers the requirement at once.
pub fn you_need_fewer_agenda_points_to_win(amount: Quantity) -> StaticDecl {
    StaticDecl::AgendaPointsToWinMod {
        whose: jinteki_cr::ability::DeclSubject::Controller,
        amount: Quantity::Minus(Box::new(Quantity::c(0)), Box::new(amount)),
    }
}
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
/// "All **bioroid** ice has +1 strength." The same modification
/// [`strength_mod`] states about the card it is printed on, stated instead
/// about every card the description reaches. A subtype in the description is
/// read as PRINTED here (2.16), because the answer is gathered while the
/// characteristics pipeline is being built.
pub fn strength_mod_of(criteria: &[TargetFilter], n: i32) -> StaticDecl {
    StaticDecl::StrengthModMatching { criteria: criteria.to_vec(), delta: n }
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
/// "[interrupt] → Whenever 1 or more <described> cards would be trashed, set
/// those cards aside instead of adding them to the heap. <then…> Ignore this
/// ability if you have already removed a card from the game with it this
/// turn." (Skorpios Defense Systems; 9.9.8b + 4.8.)
///
/// The cards of ONE trash occurrence are set aside together, faceup (4.8.6),
/// as one 4.8.7 group, and `then` — the sentences after the replacement —
/// resolves once over that group before the still-set-aside cards complete
/// the movement. The last printed sentence is the helper's last word: the
/// declaration stops applying for the rest of the turn once a removal made
/// with it has removed a card (spent by the REMOVAL, not by the
/// interception — 9.4.1's statics never resolve, so 9.3.6g's use-spent flag
/// cannot be what it means; the change log answers instead, 10.2.1).
pub fn set_trashed_aside_then_until_removed_with_it_this_turn(
    criteria: &[TargetFilter],
    then: impl IntoIterator<Item = Instruction>,
) -> StaticDecl {
    StaticDecl::SetsTrashedCardsAside {
        criteria: criteria.to_vec(),
        then: then.into_iter().collect(),
        until_removed_with_it_this_turn: true,
    }
}
/// "This card is not trashed until another current is played or an agenda is
/// stolen." — the whole printed sentence of a CURRENT OPERATION (8.6.6c,
/// 3.5.1b).
pub fn not_trashed_until_an_agenda_is_stolen() -> StaticDecl {
    StaticDecl::PlayedNotTrashedUntil {
        until: vec![another_current_is_played(), runner_steals_agenda()],
    }
}
/// "This card is not trashed until another current is played or an agenda is
/// scored." — the same sentence on a CURRENT EVENT (8.6.6c, 3.7.1b); the one
/// word that differs is which player putting an agenda in their score area
/// ends it.
pub fn not_trashed_until_an_agenda_is_scored() -> StaticDecl {
    StaticDecl::PlayedNotTrashedUntil {
        until: vec![another_current_is_played(), corp_scores_agenda()],
    }
}
/// "…another current is played" (3.5.1b/3.7.1b: a current is an operation or
/// an event with the subtype, played by either player, and "another" is any
/// but this one).
pub fn another_current_is_played() -> TriggerCond {
    TriggerCond::CardPlayed {
        by: None,
        of_types: vec![CardType::Operation, CardType::Event],
        of_subtypes: vec!["Current"],
        criteria: Vec::new(),
        other_than_source: true,
        also_installed: false,
        matching_choice: None,
    }
}
/// "The <side>'s identity loses its printed abilities." (Employee Strike
/// class; 9.1.9a — "if an object loses an ability, that ability is completely
/// ignored", and 9.1.9a reaches gained abilities too, since those are
/// abilities the object has.)
pub fn identity_of_loses_its_abilities(side: Side) -> StaticDecl {
    StaticDecl::RemoveAbilitiesOfMatching {
        criteria: vec![TargetFilter::CardTypeIs(CardType::Identity), controlled_by(side)],
    }
}
/// "<This card> gains the text of <the described cards>." (DJ Fenris class;
/// 9.1.9b.) The abilities gained are the described cards' EFFECTIVE abilities,
/// read through the 9.12.1d/e characteristics pipeline — so an identity that
/// had lost its abilities passes on nothing — and the gaining card is their
/// source, which is what "this card" means inside them.
pub fn gains_the_text_of(criteria: &[TargetFilter]) -> StaticDecl {
    StaticDecl::GainAbilitiesOf { criteria: criteria.to_vec() }
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
/// "The advancement requirement of all agendas is increased by N."
/// (The Source class — every agenda in the game, not just installed ones.)
pub fn all_agendas_cost_more(n: i32) -> StaticDecl {
    StaticDecl::ScoreRequirementMod { scope: ReqScope::AllAgendas, amount: n }
}
/// "Agendas in this server may be scored with N fewer advancement counters."
/// (SanSan City Grid class — scoped to the source's own server.)
pub fn agendas_here_cost_less(n: i32) -> StaticDecl {
    StaticDecl::ScoreRequirementMod { scope: ReqScope::SourceServer, amount: -n }
}
/// "You can advance this card." (1.18.3.)
/// "…they play with the grip revealed." (Harishchandra Ent.; CR 4.3.2 —
/// the named player's hand stops being hidden from their opponent.)
pub fn hand_revealed(whose: Side) -> StaticDecl {
    StaticDecl::HandRevealed { whose }
}
/// "You must maintain the order of your heap." (Wyvern; CR 4.4.2 — discard
/// piles are NOT ordered and a player may rearrange one at any time, and this
/// takes that freedom away.) It is what gives [`top_of_heap`] a card to name.
pub fn discard_pile_is_ordered(whose: Side) -> StaticDecl {
    StaticDecl::DiscardPileIsOrdered { whose }
}
/// "The Runner cannot steal more than N agendas each turn." (Haarpsichord
/// Studios; CR 1.17.7 and 1.2.2's absolute "cannot".)
pub fn cannot_steal_more_than_each_turn(n: u32) -> StaticDecl {
    StaticDecl::StealsPerTurnAtMost(n)
}
/// "The Runner is considered to have N additional tags (even if they have
/// 0)…" (Acme Consulting; CR 10.5.2.) A declaration about the NUMBER a tag
/// reader sees, not about the tag counters: "tagged", `Quantity::RunnerTags`
/// and 5.2.6g's gate read it, while removing a tag still finds only the real
/// ones.
pub fn considered_additional_tags(n: i64) -> StaticDecl {
    StaticDecl::ConsideredTagsMod { delta: n }
}
/// "The trash cost of each card is increased by <an amount>." (Industrial
/// Genomics; CR 7.1.5a.) The criteria are what the sentence says about the
/// cards it reaches — empty is the printed "each card".
pub fn trash_costs_increased_by(criteria: &[TargetFilter], amount: Quantity) -> StaticDecl {
    StaticDecl::TrashCostMod { criteria: criteria.to_vec(), amount }
}
/// "The Runner cannot run on remote servers." (Jinteki: Replicating
/// Perfection; CR 6.3.2a — the prohibition is on ANNOUNCING the server, so a
/// run already in progress can still be moved onto one.)
pub fn cannot_initiate_runs_on_remote_servers() -> StaticDecl {
    StaticDecl::CannotInitiateRunOn(jinteki_cr::instr::RunServerSet::AnyRemote)
}
pub fn can_be_advanced() -> StaticDecl {
    StaticDecl::CanBeAdvancedSelf
}
/// "You cannot install **non-virtual** resources." (Apex; CR 1.2.2 — a
/// "cannot" takes precedence over every ability that would direct the
/// install, including the basic action.) The criteria are the sentence's
/// description of the cards it forbids.
pub fn cannot_install(criteria: &[TargetFilter]) -> StaticDecl {
    StaticDecl::CannotInstallMatching { criteria: criteria.to_vec() }
}
/// "You may install agendas faceup." (BANGUN; CR 8.5.16a / 8.5.2 — the
/// opposite number of [`cannot_install`]: a PERMISSION the declaring player
/// states about every install they perform, 5.2.6d's basic action included.
/// It installs nothing itself: where 8.5.2 would settle the placed card's
/// face facedown with nobody asked, the installer is asked instead.) The
/// criteria are the sentence's description of the cards the permission
/// reaches.
pub fn may_install_faceup(criteria: &[TargetFilter]) -> StaticDecl {
    StaticDecl::MayInstallFaceup { criteria: criteria.to_vec() }
}
/// "Lower the install cost of the first <described card> you install each
/// turn by N." (Kate "Mac" McCaffrey, Az McCaffrey.) The reduction happens of
/// its own accord — nothing is paid for it and nothing is chosen, which is
/// what distinguishes it from Patchwork's.
pub fn first_installed_each_turn_costs_less(criteria: &[TargetFilter], n: i32) -> StaticDecl {
    StaticDecl::InherentCostMod {
        which: InherentCost::Install,
        criteria: criteria.to_vec(),
        amount: -n,
        first_each_turn: true,
    }
}
/// "For the first <kind> damage the Runner suffers each turn, you may look at
/// the Runner's grip and select the card that is trashed." (Chronos Protocol:
/// Selective Mind-mapping; CR 10.4.3a — the declaration modifies the damage
/// procedure so the cards are selected rather than random.)
///
/// The "you may" is asked before the grip is named, because the printed word
/// covers the looking as well as the selecting.
pub fn may_select_first_damage_trashes_each_turn(
    by: Side,
    kind: DamageKind,
    count: Quantity,
) -> StaticDecl {
    StaticDecl::SelectsDamageTrashes {
        by,
        count,
        kinds: vec![kind],
        first_each_turn: true,
        optional: true,
    }
}
/// "You choose the cards you trash to damage." (Titanium Ribs; CR 10.4.3a.)
/// No damage type, no ordinal, no choice about it.
pub fn selects_damage_trashes(by: Side, count: Quantity) -> StaticDecl {
    StaticDecl::SelectsDamageTrashes {
        by,
        count,
        kinds: Vec::new(),
        first_each_turn: false,
        optional: false,
    }
}
/// "The first <described card> the Corp rezzes each turn costs N[credit] more
/// to rez." (Reina Roja.)
pub fn first_rezzed_each_turn_costs_more(criteria: &[TargetFilter], n: i32) -> StaticDecl {
    StaticDecl::InherentCostMod {
        which: InherentCost::Rez,
        criteria: criteria.to_vec(),
        amount: n,
        first_each_turn: true,
    }
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
    Quantity::Count(vec![f])
}
/// "N for each …" — scale a quantity.
pub fn times(n: i64, q: Quantity) -> Quantity {
    Quantity::Times(n, Box::new(q))
}
/// "N plus 1 for each …" — a printed base plus a count.
pub fn plus(a: Quantity, b: Quantity) -> Quantity {
    Quantity::Plus(Box::new(a), Box::new(b))
}
/// "…cards **that share a type**" (Slot Machine) — among the cards the
/// description reaches, how many are of the commonest card type (2.15: a card
/// has exactly one). Pair it with [`if_met`] and the printed number.
pub fn sharing_a_card_type(criteria: &[TargetFilter]) -> Quantity {
    Quantity::LargestGroupSharingCardType(criteria.to_vec())
}
/// "…the cards **you revealed when this encounter began**" (Slot Machine) —
/// 1.21.3a puts a revealed card back exactly as it was, so the encounter is
/// what remembers, and the memory dies with it.
pub fn revealed_this_encounter() -> TargetFilter {
    TargetFilter::RevealedThisEncounter
}
/// "If <amount> is N or more, …" (Slot Machine's "if you revealed 2 or more
/// cards that share a type") — a calculated amount against a printed
/// threshold.
/// "…for each time you accessed a card during that run" (Zahya Sadeghi;
/// 7.3.6).
pub fn accesses_this_run() -> Quantity {
    Quantity::AccessesThisRun
}
/// "…if you accessed a card this turn" / "…if you did not access any cards
/// this turn" (Hoshiko Shiro's two faces; 7.3.6 over the turn's history
/// window) — pair with [`at_least`] 1 for the first and [`at_most`] 0 for
/// the second, the direction the sentence prints.
pub fn accesses_this_turn() -> Quantity {
    Quantity::AccessesThisTurn
}
/// "…if your [mu] is full" / "…if you have at least 1 unused [mu]" (Dewi
/// Subrotoputri's two faces) — CR 1.20.4a's own calculated value, the memory
/// limit minus installed programs' memory costs. "Full" is [`at_most`] 0
/// unused; "at least 1 unused" is [`at_least`] 1.
pub fn unused_mu() -> Quantity {
    Quantity::UnusedMemory
}
/// "…the subroutines you broke during that run" (Mercury; 9.8.7).
pub fn subroutines_broken_this_run() -> Quantity {
    Quantity::SubroutinesBrokenThisRun
}
/// "…the subroutines that resolved during that run" (Ryō "Phoenix" Ōno;
/// 9.8.10).
pub fn subroutines_resolved_this_run() -> Quantity {
    Quantity::SubroutinesResolvedThisRun
}
/// "…if you did **not** break any subroutines during that run" — an amount
/// compared against a threshold from the other end.
pub fn at_most(amount: Quantity, n: i64) -> TriggerRequirement {
    TriggerRequirement::QuantityAtMost { amount, at_most: n }
}
/// "…**during a run**" (6.1.1) — there is a run in progress, and nothing else
/// is asked about it.
pub fn during_a_run() -> TriggerRequirement {
    TriggerRequirement::RunInProgress { on: Vec::new() }
}
/// "…**during a run on HQ**" (Méliès U's backs) — the same question with the
/// sentence's server stipulation as content.
pub fn during_a_run_on(servers: &[ServerId]) -> TriggerRequirement {
    TriggerRequirement::RunInProgress { on: servers.to_vec() }
}
pub fn at_least(amount: Quantity, n: i64) -> TriggerRequirement {
    TriggerRequirement::QuantityAtLeast { amount, at_least: n }
}
/// "…if you have **the same number of** cards in your grip **as** the Corp
/// has in HQ" (Lat) — two amounts compared against each other rather than
/// against a printed number.
pub fn same_number(left: Quantity, right: Quantity) -> TriggerRequirement {
    TriggerRequirement::QuantitiesEqual { left, right }
}
/// "…if you do **not** have cards in your grip equal to or greater than your
/// maximum hand size" (Safety First) — NOT (left ≥ right) is left < right,
/// the strict inequality between two calculated amounts.
pub fn fewer_than(left: Quantity, right: Quantity) -> TriggerRequirement {
    TriggerRequirement::QuantityLessThan { left, right }
}
/// "…cards in your grip" / "…cards in HQ" as a NUMBER (4.3.4 makes it open
/// information) — the count "draw until you have 5 cards in HQ" subtracts
/// and "cards in your grip equal to or greater than…" compares.
pub fn cards_in_hand_count(side: Side) -> Quantity {
    Quantity::CardsInHandOf(side)
}
/// "…your maximum hand size" as a NUMBER — as modified, read through the
/// same 9.12.1a pipeline the discard step reads, so a card's own "-2"
/// (Safety First) is already inside what it compares against.
pub fn maximum_hand_size_of(side: Side) -> Quantity {
    Quantity::MaxHandSizeOf(side)
}
/// "…for each credit lost" — the credits this ability has ACTUALLY caused
/// `side` to lose, which 1.10.3b caps at what their pool held. That is what
/// makes "lose up to 5" and "for each credit lost" agree.
pub fn per_credit_lost_by(side: Side) -> Quantity {
    Quantity::CreditsLostThisAbility(side)
}
/// "the number of cards you would draw" (9.9.6) — the amount the draw this
/// [interrupt] is interrupting is about to draw, read as it now stands. It is
/// 0 anywhere else, so only an ability that runs while a draw is imminent can
/// say anything with it.
pub fn cards_you_would_draw() -> Quantity {
    Quantity::ImminentValueOf(EffectClass::Draw)
}

// ---- naming (1.15.1b) ------------------------------------------------------
//
// "Name a card", "name a card type", "name **sentry**, **code gate** or
// **barrier**", "name a number". CR 1.15.1b lists them all in one breath and
// says the same thing about all of them: naming is NOT a target announcement,
// so the value is not chosen until the instruction resolves. Each call below
// is one printed "name …" sentence; the value is then remembered under a
// `key` you pick, and the LATER sentences of the same card refer back to it
// with [`the_named_card`] / [`named_by`].
//
// A key is just a word for "what this card is remembering" — write the
// card's own words for it ("marketing target", "salem's name").

/// "Choose 1 installed piece of ice." (Boomerang, Femme Fatale — CR 9.10.3:
/// the choice is an ordinary 1.15.2 announcement, and a lingering effect
/// remembers it under `key` for as long as this card is active.)
pub fn choose_and_remember(key: &'static str, count: i64, criteria: &[TargetFilter]) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::Object(choose(count, criteria)),
        duration: WantedDuration::WhileSourceActive,
    }
}
/// "…**that ice**", "…the chosen card" — the object this card remembers.
pub fn the_remembered(key: &'static str) -> TargetSpec {
    TargetSpec::MaintainedChoice(key)
}
/// "You cannot <act on> **that card** [for a duration]." (Saraswati
/// Mnemonics; CR 1.2.2 + 9.10.1.) The card is NAMED rather than described, so
/// this is a lingering effect about one object and not a declaration about
/// every card a description reaches — a second copy installed later is
/// untouched. The acts the sentence names are a list because one sentence can
/// name several ("score **or** rez") and that is still one prohibition.
pub fn cannot_be(
    target: TargetSpec,
    actions: &[ProhibitedAction],
    duration: WantedDuration,
) -> Instruction {
    Instruction::CreateLingeringEffect {
        payload: LingeringSpec::Prohibit { targets: target, actions: actions.to_vec() },
        duration,
    }
}
/// "…until **your** next turn begins." (CR 5.1: the turns alternate, so this
/// span covers the rest of this turn and the whole of the opponent's.)
pub fn until_next_turn_begins_of(side: Side) -> WantedDuration {
    WantedDuration::UntilNextTurnBeginsOf(side)
}
/// "…**this turn**." (A Teia.) The shorter of the two spans beside
/// [`until_next_turn_begins_of`]: it ends with the turn being played, whoever
/// is playing it.
pub fn this_turn() -> WantedDuration {
    WantedDuration::ThisTurn
}
/// "When this run ends, …" as a DELAYED conditional (9.6.13) — an ability
/// created now that waits for the run to end, which is what a card trashed to
/// pay its own trigger cost needs: nothing of the source is left to carry a
/// conditional ability. 9.6.13d: created outside a run, it is never created
/// at all.
pub fn when_this_run_ends(
    label: &'static str,
    successful_only: bool,
    optional: bool,
    instrs: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::CreateDelayedConditional {
        def: Box::new(
            AbilityDef::conditional(
                TriggerCond::RunEnds { successful_only, on: Vec::new() },
                instrs.into_iter().collect(),
                optional,
            )
            .labeled(label),
        ),
        duration: WantedDuration::ThisRun,
    }
}
/// "Name a card." (Ark Lockdown, Salem's Hospitality, Targeted Marketing —
/// 2.1.1: a card's identifier is its name.) The name is remembered under
/// `key` for as long as this card is active (9.10.3c).
pub fn name_a_card(key: &'static str) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::Named {
            of: jinteki_cr::instr::NameSpace::CardName,
            excluding: None,
        },
        duration: WantedDuration::WhileSourceActive,
    }
}
/// "Name a card other than <this card>." (Reclamation Order — 10.1.5 reads a
/// card's own name as "this object", so the exclusion needs no name.)
pub fn name_a_card_other_than_this_one(key: &'static str) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::Named {
            of: jinteki_cr::instr::NameSpace::CardName,
            excluding: Some(jinteki_cr::instr::NameExclusion::SourceName),
        },
        duration: WantedDuration::WhileSourceActive,
    }
}
/// "…choose a card name." with a stated duration — Whistleblower's choice
/// lasts the run and no longer, and its card is trashed to make it, so
/// 9.10.3c's "until the source becomes inactive" would be no duration at all.
pub fn name_a_card_for(key: &'static str, duration: WantedDuration) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::Named {
            of: jinteki_cr::instr::NameSpace::CardName,
            excluding: None,
        },
        duration,
    }
}
/// "Name a number." (RNG Key — 1.1.3: numbers in this game are integers.)
pub fn name_a_number(key: &'static str, duration: WantedDuration) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::Named {
            of: jinteki_cr::instr::NameSpace::Number,
            excluding: None,
        },
        duration,
    }
}

/// "Name a card type." (Azmari EdTech, Falsified Credentials, Ibrahim
/// Salem.) A card has exactly one type and 2.15.2 lists all ten of them, so
/// this is 9.11.4g's choice between options — one branch per type, each
/// remembering its own — and not an open namespace at all.
pub fn name_a_card_type(key: &'static str) -> Instruction {
    name_one_of_these_card_types(key, ALL_CARD_TYPES)
}
/// "Name **asset**, **ice**, **operation** or **upgrade**." (Embezzle.) The
/// same sentence with the types the card lists.
pub fn name_one_of_these_card_types(key: &'static str, types: &[CardType]) -> Instruction {
    Instruction::ChooseOne {
        options: types
            .iter()
            .map(|t| (card_type_word(*t), vec![name_the_card_type(key, *t)]))
            .collect(),
    }
}
/// "Name **sentry**, **code gate** or **barrier**." (Wari.) One branch per
/// printed subtype, exactly as the card writes them.
pub fn name_one_of_these_subtypes(key: &'static str, subtypes: &[&'static str]) -> Instruction {
    Instruction::ChooseOne {
        options: subtypes
            .iter()
            .map(|s| (*s, vec![name_the_subtype(key, s)]))
            .collect(),
    }
}
/// One branch of a "name a card type" choice: remember exactly this type.
pub fn name_the_card_type(key: &'static str, t: CardType) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::CardType(t),
        duration: WantedDuration::WhileSourceActive,
    }
}
/// One branch of a "name a subtype" choice: remember exactly this subtype.
pub fn name_the_subtype(key: &'static str, s: &'static str) -> Instruction {
    Instruction::MaintainChoice {
        key,
        of: jinteki_cr::instr::ChoiceSpec::Subtype(s),
        duration: WantedDuration::WhileSourceActive,
    }
}
/// The same, with a stated duration — for a card that is trashed to make the
/// choice (Wari), so that 9.10.3c would otherwise leave nothing remembered.
pub fn name_one_of_these_subtypes_for(
    key: &'static str,
    subtypes: &[&'static str],
    duration: WantedDuration,
) -> Instruction {
    Instruction::ChooseOne {
        options: subtypes
            .iter()
            .map(|s| {
                (
                    *s,
                    vec![Instruction::MaintainChoice {
                        key,
                        of: jinteki_cr::instr::ChoiceSpec::Subtype(s),
                        duration,
                    }],
                )
            })
            .collect(),
    }
}

/// CR 2.15.2: the ten card types, in the order the rule lists them.
pub const ALL_CARD_TYPES: &[CardType] = &[
    CardType::Identity,
    CardType::Agenda,
    CardType::Asset,
    CardType::Ice,
    CardType::Operation,
    CardType::Upgrade,
    CardType::Event,
    CardType::Hardware,
    CardType::Program,
    CardType::Resource,
];

/// The printed word for a card type, for the option label of a "name a card
/// type" choice.
fn card_type_word(t: CardType) -> &'static str {
    match t {
        CardType::Identity => "identity",
        CardType::Agenda => "agenda",
        CardType::Asset => "asset",
        CardType::Ice => "ice",
        CardType::Operation => "operation",
        CardType::Upgrade => "upgrade",
        CardType::Event => "event",
        CardType::Hardware => "hardware",
        CardType::Program => "program",
        CardType::Resource => "resource",
    }
}

/// "…a copy of **that card**" (2.1.4), "…all cards with **the chosen name**",
/// "…1 card of **the named type**", "…if it has **the named subtype**" — a
/// description of whatever this card named under `key`. Which characteristic
/// is compared is decided by what was named, so one word says all four.
pub fn named_by(key: &'static str) -> TargetFilter {
    TargetFilter::MatchesMaintainedChoice(key)
}
/// "…all copies of that card in the heap", "…any number of copies of the
/// named card from Archives" — every card in that discard pile matching what
/// was named. "All" is written as a count equal to how many there are, which
/// is 1.15.2e's "as many distinct targets as are available".
pub fn all_named_cards_in_discard_of(side: Side, key: &'static str) -> TargetSpec {
    all_matching(&[TargetFilter::InDiscardOf(side), named_by(key)])
}
/// "…the Runner reveals the grip and trashes all cards with the chosen name"
/// — every card in that hand matching what was named.
pub fn all_named_cards_in_hand_of(side: Side, key: &'static str) -> TargetSpec {
    all_matching(&[TargetFilter::CardsInHandOf(side), named_by(key)])
}
/// "…any number of copies of the named card from Archives" — the naming
/// player picks however many they like, up to all of them (1.15.2e caps it).
pub fn any_number_of_named_cards_in_discard_of(side: Side, key: &'static str) -> TargetSpec {
    let criteria = vec![TargetFilter::InDiscardOf(side), named_by(key)];
    TargetSpec::Choose {
        count: Quantity::Count(criteria.clone()),
        criteria,
        up_to: true,
    }
}
/// "**all** <description>" — every card the description reaches. Written as a
/// count equal to how many there are, so 1.15.2e leaves no choice at all.
pub fn all_matching(criteria: &[TargetFilter]) -> TargetSpec {
    TargetSpec::Choose {
        count: Quantity::Count(criteria.to_vec()),
        criteria: criteria.to_vec(),
        up_to: false,
    }
}
/// "…if **the exposed card** has the named card type" (Falsified Credentials),
/// "…add it to HQ **if it** has the named subtype" (Wari) — a question about
/// a card this ability already chose. `nth` counts this ability's choices from
/// 0, so the first card it chose is `0`.
pub fn earlier_choice_matches(nth: usize, criteria: &[TargetFilter]) -> TriggerRequirement {
    TriggerRequirement::EarlierTargetMatches { nth, criteria: criteria.to_vec() }
}
/// "…**if this card is uninstalled**, <do this to what it chose>" (DJ Fenris)
/// — a delayed conditional (9.6.13) created by the ability that made the
/// choice, so 1.15.4 lets the later ability act on that card without choosing
/// it again, and 9.10.1 keeps the effect alive after its source has left. A
/// printed conditional would work too (9.1.8g keeps one active for exactly
/// this move), but only a lingering one still knows WHICH card, since the
/// hosting relationship is gone by the time it resolves.
pub fn when_this_card_is_uninstalled(
    label: &'static str,
    instrs: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::CreateDelayedConditional {
        def: Box::new(
            AbilityDef::conditional(
                TriggerCond::SelfUninstalled,
                instrs.into_iter().collect(),
                false,
            )
            .labeled(label),
        ),
        // 9.6.13c: no stated duration — it exists until it first resolves.
        duration: WantedDuration::UntilResolved,
    }
}
/// "…when your turn ends, <do this>" as a DELAYED conditional (9.6.13) — an
/// ability created now that waits for the end of the turn it was created in.
/// A printed conditional ability of the source would fire every turn; this one
/// exists until it first resolves (9.6.13c), which is what a sentence speaking
/// of "that card" needs, since the card is only the one THIS use of the ability
/// dealt with.
pub fn when_your_turn_ends(
    side: Side,
    label: &'static str,
    instrs: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::CreateDelayedConditional {
        def: Box::new(
            AbilityDef::conditional(
                TriggerCond::TurnEnds { side, requires: Vec::new() },
                instrs.into_iter().collect(),
                false,
            )
            .labeled(label),
        ),
        // 9.6.13c: no stated duration — it exists until it first resolves.
        duration: WantedDuration::UntilResolved,
    }
}
/// "…it", "…that card" — a card this ability already chose (1.15.4), acted on
/// again without choosing it a second time.
pub fn earlier_choice(nth: usize) -> TargetSpec {
    TargetSpec::EarlierTarget { nth }
}
/// "…them" — ALL the cards this ability already chose (1.15.4, 1.15.2d: one
/// choice can name several cards).
pub fn earlier_choices() -> TargetSpec {
    TargetSpec::EarlierTargets
}
/// "…you may **choose 2 cards in your heap**." (Steve Cambridge class) — a
/// sentence that only chooses targets and does not act on the choice
/// (9.11.4c). It forms one instruction with the sentence that follows it, so
/// write it as the first half of that sentence's [`combined`]; the later
/// halves act on the chosen cards through [`among_earlier_choices`] and
/// [`the_other_card`]. Resolving it does nothing — the choice was the whole
/// of it.
pub fn choose_cards(n: i64, criteria: &[TargetFilter]) -> Instruction {
    Instruction::ChooseCards { targets: choose(n, criteria) }
}
/// "…the Corp removes **1 of those cards** from the game" (Steve Cambridge
/// class) — "those cards" being the ones this ability already CHOSE (1.15.4),
/// as a description a later choice picks from. [`among_those_cards`] is the
/// same words about the cards an OCCURRENCE named; this is about the cards an
/// announcement did.
pub fn among_earlier_choices() -> TargetFilter {
    TargetFilter::AmongEarlierTargets
}
/// "…then you add **the other card** to your grip." (Steve Cambridge class) —
/// of the cards this instruction chose earlier, the ones its latest choice
/// did NOT name (1.15.4). With nothing left over — the heap held one card, so
/// both choices named it — the position is empty and the rest of the sentence
/// resolves without it (1.15.3).
pub fn the_other_card() -> TargetSpec {
    TargetSpec::EarlierTargetsExceptLatest
}
/// "Expose <a card>." (1.21.4 — revealing an installed, unrezzed card.)
pub fn expose(cards: TargetSpec) -> Instruction {
    Instruction::ExposeCards { cards }
}
/// "…whenever the Runner plays or installs a copy of that card" (Targeted
/// Marketing), "…the Runner plays or installs a card that has the type you
/// last named this way" (Azmari EdTech). ONE trigger condition: the sentence
/// is one, and its "first time each turn" has to be spent by whichever of the
/// two happens first.
pub fn plays_or_installs_named_by(by: Side, key: &'static str) -> TriggerCond {
    TriggerCond::CardPlayed {
        by: Some(by),
        of_types: Vec::new(),
        of_subtypes: Vec::new(),
        criteria: Vec::new(),
        other_than_source: false,
        also_installed: true,
        matching_choice: Some(key),
    }
}
/// "…you and the Corp secretly spend 0[credit], 1[credit], or 2[credit].
///  Reveal spent credits. If you and the Corp spent the same number of
///  credits, <effect>." (Akiko Nisei class.) CR 10.14.6 calls the whole
/// construction ONE instruction — sealed bids, reveal, immediate spend, then
/// the outcome branch — so the three printed sentences that describe it are
/// one call, and only the branch is written out.
pub fn psi_game(
    on_match: impl IntoIterator<Item = Instruction>,
    on_differ: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::PsiGame {
        on_match: on_match.into_iter().collect(),
        on_differ: on_differ.into_iter().collect(),
    }
}
/// "Whenever you encounter a piece of ice, …" (Rielle "Kit" Peddler class) —
/// the encounter with no stipulation about which ice. Pair it with
/// [`CardBuilder::when_first_each_turn`] for the printed "the first time each
/// turn".
pub fn encounters_any_ice() -> TriggerCond {
    TriggerCond::encounter_begins()
}
/// "Whenever you encounter a piece of ice **after an approach during which
/// that ice was rezzed**, …" (Nasir Meidan) — the same encounter with 6.9.2b's
/// rez window as a 9.6.5c requirement listed inside the condition.
pub fn encounters_ice_rezzed_on_its_approach() -> TriggerCond {
    TriggerCond::EncounterBegins {
        of_subtypes: Vec::new(),
        requires: vec![TriggerRequirement::EncounteredIceRezzedDuringApproach],
    }
}
/// "…the rez cost of that ice." (Nasir Meidan) — the printed cost of the ice
/// of the encounter in progress.
pub fn rez_cost_of_the_encountered_ice() -> Quantity {
    Quantity::RezCostOfEncounteredIce
}
/// "…that card's **rez or play cost**", said of the card being accessed
/// (Freedom Khumalo; 1.16.4). Which cost it is is the card type's business —
/// 1.16.4a's rez cost for assets, ice and upgrades, 1.16.4b's play cost for
/// events and operations — and both are the printed corner number. An agenda
/// has neither and reads 0; a printed 0 reads 0 because the cost exists and
/// is zero (1.16.1d pays it by announcing it).
pub fn rez_or_play_cost_of_the_accessed_card() -> Quantity {
    Quantity::RezOrPlayCostOfAccessedCard
}
/// "…gain credits equal to **its** rez cost." (Blue Sun) — the printed cost
/// of a card this ability chose, [`earlier_choice`] asked for a number. `nth`
/// is 0-based over the ability's announcements in order, so a sentence whose
/// other half chose one card reads it at 0.
pub fn rez_cost_of_earlier_choice(nth: usize) -> Quantity {
    Quantity::RezCostOfEarlierTarget { nth }
}
/// "…it gains **code gate** for the remainder of this run." (Rielle "Kit"
/// Peddler class; 2.16.5 counts instances, so a granted subtype coexists with
/// a printed one.)
pub fn gains_subtypes(
    target: TargetSpec,
    add: &[&'static str],
    duration: WantedDuration,
) -> Instruction {
    Instruction::ModifySubtypes {
        target,
        add: add.to_vec(),
        remove: Vec::new(),
        duration,
    }
}
/// "…gains \"[subroutine] …\" **after its other subroutines** for the
/// remainder of <duration>." (Thunderbolt Armaments; CR 9.8.2a puts a granted
/// subroutine after the printed ones unless the card says otherwise, and this
/// card says exactly that.) `before` is the other polarity of the same
/// position, so both are content on one instruction.
pub fn gains_subroutine(
    target: TargetSpec,
    before: bool,
    duration: WantedDuration,
    instrs: impl IntoIterator<Item = Instruction>,
) -> Instruction {
    Instruction::GrantSubroutines {
        to: target,
        grant: jinteki_cr::instr::SubroutineGrant::Stated {
            count: 1,
            sub: Box::new(AbilityDef::subroutine(instrs.into_iter().collect())),
        },
        before,
        any_order: false,
        duration,
    }
}
/// "Swap 2 installed pieces of ice." (Tāo Salonga class; 8.8.1/8.8.2.) Each
/// side of the swap is its own target position, and 8.8.2 filters the second
/// against the first, so the two descriptions are written the same way and
/// the same card can never be chosen twice.
pub fn swap(a: TargetSpec, b: TargetSpec) -> Instruction {
    Instruction::SwapCards { a, b }
}
/// "Whenever you install a **program** from your heap, …" (Exile class; CR
/// 4.8.3 — the set-aside zone is never the zone the sentence names, because
/// 4.8.3 reports where the card was before it was set aside).
pub fn installs_a_from(side: Side, of: CardType, from: Zone) -> TriggerCond {
    TriggerCond::CardInstalledFrom { side, from, of_types: vec![of] }
}
/// "Whenever you forfeit an agenda, …" (Jemison Astronautics class; CR 8.2.5
/// — the agenda leaves the score area for the removed-from-game zone, and
/// 1.15.4 still names it afterwards).
pub fn forfeits_agenda(by: Side) -> TriggerCond {
    TriggerCond::AgendaForfeited { by }
}
/// "…you reveal a card" (Hyoubu Institute class; CR 1.21.3 — shown to all
/// players. `side` is the sentence's "you", the player who reveals it, so a
/// card revealed out of the OPPONENT's hand still meets it.)
pub fn reveals_a_card(by: Side) -> TriggerCond {
    TriggerCond::CardRevealed { by }
}
/// "Reveal N cards from <a hand> at random." (1.21.3 with 1.15.2b's choice
/// taken away from both players — so this is not a description and nothing
/// is announced.)
pub fn reveal_at_random_from_hand_of(side: Side, n: i64) -> Instruction {
    Instruction::RevealRandomFromHand { side, count: Quantity::c(n) }
}
/// "…you gain credits through an ability on <a description>" (The Zwicky
/// Group class; CR 9.1.4 — the credits came through the ability's SOURCE, so
/// the description is about that card). An empty criteria list is the plain
/// "whenever you gain credits", which the basic credit action meets too.
pub fn gains_credits_through(side: Side, criteria: &[TargetFilter]) -> TriggerCond {
    TriggerCond::PlayerGainsCredits { side, criteria: criteria.to_vec() }
}
/// "…you create a remote server" (Near-Earth Hub class; CR 4.6.8d — a remote
/// server exists while a card is in its root or protecting it, so the
/// installation that puts the first card there is what creates it).
pub fn creates_a_remote_server(by: Side) -> TriggerCond {
    TriggerCond::RemoteServerCreated { by }
}
/// "…the agenda point value of <a description>" (1.17.2) — the points
/// PRINTED on those cards, summed, so a card that has left the board still
/// answers.
pub fn agenda_points_of(criteria: &[TargetFilter]) -> Quantity {
    Quantity::AgendaPointsOf(criteria.to_vec())
}
/// "…that card" / "…the forfeited agenda" (1.15.4) as a DESCRIPTION — the
/// card the ability's triggering occurrence named, for the quantity and
/// filter positions where [`the_triggering_card`] (a target) does not fit.
pub fn the_triggering_card_matching() -> TargetFilter {
    TargetFilter::IsTriggeringCard
}
/// "…your heap" (CR 4.3) — the Runner's discard pile, named as a ZONE, for
/// the sentences that stipulate where a card came from rather than describing
/// a card sitting there. [`in_heap`] is the description of the same place.
pub fn the_heap() -> Zone {
    Zone::Discard(Runner)
}
