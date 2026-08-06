//! Deck of the week — "kit costume party", Nyusha "Sable" Sintashta.
//!
//! NetrunnerDB #97727; the list is in `docs/vm/DECK-OF-THE-WEEK.md` and this
//! module is its Runner half.
//!
//! Printed text is copied from NSG's official card data. Behaviour is written
//! from that text and from nowhere else (SYS-D-10): the doc comment above each
//! card carries the text for whoever is reading, `.text(…)` carries the same
//! text as data for whatever is checking, and `tests/decks.rs` asserts the two
//! agree. Sentences the vocabulary cannot say yet carry `.unimplemented(…)`
//! rather than an approximation, and the kernel capability each one waits on
//! is on the Blockers list in `docs/vm/DECK-OF-THE-WEEK.md`.
//!
//! Five of the eighteen distinct cards are written elsewhere and REUSED, never
//! copied: the identity out of the identity queue, Boomerang, Clean Getaway,
//! Mutual Favor and The Class Act out of Andromeda, and Asmund Pudlat out of
//! `unlisted.rs`. [`deck`] lists them from there.

use jinteki_cr::Subtype;

use crate::edsl::*;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Always Have a Backup Plan — Event: Run. Cost 2.
/// "Run any server. When that run ends, if it was unsuccessful, you may run
///  the attacked server again, ignoring any additional costs to run. During
///  the second run, whenever you encounter the last piece of ice you
///  encountered during the first run, bypass it."
///
/// PARTIAL: the first sentence resolves; the second and third do not.
///
/// "Run any server" is 6.9.1a with no server named by the effect, so the
/// Runner announces one at the Initiation Phase from everything 6.7.4a allows.
///
/// The second sentence wants two words the kernel does not have. "When that
/// run ends, **if it was unsuccessful**" is 6.9.7's run-ends occurrence with
/// the negative half of 6.9.5's declaration as its stipulation, and the
/// condition carries only the positive one (`successful_only: bool`, where
/// `false` is a sentence naming no outcome at all rather than the failing
/// one). "Run **the attacked server** again" is a run whose server is read off
/// the run that just ended, where every run instruction the kernel has either
/// names a fixed server or leaves the choice to the Runner. Written with the
/// words that exist it would offer a second run on any server at all after
/// every first run — a larger card than the printed one, which is the case
/// EDSL.md says takes the marker even when the words nearly fit.
///
/// The third sentence names "the last piece of ice you encountered during the
/// first run", a description of a card by its position in an EARLIER run's
/// encounter history. Nothing in the description vocabulary reaches across
/// runs like that. Both capabilities are on the Blockers list.
pub fn always_have_a_backup_plan() -> Card {
    card("Always Have a Backup Plan")
        .runner()
        .event()
        .faction("Criminal")
        .subtypes(&[Subtype::Run])
        .cost(2)
        .text("Run any server. When that run ends, if it was unsuccessful, you may run the attacked server again, ignoring any additional costs to run. During the second run, whenever you encounter the last piece of ice you encountered during the first run, bypass it.")
        .play([run_any_server([])])
        .unimplemented("When that run ends, if it was unsuccessful, you may run the attacked server again, ignoring any additional costs to run.")
        .unimplemented("During the second run, whenever you encounter the last piece of ice you encountered during the first run, bypass it.")
        .build()
}

