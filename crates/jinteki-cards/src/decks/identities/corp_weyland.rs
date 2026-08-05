//! Corp — Weyland Consortium identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).

use crate::edsl::*;

/// Argus Security: Protection Guaranteed — Identity: Corp.
/// "Whenever the Runner steals an agenda, they must take 1 tag or suffer 2
///  meat damage."
///
/// COMPLETE. The option choice is 9.11.4g's, but "**they** must" is 1.14.5's
/// attribution putting it to the RUNNER — the only thing that separates this
/// card from Sportsmetal's, whose sentence names nobody and so leaves the
/// choice with 9.1.1a's controller.
///
/// "Must" is 9.12.3: the Runner has no choice about whether, only about
/// which, so neither option may be declined. The damage is the RUNNER's:
/// 10.4.1 splits responsibility on the printed verb, and this sentence
/// directs the Runner to "suffer" the damage, which is the branch that makes
/// the Runner and the source responsible — so a "+N to meat damage done by
/// the Corp" bonus does not reach it, and the trashes it performs are the
/// Runner's own.
pub fn argus_security() -> Card {
    card("Argus Security: Protection Guaranteed")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Whenever the Runner steals an agenda, they must take 1 tag or suffer 2 meat damage.")
        .when(
            runner_steals_agenda(),
            [performed_by(
                Runner,
                choose_one([
                    ("take 1 tag", vec![give_tags(1)]),
                    ("suffer 2 meat damage", vec![meat_damage(Runner, 2)]),
                ]),
            )],
        )
        .named("protection guaranteed")
        .build()
}

/// GRNDL: Power Unleashed — Identity: Division, Liability.
/// "You start the game with 10[credit] and 1 bad publicity."
///
/// COMPLETE. One sentence, two facts about the game's SETUP — there is no
/// condition to meet and nothing to resolve, and both are already true when
/// the first turn begins. They go where Andromeda's starting hand of nine
/// goes, among the printed facts, which is also why "and" here is not
/// 9.11.3's instruction question at all.
pub fn grndl() -> Card {
    card("GRNDL: Power Unleashed")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Division", "Liability"])
        .text("You start the game with 10[credit] and 1 bad publicity.")
        .starting_credits(10)
        .starting_bad_publicity(1)
        .build()
}

/// Gagarin Deep Space: Expanding the Horizon — Identity: Corp.
/// "As an additional cost to access a card in the root of a remote server,
///  the Runner must pay 1[credit]."
///
/// COMPLETE. CR 1.16.10 makes an additional cost part of the payment for
/// something that would otherwise have none, and 7.4.3 is where an access
/// pays it — before the card is accessed, so a Runner who cannot pay does not
/// access the card at all and the breach moves on.
///
/// It is a permanent fact about every access matching the description, not
/// something that happens, so it is a static declaration. The description is
/// the root of a REMOTE server: 4.6.5's centrals are untouched, and so is ice
/// protecting the remote, which is not in its root (4.6.6).
pub fn gagarin_deep_space() -> Card {
    card("Gagarin Deep Space: Expanding the Horizon")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("As an additional cost to access a card in the root of a remote server, the Runner must pay 1[credit].")
        .declares([additional_cost_to_access_a_card_in_a_remote_root(credits(1))])
        .named("expanding the horizon")
        .build()
}

/// The Outfit: Family Owned and Operated — Identity: Subsidiary.
/// "Whenever you take 1 or more bad publicity, gain 3[credit]."
///
/// COMPLETE. 10.6.1's bad publicity, taken by the player the sentence names.
/// "1 or more" is not a threshold to check: bad publicity is taken in an
/// amount and the condition is met once per TAKING, so a card handing over
/// two at once pays this identity once — which is exactly what the phrase is
/// there to say.
pub fn the_outfit() -> Card {
    card("The Outfit: Family Owned and Operated")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Subsidiary"])
        .text("Whenever you take 1 or more bad publicity, gain 3[credit].")
        .when(takes_bad_publicity(Corp), [gain(Corp, 3)])
        .named("family owned and operated")
        .build()
}

