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
        .faction("Criminal")
        .subtypes(&["Natural"])
        .link(1)
        .text("You draw a starting hand of 9 cards.")
        .starting_hand(9)
        .build()
}

/// Ken "Express" Tenma: Disappeared Clone — Identity: Clone. Link 0.
/// "The first time each turn you play a run event, gain 1[credit]."
///
/// COMPLETE. Not a card of the deck: CR 1.5.4a's pile, which a player brings
/// "along with their deck" and which is what 1.5.4b's "another identity"
/// refers to. This deck plays Rebirth, so it has to bring something for
/// Rebirth to switch to, and 1.5.4b's "from the same faction" makes that a
/// second Criminal.
pub fn ken_tenma() -> Card {
    card("Ken \"Express\" Tenma: Disappeared Clone")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Clone"])
        .text("The first time each turn you play a run event, gain 1[credit].")
        .when_first_each_turn(plays_a_subtyped(Runner, CardType::Event, "Run"), [gain(Runner, 1)])
        .named("a cut of every job")
        .build()
}

/// Sure Gamble — Event. Cost 5.
/// "Gain 9[credit]."
pub fn sure_gamble() -> Card {
    card("Sure Gamble")
        .runner()
        .event()
        .faction("Neutral")
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
        .faction("Shaper")
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
        .faction("Criminal")
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
        .faction("Criminal")
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
/// COMPLETE. "Paying 3[credit] less" is 1.16.6's reduction of the INSTALL
/// cost, which is not 1.16.2f's divisible "total" — nothing is divided, so it
/// needs no rez cost beside it and applies to a plain install. 1.16.2a floors
/// the result at 0, so a resource costing less than 3 is simply free rather
/// than paying the Runner the difference.
pub fn career_fair() -> Card {
    card("Career Fair")
        .runner()
        .event()
        .faction("Criminal")
        .cost(0)
        .text("Install 1 resource from your grip, paying 3[credit] less.")
        .play([install_paying_less(
            choose(1, &[in_hand_of(Runner), of_type(CardType::Resource)]),
            InstallDest::DeclaredByInstaller,
            3,
        )])
        .build()
}

/// Employee Strike — Event: Current. Cost 1.
/// "This event is not trashed until another current is played or an agenda is
///  scored.
///  The Corp's identity loses its printed abilities."
///
/// COMPLETE. Both sentences are declarations, and both are true for exactly
/// as long as the event sits in the play area — which is the whole point of
/// the first one. CR 3.7.1b prints the current EVENT's ending occurrences and
/// 3.5.1b the current OPERATION's, differing in one word, so the pair rides
/// as content on one declaration: "another current operation or event is
/// played" plus, here, the Corp scoring an agenda.
///
/// "Loses its PRINTED abilities" is 9.1.9a with nothing left over: an
/// object's abilities in this kernel are a presence mask over
/// `printed.abilities`, so removing them all removes exactly the printed
/// ones. The description reaches the identity through 1.14.2's controller
/// rather than through an installed-card criterion — an identity is never
/// installed — and therefore leaves the Runner's own identity alone.
pub fn employee_strike() -> Card {
    card("Employee Strike")
        .runner()
        .event()
        .faction("Neutral")
        .subtypes(&["Current"])
        .cost(1)
        .text("This event is not trashed until another current is played or an agenda is scored.")
        .text("The Corp's identity loses its printed abilities.")
        .declares([
            not_trashed_until_an_agenda_is_scored(),
            identity_of_loses_its_abilities(Corp),
        ])
        .build()
}

/// Mutual Favor — Event. Cost 0.
/// "Search your stack for 1 icebreaker and reveal it. (Shuffle your stack
///  after searching it.) If you made a successful run this turn, you may
///  install that program. If you do not, add it to your grip."
///
/// COMPLETE. 8.7.2e makes the search able to fail to find, and 8.7.4's
/// "the cards found by this ability's search" is `TargetSpec::FoundBySearch`,
/// so the reveal and both branches act on whatever the search actually got.
///
/// "You may install that program. If you do not, add it to your grip." is one
/// choice, not a permission followed by a separate sentence: declining the
/// install IS the "if you do not" branch, so both readings of "do not" — no
/// successful run, or a run but no install — put the card in the grip. The
/// requirement rides in the INSTRUCTIONS (9.6.5d), because it is checked when
/// this instruction resolves rather than when the event was played.
pub fn mutual_favor() -> Card {
    card("Mutual Favor")
        .runner()
        .event()
        .faction("Criminal")
        .cost(0)
        .text("Search your stack for 1 icebreaker and reveal it. (Shuffle your stack after searching it.) If you made a successful run this turn, you may install that program. If you do not, add it to your grip.")
        .play([
            search_stack(&[with_subtype("Icebreaker")], 1),
            reveal(TargetSpec::FoundBySearch),
            if_met_else(
                &[made_a_successful_run_this_turn()],
                [choose_one([
                    (
                        "install that program",
                        vec![install(
                            TargetSpec::FoundBySearch,
                            InstallDest::DeclaredByInstaller,
                        )],
                    ),
                    ("add it to your grip", vec![add_to_hand(TargetSpec::FoundBySearch)]),
                ])],
                [add_to_hand(TargetSpec::FoundBySearch)],
            ),
        ])
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
        .faction("Criminal")
        .subtypes(&["Run"])
        .cost(1)
        .text("Run any server. If successful, instead of breaching the attacked server, access 1 card in the root of another server. If that card is an agenda, you cannot steal or trash it during this access.")
        .play([Instruction::InitiateRun {
            server: None,
            allowed: RunServerSet::Any,
            if_successful: vec![Instruction::CreateLingeringEffect {
                payload: LingeringSpec::Replacement {
                    applies_to: EffectClass::Breach,
                    with: ReplacementTransform::SuppressAndResolve(vec![
                        access_one_root_of_another_server_restricted(),
                    ]),
                    optional: false,
                },
                duration: WantedDuration::ThisRun,
            }],
            if_would_be_successful: Vec::new(),
        }])
        .build()
}

/// Rebirth — Event. Cost 0.
/// "Switch your identity with another identity from the same faction. Remove
///  Rebirth from the game instead of trashing it.
///  Limit 1 per deck."
///
/// COMPLETE. "Another identity" is CR 1.5.4a's pile — 1.5.4b: "when an
/// ability refers to an identity other than the Runner's current identity, it
/// refers to the cards provided this way" — so the description names that
/// zone and stipulates a faction, and both are ordinary criteria. The
/// identity Rebirth replaces goes back to the pile, which is the rest of
/// 1.5.4b, and a double-sided one arrives front side faceup (1.5.4d).
///
/// With an empty pile, or none of the right faction, the announcement has no
/// candidate and the switch does nothing — but the event is still played, so
/// the second sentence still removes it from the game.
/// ("Limit 1 per deck" is a deckbuilding restriction, not a sentence a card
/// does.)
pub fn rebirth() -> Card {
    card("Rebirth")
        .runner()
        .event()
        .faction("Neutral")
        .cost(0)
        .text("Switch your identity with another identity from the same faction. Remove Rebirth from the game instead of trashing it.")
        .text("Limit 1 per deck.")
        .play([switch_identity(
            Runner,
            choose(1, &[in_identity_pile_of(Runner), faction_matching_identity_of(Runner, true)]),
        )])
        .declares([removed_from_game_instead_of_trashed()])
        .build()
}

/// Boomerang — Hardware. Install 2. ◆
/// "When you install this hardware, choose 1 installed piece of ice. Use this
///  hardware only during encounters with that ice.
///  [trash]: Break up to 2 subroutines. When this run ends, if it was
///  successful, you may shuffle 1 copy of Boomerang from your heap into your
///  stack."
///
/// COMPLETE. "Choose 1 installed piece of ice" is CR 9.10.3's maintained
/// choice — an ordinary 1.15.2 announcement, remembered by a lingering effect
/// for as long as the hardware is active (9.10.3c) — and the second sentence
/// is a 9.3.3c restriction stated against that choice, not against a subtype:
/// the ability is offered only while the ice being encountered is the one
/// this copy remembers. (The restriction reads "use this HARDWARE", and this
/// hardware has exactly one ability, so restricting the ability is restricting
/// the card.)
///
/// The third sentence has to be a DELAYED conditional (9.6.13): the [trash]
/// cost has already put the card in the heap by the time the ability
/// resolves, so there is no source left to carry a "when this run ends"
/// ability. 9.6.13d is the other half — created outside a run it is never
/// created at all — and the duration is the run, which is exactly what the
/// sentence says.
///
/// "1 copy of Boomerang" is 10.1.5: a card's own name used WITH the word
/// "copy" is not self-reference, so the description reaches any card of that
/// name in the heap — including a different copy, which is the only kind it
/// can ever reach, since this one is the copy that was trashed.
pub fn boomerang() -> Card {
    card("Boomerang")
        .runner()
        .hardware()
        .faction("Criminal")
        .cost(2)
        .unique()
        .text("When you install this hardware, choose 1 installed piece of ice. Use this hardware only during encounters with that ice.")
        .text("[trash]: Break up to 2 subroutines. When this run ends, if it was successful, you may shuffle 1 copy of Boomerang from your heap into your stack.")
        .when(
            installed(),
            [choose_and_remember("boomerang ice", 1, &[of_type(CardType::Ice)])],
        )
        .named("choose the ice")
        .paid_during_encounters_with(
            trash_this_card(),
            "boomerang ice",
            [
                break_up_to(2),
                when_this_run_ends(
                    "boomerang: shuffle a copy back into the stack",
                    true,
                    true,
                    [shuffle_into_deck(
                        choose(1, &[in_heap(), named_card("Boomerang")]),
                        Runner,
                    )],
                ),
            ],
        )
        .named("break up to 2 subroutines")
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
        .faction("Criminal")
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
/// UNIMPLEMENTED: the third sentence. `IcePassed { fully_broken: true }` is
/// the PASS that follows a full break, not the break itself.
pub fn bukhgalter() -> Card {
    card("Bukhgalter")
        .runner()
        .program()
        .faction("Criminal")
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
/// COMPLETE. The first sentence works from the HEAP, and 9.1.8b is what puts
/// it there: the ability states the zone it acts from, so it is active in the
/// heap — the requirement is the statement, and it is also what keeps the
/// same ability from offering an install out of the grip, where the printed
/// words do not reach.
///
/// "If this program can interface with the barrier you are encountering" is
/// deliberately NOT 9.3.6d's interface flag, even though 3.9.5g is exactly
/// the question it asks. The flag is checked when the ability is OFFERED;
/// this sentence is checked when the break instruction resolves (9.6.5d),
/// which is after "+X strength" — and a Paperclip that had to match the
/// barrier's strength BEFORE pumping could never break anything it was not
/// already big enough for.
pub fn paperclip() -> Card {
    card("Paperclip")
        .runner()
        .program()
        .faction("Anarch")
        .subtypes(&["Icebreaker", "Fracter"])
        .cost(4)
        .strength(1)
        .memory(1)
        .text("Whenever you encounter a barrier, you may install this program from your heap.")
        .text("X[credit]: +X strength. Then, if this program can interface with the barrier you are encountering, break up to X subroutines.")
        .may_when(
            encounters_a("Barrier", &[source_in_discard()]),
            [install_this_card()],
        )
        .named("install itself out of the heap")
        .paid(
            credits_x(),
            [
                pump_x(),
                if_met(
                    &[can_interface_with_the_encountered("Barrier")],
                    [break_up_to_x()],
                ),
            ],
        )
        .named("pump and break")
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
        .faction("Criminal")
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
        .faction("Criminal")
        .cost(1)
        .strength(0)
        .memory(1)
        .text("Limit 1 hosted card.")
        .text("Access → 1[credit]: Host the non-agenda card you are accessing faceup on this program. (If it was installed, it becomes uninstalled.)")
        .text("Whenever you breach HQ, if this program has a hosted Corp card, you may pay 1[credit] and trash this program to access 2 additional cards.")
        .declares([can_host(&[], Some(1))])
        .paid_access(credits(1), [host_accessed_on_self()])
        .named("cupellation: pocket the evidence")
        .may_when(
            breaches_server_if(ServerId::Hq, &[source_hosts_corp_card()]),
            [may_pay(
                Cost { credits: Quantity::c(1), trash_self: true, ..Default::default() },
                additional_accesses(2),
            )],
        )
        .named("cupellation: deep dig")
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
        .faction("Neutral")
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
        .faction("Neutral")
        .subtypes(&["Location", "Ritzy"])
        .cost(4)
        .unique()
        .text("When you install this resource, load 3 power counters onto it. When it is empty, trash it.")
        .text("When your turn begins, remove 1 hosted power counter and draw 2 cards.")
        .when(installed(), [load(CounterKind::Power, 3)])
        .when(empty_of(CounterKind::Power), [trash_self()])
        // 9.11.3: "usually, each SENTENCE in the text of an ability forms a
        // single instruction", and 9.11.4's exceptions are about plays,
        // installs, accesses, choices, nested costs, searches and reveals —
        // none of them splits a sentence because its effects are of different
        // classes. "Remove 1 hosted power counter and draw 2 cards" is one
        // sentence, so it is ONE instruction: one checkpoint, one reaction
        // window and one interrupt window cover both halves.
        .when(
            turn_begins(Runner),
            [combined([remove_counters(CounterKind::Power, 1), draw(Runner, 2)])],
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
        .faction("Neutral")
        .subtypes(&["Location", "Seedy"])
        .cost(4)
        .unique()
        .text("When your turn begins, you may remove 1 card in the heap from the game. If you do, gain 2[credit].")
        .may_when(
            TriggerCond::turn_begins(Runner),
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
/// COMPLETE. "When YOUR discard phase ends" names whose, which is the whole
/// difference between this card and The Class Act's "when a discard phase
/// ends" — one stipulation on one condition (§12 rule 2).
pub fn citadel_sanctuary() -> Card {
    card("Citadel Sanctuary")
        .runner()
        .resource()
        .faction("Neutral")
        .subtypes(&["Location"])
        .cost(2)
        .unique()
        .text("When your discard phase ends while you are tagged, the Corp must trace[1]. If unsuccessful, remove 1 tag.")
        .text("[interrupt] → [trash], trash all cards from your grip: Prevent all meat damage.")
        .when(
            your_discard_phase_ends_if(Runner, &[runner_tags_at_least(1)]),
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
        .faction("Shaper")
        .subtypes(&["Connection"])
        .cost(1)
        .text("Film Critic can host a single agenda.")
        .text("Whenever you access an agenda, you may host that agenda on Film Critic (the agenda is no longer being accessed and is uninstalled).")
        .text("[click],[click]: Add an agenda hosted on Film Critic to your score area.")
        .declares([can_host(&[TargetFilter::CardTypeIs(CardType::Agenda)], Some(1))])
        .may_when(
            TriggerCond::RunnerAccessesCard { of_types: vec![CardType::Agenda] },
            [host_accessed_on_self()],
        )
        .named("film critic: above the fray")
        .paid(clicks(2), [Instruction::AddToScoreArea {
            cards: TargetSpec::Choose {
                count: Quantity::c(1),
                criteria: vec![TargetFilter::HostedOnSource],
                up_to: false,
            },
            to: Runner,
            as_agenda: None,
        }])
        .named("film critic: publish the story")
        .build()
}

/// Miss Bones — Resource: Connection. Install 2. ◆
/// "Place 12[credit] from the bank on Miss Bones when she is installed. When
///  there are no credits left on Miss Bones, trash her.
///  Use these credits to trash installed cards."
///
/// COMPLETE. CR 1.10.3a is why the last sentence is not about the counters at
/// all: credits taken from a card ENTER the credit pool, so nothing about a
/// hosted credit differs from any other. 1.10.3c is the sentence — "credits
/// hosted on cards may only be spent as the card's ability allows" — and what
/// this card allows is one class of PAYMENT: one made to trash an installed
/// card. The description is written with the ordinary filter words and no
/// criterion at all, because 1.15.2c already reads that as "the installed
/// cards".
pub fn miss_bones() -> Card {
    card("Miss Bones")
        .runner()
        .resource()
        .faction("Criminal")
        .subtypes(&["Connection"])
        .cost(2)
        .unique()
        .text("Place 12[credit] from the bank on Miss Bones when she is installed. When there are no credits left on Miss Bones, trash her.")
        .text("Use these credits to trash installed cards.")
        .credits_only_for_trashing(&[])
        .when(installed(), [load(CounterKind::Credit, 12)])
        .when(empty_of(CounterKind::Credit), [trash_self()])
        .build()
}

/// The Class Act — Resource: Connection - Ritzy. Install 4. ◆
/// "When a discard phase ends, if you installed this resource this turn, draw
///  4 cards.
///  [interrupt] → The first time each turn you would draw any number of cards,
///  look at the top X cards of your stack. Add 1 of those cards to the bottom
///  of your stack. X is equal to the number of cards you would draw plus 1."
///
/// COMPLETE. The first sentence names no player — "when A discard phase ends"
/// — and the requirement is what scopes it: only a discard phase of the turn
/// the resource was installed in can find "you installed this resource this
/// turn" true.
///
/// The second is an [interrupt] on the draw itself, so "X is equal to the
/// number of cards you would draw plus 1" reads 9.9.6's modifiable value of
/// the instruction the window was opened over — which is why the look is
/// always exactly one card deeper than the draw, whatever the draw was. The
/// look ENDS an instruction (9.11.4e), so the card that goes to the bottom is
/// announced with all X already visible, and 1.12.3 stamps them: a card that
/// left for an unknown location would stop being one of "those cards".
pub fn the_class_act() -> Card {
    card("The Class Act")
        .runner()
        .resource()
        .faction("Criminal")
        .subtypes(&["Connection", "Ritzy"])
        .cost(4)
        .unique()
        .text("When a discard phase ends, if you installed this resource this turn, draw 4 cards.")
        .text("[interrupt] → The first time each turn you would draw any number of cards, look at the top X cards of your stack. Add 1 of those cards to the bottom of your stack. X is equal to the number of cards you would draw plus 1.")
        .when(
            discard_phase_ends_if(&[self_installed_this_turn()]),
            [draw(Runner, 4)],
        )
        .named("the class act: settling in")
        .interrupt_first_each_turn(
            would_draw(Runner),
            [
                look_at(top_of_stack(plus(cards_you_would_draw(), amount(1))), Runner),
                add_to_deck(choose(1, &[looked_at_by_this_ability()]), false),
            ],
        )
        .named("the class act: reading ahead")
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
/// COMPLETE. The first sentence reaches EVERY agenda, wherever it sits, so
/// an agenda still in HQ already has the raised requirement — which is why
/// the reach is carried as scope on one declaration rather than as a second
/// declaration next to SanSan City Grid's server-scoped one.
pub fn the_source() -> Card {
    card("The Source")
        .runner()
        .resource()
        .faction("Neutral")
        .subtypes(&["Connection"])
        .cost(2)
        .unique()
        .text("The advancement requirement of all agendas is increased by 1.")
        .text("As an additional cost to steal an agenda, you must pay 3[credit].")
        .text("Trash The Source when an agenda is scored or stolen.")
        .declares([all_agendas_cost_more(1), additional_cost_to_steal_any_agenda(credits(3))])
        .when(corp_scores_agenda(), [trash_self()])
        .when(runner_steals_agenda(), [trash_self()])
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

/// CR 1.5.4a: the additional identities this deck brings along with it, kept
/// in a pile outside the game. Which identities a player brings is a choice
/// at the table rather than part of the printed deck list — this deck plays
/// Rebirth, so it brings the Criminals that give Rebirth something to switch
/// to.
///
/// 1.5.4a allows "any number", and the point of the identity queue
/// (`docs/vm/IDENTITY-QUEUE.md`) is that one identity is not a choice: every
/// Criminal the card layer carries WHOLE comes to the table, so Rebirth's
/// "another identity from the same faction" names a real decision. An
/// identity joins this list only when it is complete — `cr::readiness()`
/// holds a pile card to the same bar as a deck card.
pub fn additional_identities() -> Vec<Card> {
    let mut out = vec![ken_tenma()];
    out.extend(super::identities::runner_criminal::identities());
    out
}
