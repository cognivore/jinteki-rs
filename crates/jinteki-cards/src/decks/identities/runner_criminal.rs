//! Runner — Criminal identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). Andromeda and Ken Tenma are the two Criminals that
//! already exist — they live in `decks/andromeda.rs`, because that deck plays
//! one and carries the other in its CR 1.5.4a pile — and everything else in
//! the faction lands here.

use crate::edsl::*;

/// Gabriel Santiago: Consummate Professional — Identity: Cyborg. Link 0.
/// "The first time you make a successful run on HQ each turn, gain 2[credit]."
///
/// COMPLETE. Two ordinary readings and nothing else: 6.8.4's "makes a
/// successful run", with the server the sentence names as the stipulation on
/// it, and 9.6.5c's stipulation about the OCCURRENCE for "the first time each
/// turn" — counted from the change log since the turn began, and deliberately
/// not 9.3.6g's once-per-turn flag (9.1.6: an entirely mandatory ability is
/// never *used*, so nothing would ever spend that flag).
pub fn gabriel_santiago() -> Card {
    card("Gabriel Santiago: Consummate Professional")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .text("The first time you make a successful run on HQ each turn, gain 2[credit].")
        .when_first_each_turn(makes_successful_run_on(&[ServerId::Hq]), [gain(Runner, 2)])
        .named("the first HQ run of the turn")
        .build()
}

/// Los: Data Hijacker — Identity: G-mod. Link 0.
/// "The first time the Corp rezzes a piece of ice each turn, gain 2[credit]."
///
/// COMPLETE. The condition is 8.1.2's rez with the sentence's card-type
/// stipulation — the same one Lt. Todachine's "whenever you rez a piece of
/// ice" states, read here from the other side of the table, which is why the
/// condition names no player: only the Corp rezzes cards (8.1.1).
///
/// "The first time … each turn" is 9.6.5c's stipulation about the occurrence,
/// so the second rez of the turn does not meet the condition at all — the
/// ability is not throttled after firing, it simply never triggers again.
pub fn los() -> Card {
    card("Los: Data Hijacker")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .text("The first time the Corp rezzes a piece of ice each turn, gain 2[credit].")
        .when_first_each_turn(corp_rezzes_a(CardType::Ice), [gain(Runner, 2)])
        .named("the first ice rez of the turn")
        .build()
}

/// Liza Talking Thunder: Prominent Legislator — Identity: G-mod. Link 0.
/// "The first time you make a successful run on a central server each turn,
///  draw 2 cards and take 1 tag."
///
/// COMPLETE. "A central server" is CR 4.6.5, which names them and no others:
/// HQ, R&D and Archives. The set is fixed by the rule, so the sentence's
/// stipulation is that list of three.
///
/// "Draw 2 cards and take 1 tag" is written as TWO instructions rather than
/// one `combined([…])`: 9.11.3 splits a sentence into an instruction per
/// class, and 9.12.2c only aggregates effects of the SAME class. A draw and a
/// tag are not, so joining them would put two unrelated atoms in one
/// imminence and give a prevention effect one window for both.
pub fn liza_talking_thunder() -> Card {
    card("Liza Talking Thunder: Prominent Legislator")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .text("The first time you make a successful run on a central server each turn, draw 2 cards and take 1 tag.")
        .when_first_each_turn(
            makes_successful_run_on_a_central_server(),
            // 9.11.3: "draw 2 cards and take 1 tag" is one SENTENCE, so one
            // instruction — a tag-avoidance effect and a draw interrupt see
            // the same imminence, exactly as Snare!'s tag and damage do.
            // 9.12.2c is about aggregating a calculated quantity ("for
            // each"), not about whether a sentence splits.
            [combined([draw(Runner, 2), give_tags(1)])],
        )
        .named("the first central run of the turn")
        .build()
}

/// Laramy Fisk: Savvy Investor — Identity: Natural. Link 0.
/// "The first time you make a successful run on a central server each turn,
///  you may force the Corp to draw 1 card."
///
/// COMPLETE. The same condition Liza states, and the printed "you may" is the
/// WHOLE ability, so it is 9.6.9's declinable conditional — the decision goes
/// to the ability's controller, which 9.1.1a makes the Runner, since it is
/// the Runner's identity.
///
/// "Force the Corp to draw" needs no 1.14.5 attribution: §8.4's draw already
/// names the player who draws, and drawing offers that player no choices to
/// make, so naming the Corp in the instruction is the whole of it.
pub fn laramy_fisk() -> Card {
    card("Laramy Fisk: Savvy Investor")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("The first time you make a successful run on a central server each turn, you may force the Corp to draw 1 card.")
        .may_when_first_each_turn(makes_successful_run_on_a_central_server(), [draw(Corp, 1)])
        .named("the first central run of the turn")
        .build()
}

/// Every Criminal identity this module carries, in the queue's order.
pub fn identities() -> Vec<Card> {
    vec![gabriel_santiago(), los(), liza_talking_thunder(), laramy_fisk()]
}
