//! Gauntlet — NBN: Nebula Talent Management.
//!
//! Printed text copied from NSG's official card data; behaviour written from
//! that text alone (SYS-D-10). Unsayable sentences carry `.unimplemented(…)`
//! and the kernel capability each waits on is on the gap list in
//! `docs/vm/WAVES.md`.

use crate::edsl::*;

/// Nebula Talent Management: Making Stars — Identity: Division.
/// "When your action phase ends, if you played an operation this turn, gain
///  1[credit] and flip this identity."
///
/// UNIMPLEMENTED: there is no "when your action phase ends" condition, and
/// nothing flips a double-sided identity.
pub fn nebula_talent_management() -> Card {
    card("Nebula Talent Management: Making Stars")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("When your action phase ends, if you played an operation this turn, gain 1[credit] and flip this identity.")
        .when(
            action_phase_ends_if(Corp, &[played_operation_this_turn(Corp)]),
            [gain(Corp, 1), flip_identity(Corp)],
        )
        .named("nebula: gain 1 and flip")
        .flip_face(gemilang_arena())
        .build()
}

/// Gemilang Arena: Burning Bright — Identity: Division; the back face of
/// Nebula Talent Management (oracle: netrunner-cards-json v2, `faces[0]`).
/// "The first time each turn you play an operation, gain [click].
///  When the Runner makes a successful run on HQ or R&D, flip this identity."
pub fn gemilang_arena() -> Card {
    card("Gemilang Arena: Burning Bright")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&["Division"])
        .text("The first time each turn you play an operation, gain [click].")
        .text("When the Runner makes a successful run on HQ or R&D, flip this identity.")
        .when_first_each_turn(plays_a(Corp, CardType::Operation), [gain_clicks(Corp, 1)])
        .named("gemilang: first operation of the turn")
        .when(
            makes_successful_run_on(&[ServerId::Hq, ServerId::Rnd]),
            [flip_identity(Corp)],
        )
        .named("gemilang: flip back")
        .build()
}

/// AstroScript Pilot Program — Agenda: Initiative. 3/2.
/// "When you score this agenda, place 1 agenda counter on it.
///  Hosted agenda counter: Place 1 advancement counter on an installed card
///  you can advance."
///
/// COMPLETE. 1.18.3's permission is read as a criterion by
/// `TargetFilter::CanBeAdvanced`, which derives from the same
/// `Vm::advanceable_cards` the basic advance action uses — so the counter
/// cannot land where the action would refuse to advance. 1.18.2: this places
/// an advancement counter DIRECTLY, which is not advancing, so no "whenever
/// you advance" condition is met.
pub fn astroscript_pilot_program() -> Card {
    card("AstroScript Pilot Program")
        .corp()
        .agenda(3, 2)
        .faction("NBN")
        .subtypes(&["Initiative"])
        .text("When you score this agenda, place 1 agenda counter on it.")
        .text("Hosted agenda counter: Place 1 advancement counter on an installed card you can advance.")
        .when(scored(), [place(CounterKind::Agenda, 1)])
        .paid(
            hosted_counters(CounterKind::Agenda, 1),
            [place_on(choose(1, &[advanceable()]), CounterKind::Advancement, 1)],
        )
        .named("astroscript: spend a counter to advance")
        .build()
}

/// Bellona — Agenda: Expansion. 5/3.
/// "As an additional cost to steal this agenda, the Runner must pay 5[credit].
///  When you score this agenda, gain 5[credit]."
pub fn bellona() -> Card {
    card("Bellona")
        .corp()
        .agenda(5, 3)
        .faction("NBN")
        .subtypes(&["Expansion"])
        .text("As an additional cost to steal this agenda, the Runner must pay 5[credit].")
        .text("When you score this agenda, gain 5[credit].")
        .additional_steal_cost(credits(5))
        .when(scored(), [gain(Corp, 5)])
        .build()
}

