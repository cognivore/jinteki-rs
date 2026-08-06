//! Runner — Adam, and the directives he starts the game with.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10).
//!
//! CR 1.5.3 is Adam's own rule: the directives are extra cards brought along
//! with the deck (1.5.3a — `GameSetup::extra_cards`), exactly 3 differently
//! named ones begin the game installed (1.5.3b), and from then on they are
//! ordinary installed cards (1.5.3d). They are defined here beside the
//! identity because nothing else brings them.

use jinteki_cr::Subtype;

use crate::edsl::*;

/// Adam: Compulsive Hacker — Identity: Bioroid.
/// "You start the game with 3 different directive cards installed (these
///  cards are not considered part of your deck)."
///
/// COMPLETE. A setup FACT of the `starting_*` family (CR 1.6.2 — special
/// setup that corresponds to no setup step), not an ability that resolves:
/// 1.5.3b's "those cards begin the game installed in the play area" is a
/// state the game is built in, before credits, shuffles or hands, so no
/// install effect ran, no cost existed, and nothing a "when you install"
/// condition could meet was ever recorded. The cards come from OUTSIDE the
/// deck — 1.5.3a's extra pile brought along with it — which is what the
/// parenthetical says; "3 different" is 2.1.5's differently-named
/// stipulation, which 1.5.3a restates about what must be brought.
pub fn adam() -> Card {
    card("Adam: Compulsive Hacker")
        .runner()
        .identity()
        .faction("Adam")
        .subtypes(&[Subtype::Bioroid])
        .text("You start the game with 3 different directive cards installed (these cards are not considered part of your deck).")
        .starts_the_game_with_installed(3, &[with_subtype(Subtype::Directive)])
        .build()
}

/// Always Be Running — Resource: Directive - Virtual. Install 0. Unique.
/// "The first [click] you spend each turn must be spent to take the basic
///  action to play an event or the basic action to run a server. You cannot
///  take the action to play an event this way except if you play a run
///  event.
///  Once per turn → Lose [click][click]: Break 1 subroutine."
///
/// NOT yet complete — installed honest-with-a-marker for Adam's setup fact.
///
/// The first line is a restriction on WHICH action the action window may
/// offer as the turn's first: 5.2's action step offers every basic action,
/// and no declaration narrows that offer to a stated pair (it is the
/// opposite polarity of `ProhibitedAction`, scoped to "the first [click] you
/// spend each turn", with the second sentence narrowing the play half to run
/// events). That word has not landed, so the line is recorded rather than
/// approximated.
///
/// The second line is whole: "Lose [click][click]" is `Cost::lose_clicks` —
/// 1.11.3b keeps losing apart from spending (5.2.1a is why it is never
/// offered as an action), and Seidr Laboratories reads the difference —
/// "Once per turn →" is 9.3.6g's flag, and "Break 1 subroutine" is the
/// Quetzal shape without the subtype: 9.5.6a offers a break ability only
/// during an encounter, and this one names no ice kind, so any encounter
/// will do.
pub fn always_be_running() -> Card {
    card("Always Be Running")
        .runner()
        .resource()
        .faction("Adam")
        .subtypes(&[Subtype::Directive, Subtype::Virtual])
        .cost(0)
        .unique()
        .text("The first [click] you spend each turn must be spent to take the basic action to play an event or the basic action to run a server. You cannot take the action to play an event this way except if you play a run event.")
        .text("Once per turn → Lose [click][click]: Break 1 subroutine.")
        .unimplemented("The first [click] you spend each turn must be spent to take the basic action to play an event or the basic action to run a server. You cannot take the action to play an event this way except if you play a run event.")
        .paid_once_per_turn(Cost::lose_clicks(2), [break_subroutines(1)])
        .named("lose two clicks, break one subroutine")
        .build()
}

/// Neutralize All Threats — Resource: Directive - Virtual. Install 0. Unique.
/// "The first time each turn you access a card with a trash cost, reveal it.
///  You must trash that card by paying its trash cost, if able.
///  Whenever you breach HQ, access 1 additional card."
///
/// COMPLETE. Two printed lines, two conditional abilities. The first line's
/// condition is 7.3.6's access with the sentence's stipulation — "a card
/// with a trash cost" is 2.6's printed box, which no modifier gives to a
/// card that prints none (7.1.5a) — and 9.6.5c's ordinal about that
/// occurrence. Its two sentences are two instructions: 9.11.4e keeps the
/// reveal its own, and "you must trash that card by paying its trash cost,
/// if able" is 9.12.3b's requirement — only the basic trash ability
/// satisfies it, so a Runner who cannot pay is forced into nothing, which is
/// the whole of "if able".
///
/// The second line is 7.3's breach of the named server and "access 1
/// additional card" raises 7.3.5's access limit for that breach.
pub fn neutralize_all_threats() -> Card {
    card("Neutralize All Threats")
        .runner()
        .resource()
        .faction("Adam")
        .subtypes(&[Subtype::Directive, Subtype::Virtual])
        .cost(0)
        .unique()
        .text("The first time each turn you access a card with a trash cost, reveal it. You must trash that card by paying its trash cost, if able.")
        .text("Whenever you breach HQ, access 1 additional card.")
        .when_first_each_turn(
            accesses_a_card_matching(&[has_trash_cost()]),
            [reveal(accessed_card()), must_trash_accessed_by_paying_trash_cost()],
        )
        .named("the first trash-cost access of the turn")
        .when(breaches_server_if(ServerId::Hq, &[]), [additional_accesses(1)])
        .named("one more card from HQ")
        .build()
}

/// Safety First — Resource: Directive - Virtual. Install 0. Unique.
/// "Your maximum hand size is reduced by 2.
///  When your turn ends, draw 1 card if you do not have cards in your grip
///  equal to or greater than your maximum hand size."
///
/// COMPLETE. Two printed lines of different kinds: the first is permanently
/// true and so a 5.5.3 static declaration — the same one Cybernetics
/// Division makes, with its own amount — and the second happens, so it is a
/// conditional ability on 5.1.4b's formal end of the turn.
///
/// The "if" is stated AFTER the draw, so it is 9.6.5d's requirement in the
/// instructions, checked when the draw would resolve rather than when the
/// turn ended. "Cards in your grip equal to or greater than your maximum
/// hand size", negated, is grip < maximum — two calculated amounts with the
/// strict inequality between them, and the maximum is read through the same
/// 9.12.1a pipeline the discard step reads, so this card's own "-2" is
/// already inside it.
pub fn safety_first() -> Card {
    card("Safety First")
        .runner()
        .resource()
        .faction("Adam")
        .subtypes(&[Subtype::Directive, Subtype::Virtual])
        .cost(0)
        .unique()
        .text("Your maximum hand size is reduced by 2.")
        .text("When your turn ends, draw 1 card if you do not have cards in your grip equal to or greater than your maximum hand size.")
        .declares([max_hand_size_mod(-2)])
        .named("safety first")
        .when(
            turn_ends(Runner),
            [if_met(
                &[fewer_than(cards_in_hand_count(Runner), maximum_hand_size_of(Runner))],
                [draw(Runner, 1)],
            )],
        )
        .named("a card before sleep")
        .build()
}

/// Every Adam identity this module carries.
pub fn identities() -> Vec<Card> {
    vec![adam()]
}

/// The directive cards Adam brings along with his deck (CR 1.5.3a) — not
/// identities and not listed by any deck, but as real as any card here.
pub fn directives() -> Vec<Card> {
    vec![always_be_running(), neutralize_all_threats(), safety_first()]
}