/// Titan Transnational: Investing In Your Future — Identity: Corp.
/// "Whenever you score an agenda, you may place 1 agenda counter on it."
///
/// COMPLETE. "It" is 1.15.4's back-reference to the card the OCCURRENCE
/// named — the agenda that was just scored — and it is not a target: nothing
/// is announced, because the condition already fixed which card the sentence
/// is about, exactly as an access fixes "the card you are accessing".
///
/// The counter is an AGENDA counter (1.9), which is why it goes on a card in
/// the score area at all: 4.5.1 keeps a scored agenda there as an object, and
/// 9.1.8a keeps its abilities active, so a counter placed on it is something
/// a later ability can spend.
pub fn titan_transnational() -> Card {
    card("Titan Transnational: Investing In Your Future")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Whenever you score an agenda, you may place 1 agenda counter on it.")
        .may_when(corp_scores_agenda(), [place_on(the_triggering_card(), CounterKind::Agenda, 1)])
        .named("investing in your future")
        .build()
}

/// Weyland Consortium: Building a Better World — Identity: Megacorp.
/// "Whenever you play a transaction operation, gain 1[credit]."
///
/// COMPLETE. 8.6's play with two stipulations about the card played, 2.15's
/// type and 2.16's subtype, both riding on the one condition as content. The
/// ability is not once per turn, so every transaction pays.
pub fn weyland_building_a_better_world() -> Card {
    card("Weyland Consortium: Building a Better World")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Megacorp"])
        .text("Whenever you play a transaction operation, gain 1[credit].")
        .when(plays_a_subtyped(Corp, CardType::Operation, "Transaction"), [gain(Corp, 1)])
        .named("building a better world")
        .build()
}

/// Weyland Consortium: Built to Last — Identity: Megacorp.
/// "Whenever you advance a card, gain 2[credit] if it had no advancement
///  counters."
///
/// COMPLETE. CR 1.18.2 is what makes the condition narrow: only an ADVANCE
/// meets it, so a card that merely places an advancement counter (Mushin No
/// Shin class) pays nothing. "If it had no advancement counters" is 9.6.5c's
/// additional requirement inside the condition, asked of the card as it was
/// BEFORE the advance — which is why the second advance of the same card
/// pays nothing and the first pays.
pub fn weyland_built_to_last() -> Card {
    card("Weyland Consortium: Built to Last")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Megacorp"])
        .text("Whenever you advance a card, gain 2[credit] if it had no advancement counters.")
        .when(advances_a_card(true), [gain(Corp, 2)])
        .named("built to last")
        .build()
}

/// Weyland Consortium: Builder of Nations — Identity: Megacorp.
/// "The first time each turn an encounter with an advanced piece of ice ends,
///  do 1 meat damage."
///
/// COMPLETE. 6.5.10's end of an encounter with what the sentence says about
/// the ice — "advanced" is 1.18.2 and nothing more: a card with at least one
/// advancement counter on it. The stipulation is a fact of the encounter
/// end's MOMENT and rides on its record: ice that lost its counters
/// mid-encounter meets nothing, and counters placed or removed later in the
/// turn do not rewrite which encounters were "the times" when 9.6.5c's
/// ordinal re-asks.
///
/// 9.6.5c's ordinal is about the occurrence, so the SECOND advanced piece of
/// ice passed in the same turn does no damage — and an encounter that ends
/// with an unadvanced piece of ice leaves the ordinal unspent, because it
/// never met the condition at all.
pub fn weyland_builder_of_nations() -> Card {
    card("Weyland Consortium: Builder of Nations")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Megacorp"])
        .text("The first time each turn an encounter with an advanced piece of ice ends, do 1 meat damage.")
        .when_first_each_turn(
            encounter_with_advanced_ice_ends(),
            [meat_damage(Corp, 1)],
        )
        .named("builder of nations")
        .build()
}