/// Breaking News — Agenda. 2/1.
/// "When you score this agenda, give the Runner 2 tags.
///  When a discard phase ends, if you scored this agenda this turn, the Runner
///  removes 2 tags."
///
/// The second sentence names no player — "when A discard phase ends" — so
/// either one meets it. The "this turn" requirement is what keeps the card
/// honest either way: only the discard phase of the turn the agenda was
/// scored in can find the requirement true.
pub fn breaking_news() -> Card {
    card("Breaking News")
        .corp()
        .agenda(2, 1)
        .faction("NBN")
        .text("When you score this agenda, give the Runner 2 tags.")
        .text("When a discard phase ends, if you scored this agenda this turn, the Runner removes 2 tags.")
        .when(scored(), [give_tags(2)])
        .when(
            discard_phase_ends_if(&[self_scored_this_turn()]),
            [performed_by(Runner, remove_tags(2))],
        )
        .named("breaking news: the tags blow over")
        .build()
}

/// Tomorrow's Headline — Agenda: Ambush. 3/2.
/// "When this agenda is scored or stolen, give the Runner 1 tag.
///  Limit 1 per deck."
pub fn tomorrows_headline() -> Card {
    card("Tomorrow's Headline")
        .corp()
        .agenda(3, 2)
        .faction("NBN")
        .subtypes(&["Ambush"])
        .text("When this agenda is scored or stolen, give the Runner 1 tag.")
        .text("Limit 1 per deck.")
        .when(scored(), [give_tags(1)])
        .when(stolen(), [give_tags(1)])
        .build()
}

/// Rashida Jaheem — Asset: Character. Rez 0, trash 1. ◆
/// "When your turn begins, you may trash Rashida Jaheem to gain 3[credit] and
///  draw 3 cards."
pub fn rashida_jaheem() -> Card {
    card("Rashida Jaheem")
        .corp()
        .asset()
        .faction("Neutral")
        .subtypes(&["Character"])
        .cost(0)
        .trash_cost(1)
        .unique()
        .text("When your turn begins, you may trash Rashida Jaheem to gain 3[credit] and draw 3 cards.")
        // 1.16.11a: the nested cost IS the "may" — paying trashes her, and the
        // paid-for branch is the one printed sentence that follows.
        .when(
            turn_begins(Corp),
            [may_pay(trash_this_card(), combined([gain(Corp, 3), draw(Corp, 3)]))],
        )
        .build()
}

/// Jackson Howard — Asset: Executive. Rez 0, trash 3. ◆
/// "[click]: Draw 2 cards.
///  Remove Jackson Howard from the game: Shuffle up to 3 cards from Archives
///  into R&D."
///
/// UNIMPLEMENTED: the second ability. `Cost` has no remove-from-game variant,
/// and no instruction shuffles cards from a discard pile into a deck.
pub fn jackson_howard() -> Card {
    card("Jackson Howard")
        .corp()
        .asset()
        .faction("NBN")
        .subtypes(&["Executive"])
        .cost(0)
        .trash_cost(3)
        .unique()
        .text("[click]: Draw 2 cards.")
        .text("Remove Jackson Howard from the game: Shuffle up to 3 cards from Archives into R&D.")
        .paid(clicks(1), [draw(Corp, 2)])
        .named("jackson: draw 2")
        .paid(remove_self_cost(), [shuffle_from_discard_into_deck(Corp, 3)])
        .named("jackson: shuffle archives into r&d")
        .build()
}

/// Humanoid Resources — Asset. Rez 1, trash 1.
/// "[click][click][click], [trash]: Gain 4[credit] and draw 3 cards. Install
///  up to 2 cards from HQ (one at a time). You may play 1 operation from HQ."
pub fn humanoid_resources() -> Card {
    card("Humanoid Resources")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .cost(1)
        .trash_cost(1)
        .text("[click][click][click], [trash]: Gain 4[credit] and draw 3 cards. Install up to 2 cards from HQ (one at a time). You may play 1 operation from HQ.")
        .paid(
            clicks(3).plus_cost(trash_this_card()),
            [
                combined([gain(Corp, 4), draw(Corp, 3)]),
                install_cards_from_hand(2, Corp, InstallFilter::Any, InstallDest::DeclaredByInstaller),
                // 8.6.3 chooses one at a time and "up to", so the printed
                // "you may" is already the choice this instruction offers.
                play_cards_from_hand(1, Corp),
            ],
        )
        .build()
}

