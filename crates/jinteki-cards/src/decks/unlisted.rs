//! Cards no priority deck lists.
//!
//! The two priority decks are what the odometer counts, but a wave sometimes
//! has to write a card outside them: a mechanism the CR states as a RULE — not
//! as a card — still has to be proved against a real printed card, and
//! inventing one would be exactly the overfit ARCHITECTURE §12 forbids. CR
//! 1.5.4 names its two cards outright ("Rebirth and DJ Fenris"), and only one
//! of them is in a priority deck.
//!
//! Same discipline as every other module here: printed text in `.text(…)`,
//! one call per printed sentence, `.unimplemented(…)` rather than an
//! approximation. These cards are excluded from `priority_decks()` so the
//! deck odometer keeps measuring the decks, and `jinteki_cards::find` reaches
//! them like any other card.

use jinteki_cr::Subtype;

use crate::edsl::*;

/// DJ Fenris — Resource: Connection. Install 3. ◆
/// "Host a g-mod identity that does not match the faction of your identity on
///  DJ Fenris when he is installed. Remove hosted identity from the game if
///  DJ Fenris is uninstalled.
///  DJ Fenris gains the text of hosted identity.
///  Limit 1 per deck."
///
/// The first half of the first sentence is expressed: "a g-mod identity that
/// does not match the faction of your identity" is CR 1.5.4a's pile (1.5.4b
/// makes that what naming an identity means), a subtype and a faction — three
/// ordinary criteria, none of which this card had to invent — and 1.13.2a's
/// host-without-installing is what "host … on DJ Fenris" does.
///
/// COMPLETE, and the two sentences that were unstatable are the whole of CR
/// 9.1.9 and 9.6.13.
///
/// "DJ Fenris gains the text of hosted identity" is 9.1.9's other direction.
/// An object's abilities are no longer a presence MASK over its printed ones:
/// 9.1.9b says the abilities an object actually has come out of 9.12.1d/e's
/// procedure, so they are a list the characteristics pipeline computes, and
/// `StaticDecl::GainAbilitiesOf` adds the described cards' EFFECTIVE
/// abilities to it. The gaining card is their source (9.1.1b), which is what
/// "this card" means inside a gained ability, and the hosted identity's own
/// copy stays inactive where it sits (1.13.2a/4.6.5h) — so the text applies
/// once, not twice.
///
/// "Remove hosted identity from the game if DJ Fenris is uninstalled" belongs
/// to the same ability as the hosting — one paragraph, one card chosen — so
/// it is a 9.6.13 delayed conditional created when the hosting happens. Two
/// rules make that the only shape that works: 9.10.1 keeps a lingering effect
/// alive after its source has left the play area, and 1.15.4 lets the created
/// ability act on the card the SAME ability already chose, which is the only
/// way to still know which identity once 1.13.13 has severed the hosting.
/// (9.1.8g would keep a printed conditional active for exactly this zone
/// change, but by the time it resolved it would have nothing left to name.)
/// The identity is therefore removed from the game rather than returning to
/// the pile 1.5.4b otherwise sends it to.
///
/// ("Limit 1 per deck" is a deckbuilding restriction, not a sentence a card
/// does.)
pub fn dj_fenris() -> Card {
    card("DJ Fenris")
        .runner()
        .resource()
        .faction("Neutral")
        .subtypes(&[Subtype::Connection])
        .cost(3)
        .unique()
        .text("Host a g-mod identity that does not match the faction of your identity on DJ Fenris when he is installed. Remove hosted identity from the game if DJ Fenris is uninstalled.")
        .text("DJ Fenris gains the text of hosted identity.")
        .text("Limit 1 per deck.")
        .when(
            installed(),
            [
                host(
                    choose(
                        1,
                        &[
                            in_identity_pile_of(Runner),
                            with_subtype(Subtype::GMod),
                            faction_matching_identity_of(Runner, false),
                        ],
                    ),
                    this_card(),
                ),
                when_this_card_is_uninstalled(
                    "dj fenris: the guest leaves the game",
                    [remove_from_game(earlier_choice(0))],
                ),
            ],
        )
        .named("guest of the evening")
        .declares([gains_the_text_of(&[hosted_on_this_card()])])
        .build()
}

