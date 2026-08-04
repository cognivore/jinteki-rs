//! Every identity in the game, faction by faction.
//!
//! `docs/vm/IDENTITY-QUEUE.md` is the list and the ledger. The reason it
//! exists is CR 1.5.4a: a player brings "any number of additional Runner
//! identity cards" along with their deck, and Rebirth and DJ Fenris choose
//! from those — so with six identities implemented, "another identity of the
//! same faction" is a choice of one. These modules are the fix.
//!
//! Same discipline as every other card module: printed text copied from
//! `crates/jinteki-core/carddata/cards.json` into `.text(…)`, one call per
//! printed sentence, `.unimplemented(…)` rather than an approximation, and a
//! behaviour test per identity in `tests/behaviour.rs`.
//!
//! These identities are NOT in `priority_decks()` — the deck odometer counts
//! the two decks — but `jinteki_cards::find` reaches them like any other
//! card. An identity joins a deck's 1.5.4a pile (`jinteki-server`'s
//! `cr::ANDROMEDA_PILE`) only once it is COMPLETE: `cr::readiness()` holds a
//! pile card to the same bar as a deck card, so enlisting a partial one would
//! make both priority decks unplayable.

use crate::edsl::Card;

pub mod runner_criminal;
pub mod runner_shaper;

/// Every identity these modules carry, in queue order.
pub fn cards() -> Vec<Card> {
    let mut all = runner_criminal::identities();
    all.extend(runner_shaper::identities());
    all
}