/// Fringe Applications: Tomorrow, Today — Identity: Division.
/// "Draft format only.
///  If you have more [weyland-consortium] cards rezzed than any other faction,
///  when the Runner's turn begins, place an advancement token on a piece of
///  ice."
///
/// COMPLETE. The format restriction, then one conditional ability with
/// 9.6.5c's additional requirement inside its condition — the faction
/// partition of the rezzed cards, asked when the RUNNER's turn begins, which
/// is a turn the Corp's identity is nonetheless active for (9.1.7).
///
/// "A piece of ice" makes no stipulation about where or whose, so the
/// description is 2.15's card type and nothing else, with 1.15.2c supplying
/// the play area — and only the Corp has ice, so the sentence needs no word
/// about sides. The ability is MANDATORY: with a piece of ice on the board
/// the Corp must place, and with none it does as much as possible (1.15.3),
/// which is nothing.
///
/// 1.18.2: the counter is PLACED, not advanced, so this never meets a
/// "whenever you advance a card" condition — Built to Last stays quiet.
pub fn fringe_applications() -> Card {
    card("Fringe Applications: Tomorrow, Today")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Division"])
        .text("Draft format only.")
        .text("If you have more [weyland-consortium] cards rezzed than any other faction, when the Runner's turn begins, place an advancement token on a piece of ice.")
        .when(
            turn_begins_if(
                Runner,
                &[more_cards_of_this_faction_than_any_other(
                    "Weyland Consortium",
                    &[installed_corp_card(), rezzed()],
                )],
            ),
            [place_on(choose(1, &[of_type(CardType::Ice)]), CounterKind::Advancement, 1)],
        )
        .named("tomorrow, today")
        .build()
}

/// Jemison Astronautics: Sacrifice. Audacity. Success. — Identity: Corp.
/// "Whenever you forfeit an agenda, place X advancement counters on 1
///  installed card. X is equal to the agenda point value of the forfeited
///  agenda plus 1."
///
/// COMPLETE. Two printed sentences, and only the first is an instruction: the
/// second DEFINES X (9.12.2e), which is a calculated quantity and not
/// something that happens. So one conditional ability with one instruction,
/// and X written where the sentence puts it.
///
/// 8.2.5 has already moved the agenda to the removed-from-game zone by the
/// time this resolves — a forfeit is a cost, and 1.16.10b records it during
/// the payment. The value read is therefore the PRINTED one, which is what
/// 1.15.4's "the forfeited agenda" still names after the card has left the
/// score area. An agenda worth 0 still places 1 counter, because the printed
/// "plus 1" is part of the definition and not a floor.
///
/// 1.18.2: the counters are PLACED, not advanced, so this never meets a
/// "whenever you advance a card" condition — which is the whole difference
/// between this identity and an advance-triggered one.
///
/// "1 installed card" makes no stipulation about whose, so 1.15.2c's default
/// is every installed card and a Runner card is as valid a target as a Corp
/// one — the same description Tennin Institute writes.
pub fn jemison_astronautics() -> Card {
    card("Jemison Astronautics: Sacrifice. Audacity. Success.")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Whenever you forfeit an agenda, place X advancement counters on 1 installed card. X is equal to the agenda point value of the forfeited agenda plus 1.")
        .when(
            forfeits_agenda(Corp),
            [place_on_q(
                choose(1, &[]),
                CounterKind::Advancement,
                plus(agenda_points_of(&[the_triggering_card_matching()]), amount(1)),
            )],
        )
        .named("sacrifice, audacity, success")
        .build()
}

/// The Zwicky Group: Invisible Hands — Identity: Unsubstantiated.
/// "The first time each turn you gain credits through an ability on an agenda
///  or operation, you may draw 1 card."
///
/// COMPLETE. 9.1.4 is what makes the sentence sayable: an ability's SOURCE is
/// the card it is on, so "through an ability on an agenda or operation" is a
/// description of that card, asked in the ordinary words. A transaction
/// operation's play ability qualifies, and so does a scored agenda's; the
/// Corp's own basic credit action does not, because 5.2.6b is the PLAYER's
/// action and comes through no card at all.
///
/// 9.12.2b: one instance per OCCURRENCE, so an ability gaining credits twice
/// over meets this twice — which is exactly why the printed ordinal is there,
/// and why it is 9.6.5c's stipulation about the occurrence rather than
/// 9.3.6g's flag (the Pālanā Foods reading).
pub fn the_zwicky_group() -> Card {
    card("The Zwicky Group: Invisible Hands")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Unsubstantiated"])
        .text("The first time each turn you gain credits through an ability on an agenda or operation, you may draw 1 card.")
        .may_when_first_each_turn(
            gains_credits_through(
                Corp,
                &[of_any_type(&[CardType::Agenda, CardType::Operation])],
            ),
            [draw(Corp, 1)],
        )
        .named("invisible hands")
        .build()
}

