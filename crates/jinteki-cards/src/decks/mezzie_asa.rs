//! Mezzie's Asa — Asa Group: Security Through Vigilance.
//!
//! Printed text is copied from NSG's official card data. Behaviour is written
//! from that text and from nowhere else (SYS-D-10): the doc comment above each
//! card carries the text for whoever is reading, `.text(…)` carries the same
//! text as data for whatever is checking, and `tests/decks.rs` asserts the two
//! agree. Sentences the vocabulary cannot say yet carry `.unimplemented(…)`
//! rather than an approximation, and the kernel capability each one waits on
//! is on the Blockers list in `docs/vm/MEZZIE-QUEUE.md`.
//!
//! The deck is written in the queue's printed order and fills in as waves
//! land: a card the deck lists and nobody has written yet is simply absent
//! from [`deck`], and a card an earlier deck already carries is reused from
//! there rather than copied.

use crate::edsl::*;

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// Estelle Moon — Asset: Executive. Rez 2, trash 3. ◆
/// "Whenever you install a card in the root of a remote server, place 1 power
///  counter on this asset.
///  [trash]: For each power counter on this asset, gain
///  2[credit] and draw 1 card."
///
/// COMPLETE. Two printed sentences: a conditional ability that counts, and a
/// paid ability that spends what was counted.
///
/// The count is the card. "For each power counter on this asset" is 9.12.2's
/// calculated quantity, read when the ability RESOLVES and not when it was
/// used — and 9.12.2c is what settles the shape the UFAQ was asked about
/// (three counters: six credits and THREE cards, not six credits and one):
/// a calculated quantity aggregates into a single effect, so this is one gain
/// of 2×N and one draw of N, in one instruction, because 9.11.3 makes "gain
/// 2[credit] and draw 1 card" one sentence and `combined(…)` is how a
/// sentence with two effects is written.
///
/// 9.5.5 is what keeps the number from being zero. The [trash] trigger cost
/// uninstalls the source before the effects resolve, and the counters would
/// go to the bank with it — so the rule sets them aside as the cost is paid
/// and they are "still considered to be hosted" for this ability's own
/// effects. Nothing on the card says so; the rule says so for every card
/// shaped like this.
///
/// ANNOTATED SHAPE. "In the root of a remote server" is written as the
/// install condition's remote-server location narrowed by the three card
/// types a root can hold. See [`installs_a_card_in_the_root_of_a_remote_server`]:
/// 4.6.6e and 4.6.9d make that the same set of installs and not an
/// approximation of it — ice is never in a root and an agenda, asset or
/// upgrade never protects a server — but the kernel's location word is still
/// the wider "in the root of or protecting", and the narrower one it wants is
/// on MEZZIE-QUEUE.md's Blockers.
pub fn estelle_moon() -> Card {
    card("Estelle Moon")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .subtypes(&["Executive"])
        .cost(2)
        .trash_cost(3)
        .unique()
        .text("Whenever you install a card in the root of a remote server, place 1 power counter on this asset.")
        .text("[trash]: For each power counter on this asset, gain 2[credit] and draw 1 card.")
        .when(installs_a_card_in_the_root_of_a_remote_server(Corp), [place(CounterKind::Power, 1)])
        .named("a counter for every remote install")
        .paid(
            trash_this_card(),
            [combined([
                gain_q(Corp, times(2, per_hosted_counter(CounterKind::Power))),
                draw_q(Corp, per_hosted_counter(CounterKind::Power)),
            ])],
        )
        .named("cash the counters in")
        .build()
}

