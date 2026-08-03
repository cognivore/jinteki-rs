//! estrike Regular Andromeda — Criminal.
//!
//! Printed text is copied from NSG's official card data. Behaviour is written
//! from that text and from nowhere else (SYS-D-10): the doc comment above each
//! card carries the text for whoever is reading, `.text(…)` carries the same
//! text as data for whatever is checking, and `tests/decks.rs` asserts the two
//! agree. Sentences the vocabulary cannot say yet carry `.unimplemented(…)`
//! rather than an approximation, and the kernel capability each one waits on
//! is on the gap list in `docs/vm/WAVES.md`.

use crate::edsl::*;

/// Andromeda: Dispossessed Ristie — Identity: Natural. Link 1.
/// "You draw a starting hand of 9 cards."
pub fn andromeda() -> Card {
    card("Andromeda: Dispossessed Ristie")
        .runner()
        .identity()
        .subtypes(&["Natural"])
        .link(1)
        .text("You draw a starting hand of 9 cards.")
        .starting_hand(9)
        .build()
}

/// Sure Gamble — Event. Cost 5.
/// "Gain 9[credit]."
pub fn sure_gamble() -> Card {
    card("Sure Gamble")
        .runner()
        .event()
        .cost(5)
        .text("Gain 9[credit].")
        .play([gain(Runner, 9)])
        .build()
}

/// Diesel — Event. Cost 0.
/// "Draw 3 cards."
pub fn diesel() -> Card {
    card("Diesel")
        .runner()
        .event()
        .cost(0)
        .text("Draw 3 cards.")
        .play([draw(Runner, 3)])
        .build()
}

/// Clean Getaway — Event: Run. Cost 3.
/// "Run any server. If successful, gain 6[credit]."
///
/// "Any server" is the 6.9.1a announcement: the effect names no server, so the
/// Runner declares the attacked one as the run is initiated.
pub fn clean_getaway() -> Card {
    card("Clean Getaway")
        .runner()
        .event()
        .subtypes(&["Run"])
        .cost(3)
        .text("Run any server. If successful, gain 6[credit].")
        .play([run_any_server([gain(Runner, 6)])])
        .build()
}

/// Account Siphon — Event: Run - Sabotage. Cost 0.
/// "Run HQ. If successful, instead of breaching HQ, you may force the Corp to
///  lose up to 5[credit], then you gain 2[credit] for each credit lost and
///  take 2 tags."
///
/// The whole card is machinery that already exists: a run whose initiating
/// effect carries the "if successful" ability (6.7.4), an OPTIONAL breach
/// replacement decided where the breach would have happened (6.7.4c/9.9.2b),
/// a forced loss that takes only what the pool holds (1.10.3b — the "up to"),
/// and a gain calculated from the credits ACTUALLY lost.
pub fn account_siphon() -> Card {
    card("Account Siphon")
        .runner()
        .event()
        .subtypes(&["Run", "Sabotage"])
        .cost(0)
        .text("Run HQ. If successful, instead of breaching HQ, you may force the Corp to lose up to 5[credit], then you gain 2[credit] for each credit lost and take 2 tags.")
        .play([run_then_if_successful(
            ServerId::Hq,
            [instead_of_breaching(
                true,
                [
                    lose(Corp, 5),
                    gain_q(Runner, times(2, per_credit_lost_by(Corp))),
                    give_tags(2),
                ],
            )],
        )])
        .build()
}

/// Career Fair — Event. Cost 0.
/// "Install 1 resource from your grip, paying 3[credit] less."
///
/// UNIMPLEMENTED: `InstallCard::reduce_total` is evaluated only when the
/// effect also rezzes (1.16.2f's "total" needs two costs to divide between),
/// so a plain 1.16.6 install discount has nowhere to land.
pub fn career_fair() -> Card {
    card("Career Fair")
        .runner()
        .event()
        .cost(0)
        .text("Install 1 resource from your grip, paying 3[credit] less.")
        .unimplemented("Install 1 resource from your grip, paying 3[credit] less.")
        .build()
}

