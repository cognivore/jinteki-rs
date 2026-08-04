//! Runner — Criminal identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). Andromeda and Ken Tenma are the two Criminals that
//! already exist — they live in `decks/andromeda.rs`, because that deck plays
//! one and carries the other in its CR 1.5.4a pile — and everything else in
//! the faction lands here.

use crate::edsl::*;

/// 419: Amoral Scammer — Identity: Natural. Link 1.
/// "The first time the Corp installs a card each turn, you may expose that
///  card unless the Corp pays 1[credit]."
///
/// COMPLETE. 8.5's install with no stipulation about what was installed, and
/// 9.6.5c's ordinal about the occurrence.
///
/// "That card" is 1.15.4's back-reference to the card the OCCURRENCE named,
/// so nothing is announced — which is also what makes the sentence sayable
/// at all. 1.21.4 restricts exposing to installed UNREZZED cards, and a bare
/// "expose 1 card" would have to describe candidates, which is words the card
/// does not print; here the condition fixes the card and the restriction is
/// simply honoured when the exposure resolves.
///
/// The printed "you may" is the whole ability, so it is 9.6.9's declinable
/// conditional and the decision is the Runner's (9.1.1a); "unless the Corp
/// pays 1[credit]" is 1.16.9's alternative cost put to the player the
/// sentence names, so the Corp answers second and only if the Runner said
/// yes.
pub fn amoral_scammer() -> Card {
    card("419: Amoral Scammer")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .link(1)
        .text("The first time the Corp installs a card each turn, you may expose that card unless the Corp pays 1[credit].")
        .may_when_first_each_turn(
            installs_a_card(Corp),
            [unless_pays(Corp, credits(1), expose(the_triggering_card()))],
        )
        .named("the first corp install of the turn")
        .build()
}

/// Armand "Geist" Walker: Tech Lord — Identity: G-mod. Link 1.
/// "Whenever you use a [trash] ability, draw 1 card."
///
/// COMPLETE. The [trash] symbol is 1.19.4: a trigger cost that trashes the
/// ability's own source. That is NOT 7.1.5's basic trash ability, where the
/// Runner pays an accessed card's trash cost — two different abilities, and
/// the sentence names only the first, so the condition stipulates which.
pub fn armand_geist_walker() -> Card {
    card("Armand \"Geist\" Walker: Tech Lord")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .link(1)
        .text("Whenever you use a [trash] ability, draw 1 card.")
        .when(uses_a_trash_symbol_ability(Runner), [draw(Runner, 1)])
        .named("tech lord")
        .build()
}

/// Barry "Baz" Wong: Tri-Maf Veteran — Identity: Cyborg. Link 0.
/// "Whenever the Corp rezzes a piece of ice, you may install 1 resource or
///  piece of hardware from your grip."
///
/// COMPLETE. The condition is Los's — 8.1.2's rez with the sentence's
/// card-type stipulation — without the ordinal, so every ice rez offers it.
///
/// "1 resource **or** piece of hardware" is 2.15's type list as one
/// description word. It cannot be two: a card has exactly one type, so two
/// type words beside each other would mean a card that is both, which is no
/// card at all.
pub fn barry_baz_wong() -> Card {
    card("Barry \"Baz\" Wong: Tri-Maf Veteran")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Cyborg"])
        .text("Whenever the Corp rezzes a piece of ice, you may install 1 resource or piece of hardware from your grip.")
        .may_when(
            corp_rezzes_a(CardType::Ice),
            [install(
                choose(
                    1,
                    &[
                        in_hand_of(Runner),
                        of_any_type(&[CardType::Resource, CardType::Hardware]),
                    ],
                ),
                InstallDest::RunnerChoiceHostOrRig,
            )],
        )
        .named("tri-maf veteran")
        .build()
}

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

/// Iain Stirling: Retired Spook — Identity: Natural. Link 1.
/// "When your turn begins, gain 2[credit] if the Corp has more scored agenda
///  points than you."
///
/// COMPLETE. The "if …" clause is 9.6.5c's additional requirement listed
/// inside the trigger condition, so it is asked as the turn begins and not
/// again while the ability resolves.
///
/// It compares the two SCORE AREAS (1.17.1) rather than testing one against a
/// printed number, and "more … than" is strict: a tie leaves the requirement
/// unmet and the identity pays nothing.
pub fn iain_stirling() -> Card {
    card("Iain Stirling: Retired Spook")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .link(1)
        .text("When your turn begins, gain 2[credit] if the Corp has more scored agenda points than you.")
        .when(turn_begins_if(Runner, &[agenda_points_ahead(Corp)]), [gain(Runner, 2)])
        .named("retired spook")
        .build()
}