/// Chaos Theory: Wünderkind — Identity: G-mod. Link 0.
/// "+1[mu]"
///
/// COMPLETE. Here for CR 1.5.4: a pile (1.5.4a) holding only identities of
/// the Runner's own faction proves nothing about "from the same faction", and
/// DJ Fenris needs a **g-mod** identity of ANOTHER faction to have anything
/// to reach at all. A one-line static is also the identity whose text is
/// easiest to watch arrive on another card.
pub fn chaos_theory() -> Card {
    card("Chaos Theory: Wünderkind")
        .runner()
        .identity()
        .faction("Shaper")
        .subtypes(&[Subtype::GMod])
        .text("+1[mu]")
        .declares([plus_memory(1)])
        .build()
}

// ===========================================================================
// CR 1.15.1b — the cards that NAME something
// ===========================================================================
//
// "Only objects and subroutines are announced as targets. If an instruction
// directs a player to choose (or 'name') a number, a card type, a subtype, a
// card name, a server, or one of a specified set of effects, that choice is
// not made until the instruction resolves."
//
// One rule, eleven cards, and none of them is in a priority deck — which is
// exactly why they are here: the mechanism is stated by the CR, and proving
// it needs real printed cards rather than invented ones (ARCHITECTURE §12).
// (The twelfth, Targeted Marketing, IS in one, and lives in `gauntlet.rs`.)

/// Ark Lockdown — Operation. Cost 1.
/// "Name a card. Remove all copies of that card in the heap from the game."
///
/// COMPLETE. Two printed sentences, two instructions. 2.1.4's "copies of"
/// is a name comparison, 4.4.7b makes the heap open information so nothing
/// has to be revealed to demonstrate the match, and "all" is written as a
/// count equal to how many there are, so 1.15.2e leaves no choice.
pub fn ark_lockdown() -> Card {
    card("Ark Lockdown")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .cost(1)
        .text("Name a card. Remove all copies of that card in the heap from the game.")
        .play([
            name_a_card("ark lockdown target"),
            remove_from_game(all_named_cards_in_discard_of(Runner, "ark lockdown target")),
        ])
        .build()
}

/// Reclamation Order — Operation: Double. Cost 1.
/// "As an additional cost to play this operation, spend [click].
///  Name a card other than Reclamation Order. Reveal any number of copies of
///  the named card from Archives and add them to HQ."
///
/// COMPLETE. "Other than Reclamation Order" is self-referential language:
/// CR 10.1.5 reads a card's own name, used without the word "copy", as "this
/// object", so the exclusion is stated without any name at all. "Reveal … and
/// add them to HQ" is two instructions of different classes in one sentence
/// (9.11.3), the second acting on the first's targets by 1.15.4.
pub fn reclamation_order() -> Card {
    card("Reclamation Order")
        .corp()
        .operation()
        .faction("Haas-Bioroid")
        .subtypes(&[Subtype::Double])
        .cost(1)
        .text("As an additional cost to play this operation, spend [click].")
        .text("Name a card other than Reclamation Order. Reveal any number of copies of the named card from Archives and add them to HQ.")
        .additional_play_cost(clicks(1))
        .play([
            name_a_card_other_than_this_one("reclamation order target"),
            reveal(any_number_of_named_cards_in_discard_of(
                Corp,
                "reclamation order target",
            )),
            add_to_hand(earlier_choices()),
        ])
        .build()
}

/// Salem's Hospitality — Operation: Alliance - Gray Ops. Cost 2.
/// "This operation costs 0 influence if you have 6 or more
///  non-alliance [nbn] cards in your deck.
///  Choose a card name. The Runner reveals the grip and trashes all cards
///  with the chosen name revealed this way."
///
/// COMPLETE as a card that is PLAYED. The alliance line is a deckbuilding
/// restriction on influence (1.4.5), like "Limit 1 per deck" — it changes
/// what may go in a deck, not what happens at the table, and nothing it says
/// is a sentence this card does.
///
/// The rest is 1.15.1b and 1.21.3: the name is said when the play ability
/// resolves, the grip is revealed (which is what lets the Corp see the match
/// it is about to act on, 4.1.2a), and "all cards with the chosen name" is
/// every card in the grip the name reaches.
pub fn salems_hospitality() -> Card {
    card("Salem's Hospitality")
        .corp()
        .operation()
        .faction("NBN")
        .subtypes(&[Subtype::Alliance, Subtype::GrayOps])
        .cost(2)
        .text("This operation costs 0 influence if you have 6 or more non-alliance [nbn] cards in your deck.")
        .text("Choose a card name. The Runner reveals the grip and trashes all cards with the chosen name revealed this way.")
        .play([
            name_a_card("salem's name"),
            reveal(all_matching(&[in_hand_of(Runner)])),
            trash(all_named_cards_in_hand_of(Runner, "salem's name")),
        ])
        .build()
}