/// Employee Strike — Event: Current. Cost 1.
/// "This event is not trashed until another current is played or an agenda is
///  scored.
///  The Corp's identity loses its printed abilities."
///
/// UNIMPLEMENTED: both. `PlayedNotTrashedUntilAgendaSteal` ends on a STEAL,
/// not on a score; and no filter can name one player's identity, since an
/// identity is not installed and every side-scoped atom requires that.
pub fn employee_strike() -> Card {
    card("Employee Strike")
        .runner()
        .event()
        .subtypes(&["Current"])
        .cost(1)
        .text("This event is not trashed until another current is played or an agenda is scored.")
        .text("The Corp's identity loses its printed abilities.")
        .unimplemented("This event is not trashed until another current is played or an agenda is scored.")
        .unimplemented("The Corp's identity loses its printed abilities.")
        .build()
}

/// Mutual Favor — Event. Cost 0.
/// "Search your stack for 1 icebreaker and reveal it. (Shuffle your stack
///  after searching it.) If you made a successful run this turn, you may
///  install that program. If you do not, add it to your grip."
///
/// UNIMPLEMENTED: both sentences. The search itself is sayable, but there is
/// no REVEAL instruction (1.21.3) to finish the first sentence with, and no
/// way to ask whether a successful run has been made this turn.
pub fn mutual_favor() -> Card {
    card("Mutual Favor")
        .runner()
        .event()
        .cost(0)
        .text("Search your stack for 1 icebreaker and reveal it. (Shuffle your stack after searching it.) If you made a successful run this turn, you may install that program. If you do not, add it to your grip.")
        .unimplemented("Search your stack for 1 icebreaker and reveal it.")
        .unimplemented("If you made a successful run this turn, you may install that program. If you do not, add it to your grip.")
        .build()
}

/// Pinhole Threading — Event: Run. Cost 1.
/// "Run any server. If successful, instead of breaching the attacked server,
///  access 1 card in the root of another server. If that card is an agenda,
///  you cannot steal or trash it during this access."
///
/// UNIMPLEMENTED: both sentences. The breach replacement itself is sayable
/// now (Account Siphon uses it), but the run's server is a choice the Runner
/// makes, there is no instruction that accesses a card in the root of a
/// server other than the one being breached, and nothing states a per-access
/// prohibition on stealing or trashing.
pub fn pinhole_threading() -> Card {
    card("Pinhole Threading")
        .runner()
        .event()
        .subtypes(&["Run"])
        .cost(1)
        .text("Run any server. If successful, instead of breaching the attacked server, access 1 card in the root of another server. If that card is an agenda, you cannot steal or trash it during this access.")
        .unimplemented("Run any server. If successful, instead of breaching the attacked server, access 1 card in the root of another server.")
        .unimplemented("If that card is an agenda, you cannot steal or trash it during this access.")
        .build()
}

/// Rebirth — Event. Cost 0.
/// "Switch your identity with another identity from the same faction. Remove
///  Rebirth from the game instead of trashing it.
///  Limit 1 per deck."
///
/// UNIMPLEMENTED: the switch — no instruction exchanges the identity in play
/// for one outside the game. The removal replacement IS sayable, and is what
/// this card carries. ("Limit 1 per deck" is a deckbuilding restriction, not
/// a sentence a card does.)
pub fn rebirth() -> Card {
    card("Rebirth")
        .runner()
        .event()
        .cost(0)
        .text("Switch your identity with another identity from the same faction. Remove Rebirth from the game instead of trashing it.")
        .text("Limit 1 per deck.")
        .declares([removed_from_game_instead_of_trashed()])
        .unimplemented("Switch your identity with another identity from the same faction.")
        .build()
}