/// Hedge Fund — Operation: Transaction. Cost 5.
/// "Gain 9[credit]."
///
/// NOT part of the printed Gauntlet list (user-verified against the deck
/// photo, 2026-08-03: 49 cards, the 18 operations sum without it). The card
/// definition stays because tests use it; `deck()` does not include it.
pub fn hedge_fund() -> Card {
    card("Hedge Fund")
        .corp()
        .operation()
        .faction("Neutral")
        .subtypes(&["Transaction"])
        .cost(5)
        .text("Gain 9[credit].")
        .play([gain(Corp, 9)])
        .build()
}

/// Archived Memories — Operation. Cost 0.
/// "Add 1 card from Archives to HQ."
pub fn archived_memories() -> Card {
    card("Archived Memories")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .cost(0)
        .text("Add 1 card from Archives to HQ.")
        .play([add_to_hand(choose(1, &[in_archives()]))])
        .build()
}

/// BOOM! — Operation: Double - Black Ops. Cost 4, trash 1.
/// "Play only if the Runner has at least 2 tags.
///  As an additional cost to play this operation, spend [click].
///  Do 7 meat damage."
///
/// UNIMPLEMENTED: the play restriction. `StaticDecl::PlayOnlyIf` now exists,
/// but the requirement vocabulary counts tags only as "tagged" (≥ 1), and
/// this card asks for AT LEAST 2. Using `RunnerTagged` would let it be played
/// at 1 tag — a wrong card, so the marker stays.
pub fn boom() -> Card {
    card("BOOM!")
        .corp()
        .operation()
        .faction("Weyland Consortium")
        .subtypes(&["Double", "Black Ops"])
        .cost(4)
        .trash_cost(1)
        .text("Play only if the Runner has at least 2 tags.")
        .text("As an additional cost to play this operation, spend [click].")
        .text("Do 7 meat damage.")
        .additional_play_cost(clicks(1))
        .declares([play_only_if(&[runner_tags_at_least(2)])])
        .play([meat_damage(Corp, 7)])
        .build()
}

/// Closed Accounts — Operation: Gray Ops. Cost 1.
/// "Play only if the Runner is tagged.
///  The Runner loses all credits in their credit pool."
///
/// UNIMPLEMENTED: the second sentence — `LoseCredits` takes a `u32` rather
/// than a quantity position, so "all credits in their credit pool" has no
/// expression.
pub fn closed_accounts() -> Card {
    card("Closed Accounts")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&["Gray Ops"])
        .cost(1)
        .text("Play only if the Runner is tagged.")
        .text("The Runner loses all credits in their credit pool.")
        .declares([play_only_if(&[runner_is_tagged()])])
        .play([loses_credits(Runner, credits_in_pool_of(Runner))])
        .build()
}

/// Hard-Hitting News — Operation: Terminal. Cost 3.
/// "After you resolve this operation, your action phase ends.
///  Play only if the Runner made a run during their last turn.
///  Trace[4]. If successful, give the Runner 4 tags."
///
/// The play restriction is 9.1.8c: a declaration about WHEN the card may be
/// played, active while the card sits inactive in HQ — the only state in
/// which it could ever matter.
pub fn hard_hitting_news() -> Card {
    card("Hard-Hitting News")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&["Terminal"])
        .cost(3)
        .text("After you resolve this operation, your action phase ends.")
        .text("Play only if the Runner made a run during their last turn.")
        .text("Trace[4]. If successful, give the Runner 4 tags.")
        .declares([play_only_if(&[runner_made_a_run_last_turn()])])
        .play([trace(4, [give_tags(4)])])
        .when(after_this_resolves(), [end_action_phase(Corp)])
        .build()
}