/// SSO Industries: Fueling Innovation — Identity: Division.
/// "When your turn ends, you may choose a piece of ice with no advancement
///  tokens on it. If you do, place 1 advancement token on that piece of ice
///  for each agenda point on all installed faceup agendas."
///
/// COMPLETE. Two printed sentences and ONE instruction: 9.11.4c says a
/// sentence that only chooses targets and does not act on the choice forms a
/// single instruction with the sentence that follows it. So "choose a piece of
/// ice…" is 1.15.2's target announcement for "place 1 advancement token on
/// that piece of ice", and "if you do" is what 1.15.3 already says — an
/// ability with no legal target announces none and does nothing.
///
/// "With no advancement tokens on it" is the ordinary counter description
/// with 2.15's "non-" negation on it, so an ice the Corp has already advanced
/// this turn is not among the candidates at all.
///
/// The amount is a calculated quantity (9.12.2), read when the instruction
/// resolves, and 1.18.2 makes it PLACED rather than advanced — so this never
/// meets a "whenever you advance a card" condition, and a Built-to-Last-class
/// identity stays quiet. "All installed faceup agendas" is the pair 8.1.2
/// makes meaningful: a Corp card is installed facedown unless something says
/// otherwise, so the usual answer is 0 and the sentence is waiting for a card
/// that installs an agenda faceup. An agenda in a SCORE area is faceup but
/// not installed, and the printed word is what keeps it out.
pub fn sso_industries() -> Card {
    card("SSO Industries: Fueling Innovation")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Division"])
        .text("When your turn ends, you may choose a piece of ice with no advancement tokens on it. If you do, place 1 advancement token on that piece of ice for each agenda point on all installed faceup agendas.")
        .may_when(
            turn_ends(Corp),
            [place_on_q(
                choose(
                    1,
                    &[of_type(CardType::Ice), non(with_counters(CounterKind::Advancement, 1))],
                ),
                CounterKind::Advancement,
                times(
                    1,
                    agenda_points_of(&[
                        installed_corp_card(),
                        of_type(CardType::Agenda),
                        non(facedown()),
                    ]),
                ),
            )],
        )
        .named("fueling innovation")
        .build()
}

/// Weyland Consortium: Because We Built It — Identity: Megacorp.
/// "1[recurring-credit]
///  Use this credit to advance ice."
///
/// COMPLETE. The first line is 1.10.5's shorthand — one credit on the identity
/// from the moment it is active, which for an identity is 1.6, and refilled at
/// step 5.6.1c of every Corp turn (1.10.5d).
///
/// The second is 1.10.3c, and it names a CARD rather than a moment: the credit
/// pays for advancing, and 1.18.1 makes advancing "placing an advancement
/// counter on a card by paying for it", so the card being described is the one
/// the counter is going on. The description is the ordinary vocabulary, and
/// 1.15.2c's default is what makes the printed "ice" reach the installed
/// pieces of ice with nothing else written beside it.
///
/// The payment it is allowed for is 5.2.6f's basic action, the only one that
/// pays to advance. A card ability that advances is paid for under 9.1.6a —
/// the payment is for USING that card, not for advancing — so this credit is
/// not offered there, which is what keeps the sentence to what it says.
pub fn weyland_because_we_built_it() -> Card {
    card("Weyland Consortium: Because We Built It")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Megacorp"])
        .text("1[recurring-credit]")
        .text("Use this credit to advance ice.")
        .recurring_credits(1)
        .credits_only_for_advancing(&[of_type(CardType::Ice)])
        .build()
}