/// Boomerang — Hardware. Install 2. ◆
/// "When you install this hardware, choose 1 installed piece of ice. Use this
///  hardware only during encounters with that ice.
///  [trash]: Break up to 2 subroutines. When this run ends, if it was
///  successful, you may shuffle 1 copy of Boomerang from your heap into your
///  stack."
///
/// UNIMPLEMENTED: both. `TimingRestriction` keys on an ice SUBTYPE, so an
/// ability restricted to the ice a maintained choice remembers cannot be
/// stated — and without that restriction the break ability would be usable in
/// every encounter, which is a wrong card rather than a partial one. The
/// shuffle-from-heap-into-stack movement has no instruction either.
pub fn boomerang() -> Card {
    card("Boomerang")
        .runner()
        .hardware()
        .cost(2)
        .unique()
        .text("When you install this hardware, choose 1 installed piece of ice. Use this hardware only during encounters with that ice.")
        .text("[trash]: Break up to 2 subroutines. When this run ends, if it was successful, you may shuffle 1 copy of Boomerang from your heap into your stack.")
        .unimplemented("When you install this hardware, choose 1 installed piece of ice. Use this hardware only during encounters with that ice.")
        .unimplemented("[trash]: Break up to 2 subroutines. When this run ends, if it was successful, you may shuffle 1 copy of Boomerang from your heap into your stack.")
        .build()
}

/// Desperado — Hardware: Console. Install 3. ◆
/// "+1[mu]
///  Gain 1[credit] whenever you make a successful run.
///  Limit 1 console per player."
///
/// (The console limit is a checkpoint rule, driven by `.console()`.)
pub fn desperado() -> Card {
    card("Desperado")
        .runner()
        .hardware()
        .subtypes(&["Console"])
        .cost(3)
        .unique()
        .console()
        .text("+1[mu]")
        .text("Gain 1[credit] whenever you make a successful run.")
        .text("Limit 1 console per player.")
        .declares([plus_memory(1)])
        .when(makes_successful_run(), [gain(Runner, 1)])
        .build()
}

/// Bukhgalter — Program: Icebreaker - Killer. Install 3, strength 1, 1[mu].
/// "Interface → 1[credit]: Break 1 sentry subroutine.
///  1[credit]: +1 strength.
///  The first time each turn this program fully breaks a piece of ice, gain
///  2[credit]."
///
/// UNIMPLEMENTED: the third sentence. `PassedIceAfterFullyBreaking` is the
/// PASS that follows a full break, not the break itself.
pub fn bukhgalter() -> Card {
    card("Bukhgalter")
        .runner()
        .program()
        .subtypes(&["Icebreaker", "Killer"])
        .cost(3)
        .strength(1)
        .memory(1)
        .text("Interface → 1[credit]: Break 1 sentry subroutine.")
        .text("1[credit]: +1 strength.")
        .text("The first time each turn this program fully breaks a piece of ice, gain 2[credit].")
        .paid_interface(credits(1), Some("Sentry"), [break_subroutines(1)])
        .named("interface: break 1 sentry subroutine")
        .paid(credits(1), [pump(1)])
        .named("pump: +1 strength")
        .when_first_each_turn(TriggerCond::SelfFullyBroken, [gain(Runner, 2)])
        .named("bukhgalter: fully broke a piece of ice")
        .build()
}

/// Paperclip — Program: Icebreaker - Fracter. Install 4, strength 1, 1[mu].
/// "Whenever you encounter a barrier, you may install this program from your
///  heap.
///  X[credit]: +X strength. Then, if this program can interface with the
///  barrier you are encountering, break up to X subroutines."
///
/// UNIMPLEMENTED: both. `EncounterBegins` carries no subtype stipulation, and
/// `ModifyStrength::amount` is an `i32` rather than a quantity position, so
/// "+X strength" cannot be stated.
pub fn paperclip() -> Card {
    card("Paperclip")
        .runner()
        .program()
        .subtypes(&["Icebreaker", "Fracter"])
        .cost(4)
        .strength(1)
        .memory(1)
        .text("Whenever you encounter a barrier, you may install this program from your heap.")
        .text("X[credit]: +X strength. Then, if this program can interface with the barrier you are encountering, break up to X subroutines.")
        .unimplemented("Whenever you encounter a barrier, you may install this program from your heap.")
        .unimplemented("X[credit]: +X strength. Then, if this program can interface with the barrier you are encountering, break up to X subroutines.")
        .build()
}

