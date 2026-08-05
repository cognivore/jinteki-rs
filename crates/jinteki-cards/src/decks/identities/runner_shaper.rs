//! Runner — Shaper identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). Chaos Theory is the one Shaper that already exists
//! — it lives in `decks/gauntlet.rs`, because that deck plays it — and
//! everything else in the faction lands here.

use crate::edsl::*;

/// One installed piece of ice — the description Tāo Salonga's sentence makes
/// twice, written once here. 8.8.2 filters the second announcement against
/// the first, so the same description on both sides can never choose the same
/// card twice.
fn an_installed_piece_of_ice() -> TargetSpec {
    choose(1, &[installed_corp_card(), of_type(CardType::Ice)])
}

/// Akiko Nisei: Head Case — Identity: Clone. Link 1.
/// "Whenever you breach R&D, you and the Corp secretly spend 0[credit],
///  1[credit], or 2[credit]. Reveal spent credits. If you and the Corp spent
///  the same number of credits, access 1 additional card."
///
/// COMPLETE. Three printed sentences and ONE instruction: CR 10.14.6 defines
/// a Psi Game as a single construction — sealed bids (10.14.6b), reveal and
/// immediate spend (10.14.6c), then the outcome branch (10.14.6d) — so the
/// sentences that describe it do not each become an instruction. The legal
/// bids are 10.14.3's 0, 1 and 2, which is why the card can print them and
/// the kernel need not.
///
/// "Access 1 additional card" is 7.3.5's random-access limit for the breach
/// in progress, raised from inside it: the condition is met when the breach
/// BEGINS (7.3.2), so the ability resolves before the limit is counted out.
pub fn akiko_nisei() -> Card {
    card("Akiko Nisei: Head Case")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Clone"])
        .link(1)
        .text("Whenever you breach R&D, you and the Corp secretly spend 0[credit], 1[credit], or 2[credit]. Reveal spent credits. If you and the Corp spent the same number of credits, access 1 additional card.")
        .when(breaches_server_if(ServerId::Rnd, &[]), [psi_game([additional_accesses(1)], [])])
        .named("head case")
        .build()
}

/// Exile: Streethawk — Identity: Natural. Link 1.
/// "Whenever you install a program from your heap, draw 1 card."
///
/// COMPLETE. The sentence makes two stipulations about one occurrence — the
/// zone the card came from and its type — and both ride on the install
/// condition as content. CR 4.8.3 decides the first: a card set aside on its
/// way to being installed is reported as coming from the location it was in
/// BEFORE it was set aside, so a Test Run-class search that pulls a program
/// out of the heap meets this condition even though the program spent a
/// moment in the set-aside zone.
pub fn exile() -> Card {
    card("Exile: Streethawk")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .link(1)
        .text("Whenever you install a program from your heap, draw 1 card.")
        .when(installs_a_from(Runner, CardType::Program, the_heap()), [draw(Runner, 1)])
        .named("streethawk")
        .build()
}

/// Hayley Kaplan: Universal Scholar — Identity: Natural. Link 0.
/// "The first time you install a card each turn, you may install another card
///  of the same type from your grip (paying its install cost)."
///
/// COMPLETE. 8.5's install with no stipulation about what was installed, and
/// 9.6.5c's ordinal about the occurrence.
///
/// "Of the same type" is 1.15.4's back-reference: the type is read off the
/// card the OCCURRENCE named, and 2.15 gives a card exactly one type, so this
/// is an equality rather than a list. "Another" needs no word of its own —
/// the card that met the condition has left the grip, and the description
/// names the grip.
///
/// The parenthesis is 1.4's reminder text: 8.5.11 already makes an install
/// pay its cost unless the sentence says otherwise, so it is not a second
/// instruction. 9.11.4b is why the install is its own instruction and not
/// something the condition's ability could absorb.
pub fn hayley_kaplan() -> Card {
    card("Hayley Kaplan: Universal Scholar")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .text("The first time you install a card each turn, you may install another card of the same type from your grip (paying its install cost).")
        .may_when_first_each_turn(
            installs_a_card(Runner),
            [install(
                choose(1, &[in_hand_of(Runner), of_the_same_type_as_the_triggering_card()]),
                InstallDest::RunnerChoiceHostOrRig,
            )],
        )
        .named("universal scholar")
        .build()
}