/// Carpe Diem — Event: Run. Cost 1.
/// "Identify your mark. (If you don’t have a mark, a random central server
///  becomes your mark for this turn.)
///  Gain 4[credit]. You may run your mark."
///
/// PARTIAL: the last sentence.
///
/// "Identify your mark" is 10.11.2 in one instruction, and the parenthesis is
/// 1.4's reminder text — it restates 10.11.2a's random central and 10.11.3's
/// "if a server is already the mark, nothing happens", both of which the
/// instruction already is, so it is not a second sentence. This is the card
/// CR 10.11.2's own worked example is written about, and the identity above it
/// says the same sentence.
///
/// "Gain 4[credit]" and "You may run your mark" are two sentences and
/// therefore two instructions (9.11.3), with the checkpoint between them where
/// the full stop is — which matters, because the credits are the Runner's
/// before they decide whether to spend the rest of the turn running.
///
/// The last sentence is where the vocabulary stops. 10.11.1 makes the mark a
/// designated SERVER, and the kernel can read it — a "successful run on your
/// mark" condition compares against it — but a run INSTRUCTION names its
/// server as a fixed `ServerId` or leaves the choice to the Runner, and
/// neither of those is "the mark". Written as a free choice the card would
/// offer a run on any server, which is the larger card again. The capability
/// is on the Blockers list.
pub fn carpe_diem() -> Card {
    card("Carpe Diem")
        .runner()
        .event()
        .faction("Criminal")
        .subtypes(&[Subtype::Run])
        .cost(1)
        .text("Identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.)")
        .text("Gain 4[credit]. You may run your mark.")
        .play([identify_mark(), gain(Runner, 4)])
        .unimplemented("You may run your mark.")
        .build()
}

/// S-Dobrado — Event: Run. Cost 2.
/// "Run a central server. The first time you encounter a piece of ice during
///  that run, bypass it.
///  Threat 4 → The second time you encounter a piece of ice during that run,
///  you may spend [click] to bypass it. (This ability is active if any player
///  has 4 or more agenda points.)"
///
/// PARTIAL: the Threat 4 line.
///
/// The first line is Blackmail's shape, and the second sentence rides on the
/// run for Blackmail's reason: 5.2.2b suspends the ability that initiated a
/// run until the run is complete, so an instruction written AFTER the run does
/// not resolve until the run is over and 9.10.4 would expire the "during that
/// run" duration before anything could read it. The run instruction carries
/// what the sentence says about its own run instead.
///
/// "Run a central server" is 6.7.4a's allowed set stated by the effect —
/// 4.6.6 names the three, the Runner announces which at 6.9.1a — and it is the
/// same position "run any server" leaves open, with the sentence's stipulation
/// as content.
///
/// "The first time you encounter a piece of ice during that run, bypass it" is
/// a delayed conditional (9.6.13) lasting the run, with 9.6.5c's ordinal on
/// it. The ordinal's SPAN is the printed "during that run" — `OrdinalScope`
/// carries "each turn" and "each run" as content on the one stipulation — so
/// the ability is relevant to the first encounter of this run and to no other.
/// The bypass itself is 6.5.8.
///
/// The Threat 4 line prints the same stipulation about a DIFFERENT occurrence:
/// "the SECOND time you encounter a piece of ice during that run". The
/// kernel's ordinal is a span and nothing else — an `Option<OrdinalScope>`,
/// where `Some(span)` means the first time in that span and `None` means no
/// ordinal at all — so there is no way to name the second. Written as the
/// first it would fire on the encounter the line above already bypassed, and
/// the Runner would be offered a click for an encounter that is already over.
/// The general capability is on the Blockers list.
pub fn s_dobrado() -> Card {
    card("S-Dobrado")
        .runner()
        .event()
        .faction("Criminal")
        .subtypes(&[Subtype::Run])
        .cost(2)
        .text("Run a central server. The first time you encounter a piece of ice during that run, bypass it.")
        .text("Threat 4 → The second time you encounter a piece of ice during that run, you may spend [click] to bypass it. (This ability is active if any player has 4 or more agenda points.)")
        .play([run_a_central_server_during_which([
            the_first_time_this_run(
                "s-dobrado: bypass the first ice",
                encounters_any_ice(),
                [bypass_encountered_ice()],
            ),
        ])])
        .unimplemented("Threat 4 → The second time you encounter a piece of ice during that run, you may spend [click] to bypass it.")
        .build()
}

// ---------------------------------------------------------------------------
// Hardware
// ---------------------------------------------------------------------------