/// Jeeves Model Bioroids — Asset: Alliance. Rez 2, trash 5. ◆
/// "This card costs 0 influence if you have 6 or more
///  non-alliance [haas-bioroid] cards in your deck.
///  The first time you spend 3[click] on the same action each turn, gain
///  [click]."
///
/// UNIMPLEMENTED: the second sentence, which is the whole of what this card
/// does at the table.
///
/// The alliance line is not a sentence this card does. Like "Limit 1 per
/// deck" it is a deckbuilding restriction on influence (1.4.5) — it changes
/// what may go in a deck, and nothing about it is ever asked during a game.
/// It is carried as printed text and denotes into nothing, which is the same
/// treatment Salem's Hospitality already has.
///
/// "The first time you spend 3[click] on the same action each turn" counts
/// CLICKS, and the trigger vocabulary counts ACTIONS. The nearest words are
/// The Collective's "the same action three times in a row" and MirrorMorph's
/// "three different actions", and neither is this sentence: 5.2.6h's basic
/// purge action is ONE action costing three clicks and meets Jeeves, and a
/// double operation followed by an ordinary one is TWO actions costing three
/// clicks between them and meets Jeeves — both of which the official rulings
/// list, and neither of which is any number of repeated actions. Written with
/// the words that exist the card would silently fire less often than it
/// should, so it is marked instead. The general capability wanted is on
/// MEZZIE-QUEUE.md's Blockers.
pub fn jeeves_model_bioroids() -> Card {
    card("Jeeves Model Bioroids")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .subtypes(&["Alliance"])
        .cost(2)
        .trash_cost(5)
        .unique()
        .text("This card costs 0 influence if you have 6 or more non-alliance [haas-bioroid] cards in your deck.")
        .text("The first time you spend 3[click] on the same action each turn, gain [click].")
        .unimplemented("The first time you spend 3[click] on the same action each turn, gain [click].")
        .build()
}

/// Lakshmi Smartfabrics — Asset. Rez 1, trash 3.
/// "Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics.
///  X hosted power counters: Reveal an agenda worth X points
///  from HQ. The Runner cannot steal copies of that agenda for the remainder
///  of this turn."
///
/// PARTIAL: the counting works; the ability that spends the counters does
/// not.
///
/// The first sentence says nothing whatever about the card rezzed, so it is
/// met by every rez the Corp makes — including this card's own. The UFAQ was
/// asked exactly that ("does Lakshmi get a power counter when it is rezzed?")
/// and the answer is yes: 8.1.3 turns the card faceup and active as part of
/// the rez, so the ability is there in time to be met by the occurrence that
/// activated it.
///
/// The paid ability is marked, on two counts that are each enough on their
/// own. "The Runner cannot steal copies of that agenda for the remainder of
/// this turn" is 9.10.1's prohibition, and the prohibition vocabulary names
/// scoring and rezzing and nothing else — stealing (7.5) is not among the
/// acts a card can forbid, and 1.2.2 gives a "cannot" precedence over every
/// permission, so a wrong one is not a small error. And "an agenda worth X
/// points" is a description stipulating a characteristic — the card's agenda
/// points, compared against the X announced for the cost (1.16.2c) — which
/// the description vocabulary cannot say either. Both are on
/// MEZZIE-QUEUE.md's Blockers as general capabilities.
pub fn lakshmi_smartfabrics() -> Card {
    card("Lakshmi Smartfabrics")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .cost(1)
        .trash_cost(3)
        .text("Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics.")
        .text("X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn.")
        .when(corp_rezzes_a_card(), [place(CounterKind::Power, 1)])
        .named("a counter for every rez")
        .unimplemented("X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn.")
        .build()
}

/// Marilyn Campaign — Asset: Advertisement. Rez 2, trash 3.
/// "When you rez this asset, load 8[credit] onto it. When it is empty, trash
///  it.
///  When your turn begins, take 2[credit] from this asset.
///  [interrupt] → When this asset would be trashed, you may shuffle it into
///  R&D instead of adding it to Archives. (It is still considered
///  trashed.)"
///
/// PARTIAL: the campaign pays out and empties itself; the escape into R&D is
/// marked.
///
/// The first printed line is two sentences and two abilities, which is what
/// Daily Casts already is on the Runner's side of the table — LOADING (1.9.4)
/// is what links the "when it is empty" ability to this card, so an asset
/// that had credits placed on it some other way would never trash itself.
/// The one word that differs from Daily Casts is the trigger: "when you rez
/// this asset" rather than "when you install this resource", because a Corp
/// card installed facedown is inactive (9.1.8) and has no ability to meet
/// anything with until it is rezzed.
///
/// "Take 2[credit] from this asset" is 1.10.3a and not a gain of 2 from the
/// bank: the credits move from the card into the pool, which is why the card
/// runs out. An asset holding only 1 gives the 1 it has.
///
/// UNIMPLEMENTED: the interrupt. It is a 9.9.8a replacement of where a trash
/// puts the card, and the kernel replaces a trash destination in exactly one
/// shape — a static declaration, mandatory, naming the removed-from-game zone
/// or a facedown card in play. Marilyn needs the destination to be a deck and
/// needs the replacement to be one the Corp MAY decline, and neither is
/// content on the word that exists. Writing it with what is there would make
/// every trash of this card a shuffle whether the Corp wanted it or not; 8.2.2
/// is the part both readings must keep, and the parenthetical restates it —
/// the card is still trashed, only where it lands changes. The general
/// capability wanted is on MEZZIE-QUEUE.md's Blockers.
pub fn marilyn_campaign() -> Card {
    card("Marilyn Campaign")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .subtypes(&["Advertisement"])
        .cost(2)
        .trash_cost(3)
        .text("When you rez this asset, load 8[credit] onto it. When it is empty, trash it.")
        .text("When your turn begins, take 2[credit] from this asset.")
        .text("[interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. (It is still considered trashed.)")
        .when(self_rezzed(), [load(CounterKind::Credit, 8)])
        .named("load eight on the rez")
        .when(empty_of(CounterKind::Credit), [trash_self()])
        .named("empty, so gone")
        .when(turn_begins(Corp), [take_hosted_credits(this_card(), 2, Corp)])
        .named("two a turn")
        .unimplemented("[interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. (It is still considered trashed.)")
        .build()
}