/// Nuvem SA: Law of the Land — Identity: Megacorp.
/// "Whenever you finish resolving an operation or an action on an expendable
///  card, look at the top card of R&D. You may trash that card.
///  The first time you trash a card from R&D during each of your turns, gain
///  2[credit]."
///
/// COMPLETE. Two printed lines, two abilities, and the first is what most
/// often meets the second.
///
/// The first line's "or" joins two OCCURRENCES, so it is one condition with
/// two alternatives (9.6.1a gives an ability one primary condition) rather
/// than two abilities — the same reading Epiphany Analytica's "steals or
/// trashes" gets. The halves are genuinely different moments and neither is
/// the other: 8.6.7h is the step where "conditions related to finishing
/// resolving" a played card are met, and 5.2.2d is where an ACTION finishing
/// is, which 5.2.2a puts at the end of the action step that ran it. Playing
/// an operation with 5.2.6e's basic action meets the first half only — the
/// description on the second half is about a card, and 9.1.3 makes a basic
/// action's source a game rule.
///
/// "An operation" is a description of the card played, asked of the card even
/// though step (g) has already trashed it: a description reads the card's own
/// characteristics and those travel with it to Archives. "You" is 8.6.2's
/// player, read off the occurrence, because 1.14.1's owner is a different
/// question and the card is no longer anywhere that would answer it.
///
/// "That card" in the second sentence is the card the first looked at (1.21.2)
/// — named by description, so the ability offers no choice at all, there being
/// exactly one. The printed "you may" is 9.6.9's, governing the trash alone:
/// the looking is not optional.
///
/// The second line's ordinal is 9.6.5c's, and "during each of **your** turns"
/// is the other half of it: the ordinal counts inside whichever turn is being
/// played, so the sentence states whose turn it must be (9.2.1) as well. The
/// zone is the one the card left, which for a card the first ability trashed
/// off the top of R&D is R&D — so the two lines feed each other, once a turn.
pub fn nuvem_sa() -> Card {
    card("Nuvem SA: Law of the Land")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Whenever you finish resolving an operation or an action on an expendable card, look at the top card of R&D. You may trash that card.")
        .text("The first time you trash a card from R&D during each of your turns, gain 2[credit].")
        .when(
            either_of(&[
                finishes_resolving_a_played_card(Corp, &[of_type(CardType::Operation)]),
                finishes_an_action_on(Corp, &[with_subtype("Expendable")]),
            ]),
            [
                look_at(top_of_rnd(amount(1)), Corp),
                may(trash(all_matching(&[looked_at_by_this_ability()]))),
            ],
        )
        .named("law of the land")
        .when_first_each_turn(
            trashes_a_card_from(Corp, Zone::Deck(Corp), &[during_the_turn_of(Corp)]),
            [gain(Corp, 2)],
        )
        .named("the first time you trash a card from R&D")
        .build()
}

/// Blue Sun: Powering the Future — Identity: Corp.
/// "When your turn begins, you may add 1 rezzed card to HQ and gain credits
///  equal to its rez cost."
///
/// COMPLETE. One sentence, so one instruction (9.11.3): the "and" is
/// `combined`, and splitting it would invent a checkpoint, a reaction window
/// and an interrupt window the card does not print. The "you may" governs the
/// whole of it (9.6.9), which is `may_when`.
///
/// "1 rezzed card" is 8.1.2's installed faceup Corp card — the identity
/// itself is never installed — and it is a TARGET: chosen by the Corp and
/// announced where 1.15.2 puts every announcement, before the sentence
/// resolves. That is what makes "its" readable at all: 1.15.4's
/// back-reference asked for a number (`rez_cost_of_earlier_choice`, the
/// pointing twin of Nasir's encountered-ice quantity) reads the announced
/// card while the sentence's first half has yet to move it.
///
/// The rez cost is the card's printed, inherent cost (1.16.4a) — not a record
/// of what the Corp once paid for the rez, and not 0 because the card will be
/// unrezzed by the time it sits in HQ: the number is read off the announced
/// card, and printed numbers travel with it. "Add" is 8.2.1a's non-movement,
/// an ordinary move to HQ.
pub fn blue_sun() -> Card {
    card("Blue Sun: Powering the Future")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("When your turn begins, you may add 1 rezzed card to HQ and gain credits equal to its rez cost.")
        .may_when(
            turn_begins(Corp),
            [combined([
                add_to_hand(choose(1, &[rezzed()])),
                gain_q(Corp, rez_cost_of_earlier_choice(0)),
            ])],
        )
        .named("powering the future")
        .build()
}