/// Silhouette: Stealth Operative — Identity: Natural. Link 0.
/// "The first time you make a successful run on HQ each turn, you may expose
///  1 card."
///
/// COMPLETE. Gabriel Santiago's condition exactly, and the printed "you may"
/// is the whole ability, so 9.6.9 puts the decision with the Runner.
///
/// "1 card" describes nothing else, and it does not have to: CR 1.21.4
/// restricts exposing to installed cards that are not rezzed, and that
/// restriction is the INSTRUCTION's rather than the card's words, so it
/// narrows the candidates without being written here. Writing it into the
/// description would be words the card does not print.
pub fn silhouette() -> Card {
    card("Silhouette: Stealth Operative")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("The first time you make a successful run on HQ each turn, you may expose 1 card.")
        .may_when_first_each_turn(
            makes_successful_run_on(&[ServerId::Hq]),
            [expose(choose(1, &[]))],
        )
        .named("the first hq run of the turn")
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

/// Leela Patel: Trained Pragmatist — Identity: Natural. Link 0.
/// "Whenever an agenda is scored or stolen, add 1 unrezzed card to HQ."
///
/// COMPLETE. One printed sentence with two conditions, so it is two
/// conditional abilities with the same effect (9.6.1: a card may have several)
/// — the same shape The Source's "trash this when an agenda is scored or
/// stolen" already takes. Writing it as one ability is not available: 9.6.1
/// gives an ability ONE primary condition, and an agenda being scored
/// (1.17.3a) and one being stolen (1.17.3b) are different occurrences.
///
/// "1 unrezzed card" is 8.1.2's other half — an installed facedown Corp card.
/// It reaches an installed agenda too, which can never be rezzed at all, and
/// it is not a stipulation about a CHARACTERISTIC (rez state is open
/// information, 1.12.1), so nothing is revealed by choosing one.
///
/// The choice belongs to the ability's controller, which 9.1.1a makes the
/// Runner for a Runner identity — even on the half that fires when the CORP
/// scores.
pub fn leela_patel() -> Card {
    card("Leela Patel: Trained Pragmatist")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Natural"])
        .text("Whenever an agenda is scored or stolen, add 1 unrezzed card to HQ.")
        .when(corp_scores_agenda(), [add_to_hand(choose(1, &[unrezzed()]))])
        .named("an agenda was scored")
        .when(runner_steals_agenda(), [add_to_hand(choose(1, &[unrezzed()]))])
        .named("an agenda was stolen")
        .build()
}

/// Nyusha "Sable" Sintashta: Symphonic Prodigy — Identity: G-mod. Link 0.
/// "When your turn begins, identify your mark. (If you don’t have a mark, a
///  random central server becomes your mark for this turn.)
///  The first time each turn you make a successful run on your mark, gain
///  [click]."
///
/// COMPLETE. The parenthesis is 1.4's reminder text: it restates 10.11.2a's
/// method and 10.11.3's "if one already is, nothing happens", both of which
/// "identify your mark" already is, so it is not a second instruction.
///
/// The ordinal on the second sentence rides on the CONDITION rather than on
/// 9.6.5c's usual stipulation, because 10.11.5 counts it from a different
/// moment: a condition checking a property related to the mark only checks
/// from the designation, so a successful run on that same server earlier in
/// the turn — before it was the mark — is not one of the times counted.
pub fn nyusha_sintashta() -> Card {
    card("Nyusha \"Sable\" Sintashta: Symphonic Prodigy")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["G-mod"])
        .text("When your turn begins, identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.)")
        .text("The first time each turn you make a successful run on your mark, gain [click].")
        .when(turn_begins(Runner), [identify_mark()])
        .named("identify your mark")
        .when(makes_successful_run_on_your_mark(true), [gain_clicks(Runner, 1)])
        .named("the first run on the mark")
        .build()
}

/// Virtual Intelligence, P.I.: "You Can Call Me Vic" — Identity: Digital.
/// Link 0.
/// "Once per turn → [click], 1[credit]: Draw 1 card and remove 1 tag."
///
/// COMPLETE. Everything printed before the colon is the cost — a click and a
/// credit, one cost with two components (1.16.2) — and "Once per turn →" is
/// 9.3.6g's flag, which a PAID ability has something to spend it with (9.1.6:
/// a player uses a paid ability, which is what expends the flag).
///
/// "Draw 1 card and remove 1 tag" is one printed sentence, so one instruction
/// (9.11.3): joining them with `combined` keeps the single checkpoint and the
/// single interrupt window the card gives. The ability is usable with no tags
/// to remove — 9.5.3 only asks that the cost be payable.
pub fn virtual_intelligence_pi() -> Card {
    card("Virtual Intelligence, P.I.: \"You Can Call Me Vic\"")
        .runner()
        .identity()
        .faction("Criminal")
        .subtypes(&["Digital"])
        .text("Once per turn → [click], 1[credit]: Draw 1 card and remove 1 tag.")
        .paid_once_per_turn(
            clicks(1).plus_cost(credits(1)),
            [combined([draw(Runner, 1), remove_tags(1)])],
        )
        .named("draw 1 card and remove 1 tag")
        .build()
}

/// Every Criminal identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        amoral_scammer(),
        armand_geist_walker(),
        barry_baz_wong(),
        gabriel_santiago(),
        iain_stirling(),
        silhouette(),
        los(),
        liza_talking_thunder(),
        laramy_fisk(),
        leela_patel(),
        nyusha_sintashta(),
        virtual_intelligence_pi(),
    ]
}