/// MCA Austerity Policy — Asset. Rez 1, trash 3. ◆
/// "Once per turn → [click]: Place 1 power counter on this
///  asset. When the Runner's next turn begins, they lose [click].
///  [click], [trash], 3 hosted power counters: Gain
///  [click][click][click][click]."
///
/// COMPLETE. Two paid abilities; the first carries 9.3.6g's once-per-turn
/// flag and the second, as the UFAQ says in so many words, does not — so the
/// Corp may cash the card in on the same turn the third counter lands.
///
/// The first ability is TWO instructions, because 9.11.3 makes each sentence
/// one: the counter is placed, and then a delayed conditional ability
/// (9.6.13) is created that waits for the Runner's next turn to begin. It has
/// no stated duration, so 9.6.13c has it exist until it first resolves — a
/// second use on a later Corp turn arms a second one rather than re-arming
/// this. The click the Runner then loses is 1.11.3b's LOSS and not a spend:
/// the two are not synonymous for meeting conditions, a Runner with none left
/// simply stays at zero, and this is why the sentence needs no way for one
/// player's ability to reach into the other's pool — nothing here is
/// controlled by the Runner or paid by them. 1.14.4 leaves both abilities with
/// the Corp throughout, which is the default and not a departure from it.
///
/// The second ability's cost is three costs paid as one (1.16.10b), and 9.5.5
/// is what makes it payable at all: the [trash] uninstalls the source, so the
/// three hosted counters are set aside as the whole cost is paid rather than
/// returning to the bank ahead of the counters half.
pub fn mca_austerity_policy() -> Card {
    card("MCA Austerity Policy")
        .corp()
        .asset()
        .faction("Haas-Bioroid")
        .cost(1)
        .trash_cost(3)
        .unique()
        .text("Once per turn → [click]: Place 1 power counter on this asset. When the Runner's next turn begins, they lose [click].")
        .text("[click], [trash], 3 hosted power counters: Gain [click][click][click][click].")
        .paid_once_per_turn(
            clicks(1),
            [
                place(CounterKind::Power, 1),
                when_the_next_turn_begins_of(
                    Runner,
                    "mca austerity policy: the runner loses a click",
                    [lose_clicks(Runner, 1)],
                ),
            ],
        )
        .named("a counter, and a click off the runner")
        .paid(
            clicks(1).plus_cost(trash_this_card()).plus_cost(hosted_counters(CounterKind::Power, 3)),
            [gain_clicks(Corp, 4)],
        )
        .named("cash in for four clicks")
        .build()
}

// ---------------------------------------------------------------------------
// Ice
// ---------------------------------------------------------------------------