/// Azmari EdTech: Shaping the Future — Identity: Division.
/// "When your turn ends, you may name a card type. Gain 2[credit] the first
///  time each turn the Runner plays or installs a card that has the type you
///  last named this way."
///
/// COMPLETE. A card has exactly one type and CR 2.15.2 lists all ten, so
/// "name a card type" is 9.11.4g's choice between options — ten branches,
/// each remembering its own — and not an open namespace.
///
/// The duration is 9.10.3c and not 9.10.3b: 9.10.3b is written for a "when
/// your turn BEGINS" ability with no effects other than making a choice, and
/// this one triggers at the end of the turn, so the choice lasts until the
/// source becomes inactive. An identity in the play area never does, which is
/// what makes "the type you LAST named" a replacement each turn rather than
/// an expiry.
///
/// "Plays or installs" is ONE trigger condition. Written as two abilities the
/// "first time each turn" (9.3.6g) would be two flags, and the identity would
/// pay out twice in a turn the Runner both played and installed.
pub fn azmari_edtech() -> Card {
    card("Azmari EdTech: Shaping the Future")
        .corp()
        .identity()
        .faction("NBN")
        .subtypes(&[Subtype::Division])
        .text("When your turn ends, you may name a card type. Gain 2[credit] the first time each turn the Runner plays or installs a card that has the type you last named this way.")
        .may_when(turn_ends(Corp), [name_a_card_type("azmari type")])
        .when_first_each_turn(
            plays_or_installs_named_by(Runner, "azmari type"),
            [gain(Corp, 2)],
        )
        .build()
}

/// Falsified Credentials — Event. Cost 1.
/// "Name a card type. Expose a card in a remote server, then gain 5[credit]
///  if the exposed card has the named card type."
///
/// COMPLETE. 1.21.4: exposing is revealing, restricted to installed unrezzed
/// cards, so the exposure is what demonstrates the match. "The exposed card"
/// is 1.15.4's back-reference to the target this ability already chose, and
/// the "if" is asked of that card through the same criteria vocabulary.
pub fn falsified_credentials() -> Card {
    card("Falsified Credentials")
        .runner()
        .event()
        .faction("Criminal")
        .cost(1)
        .text("Name a card type. Expose a card in a remote server, then gain 5[credit] if the exposed card has the named card type.")
        .play([
            name_a_card_type("falsified type"),
            expose(choose(1, &[in_a_remote_server()])),
            if_met(
                &[earlier_choice_matches(0, &[named_by("falsified type")])],
                [gain(Runner, 5)],
            ),
        ])
        .build()
}

/// Ibrahim Salem — Asset: Alliance - Character. Rez 2, trash 5. ◆
/// "This card costs 0 influence if you have 6 or more
///  non-alliance [nbn] cards in your deck.
///  As an additional cost to rez Ibrahim Salem, forfeit an agenda.
///  When your turn begins, name a card type. Look at the Runner's grip and
///  trash 1 card in it of the named type."
///
/// COMPLETE as a card that is REZZED and used. The alliance line is a
/// deckbuilding restriction on influence (1.4.5), not a sentence this card
/// does — the same treatment Salem's Hospitality gets.
///
/// The duration is 9.10.3c, not 9.10.3b: 9.10.3b applies to a "when your turn
/// begins" ability with **no effects other than making a choice**, and this
/// one goes on to look and to trash.
pub fn ibrahim_salem() -> Card {
    card("Ibrahim Salem")
        .corp()
        .asset()
        .faction("NBN")
        .subtypes(&[Subtype::Alliance, Subtype::Character])
        .cost(2)
        .trash_cost(5)
        .unique()
        .text("This card costs 0 influence if you have 6 or more non-alliance [nbn] cards in your deck.")
        .text("As an additional cost to rez Ibrahim Salem, forfeit an agenda.")
        .text("When your turn begins, name a card type. Look at the Runner's grip and trash 1 card in it of the named type.")
        .additional_rez_cost(forfeit_agenda(1))
        .when(
            turn_begins(Corp),
            [
                name_a_card_type("salem type"),
                look_at_whole_hand_of(Runner, Corp),
                trash(choose(1, &[in_hand_of(Runner), named_by("salem type")])),
            ],
        )
        .build()
}

