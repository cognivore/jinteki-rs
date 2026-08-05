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
// Ice
// ---------------------------------------------------------------------------

/// Tatu-Bola — ICE: Barrier. Rez 2, strength 1.
/// "When the Runner passes this ice, you may swap it with a piece of ice from
///  HQ. If you do, gain 4[credit]. <em>(The new ice is installed unrezzed. You
///  do not pay an install cost.)</em>
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
        .text("When the Runner passes this ice, you may swap it with a piece of ice from HQ. If you do, gain 4[credit]. <em>(The new ice is installed unrezzed. You do not pay an install cost.)</em>")
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
/// "<strong>Lose [click][click][click]:</strong> Break up to 3 subroutines on
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
        .text("<strong>Lose [click][click][click]:</strong> Break up to 3 subroutines on this ice. Only the Runner can use this ability.")
        .text("[subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards.")
        .text("[subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards.")
        .text("[subroutine] Do 1 core damage or end the run.")
        .unimplemented("<strong>Lose [click][click][click]:</strong> Break up to 3 subroutines on this ice. Only the Runner can use this ability.")
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