/// Petty Cash — Operation: Transaction. Cost 3.
/// "Play only if you have not finished an action yet this turn.
///  Gain 5[credit]. If you played this operation from anywhere except HQ, gain
///  [click].
///  [click]: Play this operation from Archives. After it resolves, remove it
///  from the game."
///
/// UNIMPLEMENTED: three of the four. No play restriction, no instruction
/// gains a [click] (`GainAllottedClicks` is the turn-structure step), and no
/// ability plays its own source out of a discard pile.
pub fn petty_cash() -> Card {
    card("Petty Cash")
        .corp()
        .operation()
        .faction("Neutral")
        .subtypes(&["Transaction"])
        .cost(3)
        .text("Play only if you have not finished an action yet this turn.")
        .text("Gain 5[credit]. If you played this operation from anywhere except HQ, gain [click].")
        .text("[click]: Play this operation from Archives. After it resolves, remove it from the game.")
        .play([gain(Corp, 5)])
        .unimplemented("Play only if you have not finished an action yet this turn.")
        .unimplemented("If you played this operation from anywhere except HQ, gain [click].")
        .unimplemented("[click]: Play this operation from Archives. After it resolves, remove it from the game.")
        .build()
}

/// Predictive Planogram — Operation: Transaction. Cost 0.
/// "Resolve 1 of the following. If the Runner is tagged, you may resolve both
///  instead.
///  Gain 3[credit].
///  Draw 3 cards."
///
/// COMPLETE. `ChooseOne` offers a fixed list, so the state-dependent extra
/// option is the LIST being chosen by 9.6.5d's requirement rather than an
/// option carrying a condition of its own: tagged, the Corp picks from three
/// options; untagged, from the printed two. "You MAY resolve both" is the
/// third option existing, not a separate permission — declining it is
/// picking one of the other two.
pub fn predictive_planogram() -> Card {
    card("Predictive Planogram")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&["Transaction"])
        .cost(0)
        .text("Resolve 1 of the following. If the Runner is tagged, you may resolve both instead.")
        .text("Gain 3[credit].")
        .text("Draw 3 cards.")
        .play([if_met_else(
            &[runner_is_tagged()],
            [choose_one([
                ("Gain 3[credit].", vec![gain(Corp, 3)]),
                ("Draw 3 cards.", vec![draw(Corp, 3)]),
                ("Resolve both.", vec![gain(Corp, 3), draw(Corp, 3)]),
            ])],
            [choose_one([
                ("Gain 3[credit].", vec![gain(Corp, 3)]),
                ("Draw 3 cards.", vec![draw(Corp, 3)]),
            ])],
        )])
        .build()
}

/// Seamless Launch — Operation. Cost 1.
/// "Place 2 advancement counters on 1 installed card that you did not install
///  this turn."
///
/// "That you did not install this turn" is a game-history criterion (1.12.6),
/// read from the change log since the turn began. 1.18.2: placing an
/// advancement counter is not ADVANCING, so no "whenever you advance"
/// condition is met by it.
pub fn seamless_launch() -> Card {
    card("Seamless Launch")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .cost(1)
        .text("Place 2 advancement counters on 1 installed card that you did not install this turn.")
        .play([place_on(
            choose(1, &[installed_corp_card(), installed_this_turn(false)]),
            CounterKind::Advancement,
            2,
        )])
        .build()
}

/// Self-Growth Program — Operation: Gray Ops. Cost 0.
/// "Play only if the Runner is tagged.
///  Add 2 installed Runner cards to the grip."
pub fn self_growth_program() -> Card {
    card("Self-Growth Program")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&["Gray Ops"])
        .cost(0)
        .text("Play only if the Runner is tagged.")
        .text("Add 2 installed Runner cards to the grip.")
        .declares([play_only_if(&[runner_is_tagged()])])
        .play([add_to_hand(choose(2, &[installed_runner_card()]))])
        .build()
}