/// Wari — Program. Install 1, strength 0. ◆
/// "The first time you make a successful run on HQ each turn, you may trash
///  Wari to name sentry, code gate or
///  barrier. Expose a piece of ice, then add it to HQ if it
///  has the named subtype."
///
/// COMPLETE. Three printed subtypes, three branches (9.11.4g) — the choice
/// between them is an option choice and never an open namespace.
///
/// The duration is stated rather than left to 9.10.3c, and it has to be:
/// the card is TRASHED to make the choice, so "until the source becomes
/// inactive" would expire the choice at the very next checkpoint, before the
/// exposure it exists for. 9.10.3's cases are written for a choice a LATER
/// ability reads; this one is read by the next instruction of the same
/// ability, during the run the trigger condition names, so the run is the
/// duration the card means.
pub fn wari() -> Card {
    card("Wari")
        .runner()
        .program()
        .faction("Criminal")
        .cost(1)
        .strength(0)
        .memory(1)
        .unique()
        .text("The first time you make a successful run on HQ each turn, you may trash Wari to name sentry, code gate or barrier. Expose a piece of ice, then add it to HQ if it has the named subtype.")
        .when_first_each_turn(
            makes_successful_run_on(&[ServerId::Hq]),
            [
                may_pay(
                    trash_this_card(),
                    name_one_of_these_subtypes_for(
                        "wari subtype",
                        &[Subtype::Sentry, Subtype::CodeGate, Subtype::Barrier],
                        WantedDuration::ThisRun,
                    ),
                ),
                expose(choose(1, &[of_type(CardType::Ice)])),
                if_met(
                    &[earlier_choice_matches(0, &[named_by("wari subtype")])],
                    [add_to_hand(earlier_choice(0))],
                ),
            ],
        )
        .build()
}

/// Whistleblower — Resource: Connection. Install 2. ◆
/// "Whenever you make a successful run, you may trash this resource to choose
///  a card name. The next time this run you access an agenda with the chosen
///  name, steal it, ignoring all costs. (You are no longer accessing
///  it.)"
///
/// The first sentence is expressed: 1.15.1b's naming, paid for by trashing
/// the source, with the run as the stated duration for the same reason Wari
/// needs one — the card that would keep the choice alive under 9.10.3c is in
/// the heap by the time the choice is made.
///
/// UNIMPLEMENTED: the second sentence. It needs two things the vocabulary
/// does not have. "The next time this run you access an agenda with the
/// chosen name" is a 9.6.13 delayed conditional that fires ONCE and then
/// expires, and nothing states a one-shot access condition; "steal it,
/// ignoring all costs" is a steal that overrides 1.16.10's additional steal
/// costs, and `Instruction::StealIfAgenda` has no such position. Stating it
/// with what exists would produce a card that steals while an Obokata-class
/// cost stands unpaid, which is worse than a card that says it is partial.
pub fn whistleblower() -> Card {
    card("Whistleblower")
        .runner()
        .resource()
        .faction("Neutral")
        .subtypes(&[Subtype::Connection])
        .cost(2)
        .unique()
        .text("Whenever you make a successful run, you may trash this resource to choose a card name. The next time this run you access an agenda with the chosen name, steal it, ignoring all costs. (You are no longer accessing it.)")
        .when(
            makes_successful_run(),
            [may_pay(
                trash_this_card(),
                name_a_card_for("whistleblower name", WantedDuration::ThisRun),
            )],
        )
        .unimplemented("The next time this run you access an agenda with the chosen name, steal it, ignoring all costs.")
        .build()
}