/// Tatu-Bola — ICE: Barrier. Rez 2, strength 1.
/// "When the Runner passes this ice, you may swap it with a piece of ice from
///  HQ. If you do, gain 4[credit]. (The new ice is installed unrezzed. You
///  do not pay an install cost.)
///  [subroutine] End the run."
///
/// COMPLETE. Two printed sentences, two instructions (9.11.3) — the swap, and
/// the gain that reads back whether it happened.
///
/// The printed "you may" governs the whole conditional ability (9.6.9): a
/// Corp who declines swaps nothing and gains nothing. "If you do" is the
/// other half of that, and it is a real question rather than a restatement:
/// with no ice in HQ the ability is still offered, the swap announces
/// nothing, and the gain must not happen. 1.15.4 is what lets the second
/// sentence ask — it names the card the first sentence chose — and 8.8.2 is
/// why an announced partner always means a completed swap: the candidate list
/// is already filtered to cards that may legally occupy each other's
/// locations, so there is no announcement that fails to exchange.
///
/// The parenthetical is 1.4 reminder text for 8.8.4a/b, which the swap
/// already is: exactly one of the two was installed, so the other takes its
/// position without the 8.5.16 install procedure — no cost paid — and enters
/// the play area in the state a Corp card normally enters it, which is
/// unrezzed.
///
/// ANNOTATED SHAPE. The swap is written as ONE instruction with two halves —
/// the 1.15.2 announcement of the piece of ice, then the 8.8.1 exchange of
/// that card with this one — rather than as a swap whose two sides are two
/// descriptions. That is the same single instruction either way (no extra
/// checkpoint, no extra reaction or interrupt window, one announcement), but
/// the shorter spelling cannot be used yet: the vocabulary's swap draws BOTH
/// its sides from one description, because 8.8.2's "may occupy the other's
/// location" filter is applied within that description's own candidates. A
/// swap with one side fixed by the sentence ("swap **it** with …") therefore
/// finds no partner and silently does nothing, which is worse than saying it
/// long-hand. The general capability wanted is in MEZZIE-QUEUE.md's Blockers;
/// when it lands this becomes one call.
pub fn tatu_bola() -> Card {
    card("Tatu-Bola")
        .corp()
        .ice(1)
        .faction("Jinteki")
        .subtypes(&["Barrier"])
        .cost(2)
        .text("When the Runner passes this ice, you may swap it with a piece of ice from HQ. If you do, gain 4[credit]. (The new ice is installed unrezzed. You do not pay an install cost.)")
        .text("[subroutine] End the run.")
        .may_when(
            passed(),
            [
                combined([
                    choose_cards(1, &[in_hand_of(Corp), of_type(CardType::Ice)]),
                    swap(this_card(), earlier_choice(0)),
                ]),
                if_met(
                    &[earlier_choice_matches(0, &[of_type(CardType::Ice)])],
                    [gain(Corp, 4)],
                ),
            ],
        )
        .named("trade places with hq")
        .subroutine([end_the_run()])
        .build()
}

/// Vanilla — ICE: Barrier. Rez 0, strength 0.
/// "[subroutine] End the run."
///
/// COMPLETE.
pub fn vanilla() -> Card {
    card("Vanilla")
        .corp()
        .ice(0)
        .faction("Neutral")
        .subtypes(&["Barrier"])
        .cost(0)
        .text("[subroutine] End the run.")
        .subroutine([end_the_run()])
        .build()
}