/// Shibboleth — Program: Icebreaker - Decoder. Install 1, strength 3, 1[mu].
/// "Threat 4 → This program gets −2 strength. (This ability is active if any
///  player has 4 or more agenda points.)
///  Interface → 1[credit]: Break 1 code gate subroutine.
///  2[credit]: +2 strength."
///
/// The threat sentence is 9.3.6f: the ability is active only once a player has
/// 4 or more agenda points, which `Vm::char_effects` honours as of W16d.
pub fn shibboleth() -> Card {
    card("Shibboleth")
        .runner()
        .program()
        .subtypes(&["Icebreaker", "Decoder"])
        .cost(1)
        .strength(3)
        .memory(1)
        .text("Threat 4 → This program gets −2 strength. (This ability is active if any player has 4 or more agenda points.)")
        .text("Interface → 1[credit]: Break 1 code gate subroutine.")
        .text("2[credit]: +2 strength.")
        .declares_at_threat(4, [strength_mod(-2)])
        .paid_interface(credits(1), Some("Code Gate"), [break_subroutines(1)])
        .named("interface: break 1 code gate subroutine")
        .paid(credits(2), [pump(2)])
        .named("pump: +2 strength")
        .build()
}

/// Cupellation — Program. Install 1, strength 0, 1[mu].
/// "Limit 1 hosted card.
///  Access → 1[credit]: Host the non-agenda card you are accessing faceup on
///  this program. (If it was installed, it becomes uninstalled.)
///  Whenever you breach HQ, if this program has a hosted Corp card, you may
///  pay 1[credit] and trash this program to access 2 additional cards."
///
/// UNIMPLEMENTED: all three. `TargetSpec::AccessedCard` takes no criteria, so
/// "the non-agenda card you are accessing" cannot be stated; `ThisServerBreached`
/// is for a source in the breached server's root, not a Runner program. And
/// the hosting capacity is left unstated deliberately: `Vm::hosts_onto_itself`
/// derives 1.13.6b by scanning instruction lists, so with the hosting ability
/// missing a bare `CanHost` would turn the capacity into an install PERMISSION
/// — measured, not guessed (`eligible_hosts_for` then offers this card as a
/// host for any program).
pub fn cupellation() -> Card {
    card("Cupellation")
        .runner()
        .program()
        .cost(1)
        .strength(0)
        .memory(1)
        .text("Limit 1 hosted card.")
        .text("Access → 1[credit]: Host the non-agenda card you are accessing faceup on this program. (If it was installed, it becomes uninstalled.)")
        .text("Whenever you breach HQ, if this program has a hosted Corp card, you may pay 1[credit] and trash this program to access 2 additional cards.")
        .unimplemented("Limit 1 hosted card.")
        .unimplemented("Access → 1[credit]: Host the non-agenda card you are accessing faceup on this program.")
        .unimplemented("Whenever you breach HQ, if this program has a hosted Corp card, you may pay 1[credit] and trash this program to access 2 additional cards.")
        .build()
}

/// Daily Casts — Resource. Install 3.
/// "When you install this resource, load 8[credit] onto it. When it is empty,
///  trash it.
///  When your turn begins, take 2[credit] from this resource."
pub fn daily_casts() -> Card {
    card("Daily Casts")
        .runner()
        .resource()
        .cost(3)
        .text("When you install this resource, load 8[credit] onto it. When it is empty, trash it.")
        .text("When your turn begins, take 2[credit] from this resource.")
        .when(installed(), [load(CounterKind::Credit, 8)])
        .when(empty_of(CounterKind::Credit), [trash_self()])
        .when(turn_begins(Runner), [take_hosted_credits(this_card(), 2, Runner)])
        .build()
}

/// Earthrise Hotel — Resource: Location - Ritzy. Install 4. ◆
/// "When you install this resource, load 3 power counters onto it. When it is
///  empty, trash it.
///  When your turn begins, remove 1 hosted power counter and draw 2 cards."
pub fn earthrise_hotel() -> Card {
    card("Earthrise Hotel")
        .runner()
        .resource()
        .subtypes(&["Location", "Ritzy"])
        .cost(4)
        .unique()
        .text("When you install this resource, load 3 power counters onto it. When it is empty, trash it.")
        .text("When your turn begins, remove 1 hosted power counter and draw 2 cards.")
        .when(installed(), [load(CounterKind::Power, 3)])
        .when(empty_of(CounterKind::Power), [trash_self()])
        // 9.11.4a: two effects of DIFFERENT classes in one printed sentence
        // are two instructions resolved in order — `combined(…)` is for the
        // 9.12.2c aggregated case, and the kernel's aggregation only carries
        // atom classes that have a value (see the gap list).
        .when(
            turn_begins(Runner),
            [remove_counters(CounterKind::Power, 1), draw(Runner, 2)],
        )
        .build()
}