/// Subliminal Messaging — Operation: Gray Ops. Cost 0.
/// "Gain 1[credit].
///  The first time each turn you play a copy of Subliminal Messaging, gain
///  [click].
///  When your turn begins, if this card is in Archives and the Runner did not
///  initiate any runs during their last turn, you may reveal this card and add
///  it to HQ."
///
/// UNIMPLEMENTED: the second and third. No instruction gains a [click], and
/// no condition is met by a card sitting in a discard pile (9.1.8b).
pub fn subliminal_messaging() -> Card {
    card("Subliminal Messaging")
        .corp()
        .operation()
        .faction("Neutral")
        .subtypes(&["Gray Ops"])
        .cost(0)
        .text("Gain 1[credit].")
        .text("The first time each turn you play a copy of Subliminal Messaging, gain [click].")
        .text("When your turn begins, if this card is in Archives and the Runner did not initiate any runs during their last turn, you may reveal this card and add it to HQ.")
        .play([gain(Corp, 1)])
        .unimplemented("The first time each turn you play a copy of Subliminal Messaging, gain [click].")
        .unimplemented("When your turn begins, if this card is in Archives and the Runner did not initiate any runs during their last turn, you may reveal this card and add it to HQ.")
        .build()
}

/// Targeted Marketing — Operation: Current. Cost 0.
/// "This card is not trashed until another current is played or an agenda is
///  stolen.
///  Name a card. Gain 10[credit] whenever the Runner plays or installs a copy
///  of that card."
///
/// UNIMPLEMENTED: the second sentence. Nothing names a card, and no condition
/// is met by the Runner playing or installing a copy of a named one.
pub fn targeted_marketing() -> Card {
    card("Targeted Marketing")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&["Current"])
        .cost(0)
        .text("This card is not trashed until another current is played or an agenda is stolen.")
        .text("Name a card. Gain 10[credit] whenever the Runner plays or installs a copy of that card.")
        .declares([not_trashed_until_an_agenda_is_stolen()])
        .unimplemented("Name a card. Gain 10[credit] whenever the Runner plays or installs a copy of that card.")
        .build()
}

/// 24/7 News Cycle — Operation. Cost 0.
/// "As an additional cost to play 24/7 News Cycle, forfeit an agenda.
///  Resolve the \"when scored\" ability on an agenda in your score area."
pub fn news_cycle() -> Card {
    card("24/7 News Cycle")
        .corp()
        .operation()
        .faction("NBN")
        .cost(0)
        .text("As an additional cost to play 24/7 News Cycle, forfeit an agenda.")
        .text("Resolve the \"when scored\" ability on an agenda in your score area.")
        .additional_play_cost(forfeit_agenda(1))
        .play([resolve_when_scored_ability_of(choose(1, &[in_score_area_of(Corp)]))])
        .build()
}

/// Archangel — ICE: Code Gate - Tracer - Ambush. Rez 4, strength 6.
/// "While the Runner is accessing this ice in R&D, they must reveal it.
///  When the Runner accesses this ice anywhere except in Archives, you may pay
///  3[credit]. If you do, they encounter it.
///  [subroutine] Trace[6]. If successful, add 1 installed Runner card to the
///  grip."
///
/// UNIMPLEMENTED: the first two. Nothing states a reveal requirement scoped to
/// a zone, and a trigger condition cannot carry "anywhere except in Archives"
/// — `TriggerRequirement` has one variant and it is about tags.
pub fn archangel() -> Card {
    card("Archangel")
        .corp()
        .ice(6)
        .faction("NBN")
        .subtypes(&["Code Gate", "Tracer", "Ambush"])
        .cost(4)
        .text("While the Runner is accessing this ice in R&D, they must reveal it.")
        .text("When the Runner accesses this ice anywhere except in Archives, you may pay 3[credit]. If you do, they encounter it.")
        .text("[subroutine] Trace[6]. If successful, add 1 installed Runner card to the grip.")
        .when(
            TriggerCond::SelfAccessed { requires: vec![source_in_rnd()] },
            [reveal_self()],
        )
        .named("archangel: they must reveal it")
        .may_when(
            TriggerCond::SelfAccessed { requires: vec![source_not_in_archives()] },
            [may_pay(credits(3), force_encounter_self())],
        )
        .named("archangel: the ambush")
        .subroutine([trace(6, [add_installed_runner_card_to_grip()])])
        .named("archangel: the hook")
        .build()
}

