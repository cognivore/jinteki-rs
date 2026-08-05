//! Corp — Neutral identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).
//!
//! Three of the four print only DECK-CONSTRUCTION or FORMAT restrictions, and
//! `runner_neutral.rs` says why that is complete rather than a gap: CR 1.4.2
//! settles deck legality before the game begins, so there is no condition to
//! meet and nothing to resolve, and the writing guide's third rule of thumb
//! puts such a sentence in the facts or nowhere. The fourth — Cyber Bureau —
//! is the queue's one Neutral Corp identity that PLAYS, both faces of it.

use crate::edsl::*;

/// Ampère: Cybernetics For Anyone — Identity: Corp.
/// "Your deck cannot include more than 1 copy of any card.
///  Your deck may include up to 2 different agenda cards from each Corp
///  faction."
///
/// COMPLETE. Two deck-construction restrictions (CR 1.4) and nothing else —
/// the second is a WIDENING of 1.4.5's out-of-faction rule rather than a
/// narrowing, but both are settled before the game begins and neither is read
/// again once it has.
pub fn ampere() -> Card {
    card("Ampère: Cybernetics For Anyone")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Corp"])
        .text("Your deck cannot include more than 1 copy of any card.")
        .text("Your deck may include up to 2 different agenda cards from each Corp faction.")
        .build()
}

/// The Shadow: Pulling the Strings — Identity: Megacorp.
/// "Draft format only.
///  You can use agendas from all factions in this deck."
///
/// COMPLETE. A format restriction and the deck-construction permission that
/// goes with it. 1.4.2 checks both before the game begins.
pub fn the_shadow() -> Card {
    card("The Shadow: Pulling the Strings")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Megacorp"])
        .text("Draft format only.")
        .text("You can use agendas from all factions in this deck.")
        .build()
}

/// The Syndicate: Profit over Principle — Identity: Megacorp.
/// "Starter game only."
///
/// COMPLETE. The format restriction The Catalyst prints on the Runner side,
/// and nothing else is on the card.
pub fn the_syndicate() -> Card {
    card("The Syndicate: Profit over Principle")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Megacorp"])
        .text("Starter game only.")
        .build()
}

/// Cyber Bureau: Keeping the Peace — Identity: Police Department.
/// "You draw a starting hand of 10 cards.
///  Before taking your first turn, install up to 5 cards, ignoring all
///  install costs. Rez any number of them, lowering the total rez cost among
///  all cards by 20. Flip this identity."
///
/// COMPLETE, both faces. DATA SHAPE: upstream's v2 file gives this card
/// `faces: []` (empty — an upstream defect, ignored rather than worked
/// around, per the explicit user directive), and ships the ENTIRE card,
/// back face included, in the main `text` field; that field flows through
/// the pipeline into `cards.json`, and both faces here are written from it
/// (SYS-D-10, user-authorized). Migrate to `faces[]` when upstream ships it.
/// The two faces are still two faces of one identity
/// (rule_identity_double_sided), so the back is built as its own card and
/// carried by `.flip_face(…)` exactly as every other double-sider's is.
///
/// The first line is Andromeda's setup FACT (1.6.6): the number 1.6.6 draws
/// — and a mulligan redraws — is 10, not 5.
///
/// The second line is NEXT Design's window, 1.6.7a: the ability resolves
/// immediately before the Corp's first turn, after both mulligan decisions
/// "and thus before the game starts". Three printed sentences, three
/// instructions (9.11.3). "Install up to 5 cards" is 8.5.5's one-at-a-time
/// choice, declinable at every pick, "ignoring all install costs" 1.16.5c —
/// every element of each install's cost removed, 8.5.11a's per-ice surcharge
/// included, so the credit pool does not move. "Rez any number of them" is
/// 8.1.2b's by-ability rez said about 1.15.4's "them" — the cards this
/// ability installed — one at a time and declinable ("any number" includes
/// zero); "lowering the total rez cost among all cards by 20" is 1.16.2f's
/// divide-the-modifier said about MANY rezzes: one 20[credit] pool, the
/// Corp declaring each rez's share before it pays, each rez cost floored at
/// 0 by 1.16.2a, the leftover lapsing when the Corp stops. Only install
/// costs were ignored: each rez still pays (8.1.2d, reduced), still takes
/// its cost-paid checkpoint, and a card the Corp could not fund even with
/// the whole remaining pool is not offered (1.16.1b). "Flip this identity"
/// is mandatory — no "may" anywhere in the line — so declining every
/// install and every rez still turns the card, and the game is played on
/// Detective's Bureau from before turn one.
pub fn cyber_bureau() -> Card {
    card("Cyber Bureau: Keeping the Peace")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Police Department"])
        .text("You draw a starting hand of 10 cards.")
        .text("Before taking your first turn, install up to 5 cards, ignoring all install costs. Rez any number of them, lowering the total rez cost among all cards by 20. Flip this identity.")
        .starting_hand(10)
        .when(
            before_taking_first_turn(),
            [
                install_cards_from_hand_ignoring_all_costs(
                    5,
                    Corp,
                    InstallFilter::Any,
                    InstallDest::DeclaredByInstaller,
                ),
                rez_any_of_the_installed_lowering_total_by(20),
                flip_identity(Corp),
            ],
        )
        .named("keeping the peace: the grand opening")
        .flip_face(detectives_bureau())
        .build()
}

