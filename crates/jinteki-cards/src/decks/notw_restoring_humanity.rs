//! Deck of the week — "Boring.dec", Jinteki: Restoring Humanity.
//!
//! NetrunnerDB #97714, King of Swiss and 12th overall at Cascadia. The list is
//! in `docs/vm/DECK-OF-THE-WEEK.md`; this module is its Corp half.
//!
//! Printed text is copied from NSG's official card data. Behaviour is written
//! from that text and from nowhere else (SYS-D-10): the doc comment above each
//! card carries the text for whoever is reading, `.text(…)` carries the same
//! text as data for whatever is checking, and `tests/decks.rs` asserts the two
//! agree. Sentences the vocabulary cannot say yet carry `.unimplemented(…)`
//! rather than an approximation, and the kernel capability each one waits on
//! is on the Blockers list in `docs/vm/DECK-OF-THE-WEEK.md`.
//!
//! Five of the sixteen distinct cards are written elsewhere and REUSED, never
//! copied: the identity out of the identity queue, Hedge Fund and Seamless
//! Launch out of the Gauntlet, Spin Doctor and Tatu-Bola out of Mezzie's Asa.
//! [`deck`] lists them from there.

use crate::edsl::*;

// ---------------------------------------------------------------------------
// Agendas
// ---------------------------------------------------------------------------

/// Fujii Asset Retrieval — Agenda: Ambush - Security. 5/3.
/// "When this agenda is scored or stolen, do 2 net damage."
///
/// COMPLETE.
///
/// TWO abilities for one printed sentence, which is Tomorrow's Headline's
/// identical sentence written the way that card already writes it — and it is
/// not a stylistic choice. 9.6.5 does let ONE condition describe several kinds
/// of occurrence, but 9.1.8b is what decides this card: 4.5.4 leaves an agenda
/// in the Runner's score area inactive, and the exception that reaches it is
/// "abilities that can only ever meet their conditions in a particular zone
/// are active in that zone". A steal condition can only ever be met in the
/// Runner's score area (1.17.3/1.17.7), so an ability carrying it alone is
/// active there; a condition that is met by EITHER a score or a steal names no
/// single zone and is read as reaching none, so the steal half of a
/// disjunctive spelling would be switched off in the only zone it can happen
/// in. The general capability that would let the sentence be one ability is on
/// the Blockers list.
///
/// "Do 2 net damage" is the Corp's damage in both arms (10.4.1: the side named
/// is who is RESPONSIBLE), including the arm the Runner set off by stealing —
/// 1.17.7's "when stolen" abilities are the Corp's card speaking.
pub fn fujii_asset_retrieval() -> Card {
    card("Fujii Asset Retrieval")
        .corp()
        .agenda(5, 3)
        .faction("Jinteki")
        .subtypes(&["Ambush", "Security"])
        .text("When this agenda is scored or stolen, do 2 net damage.")
        .when(scored(), [net_damage(Corp, 2)])
        .named("two net on the score")
        .when(stolen(), [net_damage(Corp, 2)])
        .named("two net on the steal")
        .build()
}

/// Proprionegation — Agenda: Security. 4/2.
/// "When you score this agenda, place 1 agenda counter on it.
///  Hosted agenda counter: The Runner moves to the outermost position of
///  Archives. (They approach any ice in that position.) Use this ability only
///  during a run."
///
/// COMPLETE.
///
/// The first sentence is 1.9.5e's agenda counter, PLACED (1.18.2) rather than
/// loaded — nothing about this card asks "when it is empty" — on the agenda
/// itself, which by then is in the Corp's score area. 4.5.4 leaves cards in
/// the Corp's score area active, so the paid ability below is offered from
/// there without anything having to state a zone.
///
/// The second line is one paid ability (9.5.1) with three printed clauses.
/// The cost is everything before the colon, 1.16.10's trigger cost paid by
/// spending the counter the first sentence placed — so the agenda is worth
/// exactly one use, and a second attempt finds no counter and is not offered
/// (1.16.1b).
///
/// "The Runner moves to the outermost position of Archives" is 6.2.8b, which
/// the CR states over a POSITION and not over a card: Archives becomes the
/// attacked server (6.1.2d follows from the move rather than replacing it),
/// and with ice protecting Archives the Runner's position becomes the
/// outermost piece's and the run's timing step becomes the Approach Ice Phase.
/// With none, the Runner ceases to have a position and the step becomes the
/// Movement Phase. The parenthetical is 1.4 reminder text for exactly that,
/// and denotes into nothing of its own.
///
/// "Use this ability only during a run" is 9.3.3c's restriction on WHEN,
/// with 6.1.1's span for its content — from the run's initiation to the end of
/// its Run Ends Phase. It is load-bearing twice over: without it the Corp
/// could spend the counter in their own action phase, where 6.2.8b has no run
/// to move anyone inside of, and the counter would be gone.
pub fn proprionegation() -> Card {
    card("Proprionegation")
        .corp()
        .agenda(4, 2)
        .faction("Jinteki")
        .subtypes(&["Security"])
        .text("When you score this agenda, place 1 agenda counter on it.")
        .text("Hosted agenda counter: The Runner moves to the outermost position of Archives. (They approach any ice in that position.) Use this ability only during a run.")
        .when(scored(), [place(CounterKind::Agenda, 1)])
        .named("one counter on the score")
        .paid_during_a_run(
            hosted_counters(CounterKind::Agenda, 1),
            [runner_moves_to_outermost_of(ServerId::Archives)],
        )
        .named("sent back out to archives")
        .build()
}