/// Buffer Drive — Hardware. Install 3. ◆
/// "The first time each turn 1 or more cards are trashed from your grip or
///  stack, you may add 1 of those cards to the bottom of your stack.
///  Remove this hardware from the game: Add 1 card from your heap to the top
///  of your stack."
///
/// COMPLETE.
///
/// The first line is PASSIVE — "1 or more cards **are** trashed" — so the
/// sentence names no player who does the trashing, which is the whole point:
/// the commonest way cards leave the grip is 10.4.2's damage, which is the
/// Corp's doing, and a condition naming the Runner as the trasher would miss
/// every one of them. What it does name is whose zones they are ("**your**
/// grip or stack"), which is the owner.
///
/// "Grip or stack" is ONE condition with two alternatives (9.6.5's disjunction)
/// and not two abilities, because the line also prints an ordinal:
/// [`AbilityDef::ordinal`] belongs to one ability, so a pair would each spend
/// their own and a turn that trashed from both would fire twice.
///
/// "1 or more cards" is 9.12.2a's plural noun, which is what makes this met
/// ONCE per trashing event however many cards it moved — 3 net damage is one
/// occurrence, not three — and it is also what lets "1 of **those** cards"
/// name the whole set (1.15.4 in the plural). By the time the ability
/// resolves the cards are in the heap; the description fixes them by identity,
/// so 1.15.2c's play-area default has nothing left to restrict.
///
/// The second line's cost is everything before the colon: 4.9's removal from
/// the game as a trigger cost (1.16.10), which is why the ability is worth
/// exactly one use. "To the top of your stack" is 8.2's add with the END of
/// the deck as content, and 4.2.1 fixes the deck as the card's owner's.
pub fn buffer_drive() -> Card {
    card("Buffer Drive")
        .runner()
        .hardware()
        .faction("Neutral")
        .cost(3)
        .unique()
        .text("The first time each turn 1 or more cards are trashed from your grip or stack, you may add 1 of those cards to the bottom of your stack.")
        .text("Remove this hardware from the game: Add 1 card from your heap to the top of your stack.")
        .may_when_first_each_turn(
            either_of(&[
                cards_are_trashed_from(Runner, Zone::Hand(Runner)),
                cards_are_trashed_from(Runner, Zone::Deck(Runner)),
            ]),
            [add_to_deck(choose(1, &[among_those_cards()]), false)],
        )
        .named("one of those cards, to the bottom")
        .paid(remove_self_cost(), [add_to_deck(choose(1, &[in_heap()]), true)])
        .named("cash it in for a card off the heap")
        .build()
}

/// Jeitinho — Hardware: Weapon. Install 1. ◆
/// "When your turn ends, if you made a successful run on HQ, R&D, and Archives
///  this turn, you may add this hardware to your score area as an assassination
///  agenda worth 0 agenda points. Then, if you have 3 assassination agendas in
///  your score area, you win the game.
///  Threat 3 → Whenever you bypass a piece of ice, you may spend [click] to
///  install this hardware from your heap."
///
/// UNIMPLEMENTED: all three printed sentences.
///
/// The first sentence is nearly sayable and stops on one word. 5.1.4's end of
/// the turn is a condition the kernel carries; "you made a successful run on
/// HQ, R&D, and Archives this turn" is three of the same 9.6.5c requirement
/// with the server as content, conjoined the way requirements always are; and
/// 10.1.3's conversion turns a non-agenda into "an agenda worth N agenda
/// points" as it is added to a score area. What 10.1.3 does not carry is the
/// SUBTYPE the sentence states. "As an **assassination** agenda" is not
/// decoration — it is exactly what the next sentence counts — so a conversion
/// that produced a 0-point agenda with no subtype would leave the card's own
/// second sentence unable to see the cards its first sentence made.
///
/// The second sentence has no word at all: nothing in the instruction
/// vocabulary ends the game. 1.17.1's win is a checkpoint condition on agenda
/// points, and `StaticDecl::AgendaPointsToWinMod` moves the threshold; an
/// alternate win stated by a card is a different thing, and there is no
/// position for it.
///
/// The Threat 3 line stops on its trigger. 6.5.8's bypass ends an encounter
/// without resolving subroutines, and the kernel records enough to tell a pass
/// after a bypass from a pass after an encounter (`IcePassed`'s
/// encounter-scoped stipulations "each require the pass to follow an encounter
/// at all; a pass with no encounter before it (a bypass) meets neither") — but
/// the bypass is not itself an occurrence any condition names. Threat itself is
/// fine: 9.3.6f's flag is general and rides on any ability.
///
/// All three capabilities are on the Blockers list.
pub fn jeitinho() -> Card {
    card("Jeitinho")
        .runner()
        .hardware()
        .faction("Criminal")
        .subtypes(&[Subtype::Weapon])
        .cost(1)
        .unique()
        .text("When your turn ends, if you made a successful run on HQ, R&D, and Archives this turn, you may add this hardware to your score area as an assassination agenda worth 0 agenda points. Then, if you have 3 assassination agendas in your score area, you win the game.")
        .text("Threat 3 → Whenever you bypass a piece of ice, you may spend [click] to install this hardware from your heap.")
        .unimplemented("When your turn ends, if you made a successful run on HQ, R&D, and Archives this turn, you may add this hardware to your score area as an assassination agenda worth 0 agenda points.")
        .unimplemented("Then, if you have 3 assassination agendas in your score area, you win the game.")
        .unimplemented("Threat 3 → Whenever you bypass a piece of ice, you may spend [click] to install this hardware from your heap.")
        .build()
}