/// Earth Station: SEA Headquarters — Identity: Division.
/// "Limit 1 remote server.
///  As an additional cost to run HQ, the Runner must pay 1[credit].
///  [click]: Flip this identity."
///
/// COMPLETE, both faces. Three printed lines: two static declarations and a
/// paid ability.
///
/// "Limit 1 remote server" is 4.6.8f, the same declaration A Teia writes with
/// a 2 (`StaticDecl::RemoteServerLimit`): a restriction (9.3.4) on install
/// DESTINATIONS, read at step 8.5.16b — with one remote up, a new remote is
/// not a destination the Corp may declare, and an install with no other
/// destination identifies none at all (8.5.14).
///
/// The second line is 1.16.10 / 6.3.4's additional cost to the basic run
/// ACTION (`StaticDecl::AdditionalRunActionCost`), with the server the
/// sentence names as the declaration's stipulation — 6.9.1a announces the
/// attacked server before any cost is paid, which is what makes "to run HQ"
/// askable at all. Runs on R&D, Archives and the remote pay nothing.
/// 1.16.10a: the Runner may decline, and declining means the action is not
/// taken — the [click] is not spent either (1.16.4c's shape).
///
/// The third is an ordinary paid ability (9.5); its whole cost is the
/// [click], and `Instruction::FlipIdentity` is rule_identity_double_sided's
/// turn-over: the 10.3.1a checkpoint after it re-derives every ability from
/// the face now showing, so the back's declarations take over at once.
pub fn earth_station_sea_headquarters() -> Card {
    card("Earth Station: SEA Headquarters")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Division"])
        .text("Limit 1 remote server.")
        .text("As an additional cost to run HQ, the Runner must pay 1[credit].")
        .text("[click]: Flip this identity.")
        .declares([remote_server_limit(1)])
        .named("limit 1 remote server")
        .declares([additional_cost_to_run(&[ServerId::Hq], credits(1))])
        .named("the toll on HQ")
        .paid(clicks(1), [flip_identity(Corp)])
        .named("flip")
        .flip_face(earth_station_ascending_to_orbit())
        .build()
}

/// Earth Station: Ascending to Orbit — Identity: Division; the back face of
/// Earth Station: SEA Headquarters (oracle: netrunner-cards-json v2,
/// `faces[0]`).
/// "Limit 1 remote server.
///  As an additional cost to run a remote server, the Runner must pay
///  6[credit].
///  When the Runner makes a successful run on HQ, flip this identity."
///
/// The first line is the front's, printed again — the limit holds on both
/// sides. The second is the same `AdditionalRunActionCost` declaration with
/// the OTHER stipulation a sentence can make about the server: 4.6.8's
/// remotes are a class the game state computes, never a list the card could
/// print, so the declaration carries the class itself
/// (`RunServerSet::AnyRemote`) and a run on any central pays nothing.
///
/// The third is the Gemilang Arena shape: 6.8.4's successful run, stipulated
/// to HQ, met exactly when the Runner would rather be taxed again — flipping
/// home is mandatory, which is the card's whole bargain.
pub fn earth_station_ascending_to_orbit() -> Card {
    card("Earth Station: Ascending to Orbit")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Division"])
        .text("Limit 1 remote server.")
        .text("As an additional cost to run a remote server, the Runner must pay 6[credit].")
        .text("When the Runner makes a successful run on HQ, flip this identity.")
        .declares([remote_server_limit(1)])
        .named("limit 1 remote server")
        .declares([additional_cost_to_run_a_remote_server(credits(6))])
        .named("the toll on remotes")
        .when(makes_successful_run_on(&[ServerId::Hq]), [flip_identity(Corp)])
        .named("flip back")
        .build()
}

/// Every Weyland Consortium identity this module carries, in the order the
/// queue reached them.
/// BANGUN: When Disaster Strikes — Identity: Corp.
/// "You may install agendas faceup. (This does not make their abilities
///  active.) Whenever the Runner accesses a faceup installed agenda, do 2
///  meat damage and give the Runner 1 tag."
///
/// COMPLETE. The first sentence is a PERMISSION, not an install ability: it
/// is the opposite number of Apex's "you cannot install" declaration, stated
/// about every install its controller performs — 5.2.6d's basic action
/// included — and wanting the one thing that prohibition does not: a
/// decision. Where 8.5.2 settles a Corp card's face facedown with nobody
/// asked, the installer is asked at step 8.5.16a, and their answer is "the
/// status it will have when the installation is complete". It is not
/// 4.6.4d's stipulation, which the installing ability itself states and
/// which asks no one.
///
/// The parenthetical restates the CR's own arrangement rather than adding
/// one: 8.1.1 leaves a faceup agenda "neither rezzed nor unrezzed", and
/// 3.2.3 keeps an agenda inactive while installed however it faces — so the
/// faceup install changes the status and nothing else. (3.2.3a's exception
/// is an agenda whose OWN printed text directs the faceup install; a third
/// card's permission is not that.) What the status DOES change is what
/// other sentences can see: SSO Industries' "all installed faceup agendas"
/// stops answering 0, and this card's second sentence exists.
///
/// The second sentence is 7.3's access with the sentence's words about the
/// card — "faceup installed" — as criteria on the occurrence, asked of the
/// card's state at the access itself (the scan runs before 7.3.4's steal
/// moves it, so the damage and tag land and the steal still completes). One
/// sentence, so one instruction (9.11.3): the "and" is `combined`. "Do 2
/// meat damage" is 10.4.1's Corp-does branch, exactly Builder of Nations'
/// verb; a facedown-installed agenda meets nothing, and so does a faceup
/// agenda accessed anywhere it is not installed (HQ, R&D, Archives).
pub fn bangun() -> Card {
    card("BANGUN: When Disaster Strikes")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("You may install agendas faceup. (This does not make their abilities active.)")
        .text("Whenever the Runner accesses a faceup installed agenda, do 2 meat damage and give the Runner 1 tag.")
        .declares([may_install_faceup(&[of_type(CardType::Agenda)])])
        .when(
            accesses_a_matching(
                CardType::Agenda,
                &[installed_corp_card(), non(facedown())],
            ),
            [combined([meat_damage(Corp, 2), give_tags(1)])],
        )
        .named("when disaster strikes")
        .build()
}