/// RNG Key — Program. Install 0, strength 0. ◆
/// "The first time you make a successful run on HQ or R&D each turn, you may
///  name a number. If you do, reveal the next card that you access this run.
///  If it has a rez cost, play cost, or advancement requirement equal to the
///  named number, either gain 3[credit] or draw 2 cards."
///
/// The first sentence is expressed: "name a number" is 1.15.1b's other open
/// namespace, and 1.1.3 makes it an integer. The number lasts the run, which
/// is the window the card's own next sentence names.
///
/// UNIMPLEMENTED: the other two sentences. "Reveal the next card that you
/// access this run" is a one-shot delayed conditional on the next access —
/// the same missing shape as Whistleblower's — and "a rez cost, play cost, or
/// advancement requirement equal to the named number" is a comparison of a
/// printed VALUE against a maintained number, which the filter vocabulary
/// cannot say: `MatchesMaintainedChoice` compares characteristics that ARE
/// the named thing, and a number is not one of them.
pub fn rng_key() -> Card {
    card("RNG Key")
        .runner()
        .program()
        .faction("Neutral")
        .cost(0)
        .strength(0)
        .memory(1)
        .unique()
        .text("The first time you make a successful run on HQ or R&D each turn, you may name a number. If you do, reveal the next card that you access this run. If it has a rez cost, play cost, or advancement requirement equal to the named number, either gain 3[credit] or draw 2 cards.")
        .when_first_each_turn(
            makes_successful_run_on(&[ServerId::Hq, ServerId::Rnd]),
            [may(name_a_number("rng key number", WantedDuration::ThisRun))],
        )
        .unimplemented("If you do, reveal the next card that you access this run.")
        .unimplemented("If it has a rez cost, play cost, or advancement requirement equal to the named number, either gain 3[credit] or draw 2 cards.")
        .build()
}

/// Complete Image — Operation: Terminal - Gray Ops. Cost 4, trash 2.
/// "Play only if the Runner has 3 or more agenda points and they made a
///  successful run during their last turn.
///  After you resolve this operation, your action phase ends.
///  Choose a card name, then do 1 net damage. If you trash a card with the
///  chosen name this way, repeat this process."
///
/// The first two sentences are expressed — 9.1.8c's play restriction, over
/// two ordinary state requirements, and 5.6.2b's Terminal — and so is the
/// first half of the third: naming a card and doing 1 net damage.
///
/// UNIMPLEMENTED: "If you trash a card with the chosen name this way, repeat
/// this process." Repeating is not a `ForEach` over a computed quantity: the
/// number of repetitions is not known when the instruction resolves, it
/// depends on what the RANDOM damage trash turned up, and each pass names a
/// NEW card. Nothing in the instruction vocabulary loops on its own result,
/// and 10.1.6a's loop machinery is about a loop that has already been created
/// by abilities resolving each other. Written without it the card names and
/// damages once, which is what it does say — the repetition is the marked
/// sentence.
pub fn complete_image() -> Card {
    card("Complete Image")
        .corp()
        .operation()
        .faction("Jinteki")
        .subtypes(&[Subtype::Terminal, Subtype::GrayOps])
        .cost(4)
        .trash_cost(2)
        .text("Play only if the Runner has 3 or more agenda points and they made a successful run during their last turn.")
        .text("After you resolve this operation, your action phase ends.")
        .text("Choose a card name, then do 1 net damage. If you trash a card with the chosen name this way, repeat this process.")
        .declares([play_only_if(&[
            agenda_points_at_least(Runner, 3),
            runner_made_a_successful_run_last_turn(),
        ])])
        .when(after_this_resolves(), [end_action_phase(Corp)])
        .play([name_a_card("complete image name"), net_damage(Corp, 1)])
        .unimplemented("If you trash a card with the chosen name this way, repeat this process.")
        .build()
}