/// Rielle "Kit" Peddler: Transhuman — Identity: Cyborg. Link 0.
/// "The first time each turn you encounter a piece of ice, it gains code gate
///  for the remainder of this run."
///
/// COMPLETE. The condition is the encounter with no stipulation about which
/// ice, and the printed ordinal is 9.6.5c's stipulation about the occurrence,
/// exactly as it is on Gabriel Santiago.
///
/// "It gains code gate" ADDS a subtype rather than replacing what is there:
/// 2.16.5 counts instances, so a barrier the ability names is a barrier AND a
/// code gate, and an icebreaker that can interface with either one may do so.
/// "For the remainder of this run" is the duration the sentence names, so the
/// grant survives the encounter it was made in and dies with the run.
pub fn rielle_kit_peddler() -> Card {
    card("Rielle \"Kit\" Peddler: Transhuman")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Cyborg"])
        .text("The first time each turn you encounter a piece of ice, it gains code gate for the remainder of this run.")
        .when_first_each_turn(
            encounters_any_ice(),
            [gains_subtypes(encountered_ice(), &["Code Gate"], WantedDuration::ThisRun)],
        )
        .named("transhuman")
        .build()
}

/// The Professor: Keeper of Knowledge — Identity: Natural. Link 0.
/// "The first copy of each program in this deck does not count against your
///  influence limit."
///
/// COMPLETE. A DECK-CONSTRUCTION rule and nothing else: CR 1.4.5 counts
/// influence by copy when a deck is built, and 1.4.2 settles legality before
/// the game begins. There is no condition to meet, nothing to resolve and no
/// game state it reads — the same class of sentence as Ampère's singleton
/// rule and Custom Biotics' faction ban, and the writing guide's third rule
/// of thumb puts all of them in the facts or nowhere.
pub fn the_professor() -> Card {
    card("The Professor: Keeper of Knowledge")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .text("The first copy of each program in this deck does not count against your influence limit.")
        .build()
}

/// Tāo Salonga: Telepresence Magician — Identity: Natural. Link 0.
/// "Whenever an agenda is scored or stolen, you may swap 2 installed pieces
///  of ice."
///
/// COMPLETE. One printed sentence with two conditions, so it is two
/// conditional abilities with the same effect — the shape Leela Patel already
/// takes, and for the same reason: 9.6.1 gives an ability ONE primary
/// condition, and an agenda being scored (1.17.3a) and one being stolen
/// (1.17.3b) are different occurrences.
///
/// The swap is 8.8.1's: both cards are named as targets and exchange
/// locations simultaneously, and 8.8.2 restricts the pair to cards that may
/// each occupy the other's location — which for two installed pieces of ice
/// is always true, and which is also what keeps one piece of ice from being
/// chosen as both halves.
///
/// The printed "you may" is the whole ability, so it is 9.6.9's declinable
/// conditional, and 9.1.1a gives the decision — and both announcements — to
/// the Runner even on the half the CORP's score meets.
pub fn tao_salonga() -> Card {
    card("Tāo Salonga: Telepresence Magician")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .text("Whenever an agenda is scored or stolen, you may swap 2 installed pieces of ice.")
        .may_when(
            corp_scores_agenda(),
            [swap(an_installed_piece_of_ice(), an_installed_piece_of_ice())],
        )
        .named("an agenda was scored")
        .may_when(
            runner_steals_agenda(),
            [swap(an_installed_piece_of_ice(), an_installed_piece_of_ice())],
        )
        .named("an agenda was stolen")
        .build()
}

