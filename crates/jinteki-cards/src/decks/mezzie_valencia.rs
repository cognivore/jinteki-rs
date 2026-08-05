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
//! there rather than copied. Nothing in this file yet is new — every card
//! below came out of Andromeda or the identity queue — so the module carries
//! no card FUNCTION of its own so far, only the list.

use crate::edsl::Card;

/// The deck so far, in the order `docs/vm/MEZZIE-QUEUE.md` lists it.
///
/// Every entry is REUSED: the identity came out of the identity queue, and
/// Sure Gamble, Rebirth, Boomerang, Desperado and Paperclip are Andromeda's,
/// written once and played by both decks. The rest of the queue's 23 distinct
/// cards arrive as waves land; a card nobody has written yet is absent from
/// this list rather than present as a stub, so the list and the tick-boxes
/// always say the same thing.
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::runner_anarch::valencia_estevez(),
        super::andromeda::rebirth(),
        super::andromeda::sure_gamble(),
        super::andromeda::boomerang(),
        super::andromeda::desperado(),
        super::andromeda::paperclip(),
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