/// Embezzle — Event: Run - Sabotage. Cost 1.
/// "Run HQ. If successful, instead of breaching HQ, name asset, ice,
///  operation or upgrade, then reveal 2 cards from HQ at random. Trash each
///  revealed card that has the named type, then gain 4[credit] for each card
///  trashed this way."
///
/// The run is expressed. So is the naming it would need: "name asset, ice,
/// operation or upgrade" is four branches of 9.11.4g, no different in kind
/// from Azmari EdTech's ten.
///
/// UNIMPLEMENTED: the replacement itself and the payout, for two reasons that
/// are not about naming. Nothing selects cards from a hand AT RANDOM as
/// targets — `Instruction::TrashRandomFromHand` trashes them without ever
/// naming them, so a later instruction cannot act on the same cards — and no
/// quantity counts "each card trashed this way" (the `CreditsLostThisAbility`
/// shape, for trashes). Stating the replacement without them would suppress
/// the breach and then do nothing, which is strictly worse for the Runner
/// than not playing the card.
pub fn embezzle() -> Card {
    card("Embezzle")
        .runner()
        .event()
        .faction("Criminal")
        .subtypes(&[Subtype::Run, Subtype::Sabotage])
        .cost(1)
        .text("Run HQ. If successful, instead of breaching HQ, name asset, ice, operation or upgrade, then reveal 2 cards from HQ at random. Trash each revealed card that has the named type, then gain 4[credit] for each card trashed this way.")
        .play([run(ServerId::Hq)])
        .unimplemented("If successful, instead of breaching HQ, name asset, ice, operation or upgrade, then reveal 2 cards from HQ at random.")
        .unimplemented("Trash each revealed card that has the named type, then gain 4[credit] for each card trashed this way.")
        .build()
}

// ===========================================================================
// CR 2.1.5 — "cards with different names"
// ===========================================================================
//
// "If a player is directed to choose or search for cards 'with different
// names', each card chosen or found by the search must have a different
// English name from every other card chosen or found."
//
// NOT naming: nothing is remembered and nothing is compared against a value a
// player said. It is a constraint on the SET a choice or a search produces,
// which is why it lives in the criteria vocabulary as the one atom that says
// nothing about any single card.

/// Harmony AR Therapy — Event. Cost 2.
/// "Choose up to 5 cards with different names in your heap. Shuffle those
///  cards into your stack.
///  Remove this event from the game."
///
/// COMPLETE. 2.1.5 applies to the choice, and "up to 5" makes its floor zero
/// (1.15.2e), so a heap of five copies of one card yields exactly one legal
/// pick per name.
///
/// "Remove this event from the game" is written as the trash-destination
/// replacement 8.2.2/9.9.8b describes, not as an instruction: the event is
/// still in the play area while its ability resolves, and step 8.6.7g is what
/// disposes of it afterwards. An instruction that removed it first would
/// leave 8.6.7g trashing a card that is no longer there.
pub fn harmony_ar_therapy() -> Card {
    card("Harmony AR Therapy")
        .runner()
        .event()
        .faction("Shaper")
        .cost(2)
        .text("Choose up to 5 cards with different names in your heap. Shuffle those cards into your stack.")
        .text("Remove this event from the game.")
        .play([shuffle_into_deck(
            choose_up_to(5, &[in_heap(), with_different_names()]),
            Runner,
        )])
        .declares([removed_from_game_instead_of_trashed()])
        .build()
}

/// Asmund Pudlat — Resource: Connection - Seedy. Install 2. ◆
/// "When you install this resource, search your stack for up to 2
///  virus or weapon cards with different
///  names. Host those cards faceup on this resource. (They are not
///  installed.)
///  When your turn begins, you may add 1 hosted card to your grip. If there
///  are no more hosted cards, trash this resource."
///
/// COMPLETE. Three things the printed text says that the vocabulary had to
/// learn: 2.1.5's distinctness applies to a SEARCH as well as to a choice
/// (the rule names both), "virus **or** weapon" is a disjunction inside one
/// criterion where the criteria of a description are otherwise a conjunction,
/// and hosting FACEUP is what makes the two cards open information (1.21.1 /
/// 10.2.2a) though 1.13.2a leaves them uninstalled.
pub fn asmund_pudlat() -> Card {
    card("Asmund Pudlat")
        .runner()
        .resource()
        .faction("Criminal")
        .subtypes(&[Subtype::Connection, Subtype::Seedy])
        .cost(2)
        .unique()
        .text("When you install this resource, search your stack for up to 2 virus or weapon cards with different names. Host those cards faceup on this resource. (They are not installed.)")
        .text("When your turn begins, you may add 1 hosted card to your grip. If there are no more hosted cards, trash this resource.")
        .when(
            installed(),
            [
                search_stack(&[with_any_subtype(&[Subtype::Virus, Subtype::Weapon]), with_different_names()], 2),
                host_faceup(found_by_search(), this_card()),
            ],
        )
        .when(
            turn_begins(Runner),
            [
                may(add_to_hand(choose(1, &[hosted_on_this_card()]))),
                if_met(&[board_has_at_most(&[hosted_on_this_card()], 0)], [trash_self()]),
            ],
        )
        .build()
}