/// Detective's Bureau: Upholding the Law — Identity: Police Department; the
/// back face of Cyber Bureau: Keeping the Peace (oracle: the main `text`
/// field of the upstream entry — its `faces` array is empty upstream, so the
/// back's name and lines ship inline there; see [`cyber_bureau`]).
/// "The first time the Runner initiates a run each turn, force the Runner to
///  lose 1[credit] for each agenda point in his or her score area, then you
///  gain 1[credit] for each credit lost.
///  [click]: Gain 3[credit] or draw 3 cards."
///
/// "The first time … each turn" is 9.6.5c's ordinal on the run-initiation
/// condition (6.9.1: the run begins at the Run Initiation Phase, any
/// server). The sentence is mandatory — "force" — and its two halves land
/// in order: the loss is 1.10.3b's forced loss, one credit per agenda point
/// in the RUNNER'S score area (1.17.2's printed points, `agenda_points_of`
/// over that zone), taking as many credits as the pool holds and no more;
/// "you gain 1[credit] for each credit lost" then reads the credits
/// ACTUALLY lost — the recorded `CreditsLost` amounts of this resolution,
/// not the computed request — which is Account Siphon's own wording and
/// quantity, so a Runner at 1[credit] with 3 points loses 1 and pays the
/// Bureau exactly 1.
///
/// The second line is Hyoubu's shape: a paid ability whose whole cost is
/// the [click], and the printed "or" is 9.11.4g's option choice, made by
/// the Corp when the ability resolves.
pub fn detectives_bureau() -> Card {
    card("Detective's Bureau: Upholding the Law")
        .corp()
        .identity()
        .faction("Neutral")
        .subtypes(&["Police Department"])
        .text("The first time the Runner initiates a run each turn, force the Runner to lose 1[credit] for each agenda point in his or her score area, then you gain 1[credit] for each credit lost.")
        .text("[click]: Gain 3[credit] or draw 3 cards.")
        .when_first_each_turn(
            run_begins_on(&[]),
            [
                loses_credits(Runner, agenda_points_of(&[in_score_area_of(Runner)])),
                gain_q(Corp, per_credit_lost_by(Runner)),
            ],
        )
        .named("upholding the law: the toll")
        .paid(
            clicks(1),
            [choose_one([
                ("Gain 3[credit]", vec![gain(Corp, 3)]),
                ("draw 3 cards", vec![draw(Corp, 3)]),
            ])],
        )
        .named("upholding the law: gain 3 or draw 3")
        .build()
}

/// Every Neutral Corp identity this module carries, in queue order.
pub fn identities() -> Vec<Card> {
    vec![ampere(), the_shadow(), the_syndicate(), cyber_bureau()]
}