/// Fairchild 3.0 — ICE: Code Gate - Bioroid - AP. Rez 6, strength 5.
/// "Lose [click][click][click]: Break up to 3 subroutines on
///  this ice. Only the Runner can use this ability.
///  [subroutine] The Runner must pay 3[credit] or trash 1 of their installed
///  cards.
///  [subroutine] The Runner must pay 3[credit] or trash 1 of their installed
///  cards.
///  [subroutine] Do 1 core damage or end the run."
///
/// UNIMPLEMENTED: the bioroid ability. Everything in it but the last sentence
/// has words — 5.2.1a's "Lose [click]" trigger cost, and 9.8.6's break of up
/// to 3 of this ice's unbroken subroutines — but "Only the Runner can use
/// this ability" is 1.14.4's *"by default"* clause, and the kernel has no
/// default to depart from: an ability's controller is the controller of its
/// source, full stop, and paid abilities are offered to that player alone.
/// Written without it the ability would be the CORP's, which is worse than
/// leaving it unsaid — the Corp would be able to break its own ice and the
/// Runner never could. So it is marked rather than approximated.
///
/// The three subroutines are complete. Each of the first two is 9.12.3c's
/// mandatory choice, and 9.12.3c is the whole of what makes them bite: the
/// Runner "must" choose an option **that can be fully resolved**, so a Runner
/// with 2[credit] cannot elect to pay 3 and a Runner with nothing installed
/// cannot elect to trash — and a Runner who can do neither faces an ability
/// that does nothing at all. 1.14.5 puts the choice with the player the
/// sentence names, which is why it is the Runner's and not the Corp's; the
/// last subroutine names nobody, so 1.14.4 leaves that one with the Corp.
pub fn fairchild_3_0() -> Card {
    card("Fairchild 3.0")
        .corp()
        .ice(5)
        .faction("Haas-Bioroid")
        .subtypes(&["Code Gate", "Bioroid", "AP"])
        .cost(6)
        .text("Lose [click][click][click]: Break up to 3 subroutines on this ice. Only the Runner can use this ability.")
        .text("[subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards.")
        .text("[subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards.")
        .text("[subroutine] Do 1 core damage or end the run.")
        .unimplemented("Lose [click][click][click]: Break up to 3 subroutines on this ice. Only the Runner can use this ability.")
        // The card prints this subroutine twice, so it is written twice.
        .subroutine([performed_by(
            Runner,
            choose_one([
                ("pay 3[credit]", vec![lose(Runner, 3)]),
                (
                    "trash 1 of their installed cards",
                    vec![trash(choose(1, &[installed_runner_card()]))],
                ),
            ]),
        )])
        .named("pay or trash")
        .subroutine([performed_by(
            Runner,
            choose_one([
                ("pay 3[credit]", vec![lose(Runner, 3)]),
                (
                    "trash 1 of their installed cards",
                    vec![trash(choose(1, &[installed_runner_card()]))],
                ),
            ]),
        )])
        .named("pay or trash again")
        .subroutine([choose_one([
            ("do 1 core damage", vec![core_damage(Corp, 1)]),
            ("end the run", vec![end_the_run()]),
        ])])
        .named("core damage or end the run")
        .build()
}

/// Vertigo — ICE: Code Gate. Rez 1, strength 1.
/// "When the Runner passes this ice, if they have no [click] remaining, they
///  cannot steal or trash Corp cards for the remainder of this run.
///  [subroutine] The Runner loses [click]."
///
/// UNIMPLEMENTED: the first sentence, on two counts, and neither of them is
/// close enough to fudge.
///
/// "If they have no [click] remaining" is a 9.6.5c requirement about a
/// NUMBER — the clicks in the Runner's click pool (1.11) — and the quantity
/// language has no selector that reads a click pool, so the requirement
/// cannot be stated at all.
///
/// "They cannot steal or trash Corp cards for the remainder of this run" is a
/// 9.10.1 prohibition with the run as its duration, and the prohibition
/// vocabulary names only two acts: scoring and rezzing. Stealing (7.5) and
/// trashing (7.1.5 / 1.19.4) are not among them, so there is nothing to say
/// "cannot" about. Written with what exists, the sentence would either do
/// nothing or forbid the wrong thing, and 1.2.2 gives a "cannot" precedence
/// over every permission — so getting it wrong is not a small error.
///
/// The subroutine is complete: 1.11.3b's loss, which is not a spend, and
/// which leaves a Runner with no clicks at zero rather than failing.
pub fn vertigo() -> Card {
    card("Vertigo")
        .corp()
        .ice(1)
        .faction("Haas-Bioroid")
        .subtypes(&["Code Gate"])
        .cost(1)
        .text("When the Runner passes this ice, if they have no [click] remaining, they cannot steal or trash Corp cards for the remainder of this run.")
        .text("[subroutine] The Runner loses [click].")
        .unimplemented("When the Runner passes this ice, if they have no [click] remaining, they cannot steal or trash Corp cards for the remainder of this run.")
        .subroutine([lose_clicks(Runner, 1)])
        .named("the runner loses a click")
        .build()
}