/// Trickster Taka — Resource: Companion - Stealth - Virtual. Install 1. ◆
/// "When your turn begins and whenever you steal an agenda, place 1[credit]
///  on this resource.
///  You can spend hosted credits to use programs during runs.
///  When your turn ends, if there are 3 or more hosted credits, you must
///  take 1 tag or trash this resource."
///
/// COMPLETE. The first sentence is ONE condition met by either occurrence
/// (`either_of`, the Epiphany Analytica shape): no ordinal in front of it, so
/// two abilities would also be a correct reading (Leela Patel's), but the
/// printed sentence is one and §12 rule 2 keeps the sentence's shape.
///
/// The second sentence is 1.10.3c's restriction stated about BOTH a
/// description (9.1.6a's payment for using a described card — the Smoke
/// shape) and a moment (6.1.1's run in progress — the Making News shape);
/// `CreditUse::UsingAbilitiesDuringRuns` is the word that holds the two
/// halves together, and it landed with this card.
///
/// "You must take 1 tag or trash this resource" is 9.11.4g's option choice,
/// put to this card's controller (9.1.1a) with no "may" anywhere: the
/// obligation is the choice BETWEEN harms, never permission to decline both.
/// "If there are 3 or more hosted credits" is 9.6.5c's requirement on the
/// turn-ends condition, read at the occurrence — 5.7.2d — which 5.1.4b makes
/// the SAME moment Citadel Sanctuary's "when your discard phase ends" clock
/// reads. The behaviour tests pin that shared clock from both sides: a tag
/// Taka gives resolves after the moment Citadel's stipulation was read at,
/// so it never feeds a same-turn trace; a tag already there puts both
/// abilities in one reaction window, in whichever order the Runner likes.
pub fn trickster_taka() -> Card {
    card("Trickster Taka")
        .runner()
        .resource()
        .faction("Anarch")
        .subtypes(&[Subtype::Companion, Subtype::Stealth, Subtype::Virtual])
        .cost(1)
        .unique()
        .text("When your turn begins and whenever you steal an agenda, place 1[credit] on this resource.")
        .text("You can spend hosted credits to use programs during runs.")
        .text("When your turn ends, if there are 3 or more hosted credits, you must take 1 tag or trash this resource.")
        .when(
            either_of(&[turn_begins(Runner), runner_steals_agenda()]),
            [place(CounterKind::Credit, 1)],
        )
        .named("a credit either way")
        .credits_only_for_using_during_runs(&[of_type(CardType::Program)])
        .when(
            turn_ends_if(Runner, &[hosted_counters_at_least(CounterKind::Credit, 3)]),
            [choose_one([
                ("take 1 tag", vec![give_tags(1)]),
                ("trash this resource", vec![trash_self()]),
            ])],
        )
        .named("the bill comes due")
        .build()
}

/// Every card here, in the order the file lists it.
pub fn cards() -> Vec<Card> {
    vec![
        dj_fenris(),
        chaos_theory(),
        ark_lockdown(),
        reclamation_order(),
        salems_hospitality(),
        azmari_edtech(),
        falsified_credentials(),
        ibrahim_salem(),
        wari(),
        whistleblower(),
        rng_key(),
        complete_image(),
        embezzle(),
        harmony_ar_therapy(),
        asmund_pudlat(),
        trickster_taka(),
    ]
}