/// Data Raven — ICE: Sentry - Tracer - Observer. Rez 4, strength 4.
/// "When the Runner encounters this ice, they must take 1 tag or end the run.
///  Hosted power counter: Give the Runner 1 tag.
///  [subroutine] Trace[3]. If successful, place 1 power counter on this ice."
pub fn data_raven() -> Card {
    card("Data Raven")
        .corp()
        .ice(4)
        .faction("NBN")
        .subtypes(&["Sentry", "Tracer", "Observer"])
        .cost(4)
        .text("When the Runner encounters this ice, they must take 1 tag or end the run.")
        .text("Hosted power counter: Give the Runner 1 tag.")
        .text("[subroutine] Trace[3]. If successful, place 1 power counter on this ice.")
        .when(
            encountered(),
            [choose_one([
                ("take 1 tag", vec![give_tags(1)]),
                ("end the run", vec![end_the_run()]),
            ])],
        )
        .paid(hosted_counters(CounterKind::Power, 1), [give_tags(1)])
        .subroutine([trace(3, [place(CounterKind::Power, 1)])])
        .build()
}

/// Gold Farmer — ICE: Barrier. Rez 3, strength 1.
/// "Whenever the Runner breaks a printed subroutine on this ice, they lose
///  1[credit].
///  [subroutine] End the run unless the Runner pays 3[credit].
///  [subroutine] End the run unless the Runner pays 3[credit]."
///
/// UNIMPLEMENTED: the first sentence — no condition is met by a subroutine
/// being broken.
pub fn gold_farmer() -> Card {
    card("Gold Farmer")
        .corp()
        .ice(1)
        .faction("NBN")
        .subtypes(&["Barrier"])
        .cost(3)
        .text("Whenever the Runner breaks a printed subroutine on this ice, they lose 1[credit].")
        .text("[subroutine] End the run unless the Runner pays 3[credit].")
        .text("[subroutine] End the run unless the Runner pays 3[credit].")
        .when(printed_subroutine_broken(), [lose(Runner, 1)])
        .named("gold farmer: the toll on breaking")
        .subroutine([unless_pays(Runner, credits(3), end_the_run())])
        .subroutine([unless_pays(Runner, credits(3), end_the_run())])
        .build()
}

/// IP Block — ICE: Barrier - Tracer. Rez 2, strength 4.
/// "When the Runner encounters this ice, give them 1 tag if there is an
///  installed AI program.
///  [subroutine] Trace[3]. If successful, give the Runner 1 tag.
///  [subroutine] End the run if the Runner is tagged."
///
/// COMPLETE. Both conditional sentences are 9.6.5d requirements living in the
/// INSTRUCTIONS rather than in a trigger condition: they are checked when the
/// instruction resolves, so a tag gained by the second subroutine is seen by
/// the third, and an AI installed after the encounter began is not seen by
/// the first. "There is an installed AI program" asks the board through the
/// same criteria a target announcement uses — 1.15.2c restricts it to
/// installed cards, which is exactly what "there is" means here.
pub fn ip_block() -> Card {
    card("IP Block")
        .corp()
        .ice(4)
        .faction("NBN")
        .subtypes(&["Barrier", "Tracer"])
        .cost(2)
        .text("When the Runner encounters this ice, give them 1 tag if there is an installed AI program.")
        .text("[subroutine] Trace[3]. If successful, give the Runner 1 tag.")
        .text("[subroutine] End the run if the Runner is tagged.")
        .when(
            encountered(),
            [if_met(
                &[board_has(&[of_type(CardType::Program), with_subtype("AI")], 1)],
                [give_tags(1)],
            )],
        )
        .named("ip block: the ai tax")
        .subroutine([trace(3, [give_tags(1)])])
        .subroutine([if_met(&[runner_is_tagged()], [end_the_run()])])
        .build()
}