/// Drafter — ICE: Sentry. Rez 3, strength 3.
/// "[subroutine] You may add 1 card from Archives to HQ.
///  [subroutine] You may install 1 card from Archives or HQ, ignoring all
///  costs."
///
/// COMPLETE. One sentence each (9.11.3), and each sentence's "you may" is
/// 9.6.9d's optional part inside the instruction rather than an optional
/// ability: a subroutine resolves whether or not the Corp takes what it
/// offers.
///
/// "1 card from Archives" and "1 card from Archives or HQ" both name their
/// zones, which is exactly what 1.15.2c asks for before a description may
/// reach outside the play area — and "Archives **or** HQ" is one description
/// with two alternatives, not two descriptions, so it is one announcement and
/// the Corp picks from both piles at once.
///
/// "Ignoring all costs" is 1.16.5c: every element of the install cost goes,
/// 8.5.11a's 1[credit] per piece of ice already protecting the destination
/// included. The sentence states no destination, so the Corp declares one at
/// step 8.5.16b.
pub fn drafter() -> Card {
    card("Drafter")
        .corp()
        .ice(3)
        .faction("Haas-Bioroid")
        .subtypes(&["Sentry"])
        .cost(3)
        .text("[subroutine] You may add 1 card from Archives to HQ.")
        .text("[subroutine] You may install 1 card from Archives or HQ, ignoring all costs.")
        .subroutine([may(add_to_hand(choose(1, &[in_archives()])))])
        .named("archives to hq")
        .subroutine([may(install_ignoring_all_costs(
            choose(1, &[any_of(&[&[in_archives()], &[in_hand_of(Corp)]])]),
            InstallDest::DeclaredByInstaller,
        ))])
        .named("install for free")
        .build()
}

/// Tour Guide — ICE: Sentry. Rez 2, strength 0.
/// "This ice gains "[subroutine] End the run." for each rezzed asset."
///
/// COMPLETE. Permanently true rather than something that happens, so a static
/// declaration — and the count is the point of the card. CR 9.12.2b's
/// calculated quantity sits in a STATIC ability, which means it is never
/// evaluated once and remembered: 9.12.1d–e recompute an object's effective
/// characteristics from its printed ones every time they are read, so the
/// number of subroutines this ice has is a question asked afresh at every
/// checkpoint and every time the encounter looks for the next unbroken
/// subroutine. Rez an asset mid-encounter and the Runner faces one more;
/// trash one and the list shrinks. A lingering effect (9.10) would have been
/// the wrong shape — it would have been created once, with the count it had
/// then, and 9.10.1 would have kept it alive at that value.
///
/// 9.8.3d places them: a static ability ON THE ICE ITSELF that states no
/// order puts its subroutines after the printed ones (there are none), in the
/// order gained, and takes the LAST one back first when the count falls. That
/// category needs no 9.8.2c "order of your choice" declaration, and the card
/// prints no words asking for one.
///
/// 9.8.4b is what makes a subroutine gained during an encounter matter: it
/// arrives unbroken, so an asset rezzed after the Runner has broken everything
/// still costs them.
pub fn tour_guide() -> Card {
    card("Tour Guide")
        .corp()
        .ice(0)
        .faction("Weyland Consortium")
        .subtypes(&["Sentry"])
        .cost(2)
        .text("This ice gains \"[subroutine] End the run.\" for each rezzed asset.")
        .declares([gains_subroutines(
            per_card_matching(&[of_type(CardType::Asset), rezzed()]),
            [end_the_run()],
        )])
        .named("one end-the-run per rezzed asset")
        .build()
}

/// The deck so far, in the order `docs/vm/MEZZIE-QUEUE.md` lists it.
///
/// The identity and Rashida Jaheem are REUSED — they came out of the identity
/// queue and out of the Gauntlet deck respectively, and a card is written
/// once. The rest of the queue's 24 distinct cards arrive as waves land; a
/// card nobody has written yet is absent from this list rather than present
/// as a stub, so the list and the tick-boxes always say the same thing.
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::corp_haas_bioroid::asa_group(),
        estelle_moon(),
        jeeves_model_bioroids(),
        lakshmi_smartfabrics(),
        marilyn_campaign(),
        mca_austerity_policy(),
        super::gauntlet::rashida_jaheem(),
        tatu_bola(),
        vanilla(),
        fairchild_3_0(),
        vertigo(),
        drafter(),
        tour_guide(),
    ]
}

/// CR 1.5.4a: the pile is the RUNNER's — "a player may bring any number of
/// additional **Runner** identity cards along with their deck" — so a Corp
/// deck brings none.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