/// Swift — Hardware: Console - Vehicle. Install 2. ◆
/// "+1[mu]
///  The first time each turn you play a run event, gain [click].
///  Limit 1 console per player."
///
/// COMPLETE.
///
/// "+1[mu]" is 1.19's memory limit, a declaration rather than something that
/// happens (9.4), read continuously.
///
/// The second line is 8.6's play as the occurrence, with two stipulations the
/// printed words make and nothing more: "**you** play" is the Runner (a Corp
/// card is not an event at all, but the sentence still names the player and so
/// does the condition), and "a **run** event" is 2.16's subtype read through
/// the same 9.12.1b pipeline every subtype query uses. "The first time each
/// turn" is 9.6.5c's ordinal about the OCCURRENCE — checked when the condition
/// would be met, not when the ability is used — and deliberately not 9.3.6g's
/// once-per-turn flag, which 9.1.6 could never spend on a mandatory ability.
///
/// "Gain [click]" is 1.11.3a, and the moment it lands is the point of the
/// card: 5.2.2b leaves the play action incomplete until the run it started is
/// over, and this ability resolves off the PLAY, so the click is in hand
/// before the run finishes.
///
/// "Limit 1 console per player" is 3.3.3's rule about the CONSOLE subtype, not
/// a sentence this card does — the same treatment Desperado's identical line
/// already has. It is carried as printed text and denotes into nothing.
pub fn swift() -> Card {
    card("Swift")
        .runner()
        .hardware()
        .faction("Criminal")
        .subtypes(&[Subtype::Console, Subtype::Vehicle])
        .cost(2)
        .unique()
        .text("+1[mu]")
        .text("The first time each turn you play a run event, gain [click].")
        .text("Limit 1 console per player.")
        .declares([plus_memory(1)])
        .named("+1[mu]")
        .when_first_each_turn(
            plays_a_subtyped(Runner, CardType::Event, Subtype::Run),
            [gain_clicks(Runner, 1)],
        )
        .named("a click back for the first run event")
        .build()
}