/// Bloo Moose — Resource: Location - Seedy. Install 4. ◆
/// "When your turn begins, you may remove 1 card in the heap from the game. If
///  you do, gain 2[credit]."
///
/// UNIMPLEMENTED: `RemoveSelfFromGame` removes the SOURCE; no instruction
/// removes another card from the game.
pub fn bloo_moose() -> Card {
    card("Bloo Moose")
        .runner()
        .resource()
        .subtypes(&["Location", "Seedy"])
        .cost(4)
        .unique()
        .text("When your turn begins, you may remove 1 card in the heap from the game. If you do, gain 2[credit].")
        .may_when(
            TriggerCond::TurnBegins(Runner),
            [remove_from_heap_from_game(1), gain(Runner, 2)],
        )
        .named("bloo moose: cash in a memory")
        .build()
}

/// Citadel Sanctuary — Resource: Location. Install 2. ◆
/// "When your discard phase ends while you are tagged, the Corp must trace[1].
///  If unsuccessful, remove 1 tag.
///  [interrupt] → [trash], trash all cards from your grip: Prevent all meat
///  damage."
///
/// UNIMPLEMENTED: both. There is no "when a discard phase ends" condition
/// (5.5.4), and `Cost::trash_from_hand` is a `u32` rather than a quantity, so
/// "trash ALL cards from your grip" cannot be stated as a cost.
pub fn citadel_sanctuary() -> Card {
    card("Citadel Sanctuary")
        .runner()
        .resource()
        .subtypes(&["Location"])
        .cost(2)
        .unique()
        .text("When your discard phase ends while you are tagged, the Corp must trace[1]. If unsuccessful, remove 1 tag.")
        .text("[interrupt] → [trash], trash all cards from your grip: Prevent all meat damage.")
        .when(
            discard_phase_ends_if(Runner, &[runner_tags_at_least(1)]),
            [performed_by(Corp, trace_if_unsuccessful(1, [performed_by(Runner, remove_tags(1))]))],
        )
        .named("citadel sanctuary: the corp must trace")
        .interrupt_paid(
            trash_self_and_grip(),
            [prevent_all_damage(DamageKind::Meat)],
        )
        .named("citadel sanctuary: burn it all")
        .build()
}

/// Film Critic — Resource: Connection. Install 1.
/// "Film Critic can host a single agenda.
///  Whenever you access an agenda, you may host that agenda on Film Critic
///  (the agenda is no longer being accessed and is uninstalled).
///  [click],[click]: Add an agenda hosted on Film Critic to your score area."
///
/// UNIMPLEMENTED: the first two. `RunnerAccessesCard` carries no card-type
/// stipulation, so "whenever you access an agenda" cannot be said — and with
/// that ability missing, the hosting DECLARATION has to stay unstated too:
/// `Vm::hosts_onto_itself` derives 1.13.6b by scanning for a self-hosting
/// instruction, so a bare `CanHost` here would make Film Critic a legal
/// destination for INSTALLING an agenda. The paid ability IS sayable now
/// (W17a's `AddToScoreArea`), and is inert until the other two land, which is
/// exactly what it should be.
pub fn film_critic() -> Card {
    card("Film Critic")
        .runner()
        .resource()
        .subtypes(&["Connection"])
        .cost(1)
        .text("Film Critic can host a single agenda.")
        .text("Whenever you access an agenda, you may host that agenda on Film Critic (the agenda is no longer being accessed and is uninstalled).")
        .text("[click],[click]: Add an agenda hosted on Film Critic to your score area.")
        .paid(
            clicks(2),
            [add_to_score_area(
                choose(1, &[hosted_on_this_card(), of_type(CardType::Agenda)]),
                Runner,
                None,
            )],
        )
        .unimplemented("Film Critic can host a single agenda.")
        .unimplemented("Whenever you access an agenda, you may host that agenda on Film Critic.")
        .build()
}