/// Ob Superheavy Logistics: Extract. Export. Excel. — Identity: Corp.
/// "Once per turn → When you trash a rezzed card, except during installation,
///  you may search R&D for 1 card with a printed rez cost exactly 1[credit]
///  less than the trashed card's printed rez cost. Install and rez the card
///  you found, ignoring credit costs."
///
/// COMPLETE. "Once per turn →" is 9.3.6g's flag, and it has something to
/// spend it with because the ability is OPTIONAL (9.1.6's second sentence —
/// the Mercury shape). The condition stipulates two facts about the MOMENT
/// of the trash, both read from the record because the state can no longer
/// answer either: 8.1.2's "rezzed" (the card was a faceup installed Corp
/// card then, whatever 10.3.1a has done to it since) and "except during
/// installation", which is 8.5.11a's like-card trash — the one the install
/// procedure itself performs.
///
/// The printed line splits where 9.11.4 says it does: (d) a search is its
/// own instruction, and the second sentence is the install. "A printed rez
/// cost exactly 1[credit] less than the trashed card's" is a relational
/// question about a printed number (2.3), asked of the card the occurrence
/// named (1.15.4) the way The Foundry asks about the name — both sides must
/// HAVE a printed rez cost (8.1.2's assets, ice and upgrades), so an
/// operation or an agenda matches nothing, and 8.7.2b narrows the find to
/// what the install could take.
///
/// "Ignoring credit costs" selects cost components by KIND, cutting across
/// 1.16.4's inherent/additional split: the install cost, the rez cost and
/// the credit part of an additional cost all become 0, while an
/// Archer-class forfeit is still paid — which is why it is not 1.16.5c's
/// "ignoring all costs". The card states no destination, so the Corp
/// declares one at step 8.5.16b (8.5.16's "or as normal").
pub fn ob_superheavy_logistics() -> Card {
    card("Ob Superheavy Logistics: Extract. Export. Excel.")
        .corp()
        .identity()
        .faction("Weyland Consortium")
        .subtypes(&["Corp"])
        .text("Once per turn → When you trash a rezzed card, except during installation, you may search R&D for 1 card with a printed rez cost exactly 1[credit] less than the trashed card's printed rez cost. Install and rez the card you found, ignoring credit costs.")
        .may_when_once_per_turn(
            trashes_a_rezzed_card_except_during_install(Corp),
            [
                search_rnd(&[rez_cost_exactly_less_than_the_triggering_cards(1)], 1),
                install_and_rez_found_ignoring_credit_costs(),
            ],
        )
        .named("extract export excel")
        .build()
}


pub fn identities() -> Vec<Card> {
    vec![
        sso_industries(),
        jemison_astronautics(),
        the_zwicky_group(),
        fringe_applications(),
        argus_security(),
        weyland_builder_of_nations(),
        gagarin_deep_space(),
        grndl(),
        the_outfit(),
        titan_transnational(),
        weyland_building_a_better_world(),
        weyland_built_to_last(),
        weyland_because_we_built_it(),
        nuvem_sa(),
        blue_sun(),
        earth_station_sea_headquarters(),
        bangun(),
    ob_superheavy_logistics(),
    ]
}