/// Captain Padma Isbister: Intrepid Explorer — Identity: Cyborg. Link 0.
/// "The first time each turn a run on R&D begins, you may charge 1 of your
///  installed cards. (Add 1 power counter to a card that already has one.)"
///
/// COMPLETE. The parenthesis is 1.4's reminder text — it restates what the
/// charge keyword already is, so it is not a second instruction — and it is
/// also where the description comes from: charging places a power counter on
/// a card that ALREADY has one, so "a card that already has one" is a
/// criterion about the counters the card hosts and not a second effect.
///
/// "A run on R&D begins" is 6.9.1's Run Initiation Phase, whose first step
/// announces the attacked server — so the server is known when the condition
/// is met. It is deliberately not the approach of R&D: 6.9.2 approaches a
/// server only after every piece of ice protecting it has been passed, which
/// is later and may not happen at all.
///
/// A run the Runner did not make still meets it: the sentence says "a run",
/// not "you make a run".
pub fn captain_padma_isbister() -> Card {
    card("Captain Padma Isbister: Intrepid Explorer")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Cyborg"])
        .text("The first time each turn a run on R&D begins, you may charge 1 of your installed cards. (Add 1 power counter to a card that already has one.)")
        .may_when_first_each_turn(
            run_begins_on(&[ServerId::Rnd]),
            [place_on(
                choose(
                    1,
                    &[installed_runner_card(), with_counters(CounterKind::Power, 1)],
                ),
                CounterKind::Power,
                1,
            )],
        )
        .named("intrepid explorer")
        .build()
}

/// Hiram "0mission" Svensson: Shadow of the Past — Identity: Natural. Link 0.
/// "Whenever you install or trash a piece of hardware (from any location),
///  look at the top card of R&D."
///
/// COMPLETE. One printed sentence with two conditions, so it is two
/// conditional abilities with the same effect — the Leela Patel shape, and
/// for the same reason: 9.6.1 gives an ability ONE primary condition, and
/// 8.5.1's install and 8.2's trash are different occurrences. Nothing is lost
/// by the split here, because the sentence states no ordinal for the pair to
/// share.
///
/// The parenthesis is not reminder text — it is a stipulation, and the one
/// this card exists to make. A trash condition is otherwise read as the
/// installed cards (1.15.2c's default read from the other side), so "(from
/// any location)" is the card saying that a piece of hardware trashed out of
/// the grip or the heap counts as much as one trashed off the rig.
///
/// "You install or trash" names the player doing it (1.14.5), which is what
/// leaves a piece of hardware the CORP trashes alone.
pub fn hiram_svensson() -> Card {
    card("Hiram \"0mission\" Svensson: Shadow of the Past")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .text("Whenever you install or trash a piece of hardware (from any location), look at the top card of R&D.")
        .when(
            installs_a(Runner, CardType::Hardware),
            [look_at(top_of_rnd(amount(1)), Runner)],
        )
        .named("a piece of hardware was installed")
        .when(
            trashes_a_from_anywhere(Runner, CardType::Hardware),
            [look_at(top_of_rnd(amount(1)), Runner)],
        )
        .named("a piece of hardware was trashed")
        .build()
}

/// The Collective: Williams, Wu, et al. — Identity: Cybernetic. Link 1.
/// "The first time you perform the same action three times in a row each
///  turn, gain [click]."
///
/// COMPLETE. 5.2.5a/b decide what "the same action" is — the same basic
/// action, or the same ability of the same card, so two different cards
/// printing the same words are still two actions — and the kernel records
/// that identity with every action taken.
///
/// "Three times in a row" reads the LAST three actions of the turn rather
/// than all of them: an action of another kind in between breaks the run and
/// the count starts again. That is what distinguishes it from MirrorMorph's
/// "each different from one another", which asks about every action of the
/// turn at once.
///
/// "The first time … each turn" is 9.6.5c's ordinal about the occurrence, so
/// a fourth identical action in the same turn does not meet the condition at
/// all — the ability is not throttled, it simply never triggers again.
pub fn the_collective() -> Card {
    card("The Collective: Williams, Wu, et al.")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Cybernetic"])
        .link(1)
        .text("The first time you perform the same action three times in a row each turn, gain [click].")
        .when_first_each_turn(same_action_in_a_row(Runner, 3), [gain_clicks(Runner, 1)])
        .named("williams, wu, et al.")
        .build()
}

