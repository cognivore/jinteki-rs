//! Mezzie's Valencia — Valencia Estevez: The Angel of Cayambe.
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
//! there rather than copied — Sure Gamble, Rebirth, Boomerang, Desperado and
//! Paperclip are all Andromeda's, and Paperclip in particular is the card
//! [`black_orchestra`] and [`mkultra`] are written as siblings of.

use crate::edsl::*;

// ---------------------------------------------------------------------------
// Hardware
// ---------------------------------------------------------------------------

/// Zer0 — Hardware. Install 1. ◆
/// "Once per turn → [click], suffer 1 net damage: Gain 1[credit] and draw 2
///  cards."
///
/// COMPLETE. One printed sentence, and every hard call in it is about the
/// COLON: everything to the left is cost, everything to the right is effect.
///
/// 1.16.1 is what makes "suffer 1 net damage" a cost at all — a cost may be
/// "an effect a player must resolve", and a player who cannot pay the full
/// cost cannot use the ability. That is not the same card as one that DOES
/// the Runner 1 net damage: the damage here is paid before anything is
/// gained, its trash is not something the ability caused, and 1.16.1a keeps
/// an optional interrupt from cancelling the act of paying it. `Cost` is
/// therefore where it goes, beside the [click], as one cost paid all at once.
///
/// 1.11.3c is why nobody has to say where this ability is offered: a paid
/// ability that begins with [click] IS an action, so the Runner is offered it
/// in an action window and never in a paid one.
///
/// "Once per turn →" is 9.3.6g's flag, and the distinction that matters is
/// what spends it. 9.3.6g points at 9.1.6, and 9.1.6 is about USING: a player
/// who resolves an optional ability has used it. That is a fact about this
/// ability's own history, not about the turn's — which is what
/// `when_first_each_turn`'s 9.6.5c stipulation would have said instead, and
/// 9.6.5c is about an OCCURRENCE the turn has already seen, so a second copy
/// of the card would inherit it. Two copies of Zer0 could not both be
/// installed (2.2.1), but the difference is real for the whole class and
/// `crates/jinteki-cr/tests/using_abilities.rs` is where it is pinned.
///
/// 9.11.3: "Gain 1[credit] and draw 2 cards" is ONE sentence, so it is one
/// instruction with two effects — one checkpoint and one reaction window
/// cover both halves, and nothing gets to react between the credit and the
/// cards.
pub fn zer0() -> Card {
    card("Zer0")
        .runner()
        .hardware()
        .faction("Anarch")
        .cost(1)
        .unique()
        .text("Once per turn → [click], suffer 1 net damage: Gain 1[credit] and draw 2 cards.")
        .paid_once_per_turn(
            clicks(1).plus_cost(suffer_net_damage(1)),
            [combined([gain(Runner, 1), draw(Runner, 2)])],
        )
        .named("bleed for a credit and two cards")
        .build()
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

/// Black Orchestra — Program: Icebreaker - Decoder. Install 3, strength 2,
/// 1[mu].
/// "Whenever you encounter a code gate, you may install this program from
///  your heap.
///  3[credit]: +2 strength. Then, if this program can interface with the code
///  gate you are encountering, break up to 2 subroutines."
///
/// COMPLETE. The conspiracy breaker, written as Paperclip's sibling and
/// deliberately not as a second mechanism: the two cards differ only in the
/// subtype they name and in the fact that Paperclip announces X (1.16.2c)
/// where this one prints its numbers.
///
/// The first sentence works from the HEAP, and 9.1.8b is what puts it there:
/// "abilities stating that they are active in a particular zone are active in
/// that zone". Without the statement, 4.4.4 would leave it inactive with the
/// rest of the discard pile. The zone is written as a requirement on the
/// trigger condition (9.6.5c) rather than as a test inside the effect,
/// because that is the half of the ability 9.1.8b reads — and it is also what
/// keeps the same ability from offering an install out of the grip, where the
/// printed words do not reach.
///
/// "If this program can interface with the code gate you are encountering" is
/// deliberately NOT 9.3.6c's interface flag, even though 3.9.5g/h is exactly
/// the question it asks. The flag is checked when the ability is OFFERED;
/// this sentence is checked when the break instruction RESOLVES, which
/// 9.6.5d permits explicitly — "the condition only needs to be met when the
/// relevant instructions resolve" — and that is the whole card: a Black
/// Orchestra whose printed 2 had to match the code gate before pumping could
/// never break anything it was not already big enough for.
///
/// 9.8.6 governs what the break may take: only unbroken subroutines can be
/// chosen, and "up to 2" is the count the Runner announces within that.
pub fn black_orchestra() -> Card {
    card("Black Orchestra")
        .runner()
        .program()
        .faction("Anarch")
        .subtypes(&["Icebreaker", "Decoder"])
        .cost(3)
        .strength(2)
        .memory(1)
        .text("Whenever you encounter a code gate, you may install this program from your heap.")
        .text("3[credit]: +2 strength. Then, if this program can interface with the code gate you are encountering, break up to 2 subroutines.")
        .may_when(
            encounters_a("Code Gate", &[source_in_discard()]),
            [install_this_card()],
        )
        .named("out of the heap")
        .paid(
            credits(3),
            [
                pump(2),
                if_met(
                    &[can_interface_with_the_encountered("Code Gate")],
                    [break_up_to(2)],
                ),
            ],
        )
        .named("pump and break")
        .build()
}

/// MKUltra — Program: Icebreaker - Killer. Install 2, strength 1, 1[mu].
/// "Whenever you encounter a sentry, you may install this program from your
///  heap.
///  3[credit]: +2 strength. Then, if this program can interface with the
///  sentry you are encountering, break up to 2 subroutines."
///
/// COMPLETE. The same two sentences as [`black_orchestra`] with a different
/// subtype in both of them, which is exactly what ARCHITECTURE §12 rule 2
/// asks for: the subtype is CONTENT on `encounters_a` and on
/// `can_interface_with_the_encountered`, so the whole conspiracy class is one
/// shape and not three.
///
/// The two rules that decide it are the same two. 9.1.8b puts the install
/// ability in the heap that 4.4.4 would otherwise make inactive; 9.6.5d puts
/// the interface question after "+2 strength" instead of before it, which for
/// this card is the difference between a breaker that works and one that
/// cannot touch a sentry stronger than its printed 1. 3.9.5g is the
/// comparison being made and 3.9.5h is the subtype half of it.
pub fn mkultra() -> Card {
    card("MKUltra")
        .runner()
        .program()
        .faction("Anarch")
        .subtypes(&["Icebreaker", "Killer"])
        .cost(2)
        .strength(1)
        .memory(1)
        .text("Whenever you encounter a sentry, you may install this program from your heap.")
        .text("3[credit]: +2 strength. Then, if this program can interface with the sentry you are encountering, break up to 2 subroutines.")
        .may_when(
            encounters_a("Sentry", &[source_in_discard()]),
            [install_this_card()],
        )
        .named("out of the heap")
        .paid(
            credits(3),
            [
                pump(2),
                if_met(
                    &[can_interface_with_the_encountered("Sentry")],
                    [break_up_to(2)],
                ),
            ],
        )
        .named("pump and break")
        .build()
}

/// Rezeki — Program. Install 2, 1[mu].
/// "When your turn begins, gain 1[credit]."
///
/// COMPLETE, and a CONDITIONAL ability (9.6.1) rather than a static
/// declaration — the one call this card asks anyone to make.
///
/// 9.4.1 is what a static ability is: one that "continuously affects the game
/// as long as it is active", that can contain declarations, and that does
/// "not resolve or have associated priority windows". Rezeki's sentence does
/// none of that. It names a moment ("when your turn begins"), it happens once
/// at that moment rather than continuously, and the credit it produces is an
/// effect that resolves — so 9.6.1's definition is the one it meets: an
/// ability with a primary condition and one or more instructions, triggered
/// at a specific point in the game. 9.6.2 then makes it pending in the next
/// reaction window, where anything watching a gain can see it; 9.4.4's "no
/// durations, no lingering effects" would have been the wrong shape for a
/// one-off payout entirely.
///
/// "YOUR turn" is the stipulation (9.6.5c) that keeps it off the Corp's turn:
/// the side is content on the condition, not a second condition.
pub fn rezeki() -> Card {
    card("Rezeki")
        .runner()
        .program()
        .faction("Shaper")
        .cost(2)
        .memory(1)
        .text("When your turn begins, gain 1[credit].")
        .when(turn_begins(Runner), [gain(Runner, 1)])
        .build()
}

/// The deck so far, in the order `docs/vm/MEZZIE-QUEUE.md` lists it.
///
/// Mixed provenance on purpose: the identity came out of the identity queue,
/// and Sure Gamble, Rebirth, Boomerang, Desperado and Paperclip are
/// Andromeda's, written once and played by both decks. Zer0, Black Orchestra,
/// MKUltra and Rezeki are this module's own. The rest of the queue's 23
/// distinct cards arrive as waves land; a card nobody has written yet is
/// absent from this list rather than present as a stub, so the list and the
/// tick-boxes always say the same thing.
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::runner_anarch::valencia_estevez(),
        super::andromeda::rebirth(),
        super::andromeda::sure_gamble(),
        super::andromeda::boomerang(),
        super::andromeda::desperado(),
        zer0(),
        black_orchestra(),
        mkultra(),
        super::andromeda::paperclip(),
        rezeki(),
    ]
}

/// CR 1.5.4a: the additional identities this deck brings along with it, kept
/// in a pile outside the game. This deck plays Rebirth, so it needs Anarchs
/// for Rebirth's "another identity from the same faction" (1.5.4b) to name a
/// real choice — but which identities a player brings is a decision at the
/// table, and enlisting one is a change to what this deck IS. The pile is
/// left empty until the wave that writes the deck's own cards can make that
/// call; `cr::readiness()` holds a pile card to the same bar as a deck card,
/// so an incomplete one would make the deck unplayable rather than richer.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