/// Miss Bones — Resource: Connection. Install 2. ◆
/// "Place 12[credit] from the bank on Miss Bones when she is installed. When
///  there are no credits left on Miss Bones, trash her.
///  Use these credits to trash installed cards."
///
/// UNIMPLEMENTED: the last sentence. `hosted_credits_spendable` makes hosted
/// credits usable for ANY cost; there is no way to restrict them to a
/// described class of cost.
pub fn miss_bones() -> Card {
    card("Miss Bones")
        .runner()
        .resource()
        .subtypes(&["Connection"])
        .cost(2)
        .unique()
        .text("Place 12[credit] from the bank on Miss Bones when she is installed. When there are no credits left on Miss Bones, trash her.")
        .text("Use these credits to trash installed cards.")
        .when(installed(), [load(CounterKind::Credit, 12)])
        .when(empty_of(CounterKind::Credit), [trash_self()])
        .unimplemented("Use these credits to trash installed cards.")
        .build()
}

/// The Class Act — Resource: Connection - Ritzy. Install 4. ◆
/// "When a discard phase ends, if you installed this resource this turn, draw
///  4 cards.
///  [interrupt] → The first time each turn you would draw any number of cards,
///  look at the top X cards of your stack. Add 1 of those cards to the bottom
///  of your stack. X is equal to the number of cards you would draw plus 1."
///
/// UNIMPLEMENTED: both. No "when a discard phase ends" condition, and
/// `TargetSpec::TopOfDeck` takes a `u32` rather than a quantity, so a count
/// derived from the imminent draw cannot be stated.
pub fn the_class_act() -> Card {
    card("The Class Act")
        .runner()
        .resource()
        .subtypes(&["Connection", "Ritzy"])
        .cost(4)
        .unique()
        .text("When a discard phase ends, if you installed this resource this turn, draw 4 cards.")
        .text("[interrupt] → The first time each turn you would draw any number of cards, look at the top X cards of your stack. Add 1 of those cards to the bottom of your stack. X is equal to the number of cards you would draw plus 1.")
        .unimplemented("When a discard phase ends, if you installed this resource this turn, draw 4 cards.")
        .unimplemented("[interrupt] → The first time each turn you would draw any number of cards, look at the top X cards of your stack. Add 1 of those cards to the bottom of your stack.")
        .build()
}

/// The Source — Resource: Connection. Install 2. ◆
/// "The advancement requirement of all agendas is increased by 1.
///  As an additional cost to steal an agenda, you must pay 3[credit].
///  Trash The Source when an agenda is scored or stolen."
///
/// The third sentence is one printed sentence with two conditions, so it is
/// two conditional abilities with the same effect (9.6.1: a card may have
/// several); whichever occurs first trashes the card, and the other has no
/// source left to act on.
///
/// UNIMPLEMENTED: the first sentence — `ScoreRequirementModInSourceServer` is
/// scoped to the source's server, not to every agenda in the game.
pub fn the_source() -> Card {
    card("The Source")
        .runner()
        .resource()
        .subtypes(&["Connection"])
        .cost(2)
        .unique()
        .text("The advancement requirement of all agendas is increased by 1.")
        .text("As an additional cost to steal an agenda, you must pay 3[credit].")
        .text("Trash The Source when an agenda is scored or stolen.")
        .declares([additional_cost_to_steal_any_agenda(credits(3))])
        .when(corp_scores_agenda(), [trash_self()])
        .when(runner_steals_agenda(), [trash_self()])
        .unimplemented("The advancement requirement of all agendas is increased by 1.")
        .build()
}

/// The whole deck, in the order the file lists it.
pub fn deck() -> Vec<Card> {
    vec![
        andromeda(),
        sure_gamble(),
        diesel(),
        clean_getaway(),
        account_siphon(),
        career_fair(),
        employee_strike(),
        mutual_favor(),
        pinhole_threading(),
        rebirth(),
        boomerang(),
        desperado(),
        bukhgalter(),
        paperclip(),
        shibboleth(),
        cupellation(),
        daily_casts(),
        earthrise_hotel(),
        bloo_moose(),
        citadel_sanctuary(),
        film_critic(),
        miss_bones(),
        the_class_act(),
        the_source(),
    ]
}