/// Send a Message — Agenda: Security. 5/3.
/// "When this agenda is scored or stolen, you may rez 1 installed piece of
///  ice, ignoring all costs."
///
/// COMPLETE. Two abilities for one printed sentence, for the reason
/// [`fujii_asset_retrieval`] gives at length: 9.1.8b reaches the Runner's
/// score area only for a condition that can be met nowhere else.
///
/// The printed "you may" governs the whole ability (9.6.9), so a Corp who
/// declines rezzes nothing — and "you" is the Corp in both arms, including the
/// one the Runner set off, because 1.14.4 gives an ability to the controller
/// of its source and 8.1.2 lets only the Corp rez at all.
///
/// "1 installed piece of ice" is a 1.15.2 announcement over two ordinary
/// description words; 1.15.2c's default is installed cards, so "installed"
/// needs no criterion of its own. 8.1.1's "only an unrezzed card can be
/// rezzed" is NOT a stipulation on the announcement — the CR states such a
/// restriction where it means one (1.15.2's charge rule names cards with a
/// hosted power counter) and states none here — so a Corp who announces a
/// rezzed piece of ice rezzes nothing, which is 1.2.3 doing as much as
/// possible.
///
/// "Ignoring all costs" is 1.16.5c on the rez: the inherent rez cost goes,
/// and with it any 1.16.10 additional cost to rez.
pub fn send_a_message() -> Card {
    card("Send a Message")
        .corp()
        .agenda(5, 3)
        .faction("Neutral")
        .subtypes(&["Security"])
        .text("When this agenda is scored or stolen, you may rez 1 installed piece of ice, ignoring all costs.")
        .may_when(
            scored(),
            [rez_ignoring_all_costs(choose(1, &[of_type(CardType::Ice)]))],
        )
        .named("a free rez on the score")
        .may_when(
            stolen(),
            [rez_ignoring_all_costs(choose(1, &[of_type(CardType::Ice)]))],
        )
        .named("a free rez on the steal")
        .build()
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// Charlotte Caçador — Asset: Clone. Rez 0, trash 2. ◆
/// "You can advance this asset.
///  When your turn begins, you may remove 1 hosted advancement counter to gain
///  4[credit] and draw 1 card.
///  [trash], hosted advancement counter: Gain 3[credit]."
///
/// COMPLETE. Three printed lines and three different kinds of thing.
///
/// "You can advance this asset" is 1.18.3's permission, which is permanently
/// true rather than something that happens — a static declaration (9.4). It is
/// also one of 9.1.8's stated exceptions (9.1.8f: "abilities that allow their
/// source card to be advanced are active while that card is installed"), so an
/// UNREZZED Charlotte can be advanced, which is what the card is for.
///
/// The second line is Rashida Jaheem's shape: 1.16.11a's nested cost is the
/// printed "you may", so the Corp who declines spends nothing, and the two
/// halves after "to" are ONE instruction (9.11.3 — "gain 4[credit] and draw 1
/// card" is a single sentence joined by "and", and splitting it would invent a
/// checkpoint and a reaction window between the credits and the draw). The
/// cost removes the counter, which 1.9.2 returns to the bank; 1.18.2 keeps
/// that apart from advancing, so nothing here meets a "whenever you advance"
/// condition.
///
/// The third line's cost is everything before the colon and it is TWO
/// components paid together (1.16.10): the [trash] symbol and one hosted
/// advancement counter. Both are required, so a Charlotte with no counters
/// cannot be cashed in (1.16.1b) — which is the difference between this card
/// and a plain trash-for-3.
pub fn charlotte_cacador() -> Card {
    card("Charlotte Caçador")
        .corp()
        .asset()
        .faction("Jinteki")
        .subtypes(&["Clone"])
        .cost(0)
        .trash_cost(2)
        .unique()
        .text("You can advance this asset.")
        .text("When your turn begins, you may remove 1 hosted advancement counter to gain 4[credit] and draw 1 card.")
        .text("[trash], hosted advancement counter: Gain 3[credit].")
        .declares([can_be_advanced()])
        .named("you can advance her")
        .when(
            turn_begins(Corp),
            [may_pay(
                hosted_counters(CounterKind::Advancement, 1),
                combined([gain(Corp, 4), draw(Corp, 1)]),
            )],
        )
        .named("cash a counter for four and a card")
        .paid(
            trash_this_card().plus_cost(hosted_counters(CounterKind::Advancement, 1)),
            [gain(Corp, 3)],
        )
        .named("cash her in for three")
        .build()
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Hansei Review — Operation: Transaction. Cost 5.
/// "Gain 10[credit]. If there are any cards in HQ, trash 1 of them."
///
/// COMPLETE. One printed line, TWO sentences, and therefore two instructions
/// (9.11.3) of the one play ability — the gain, then the trash, with a
/// checkpoint between them exactly where the full stop is.
///
/// "If there are any cards in HQ" is 9.6.5d's requirement stated inside the
/// instructions rather than on a trigger, and it is asked AFTER the gain has
/// resolved, which is the whole reason the order of the two sentences matters
/// on a card that can be the last one in hand: a Corp who plays Hansei Review
/// as their only card has an empty HQ by the time the second sentence looks,
/// and trashes nothing. "Cards in HQ" names its zone, which is what lifts
/// 1.15.2c's installed-cards default for both the question and the target.
///
/// "Trash 1 of them" is the Corp's own choice among their own hidden zone
/// (4.3): a 1.15.2 announcement, so the card is chosen when the instruction
/// becomes imminent and 1.15.2e forces one if any exist.
pub fn hansei_review() -> Card {
    card("Hansei Review")
        .corp()
        .operation()
        .faction("Jinteki")
        .subtypes(&["Transaction"])
        .cost(5)
        .text("Gain 10[credit]. If there are any cards in HQ, trash 1 of them.")
        .play([
            gain(Corp, 10),
            if_met(
                &[board_has(&[in_hand_of(Corp)], 1)],
                [trash(choose(1, &[in_hand_of(Corp)]))],
            ),
        ])
        .build()
}

// ---------------------------------------------------------------------------
// Upgrades
// ---------------------------------------------------------------------------

/// Anoetic Void — Upgrade. Rez 0, trash 1. ◆
/// "Whenever the Runner approaches this server, you may pay 2[credit] and
///  trash 2 cards from HQ. If you do, end the run."
///
/// UNIMPLEMENTED: the card's only printed line, on its trigger.
///
/// "Approaches THIS server" is 6.9.4g's step with the server as a
/// stipulation, and the approach condition takes none — the same wall
/// Manegarm Skunkworks is already stopped at. The kernel's three other
/// run conditions about a server carry it (a successful run on this server, a
/// run on this server ending, this server being breached each compare the
/// attacked server against the source's), but the approach was written for the
/// Formicary class, whose sentence names A server and means every one of them.
/// Written with the word that exists, a rezzed copy in a remote would end runs
/// on HQ and R&D for two credits — not a smaller card than the printed one but
/// a very much larger one, so the marker is the right answer even though every
/// other word of the sentence exists.
///
/// The rest is already sayable and is recorded here so the wave that lands the
/// trigger does not have to re-derive it. "You may pay 2[credit] and trash 2
/// cards from HQ. If you do, end the run" is 1.16.11a's nested cost: the two
/// components (credits, and two cards trashed from a named zone) are paid
/// together as ONE cost, "if you do" is the rule's own words for the branch
/// that follows a paid nested cost, and the whole thing is one instruction
/// (9.11.3) — which is why the marker covers the printed line whole rather
/// than half of it.
pub fn anoetic_void() -> Card {
    card("Anoetic Void")
        .corp()
        .upgrade()
        .faction("Jinteki")
        .cost(0)
        .trash_cost(1)
        .unique()
        .text("Whenever the Runner approaches this server, you may pay 2[credit] and trash 2 cards from HQ. If you do, end the run.")
        .unimplemented("Whenever the Runner approaches this server, you may pay 2[credit] and trash 2 cards from HQ. If you do, end the run.")
        .build()
}

/// La Costa Grid — Upgrade: Region - Seedy. Rez 3, trash 4.
/// "Remote server only.
///  When your turn begins, place 1 advancement counter on a card in the root
///  of this server.
///  Limit 1 region per server."
///
/// UNIMPLEMENTED: the first two lines.
///
/// "Remote server only" is an install restriction stated on the card — the
/// server-side twin of the hosted restriction the CR states at 8.5.1a and the
/// kernel carries as `StaticDecl::InstallOnlyHostedOn`. Nothing states WHERE a
/// card may be installed in terms of servers, so an unmarked La Costa Grid
/// would install into the root of HQ and then place a counter there every
/// turn, which is a card the printed one is not.
///
/// "A card in the root of this server" is a description reaching the root of
/// the SOURCE's own server. The kernel has the ice half of that idea
/// (`TargetFilter::IceProtectingSourceServer`) and no root half, and no other
/// description is the same set: "in a remote server" is every remote, and "in
/// the attacked server" reads a run this ability fires outside of. Both
/// general capabilities are on the Blockers list.
///
/// "Limit 1 region per server" is 3.6.5's rule about the REGION subtype, not
/// a sentence this card does — 3.6.5a prints it on every region, and 3.6.5b-d
/// state it as a rule of the game (a must-trash at install, and a bar on
/// swapping a second one in). It is carried as printed text and denotes into
/// nothing, which is the treatment Crisium Grid's identical line already has.
pub fn la_costa_grid() -> Card {
    card("La Costa Grid")
        .corp()
        .upgrade()
        .faction("Jinteki")
        .subtypes(&["Region", "Seedy"])
        .cost(3)
        .trash_cost(4)
        .text("Remote server only.")
        .text("When your turn begins, place 1 advancement counter on a card in the root of this server.")
        .text("Limit 1 region per server.")
        .unimplemented("Remote server only.")
        .unimplemented("When your turn begins, place 1 advancement counter on a card in the root of this server.")
        .build()
}

/// Mavirus — Upgrade: Ambush. Rez 3, trash 0.
/// "While the Runner is accessing this upgrade in R&D, they must reveal it.
///  When the Runner accesses this upgrade, you may purge virus counters. If
///  this upgrade is rezzed, do 1 net damage.
///  [trash]: Purge virus counters."
///
/// COMPLETE. Three printed lines, four printed sentences.
///
/// The first line is Archangel's sentence word for word, and it is written the
/// same way: 9.6.5c's zone stipulation on a "when accessed" condition. That
/// stipulation is doing two jobs at once. It is the requirement — the reveal
/// happens on an access in R&D and on no other — and, by 9.1.8b's first
/// sentence, it is the STATEMENT that keeps the ability alive in a deck, where
/// a card is otherwise inactive. 1.21.3 is the reveal itself: shown to both
/// players and then back where it was, which 1.21.3a distinguishes from
/// turning a card faceup.
///
/// The second line is ONE conditional ability (the trigger is stated once) and
/// TWO instructions (9.11.3), which the full stop in the middle decides. It is
/// active while its source is inactive by 9.1.8a — "abilities are active while
/// their source is the card being accessed" — which is the only reason an
/// ambush in an unrezzed remote root ever speaks.
///
/// The "you may" belongs to the purge and not to the ability: 9.6.9d's
/// optional part INSIDE an instruction, so the second sentence resolves
/// whether or not the Corp takes the first. That is the whole point of the
/// card — a Corp who purges is telling the Runner what they hit, and a Corp
/// who declines still deals the damage if the upgrade was rezzed.
///
/// "If this upgrade is rezzed" is asked of the board as the instruction
/// resolves, and it is a real question in both directions: this is a 0-trash
/// ambush that is normally accessed UNREZZED (no damage), and 1.15.2c's
/// installed-cards default is what makes a copy accessed from R&D or HQ answer
/// no as well — a card in a deck or a hand is not rezzed.
///
/// The last line is 1.16.10's [trash] as a trigger cost on a paid ability
/// (9.5.1), which is how a rezzed Mavirus purges on the Corp's own terms
/// instead of the Runner's.
pub fn mavirus() -> Card {
    card("Mavirus")
        .corp()
        .upgrade()
        .faction("Jinteki")
        .subtypes(&["Ambush"])
        .cost(3)
        .trash_cost(0)
        .text("While the Runner is accessing this upgrade in R&D, they must reveal it.")
        .text("When the Runner accesses this upgrade, you may purge virus counters. If this upgrade is rezzed, do 1 net damage.")
        .text("[trash]: Purge virus counters.")
        .when(
            TriggerCond::SelfAccessed { requires: vec![source_in_rnd()] },
            [reveal_self()],
        )
        .named("they must reveal it")
        .when(
            accessed(),
            [
                may(purge_virus_counters()),
                if_met(&[board_has(&[this_very_card(), rezzed()], 1)], [net_damage(Corp, 1)]),
            ],
        )
        .named("the ambush")
        .paid(trash_this_card(), [purge_virus_counters()])
        .named("purge on the corp's terms")
        .build()
}

// ---------------------------------------------------------------------------
// Ice
// ---------------------------------------------------------------------------

/// Brân 1.0 — ICE: Barrier - Bioroid. Rez 6, strength 6.
/// "Lose [click]: Break 1 subroutine on this ice. Only the Runner can use this
///  ability.
///  [subroutine] You may install 1 piece of ice from HQ or Archives directly
///  inward from this ice, ignoring all costs.
///  [subroutine] End the run.
///  [subroutine] End the run."
///
/// COMPLETE.
///
/// The bioroid ability is Fairchild 3.0's, with the printed numbers: 5.2.1a
/// lets a cost contain [click] symbols "without denoting an action", so it is
/// used in a paid window and not an action window — which is the only reason
/// it can ever be used, because 9.5.6a confines a break ability to an
/// encounter and an encounter is not an action window. 1.11.3b keeps "lose"
/// and "spend" apart for everything that asks. "Only the Runner can use this
/// ability" is 1.14.4b, and it says two things: the Runner is offered it, and
/// the Corp — who controls the ice — is not. "…on **this ice**" is 9.5.6c,
/// which confines the ability to an encounter with this very card.
///
/// The first subroutine is 8.5.13a's install with three stipulations.
///
/// "From HQ or Archives" is ONE description with two alternatives (1.15.2c's
/// zone statement, twice over), so it is one announcement and the Corp picks
/// from both piles at once — not two descriptions and not two instructions.
///
/// "Directly inward from this ice" is 6.2.2c, which the CR states as its own
/// rule beside 6.2.2a's outermost default: the new position is created
/// immediately inward of this card's, so the installed ice is passed BEFORE
/// this one on a later run and — since the Runner is standing on Brân when the
/// subroutine resolves — on this one too. 8.5.14 is the case with no such
/// position: this ice is not protecting a server (it was uninstalled, or is
/// being encountered while not installed), the destination cannot be
/// identified, and no installation takes place.
///
/// "Ignoring all costs" is 1.16.5c: every element of the install cost goes,
/// 8.5.11a's 1[credit] per piece of ice already protecting the server
/// included — and this subroutine always adds to a server that has at least
/// this ice on it, so there is always such a charge to ignore.
///
/// The printed "you may" is 9.6.9d's optional part inside the instruction: the
/// subroutine resolves whether or not the Corp takes what it offers.
pub fn bran_1_0() -> Card {
    card("Brân 1.0")
        .corp()
        .ice(6)
        .faction("Haas-Bioroid")
        .subtypes(&["Barrier", "Bioroid"])
        .cost(6)
        .text("Lose [click]: Break 1 subroutine on this ice. Only the Runner can use this ability.")
        .text("[subroutine] You may install 1 piece of ice from HQ or Archives directly inward from this ice, ignoring all costs.")
        .text("[subroutine] End the run.")
        .text("[subroutine] End the run.")
        .paid_used_only_by_during_encounters_with_this_card(
            Runner,
            losing_clicks(1),
            [break_subroutines(1)],
        )
        .named("bioroid break")
        .subroutine([may(install_ignoring_all_costs(
            choose(
                1,
                &[
                    of_type(CardType::Ice),
                    any_of(&[&[in_hand_of(Corp)], &[in_archives()]]),
                ],
            ),
            InstallDest::InwardFromSource,
        ))])
        .named("a free piece of ice, one step inward")
        // The card prints this subroutine twice, so it is written twice.
        .subroutine([end_the_run()])
        .named("end the run")
        .subroutine([end_the_run()])
        .named("end the run again")
        .build()
}

/// Empiricist — ICE: Sentry - AP - Observer. Rez 7, strength 5.
/// "[subroutine] Draw 1 card. You may add 1 card from HQ to the top of R&D.
///  [subroutine] Do 1 net damage. Give the Runner 1 tag.
///  [subroutine] Do 2 net damage."
///
/// COMPLETE. Three subroutines, five sentences, and the split is 9.11.3's:
/// each full stop ends an instruction, and each "and"-free pair of sentences
/// is therefore two.
///
/// That matters most on the first subroutine, where the boundary is the point
/// of the card: the draw resolves, a checkpoint happens, and only then is the
/// Corp asked which card goes back — so the card they just drew is one of the
/// cards they may choose. Written as one merged instruction the choice would
/// be announced (1.15.2) before the draw resolved, and the drawn card could
/// never be the one put back, which is the opposite of what the ice is for.
///
/// The second subroutine's two sentences are one damage and one tag, in the
/// printed order, with a checkpoint between them — so a Runner who is flatlined
/// by the net damage (1.17.2b) never reaches the tag.
///
/// "You may add 1 card from HQ to the top of R&D" is 9.6.9d's optional part
/// inside the instruction, and "from HQ" is the zone statement 1.15.2c wants
/// before a description may reach a hidden zone. 8.2's add names the END of
/// the deck as content, which is why "to the top" is not a sentence of its own.
pub fn empiricist() -> Card {
    card("Empiricist")
        .corp()
        .ice(5)
        .faction("Jinteki")
        .subtypes(&["Sentry", "AP", "Observer"])
        .cost(7)
        .text("[subroutine] Draw 1 card. You may add 1 card from HQ to the top of R&D.")
        .text("[subroutine] Do 1 net damage. Give the Runner 1 tag.")
        .text("[subroutine] Do 2 net damage.")
        .subroutine([
            draw(Corp, 1),
            may(add_to_deck(choose(1, &[in_hand_of(Corp)]), true)),
        ])
        .named("draw, then put one back on top")
        .subroutine([net_damage(Corp, 1), give_tags(1)])
        .named("a point of net and a tag")
        .subroutine([net_damage(Corp, 2)])
        .named("two net")
        .build()
}

/// Flyswatter — ICE: Code Gate. Rez 2, strength 0.
/// "When you rez this ice during a run against this server, purge virus
///  counters.
///  [subroutine] End the run."
///
/// PARTIAL: the first sentence, on the same wall Anoetic Void is stopped at.
///
/// The rez itself is sayable — 8.1.2's rez of this very card is a condition
/// the kernel carries, and "purge virus counters" is 10.1.2 in one word. What
/// is missing is "during a run against THIS server": the requirement the
/// kernel has about a run in progress names a fixed list of servers, and this
/// card names the server it is protecting, which is not knowable before the
/// game. Written without it, a Flyswatter rezzed in the Corp's own action
/// phase — or during a run on some other server — would purge, and the whole
/// point of the card is that the Runner chooses when it goes off. The general
/// capability wanted is on the Blockers list.
///
/// The subroutine is complete: 6.9.3's end of the run, on a piece of ice whose
/// strength is 0 so that the Runner can break it with anything.
pub fn flyswatter() -> Card {
    card("Flyswatter")
        .corp()
        .ice(0)
        .faction("Neutral")
        .subtypes(&["Code Gate"])
        .cost(2)
        .text("When you rez this ice during a run against this server, purge virus counters.")
        .text("[subroutine] End the run.")
        .unimplemented("When you rez this ice during a run against this server, purge virus counters.")
        .subroutine([end_the_run()])
        .named("end the run")
        .build()
}

/// Knowledge Seeker — ICE: Code Gate. Rez 5, strength 5.
/// "Whenever an encounter with this ice ends, if it has 3 or more hosted virus
///  counters, purge virus counters and derez this ice.
///  [subroutine] Place 1 virus counter on this ice.
///  [subroutine] Look at the top 4 cards of R&D and arrange them in any order.
///  [subroutine] End the run."
///
/// COMPLETE.
///
/// The first line is 6.5.10's end of an encounter, scoped to this card by an
/// ordinary description word rather than by a condition of its own — the ice
/// the encounter was with is what "this ice" describes. "If it has 3 or more
/// hosted virus counters" is 9.6.5c's requirement, a 9.12.2 count of the
/// source's own counters against the printed threshold, read when the
/// condition would be met. "Purge virus counters and derez this ice" is ONE
/// sentence joined by "and" and therefore ONE instruction (9.11.3), which is
/// load-bearing: the purge takes this ice's own counters (10.1.2 removes every
/// virus counter in play, and 1.9.5c makes these virus counters like any
/// other), so a split would let the requirement be re-read between the halves
/// with the counters already gone.
///
/// The card is its own clock: each encounter's first subroutine adds a
/// counter, and the third one ends the encounter that puts the count over the
/// line. Derezzing (8.1.3) is what it costs the Corp — 8.1.3a leaves the card
/// installed and inactive, so it must be paid for again.
///
/// "Place 1 virus counter on this ice" is 1.18.2's placement, not a load: no
/// sentence on this card asks "when it is empty".
///
/// "Look at the top 4 cards of R&D and arrange them in any order" is 9.11.4e's
/// exception in person — the look is its own instruction even though it shares
/// a sentence with what follows. 8.3.2 is the look, 8.3.3 is the arrangement:
/// the Corp "secretly puts them in the order of their choice, and returns them
/// to the top of that deck", and 1.12.3 makes every returned card a NEW object
/// — which is why a breach in progress forgets it had already chosen them.
pub fn knowledge_seeker() -> Card {
    card("Knowledge Seeker")
        .corp()
        .ice(5)
        .faction("Jinteki")
        .subtypes(&["Code Gate"])
        .cost(5)
        .text("Whenever an encounter with this ice ends, if it has 3 or more hosted virus counters, purge virus counters and derez this ice.")
        .text("[subroutine] Place 1 virus counter on this ice.")
        .text("[subroutine] Look at the top 4 cards of R&D and arrange them in any order.")
        .text("[subroutine] End the run.")
        .when(
            encounter_with_this_ice_ends_if(&[hosted_counters_at_least(CounterKind::Virus, 3)]),
            [combined([purge_virus_counters(), derez(this_card())])],
        )
        .named("three counters and it burns out")
        .subroutine([place(CounterKind::Virus, 1)])
        .named("one more virus counter")
        .subroutine([set_aside_top_of_deck(Corp, 4), arrange_set_aside(Corp)])
        .named("sort the top of r&d")
        .subroutine([end_the_run()])
        .named("end the run")
        .build()
}

// ---------------------------------------------------------------------------
// The deck
// ---------------------------------------------------------------------------

/// The deck, in the order `docs/vm/DECK-OF-THE-WEEK.md` lists it.
///
/// The identity and four of the sixteen cards are REUSED — the identity out of
/// the identity queue, Hedge Fund and Seamless Launch out of the Gauntlet,
/// Spin Doctor and Tatu-Bola out of Mezzie's Asa — because a card is written
/// once and listed wherever a deck plays it.
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::corp_jinteki::jinteki_restoring_humanity(),
        anoetic_void(),
        bran_1_0(),
        charlotte_cacador(),
        empiricist(),
        flyswatter(),
        fujii_asset_retrieval(),
        hansei_review(),
        super::gauntlet::hedge_fund(),
        knowledge_seeker(),
        la_costa_grid(),
        mavirus(),
        proprionegation(),
        super::gauntlet::seamless_launch(),
        send_a_message(),
        super::mezzie_asa::spin_doctor(),
        super::mezzie_asa::tatu_bola(),
    ]
}

/// CR 1.5.4a: the pile is the RUNNER's — "a player may bring any number of
/// additional **Runner** identity cards along with their deck" — so a Corp
/// deck brings none.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