/// Kate "Mac" McCaffrey: Digital Tinker — Identity: Natural. Link 1.
/// "Lower the install cost of the first program or piece of hardware you
///  install each turn by 1."
///
/// COMPLETE. The same declaration Az McCaffrey states about a different
/// description, written in the other order: 9.3.5 applies it continuously,
/// 8.7.2b's affordability query reads it, and it costs nothing and asks
/// nothing — which is what separates it from Patchwork's 1.16.6 reduction.
///
/// "Program or piece of hardware" is 2.15's type alone, twice, so it is the
/// "or" between single words of one kind rather than between descriptions.
/// "The first … each turn" is 9.6.5c's ordinal read of the install, and
/// 1.16.2a floors the lowered cost at 0.
pub fn kate_mac_mccaffrey() -> Card {
    card("Kate \"Mac\" McCaffrey: Digital Tinker")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .link(1)
        .text("Lower the install cost of the first program or piece of hardware you install each turn by 1.")
        .declares([first_installed_each_turn_costs_less(
            &[of_any_type(&[CardType::Program, CardType::Hardware])],
            1,
        )])
        .build()
}

/// Nasir Meidan: Cyber Explorer — Identity: Cyborg. Link 1.
/// "Whenever you encounter a piece of ice after an approach during which that
///  ice was rezzed, lose all credits in your credit pool. Gain credits equal
///  to the rez cost of that ice."
///
/// COMPLETE. TWO printed sentences on one condition, so two instructions —
/// and here the 9.11.3 boundary between them is the card: the loss finishes,
/// a checkpoint occurs, and only then is the gain imminent, which is what
/// makes the Runner momentarily broke rather than simply swapping one number
/// for another.
///
/// "After an approach during which that ice was rezzed" is 9.6.5c's
/// additional requirement listed inside the condition, asked of 6.9.2's
/// Approach Ice Phase the encounter directly follows. An ice already faceup
/// when it was approached does not meet it, and neither does one rezzed on an
/// earlier approach of the same run — the sentence names the approach, not
/// the ice's state.
///
/// "All credits in your credit pool" is 1.10.1's pool and nothing else: a
/// credit hosted on a card is not in it (1.13.3) and is not lost.
///
/// "The rez cost of that ice" is 1.16.4a's inherent cost, printed on the
/// card. It is not what the Corp paid: an ice rezzed for free through an
/// ability still has a rez cost, and the sentence still names it.
pub fn nasir_meidan() -> Card {
    card("Nasir Meidan: Cyber Explorer")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Cyborg"])
        .link(1)
        .text("Whenever you encounter a piece of ice after an approach during which that ice was rezzed, lose all credits in your credit pool. Gain credits equal to the rez cost of that ice.")
        .when(
            encounters_ice_rezzed_on_its_approach(),
            [
                loses_credits(Runner, credits_in_pool_of(Runner)),
                gain_q(Runner, rez_cost_of_the_encountered_ice()),
            ],
        )
        .named("cyber explorer")
        .build()
}

/// Jamie "Bzzz" Micken: Techno Savant — Identity: Natural. Link 0.
/// "Draft format only.
///  If you have more [shaper] cards installed than any other faction, when you
///  install a card the first time each turn, draw 1 card."
///
/// COMPLETE. The format restriction, then one conditional ability carrying
/// BOTH of 9.6.5c's stipulations about its condition at once: the leading
/// "if" is the additional requirement, and "the first time each turn" is the
/// ordinal about the occurrence. They are asked together, when the install
/// happens — a Runner who is behind on the faction count when they install
/// their first card of the turn does not bank the ordinal for later, because
/// the condition was never met at all.
///
/// The install condition makes no stipulation about the card, because the
/// sentence makes none.
pub fn jamie_bzzz_micken() -> Card {
    card("Jamie \"Bzzz\" Micken: Techno Savant")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&["Natural"])
        .text("Draft format only.")
        .text("If you have more [shaper] cards installed than any other faction, when you install a card the first time each turn, draw 1 card.")
        .when_first_each_turn(
            installs_a_card_if(
                Runner,
                &[more_cards_of_this_faction_than_any_other(
                    "Shaper",
                    &[installed_runner_card()],
                )],
            ),
            [draw(Runner, 1)],
        )
        .named("techno savant")
        .build()
}

/// Every Shaper identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        jamie_bzzz_micken(),
        akiko_nisei(),
        kate_mac_mccaffrey(),
        nasir_meidan(),
        exile(),
        hayley_kaplan(),
        rielle_kit_peddler(),
        the_professor(),
        tao_salonga(),
        captain_padma_isbister(),
        hiram_svensson(),
        the_collective(),
    ]
}