/// Resistor — ICE: Barrier - Tracer. Rez 0, strength 0.
/// "Resistor has +1 strength for each tag the Runner has.
///  [subroutine] Trace[4]. If successful, end the run."
///
/// The strength sentence is the Ice Wall reading (`cards.rs`): a card whose
/// strength is "printed value plus 1 for each X" has its strength DEFINED by
/// that expression, evaluated through the 9.12.1b characteristics pipeline.
/// Printed 0 plus 1 per tag is `Quantity::RunnerTags`.
pub fn resistor() -> Card {
    card("Resistor")
        .corp()
        .ice(0)
        .faction("NBN")
        .subtypes(&["Barrier", "Tracer"])
        .cost(0)
        .text("Resistor has +1 strength for each tag the Runner has.")
        .text("[subroutine] Trace[4]. If successful, end the run.")
        .declares([strength_is(plus(amount(0), times(1, per_runner_tag())))])
        .subroutine([trace(4, [end_the_run()])])
        .build()
}

/// Slot Machine — ICE: Code Gate. Rez 3, strength 5.
/// "When the Runner encounters this ice, they put the top card of the stack on
///  the bottom, then you reveal the top 3 cards of the stack.
///  [subroutine] The Runner loses 3[credit].
///  [subroutine] If you revealed 2 or more cards that share a type when this
///  encounter began, gain 3[credit].
///  [subroutine] If you revealed 3 or more cards that share a type when this
///  encounter began, place 3 advancement tokens on an installed card."
///
/// UNIMPLEMENTED: everything but the first subroutine. There is no REVEAL
/// instruction, nothing remembers what was revealed when an encounter began,
/// and no quantity counts cards sharing a type.
pub fn slot_machine() -> Card {
    card("Slot Machine")
        .corp()
        .ice(5)
        .faction("NBN")
        .subtypes(&["Code Gate"])
        .cost(3)
        .text("When the Runner encounters this ice, they put the top card of the stack on the bottom, then you reveal the top 3 cards of the stack.")
        .text("[subroutine] The Runner loses 3[credit].")
        .text("[subroutine] If you revealed 2 or more cards that share a type when this encounter began, gain 3[credit].")
        .text("[subroutine] If you revealed 3 or more cards that share a type when this encounter began, place 3 advancement tokens on an installed card.")
        .subroutine([lose(Runner, 3)])
        .unimplemented("When the Runner encounters this ice, they put the top card of the stack on the bottom, then you reveal the top 3 cards of the stack.")
        .unimplemented("[subroutine] If you revealed 2 or more cards that share a type when this encounter began, gain 3[credit].")
        .unimplemented("[subroutine] If you revealed 3 or more cards that share a type when this encounter began, place 3 advancement tokens on an installed card.")
        .build()
}

/// Crisium Grid — Upgrade: Region. Rez 3, trash 5.
/// "Runs against this server cannot be declared successful. (This effect does
///  not cause runs to become unsuccessful.)
///  Limit 1 region per server."
pub fn crisium_grid() -> Card {
    card("Crisium Grid")
        .corp()
        .upgrade()
        .faction("Weyland Consortium")
        .subtypes(&["Region"])
        .cost(3)
        .trash_cost(5)
        .text("Runs against this server cannot be declared successful. (This effect does not cause runs to become unsuccessful.)")
        .text("Limit 1 region per server.")
        .declares([runs_not_declared_successful()])
        .build()
}

/// The whole deck, in the order the file lists it.
pub fn deck() -> Vec<Card> {
    vec![
        nebula_talent_management(),
        astroscript_pilot_program(),
        bellona(),
        breaking_news(),
        tomorrows_headline(),
        rashida_jaheem(),
        jackson_howard(),
        humanoid_resources(),
        archived_memories(),
        boom(),
        closed_accounts(),
        hard_hitting_news(),
        petty_cash(),
        predictive_planogram(),
        seamless_launch(),
        self_growth_program(),
        subliminal_messaging(),
        targeted_marketing(),
        news_cycle(),
        archangel(),
        data_raven(),
        gold_farmer(),
        ip_block(),
        resistor(),
        slot_machine(),
        crisium_grid(),
    ]
}

/// CR 1.5.4a: the pile is the RUNNER's — "a player may bring any number of
/// additional **Runner** identity cards along with their deck" — so a Corp
/// deck brings none.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