/// The Wizard's Chest — Hardware. Install 0. ◆
/// "Use this hardware only if you made a successful run on HQ, R&D, and
///  Archives this turn.
///  [trash]: Choose hardware, program, or resource. Set aside cards from the
///  top of your stack faceup until you set aside 2 cards of the chosen type.
///  You may install 1 of those 2 cards, ignoring all costs. Shuffle the rest of
///  the set-aside cards into your stack."
///
/// UNIMPLEMENTED: the first line, and the middle sentence of the second.
///
/// The first line is 9.3.3c — "limits on when, where, or how often an ability
/// can be used are restrictions" — with a STATE for its content. The kernel's
/// restrictions all name a place in the timing structure (an encounter, an
/// approach, a run in progress, the zone the source sits in); the requirement
/// vocabulary that could answer "if you made a successful run on HQ, R&D, and
/// Archives this turn" is reachable from a trigger condition and from a static
/// ability's stated condition, and not from a restriction on a paid ability.
/// Without it the [trash] ability would be offered on any turn at all, which
/// is a very much larger card than the printed one.
///
/// "Set aside cards from the top of your stack faceup until you set aside 2
/// cards of the chosen type" is a set-aside whose COUNT is not a number: 4.8
/// sets aside a stated quantity of cards, and this sentence states a stopping
/// CONDITION over a growing group instead. The two sentences after it are
/// written against the 2 cards that search produces, so they wait on it.
///
/// Everything else is already sayable and is recorded here so the wave that
/// lands the two words does not re-derive it. "Choose hardware, program, or
/// resource" is 1.15.1b's naming with 2.15's card types as its closed list —
/// no effect of its own, which is why it is not a printed modal owing an arm
/// per branch. "You may install 1 of those 2 cards, ignoring all costs" is
/// 8.5 with 1.16.5c's waiver and 9.6.9c's optional part inside the
/// instruction. "Shuffle the rest of the set-aside cards into your stack" is
/// 8.7.3-adjacent, over the group the set-aside left.
///
/// Both capabilities are on the Blockers list.
pub fn the_wizards_chest() -> Card {
    card("The Wizard’s Chest")
        .runner()
        .hardware()
        .faction("Anarch")
        .cost(0)
        .unique()
        .text("Use this hardware only if you made a successful run on HQ, R&D, and Archives this turn.")
        .text("[trash]: Choose hardware, program, or resource. Set aside cards from the top of your stack faceup until you set aside 2 cards of the chosen type. You may install 1 of those 2 cards, ignoring all costs. Shuffle the rest of the set-aside cards into your stack.")
        .unimplemented("Use this hardware only if you made a successful run on HQ, R&D, and Archives this turn.")
        .unimplemented("Set aside cards from the top of your stack faceup until you set aside 2 cards of the chosen type.")
        .build()
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

/// Carmen — Program: Icebreaker - Killer. Install 5, strength 2, 1[mu].
/// "If you made a successful run this turn, this program costs 2[credit] less
///  to install.
///  Interface → 1[credit]: Break 1 sentry subroutine.
///  2[credit]: +3 strength."
///
/// PARTIAL: the first line.
///
/// The first line is a 9.3.7a static ability whose stated condition is a
/// question about the game — "if you made a successful run this turn", the
/// same requirement Mutual Favor asks — declaring a modification of 1.16.4a's
/// INSTALL cost of its own source. Both halves exist. What does not is the
/// ability being ACTIVE where it has to apply: the card is in the grip when
/// its install cost is calculated, 4.5.4 leaves a card in the grip inactive,
/// and 9.1.8d's exception ("abilities that modify the cost to install, rez, or
/// play their source card are active even while that card is inactive") is
/// implemented for one declaration only — the 1.16.2e alternate payment. An
/// `InherentCostMod` describing its own source is inert in exactly the one
/// place it is printed to work, so the card would cost its full 5 forever. The
/// general capability is on the Blockers list.
///
/// The other two lines are the icebreaker pair Bukhgalter already prints word
/// for word. "Interface →" is 9.3.6c's strength gate together with 9.5.6a's
/// confinement to an encounter, and the named subtype is 9.5.6c: the ability
/// is offered only against a sentry, and only while this program's strength is
/// at least the ice's. The pump is an ordinary paid ability, and 3.9.5b is why
/// it needs no duration written on it — a strength change from a card ability
/// lasts as long as the encounter it was made in.
pub fn carmen() -> Card {
    card("Carmen")
        .runner()
        .program()
        .faction("Criminal")
        .subtypes(&[Subtype::Icebreaker, Subtype::Killer])
        .cost(5)
        .strength(2)
        .memory(1)
        .text("If you made a successful run this turn, this program costs 2[credit] less to install.")
        .text("Interface → 1[credit]: Break 1 sentry subroutine.")
        .text("2[credit]: +3 strength.")
        .unimplemented("If you made a successful run this turn, this program costs 2[credit] less to install.")
        .paid_interface(credits(1), Some(Subtype::Sentry), [break_subroutines(1)])
        .named("interface: break 1 sentry subroutine")
        .paid(credits(2), [pump(3)])
        .named("pump: +3 strength")
        .build()
}

/// Curupira — Program: Icebreaker - Fracter. Install 3, strength 1, 1[mu].
/// "Whenever you encounter a barrier, you may spend 3 hosted power counters to
///  bypass it.
///  Whenever this program fully breaks a piece of ice, place 1 power counter
///  on this program.
///  Interface → 1[credit]: Break 1 barrier subroutine.
///  1[credit]: +1 strength."
///
/// COMPLETE. Four printed lines and three different kinds of thing.
///
/// The first line is Paperclip's condition — 6.5.4's encounter with 2.16's
/// subtype stipulation — carrying 1.16.11a's nested cost. The printed "you
/// may" belongs to the SPENDING and not to the ability (9.6.9c's optional part
/// inside an instruction), so a Runner with fewer than 3 counters is not
/// offered a cost they cannot pay (1.16.1) and one who declines spends
/// nothing. The counters are 1.9.5f's power counters, spent rather than
/// removed; the bypass is 6.5.8.
///
/// The second line is 6.5.7a's full break said of this program rather than of
/// a piece of ice — the same occurrence Bukhgalter reads, which the kernel
/// records at the moment the last subroutine goes. That ordering is what makes
/// the card a clock: the counter arrives on the break, so the barrier it pays
/// to bypass later is never the one that paid for it.
///
/// "Place 1 power counter on this program" is 1.18.2's placement and not a
/// load — no sentence on this card asks "when it is empty".
///
/// The last two lines are the icebreaker pair: 9.3.6c's interface flag with
/// 9.5.6a/c confining it to an encounter with a barrier, and an ordinary paid
/// pump whose 3.9.5b duration is the encounter.
pub fn curupira() -> Card {
    card("Curupira")
        .runner()
        .program()
        .faction("Criminal")
        .subtypes(&[Subtype::Icebreaker, Subtype::Fracter])
        .cost(3)
        .strength(1)
        .memory(1)
        .text("Whenever you encounter a barrier, you may spend 3 hosted power counters to bypass it.")
        .text("Whenever this program fully breaks a piece of ice, place 1 power counter on this program.")
        .text("Interface → 1[credit]: Break 1 barrier subroutine.")
        .text("1[credit]: +1 strength.")
        .when(
            encounters_a(Subtype::Barrier, &[]),
            [may_pay(
                hosted_counters(CounterKind::Power, 3),
                bypass_encountered_ice(),
            )],
        )
        .named("three counters walks you past a barrier")
        .when(TriggerCond::SelfFullyBroken, [place(CounterKind::Power, 1)])
        .named("a counter for a full break")
        .paid_interface(credits(1), Some(Subtype::Barrier), [break_subroutines(1)])
        .named("interface: break 1 barrier subroutine")
        .paid(credits(1), [pump(1)])
        .named("pump: +1 strength")
        .build()
}

/// Hyperbaric — Program: Icebreaker - Decoder. Install 3, strength 0, 1[mu].
/// "When you install this program, place 1 power counter on it.
///  This program gets +1 strength for each hosted power counter.
///  Interface → 1[credit]: Break 1 code gate subroutine.
///  2[credit]: Place 1 power counter on this program."
///
/// COMPLETE. Four printed lines, one of each kind the vocabulary has.
///
/// The first line is 8.5's install of this very card as the occurrence, and
/// 1.18.2's placement — not a load, since nothing here asks "when it is
/// empty". It is what makes a freshly installed Hyperbaric strength 1 rather
/// than the 0 printed on it.
///
/// The second line is Resistor's sentence with a different counter: the
/// program's strength IS its printed 0 plus 1 for each hosted power counter
/// (9.12.1b's reading, where the printed value is part of the expression).
/// Written that way it is recomputed as counters arrive, which is what makes
/// the fourth line worth using mid-encounter — and 9.12.2e is why a lost
/// ability leaves the strength at the printed value rather than at the last
/// number it happened to have.
///
/// The third line is 9.3.6c's interface flag with 9.5.6a/c, against a code
/// gate.
///
/// The fourth line is an ordinary paid ability, and it is deliberately NOT a
/// pump: "place 1 power counter" is a permanent placement that the second
/// line then reads, so the strength it buys does not expire at the end of the
/// encounter the way 3.9.5b would expire a strength change. That difference is
/// the whole card.
pub fn hyperbaric() -> Card {
    card("Hyperbaric")
        .runner()
        .program()
        .faction("Shaper")
        .subtypes(&[Subtype::Icebreaker, Subtype::Decoder])
        .cost(3)
        .strength(0)
        .memory(1)
        .text("When you install this program, place 1 power counter on it.")
        .text("This program gets +1 strength for each hosted power counter.")
        .text("Interface → 1[credit]: Break 1 code gate subroutine.")
        .text("2[credit]: Place 1 power counter on this program.")
        .when(installed(), [place(CounterKind::Power, 1)])
        .named("a counter on the install")
        .declares([strength_is(plus(
            amount(0),
            times(1, per_hosted_counter(CounterKind::Power)),
        ))])
        .named("+1 strength for each hosted power counter")
        .paid_interface(credits(1), Some(Subtype::CodeGate), [break_subroutines(1)])
        .named("interface: break 1 code gate subroutine")
        .paid(credits(2), [place(CounterKind::Power, 1)])
        .named("buy another counter")
        .build()
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Backstitching — Resource: Virtual. Install 2.
/// "When your turn begins, identify your mark. (If you don’t have a mark, a
///  random central server becomes your mark for this turn.)
///  Whenever you encounter a piece of ice during a run on your mark, you may
///  trash this resource to bypass that ice."
///
/// PARTIAL: the second line.
///
/// The first line is 5.6.2's turn-begins occurrence and 10.11.2's
/// identification, the identity's own opening sentence said by a resource.
/// The parenthesis is 1.4's reminder text for 10.11.2a and 10.11.3, which the
/// instruction already is, so it denotes into nothing of its own.
///
/// The second line stops on "during a run on **your mark**". The run-in-
/// progress requirement names a fixed list of servers, which a card printed
/// before the game cannot use for a designation made during it; and while the
/// kernel can compare an occurrence against the mark (a successful run on the
/// mark is a condition it carries), nothing lets a REQUIREMENT ask it. Written
/// with the requirement that exists — or with none — the resource would bypass
/// on every run on every server, which is not a smaller card than the printed
/// one but a very much larger one. The general capability is on the Blockers
/// list; the rest of the sentence (1.16.11a's nested cost paid by trashing
/// this card, and 6.5.8's bypass) is already sayable and waits only on it.
pub fn backstitching() -> Card {
    card("Backstitching")
        .runner()
        .resource()
        .faction("Criminal")
        .subtypes(&[Subtype::Virtual])
        .cost(2)
        .text("When your turn begins, identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.)")
        .text("Whenever you encounter a piece of ice during a run on your mark, you may trash this resource to bypass that ice.")
        .when(turn_begins(Runner), [identify_mark()])
        .named("identify your mark")
        .unimplemented("Whenever you encounter a piece of ice during a run on your mark, you may trash this resource to bypass that ice.")
        .build()
}

/// The Back — Resource: Job - Location. Install 1. ◆
/// "The first time each turn you use a piece of hardware during a run, place 1
///  power counter on this resource.
///  [click], remove this resource from the game: For each hosted power
///  counter, choose up to 2 cards in your heap with [trash] abilities. Shuffle
///  the chosen cards into your stack."
///
/// UNIMPLEMENTED: both printed lines.
///
/// The first line names 9.1.6's USE of an ability — "a player uses an ability
/// by paying its costs and beginning its resolution" — with the SOURCE
/// described ("a piece of hardware") and the run as a 9.6.5c requirement. The
/// kernel has exactly one use-condition and it is the [trash]-symbol special
/// case (`UsesTrashAbility`, Aeneas class), whose only content is which player
/// and whether the basic trash ability counts. There is no position for a
/// description of the card whose ability was used, so the sentence cannot say
/// "a piece of hardware".
///
/// The second line stops on its description. "Cards in your heap with [trash]
/// abilities" describes a card by the KIND OF ABILITY printed on it — 1.16.10's
/// trash symbol appearing in a trigger cost — and the nearest word the kernel
/// has is `HasTrashCost`, which is CR 2.6's printed trash COST and a different
/// thing entirely: it is a number in the corner of a Corp card, and no Runner
/// card in a heap has one. Written with it the ability would reach nothing at
/// all.
///
/// The rest is sayable and is recorded here for the wave that lands the two:
/// the cost is 1.16.10's two components paid together, a click and 4.9's
/// removal; "for each hosted power counter" is 9.12.2's count driving a
/// repetition; "choose up to 2" is 1.15.2e's floor of zero; and the shuffle is
/// 8.7.3-adjacent. Both capabilities are on the Blockers list.
pub fn the_back() -> Card {
    card("The Back")
        .runner()
        .resource()
        .faction("Criminal")
        .subtypes(&[Subtype::Job, Subtype::Location])
        .cost(1)
        .unique()
        .text("The first time each turn you use a piece of hardware during a run, place 1 power counter on this resource.")
        .text("[click], remove this resource from the game: For each hosted power counter, choose up to 2 cards in your heap with [trash] abilities. Shuffle the chosen cards into your stack.")
        .unimplemented("The first time each turn you use a piece of hardware during a run, place 1 power counter on this resource.")
        .unimplemented("[click], remove this resource from the game: For each hosted power counter, choose up to 2 cards in your heap with [trash] abilities. Shuffle the chosen cards into your stack.")
        .build()
}

/// Verbal Plasticity — Resource: Genetics. Install 3. ◆
/// "The first time each turn you take the basic action to draw 1 card, instead
///  draw 2 cards."
///
/// UNIMPLEMENTED: the card's only printed line.
///
/// The shape is right and the two words it needs are both missing. "You would
/// draw" is an interrupt condition the kernel carries (9.9.5a, which The Class
/// Act reads), and the ordinal on it is ordinary — but the sentence does not
/// name a draw, it names 5.2.6b's BASIC ACTION to draw. Nothing in the
/// requirement vocabulary asks which action an imminent instruction belongs
/// to, so an ability written without it would replace the draw of every card
/// that draws exactly one — Diesel's neighbours, an ice's subroutine, the
/// mandatory draw — which is a very much larger card than the printed one.
///
/// The second missing word is "**instead** draw 2 cards": 9.9.6's replacement
/// of a modifiable value, said of the number of cards an imminent draw moves.
/// The kernel has that shape for damage (`IncreaseImminentDamage`) and for a
/// cost (`ReduceImminentCost`) and not for a draw, and the quantity it exposes
/// to an interrupt on a draw (`cards_you_would_draw`) is readable but not
/// writable. Both capabilities are on the Blockers list.
pub fn verbal_plasticity() -> Card {
    card("Verbal Plasticity")
        .runner()
        .resource()
        .faction("Neutral")
        .subtypes(&[Subtype::Genetics])
        .cost(3)
        .unique()
        .text("The first time each turn you take the basic action to draw 1 card, instead draw 2 cards.")
        .unimplemented("The first time each turn you take the basic action to draw 1 card, instead draw 2 cards.")
        .build()
}

// ---------------------------------------------------------------------------
// The deck
// ---------------------------------------------------------------------------

/// The deck, in the order `docs/vm/DECK-OF-THE-WEEK.md` lists it.
///
/// The identity and five of the eighteen cards are REUSED — the identity out
/// of the identity queue, Boomerang, Clean Getaway, Mutual Favor and The Class
/// Act out of Andromeda, and Asmund Pudlat out of `unlisted.rs` — because a
/// card is written once and listed wherever a deck plays it.
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::runner_criminal::nyusha_sintashta(),
        always_have_a_backup_plan(),
        super::unlisted::asmund_pudlat(),
        backstitching(),
        super::andromeda::boomerang(),
        buffer_drive(),
        carmen(),
        carpe_diem(),
        super::andromeda::clean_getaway(),
        curupira(),
        hyperbaric(),
        jeitinho(),
        super::andromeda::mutual_favor(),
        s_dobrado(),
        swift(),
        the_back(),
        super::andromeda::the_class_act(),
        the_wizards_chest(),
        verbal_plasticity(),
    ]
}

/// CR 1.5.4a: "a player may bring any number of additional Runner identity
/// cards along with their deck". NRDB #97727 lists none, so the pile is empty
/// — which is a fact about the printed list and not about the rules.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
