//! Runner — Anarch identities.
//!
//! Printed text copied from NSG's official card data
//! (`crates/jinteki-core/carddata/cards.json`); behaviour written from that
//! text alone (SYS-D-10). No Anarch identity existed before the queue, so
//! every one of them lands here.

use crate::edsl::*;

/// Alice Merchant: Clan Agitator — Identity: Cyborg. Link 0.
/// "The first time you make a successful run on Archives each turn, the Corp
///  must trash 1 card from HQ."
///
/// COMPLETE. The condition is 6.8.4's "makes a successful run" with the
/// server the sentence names as its stipulation, and 9.6.5c's ordinal about
/// the occurrence — the same pair Gabriel Santiago states about HQ.
///
/// "The Corp must trash 1 card from HQ" is 1.14.5's attribution: the sentence
/// names a player other than the ability's controller, so the CORP makes the
/// choice the instruction offers, even though 9.1.1a makes the Runner the
/// controller of a Runner identity's ability. "Must" is 9.12.3: the Corp has
/// no choice about whether, only about which.
pub fn alice_merchant() -> Card {
    card("Alice Merchant: Clan Agitator")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Cyborg"])
        .text("The first time you make a successful run on Archives each turn, the Corp must trash 1 card from HQ.")
        .when_first_each_turn(
            makes_successful_run_on(&[ServerId::Archives]),
            [performed_by(Corp, trash(choose(1, &[in_hand_of(Corp)])))],
        )
        .named("the first archives run of the turn")
        .build()
}

/// Edward Kim: Humanity's Hammer — Identity: Natural. Link 1.
/// "Trash the first operation you access each turn at no cost."
///
/// COMPLETE. One sentence carrying its own condition: the occurrence is
/// 7.3.6's access with the sentence's card-type stipulation, and "the first …
/// each turn" is 9.6.5c's ordinal on it, so the second operation accessed in
/// a turn does not meet the condition at all.
///
/// "At no cost" is not an instruction — it is the absence of one. 7.5.4 lets
/// the Runner trash an accessed card by PAYING its trash cost with the basic
/// trash ability; this sentence trashes the card outright, so there is
/// nothing to pay and no card to name: 1.15.2's target is the card being
/// accessed, which the access itself already fixed.
pub fn edward_kim() -> Card {
    card("Edward Kim: Humanity's Hammer")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .link(1)
        .text("Trash the first operation you access each turn at no cost.")
        .when_first_each_turn(accesses_a(CardType::Operation), [trash(accessed_card())])
        .named("the first operation accessed this turn")
        .build()
}

/// Esâ Afontov: Eco-Insurrectionist — Identity: Cyborg. Link 0.
/// "The first time each turn you suffer core damage, you may draw 1 card and
///  sabotage 2. (The Corp trashes 2 cards of their choice from HQ and/or the
///  top of R&D.)"
///
/// COMPLETE. The parenthesis is 1.4's reminder text: it restates what 10.16's
/// sabotage keyword already is, so it is not a second instruction.
///
/// "You suffer core damage" is 10.4.1's damage with the sentence's stipulation
/// about the KIND riding on the condition as content — the same shape the
/// interrupt side already states with "…would suffer net damage" — and
/// 9.6.5c's ordinal counts the occurrences from the turn's start.
///
/// "Draw 1 card and sabotage 2" is one printed sentence, so one instruction
/// (9.11.3): splitting it would invent a checkpoint between the draw and the
/// sabotage that the card does not print.
pub fn esa_afontov() -> Card {
    card("Esâ Afontov: Eco-Insurrectionist")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Cyborg"])
        .text("The first time each turn you suffer core damage, you may draw 1 card and sabotage 2. (The Corp trashes 2 cards of their choice from HQ and/or the top of R&D.)")
        .may_when_first_each_turn(
            suffers_damage(DamageKind::Core),
            [combined([draw(Runner, 1), sabotage(2)])],
        )
        .named("the first core damage of the turn")
        .build()
}

/// MaxX: Maximum Punk Rock — Identity: G-mod. Link 0.
/// "When your turn begins, trash the top 2 cards of your stack. Draw 1 card."
///
/// COMPLETE. TWO printed sentences on one condition, so two instructions —
/// 9.11.3's ordinary reading, and the one case where the boundary is real:
/// the trash finishes, a checkpoint occurs, and only then is the draw
/// imminent, which is what lets a card trashed by the first sentence act
/// before the second.
///
/// "The top 2 cards of your stack" names a zone, so 1.15.2c's play-area
/// restriction lifts and the two cards are the targets the description
/// itself fixes — no announcement is made.
pub fn maxx() -> Card {
    card("MaxX: Maximum Punk Rock")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("When your turn begins, trash the top 2 cards of your stack. Draw 1 card.")
        .when(turn_begins(Runner), [trash(top_of_stack(amount(2))), draw(Runner, 1)])
        .named("maximum punk rock")
        .build()
}

/// Nathaniel "Gnat" Hall: One-of-a-Kind — Identity: Natural. Link 0.
/// "When your turn begins, gain 1[credit] if you have 2 or fewer cards in
///  your grip."
///
/// COMPLETE. "If you have 2 or fewer cards in your grip" is 9.6.5c's
/// additional requirement listed inside the trigger condition, so it is
/// checked when the condition would be met — at the start of the turn — and
/// not again when the ability resolves.
///
/// The grip is described with the ordinary filter words, and naming a zone is
/// what lifts 1.15.2c's play-area default: without it the count would be of
/// installed cards, which is not what the sentence says.
pub fn nathaniel_gnat_hall() -> Card {
    card("Nathaniel \"Gnat\" Hall: One-of-a-Kind")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("When your turn begins, gain 1[credit] if you have 2 or fewer cards in your grip.")
        .when(
            turn_begins_if(Runner, &[board_has_at_most(&[in_hand_of(Runner)], 2)]),
            [gain(Runner, 1)],
        )
        .named("one-of-a-kind")
        .build()
}

/// Noise: Hacker Extraordinaire — Identity: G-mod. Link 0.
/// "Whenever you install a virus program, the Corp trashes the top card of
///  R&D."
///
/// COMPLETE. Two stipulations about one occurrence — 2.15's card type and
/// 2.16's subtype — both riding on 8.5's install condition as content. The
/// sentence names no zone the card came from, so the condition names none
/// either: a virus program installed out of the heap or the stack meets it
/// just as one installed from the grip does.
///
/// "The Corp trashes the top card of R&D" is 1.14.5's attribution. The card
/// is named by the description, not chosen, so naming the Corp changes
/// nothing the Corp decides — but 10.3.1a makes the trash the CORP's, which
/// is what puts the card in Archives facedown rather than faceup.
pub fn noise() -> Card {
    card("Noise: Hacker Extraordinaire")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("Whenever you install a virus program, the Corp trashes the top card of R&D.")
        .when(
            installs_a_subtyped(Runner, CardType::Program, "Virus"),
            [performed_by(Corp, trash(top_of_rnd(amount(1))))],
        )
        .named("hacker extraordinaire")
        .build()
}

/// Quetzal: Free Spirit — Identity: G-mod. Link 0.
/// "Once per turn → 0[credit]: Break 1 barrier subroutine."
///
/// COMPLETE. Everything before the colon is the cost — a printed 0[credit],
/// which 1.16.2a makes a payable cost of nothing rather than no cost at all —
/// and "Once per turn →" is 9.3.6g's flag, which a PAID ability has something
/// to spend it with (9.1.6: a player uses a paid ability).
///
/// "Break 1 barrier subroutine" is a break ability but NOT an interface
/// ability: 9.3.6c gates an interface ability on the source's strength, and
/// an identity has no strength at all, so the subtype is the whole
/// restriction. 9.5.6c is what that restriction is — the ability is offered
/// only during an encounter with a barrier, and every subroutine of a barrier
/// is a barrier subroutine.
pub fn quetzal() -> Card {
    card("Quetzal: Free Spirit")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("Once per turn → 0[credit]: Break 1 barrier subroutine.")
        .paid_once_per_turn_during_encounters_with(credits(0), "Barrier", [break_subroutines(1)])
        .named("break 1 barrier subroutine")
        .build()
}

/// Valencia Estevez: The Angel of Cayambe — Identity: Natural. Link 0.
/// "The Corp starts the game with 1 bad publicity."
///
/// COMPLETE. A fact about the game's SETUP, not an ability: there is no
/// condition to meet and nothing to resolve, and by the time the first turn
/// begins it has already happened. It goes where Andromeda's starting hand of
/// nine goes — among the printed facts — and it says only how much, because
/// 10.6 makes bad publicity always the Corp's, which is what lets a RUNNER
/// identity print it about the other player.
pub fn valencia_estevez() -> Card {
    card("Valencia Estevez: The Angel of Cayambe")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("The Corp starts the game with 1 bad publicity.")
        .starting_bad_publicity(1)
        .build()
}

/// Null: Whistleblower — Identity: Natural. Link 0.
/// "Once per turn → When you encounter a piece of ice, you may trash 1 card
///  from your grip. If you do, that ice gets –2 strength for the remainder of
///  this run."
///
/// COMPLETE. "You may trash 1 card from your grip. If you do, …" is 1.16.11a's
/// OPTIONAL COST: the trash is not an effect the ability produces but the
/// price of the one that follows, which is why the two sentences are one
/// instruction and why declining costs nothing.
///
/// "1 card from your grip" names a zone, and that is what lifts 1.15.2c's
/// installed-cards default — so the cards offered are the grip's, and the
/// Runner chooses which. 9.3.6g's flag has something to spend it with because
/// the ability's effect is optional (9.6.9d), which 9.1.6 requires: nothing
/// ever expends the flag on an entirely mandatory ability.
///
/// "That ice" is 1.15.4's back-reference to the ice of the encounter that met
/// the condition, so nothing is announced, and "for the remainder of this
/// run" is the duration the sentence names — the modification outlives the
/// encounter it was made in and dies with the run.
pub fn null_whistleblower() -> Card {
    card("Null: Whistleblower")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("Once per turn → When you encounter a piece of ice, you may trash 1 card from your grip. If you do, that ice gets –2 strength for the remainder of this run.")
        .when_once_per_turn(
            encounters_any_ice(),
            [may_pay(
                trash_cards_from_hand_of(Runner, 1),
                modify_strength_of(encountered_ice(), -2, WantedDuration::ThisRun),
            )],
        )
        .named("whistleblower")
        .build()
}

/// Ryō "Phoenix" Ōno: Out of the Ashes — Identity: G-mod. Link 0.
/// "The first time each turn a run becomes successful after a subroutine
///  resolved during that run, gain 1[credit] and the Corp trashes 1 card from
///  HQ."
///
/// COMPLETE. The condition is 6.8.4's declaration of success with the
/// sentence's own stipulation inside it — "after a subroutine resolved
/// during that run" — which is what keeps the printed ordinal honest: a
/// successful run with no subroutine resolved does not meet the condition at
/// all, so it does not spend the one time each turn. The stipulation is a
/// fact of the declaration's MOMENT and rides on its record, because 9.6.5c
/// re-asks this condition of every earlier change in the turn and the run's
/// history window has closed by then — a state-read requirement here would
/// let a plain successful run spend the one time it never met.
///
/// "A subroutine resolved during that run" is the run's whole history and not
/// the last encounter's: a subroutine that resolved on the first piece of ice
/// counts when the run becomes successful several servers' worth of ice
/// later. A subroutine that was broken never resolves (9.8.7), and one
/// resolved through a 9.8.9 replacement still resolves from the ice.
///
/// "Gain 1[credit] and the Corp trashes 1 card from HQ" is one printed
/// sentence, so one instruction (9.11.3). "The Corp trashes" is 1.14.5's
/// attribution — the choice of card is the Corp's, though 9.1.1a makes the
/// Runner the controller of a Runner identity's ability.
pub fn ryo_phoenix_ono() -> Card {
    card("Ryō \"Phoenix\" Ōno: Out of the Ashes")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("The first time each turn a run becomes successful after a subroutine resolved during that run, gain 1[credit] and the Corp trashes 1 card from HQ.")
        .when_first_each_turn(
            makes_successful_run_after_subroutine_resolved(),
            [combined([
                gain(Runner, 1),
                performed_by(Corp, trash(choose(1, &[in_hand_of(Corp)]))),
            ])],
        )
        .named("out of the ashes")
        .build()
}

/// Reina Roja: Freedom Fighter — Identity: Cyborg, G-mod. Link 1.
/// "The first piece of ice the Corp rezzes each turn costs 1[credit] more to
///  rez."
///
/// COMPLETE. A DECLARATION about an inherent cost (1.16.4a), the same one the
/// McCaffreys state about installs and with the other polarity: 9.3.5 applies
/// it continuously, so it is read wherever the rez cost is calculated — by
/// 8.1.2d's payment and by the affordability question that decides whether
/// the (R) option is offered at all, which is what makes an unaffordable ice
/// stay unrezzed rather than be rezzed for free.
///
/// "The first … each turn" is 9.6.5c's ordinal read of the rez: the increase
/// reaches the rez only while the Corp has rezzed no piece of ice yet this
/// turn. It counts ICE rezzed, not cards — an asset rezzed first leaves the
/// first piece of ice still the first piece of ice.
///
/// "The Corp rezzes" needs no words of its own: 8.1.4f makes rezzing the
/// Corp's alone, which is what lets a RUNNER identity print a sentence about
/// it.
pub fn reina_roja() -> Card {
    card("Reina Roja: Freedom Fighter")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Cyborg", "G-mod"])
        .link(1)
        .text("The first piece of ice the Corp rezzes each turn costs 1[credit] more to rez.")
        .declares([first_rezzed_each_turn_costs_more(&[of_type(CardType::Ice)], 1)])
        .build()
}

/// René "Loup" Arcemont: Party Animal — Identity: G-mod. Link 0.
/// "The first time each turn you trash a card you are accessing, gain
///  1[credit] and draw 1 card."
///
/// COMPLETE. The condition is 8.2's trash with two stipulations — 1.14.5's
/// "YOU trash" and 7.1.2's "a card you are accessing" — and 9.6.5c's ordinal
/// about the occurrence.
///
/// "A card you are accessing" names no zone and no card type: the accessed
/// card may be in HQ, R&D, Archives or a server's root, and every one of them
/// counts. It is not the same as "an installed card": a card trashed off the
/// board while some OTHER card is being accessed does not meet it, and a card
/// trashed out of HQ during a breach does.
///
/// Both ways of trashing it count, because the sentence distinguishes
/// neither: 7.5.4's basic trash ability, paid for out of the access, and a
/// card ability that trashes the accessed card.
///
/// "Gain 1[credit] and draw 1 card" is one printed sentence, so one
/// instruction (9.11.3).
pub fn rene_loup_arcemont() -> Card {
    card("René \"Loup\" Arcemont: Party Animal")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("The first time each turn you trash a card you are accessing, gain 1[credit] and draw 1 card.")
        .when_first_each_turn(
            trashes_the_card_being_accessed(Runner),
            [combined([gain(Runner, 1), draw(Runner, 1)])],
        )
        .named("party animal")
        .build()
}

/// Whizzard: Master Gamer — Identity: Natural. Link 0.
/// "3[recurring-credit]
///  Use these credits to trash cards."
///
/// COMPLETE. The first line is 1.10.5's shorthand — three credits placed on
/// the identity as soon as it is active (1.10.5b), topped back up to three at
/// step 5.7.1c of every Runner turn and never past it (1.10.5d) — and the
/// second is 1.10.3c: hosted credits may only be spent as the card's ability
/// allows, and this card allows one class of payment.
///
/// The whole of the second sentence is which cards it means. Miss Bones prints
/// "installed cards" and needs no description at all, because 1.15.2c already
/// reads a description naming no zone that way. This card prints "cards", and
/// the card the Runner pays 7.5.4's basic trash ability for while breaching HQ
/// or R&D is not installed — so the description has to say the wider thing out
/// loud, and "from any location" is that word.
pub fn whizzard() -> Card {
    card("Whizzard: Master Gamer")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("3[recurring-credit]")
        .text("Use these credits to trash cards.")
        .recurring_credits(3)
        .credits_only_for_trashing(&[in_any_location()])
        .build()
}

/// Wyvern: Chemically Enhanced — Identity: G-mod. Link 0.
/// "Draft format only.
///  You must maintain the order of your heap.
///  Whenever you trash a Corp card, if you have more [anarch] cards installed
///  than any other faction, shuffle the top card of your heap into your
///  stack."
///
/// COMPLETE. Three printed lines. The first is a FORMAT restriction, settled
/// before deck construction and never read during play (The Masque's whole
/// card is that sentence).
///
/// The second is a DECLARATION, and it is what makes the third sayable: CR
/// 4.4.2 is that discard piles are not ordered — "a player may freely arrange
/// the cards in their discard pile in any order at any time" — so a heap has
/// no top card to name until a card takes that freedom away. Without this
/// line the last line would describe nothing at all.
///
/// The third is one conditional ability. Its leading "if" is 9.6.5c's
/// additional requirement listed inside the trigger condition, so it is asked
/// when the trash occurs and not again when the ability resolves; the
/// requirement is the comparison across the faction partition of the Runner's
/// installed cards (2.13) that every draft identity opens with. The
/// occurrence is per card trashed (9.6.4b), so a sentence trashing three Corp
/// cards meets it three times.
///
/// "Shuffle the top card of your heap into your stack" is the Jackson
/// movement said about the other pile: the card enters the deck — a hidden
/// zone, so 1.12.3 makes it a new object — and the stack is shuffled.
pub fn wyvern() -> Card {
    card("Wyvern: Chemically Enhanced")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["G-mod"])
        .text("Draft format only.")
        .text("You must maintain the order of your heap.")
        .text("Whenever you trash a Corp card, if you have more [anarch] cards installed than any other faction, shuffle the top card of your heap into your stack.")
        .declares([discard_pile_is_ordered(Runner)])
        .when(
            runner_trashes_a_corp_card_if(&[more_cards_of_this_faction_than_any_other(
                "Anarch",
                &[installed_runner_card()],
            )]),
            [shuffle_into_deck(top_of_heap(amount(1)), Runner)],
        )
        .named("chemically enhanced")
        .build()
}

/// Omar Keung: Conspiracy Theorist — Identity: Natural. Link 0.
/// "Once per turn → [click]: Run Archives. If that run would be declared
///  successful, change the attacked server to HQ or R&D for the remainder of
///  that run."
///
/// COMPLETE. Two printed sentences, so two instructions (9.11.3) — except
/// that the second cannot BE the ability's second instruction: 6.1's run is a
/// nested timing structure, and everything after the instruction that
/// initiates one resolves once that run is over. The second sentence names
/// "that run", so it belongs to the run, which is exactly where 6.7.4's "If
/// successful" clause of an initiating effect goes. This one is the same
/// clause a step earlier, and 9.9.1's "would" is what moves it: an interrupt,
/// relevant to the imminence of 6.9.5a's declaration.
///
/// One instruction earlier is the whole point. The Success Phase's own step
/// declares the run successful against the attacked server as it stands when
/// it resolves, and 6.9.5b then breaches that same server — so an ability
/// that changes it during the interrupt window over 6.9.5a changes what the
/// run succeeded on and what is breached, while one reacting AFTER the
/// declaration would change neither.
///
/// "HQ or R&D" is 9.11.4g's option choice inside that one instruction, and
/// 6.1.2d is what makes each branch honest: the attacked server changes
/// DIRECTLY, without the Runner moving, so nothing on the way into HQ or R&D
/// is approached or encountered.
///
/// "For the remainder of that run" is not a duration to maintain. There is
/// one attacked server and this instruction sets it; the run ending is what
/// ends it, exactly as the run beginning is what set it in the first place.
pub fn omar_keung() -> Card {
    card("Omar Keung: Conspiracy Theorist")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("Once per turn → [click]: Run Archives. If that run would be declared successful, change the attacked server to HQ or R&D for the remainder of that run.")
        .paid_once_per_turn(
            clicks(1),
            [run_then_if_would_be_successful(
                ServerId::Archives,
                [choose_one([
                    ("HQ", vec![change_attacked_server(ServerId::Hq)]),
                    ("R&D", vec![change_attacked_server(ServerId::Rnd)]),
                ])],
            )],
        )
        .named("run archives")
        .build()
}

/// Topan: Ormas Leader — Identity: Natural. Link 0.
/// "Once per turn → [click]: Install 1 card from your grip, paying 2[credit]
///  less. When you install that card, suffer 1 meat damage."
///
/// COMPLETE. Two printed sentences, and the second is why this identity
/// waited: its condition is met WHILE the first sentence's install
/// instruction is resolving, so a delayed conditional created after the
/// install would wait for the next one (9.6.13), and a plain "you install a
/// card" condition would be met by every install the Runner makes. The
/// occurrence now records which ability's resolution performed the install,
/// and this condition compares that record against its own card — so the
/// basic action's install does not meet it, and neither does any other
/// card's.
///
/// "1 card" is untyped, and 8.5.3 is what narrows it — events are never
/// installed, so one is not a valid target (1.15.3) — enforced where targets
/// are announced rather than written into a description the card does not
/// print. "Paying 2[credit] less" is 1.16.6's reduction of the install cost
/// alone, the same word Khan uses. The damage is MANDATORY, and the Runner is
/// responsible for it: the identity is the Runner's own card (9.1.1a).
pub fn topan() -> Card {
    card("Topan: Ormas Leader")
        .runner()
        .identity()
        .faction("Anarch")
        .subtypes(&["Natural"])
        .text("Once per turn → [click]: Install 1 card from your grip, paying 2[credit] less. When you install that card, suffer 1 meat damage.")
        .paid_once_per_turn(
            clicks(1),
            [install_paying_less(
                choose(1, &[in_hand_of(Runner)]),
                InstallDest::RunnerChoiceHostOrRig,
                2,
            )],
        )
        .named("ormas leader")
        .when(installs_that_card(), [meat_damage(Runner, 1)])
        .build()
}

/// Every Anarch identity this module carries, in the order the queue reached
/// them.
pub fn identities() -> Vec<Card> {
    vec![
        alice_merchant(),
        wyvern(),
        whizzard(),
        reina_roja(),
        rene_loup_arcemont(),
        edward_kim(),
        esa_afontov(),
        maxx(),
        nathaniel_gnat_hall(),
        noise(),
        quetzal(),
        valencia_estevez(),
        null_whistleblower(),
        ryo_phoenix_ono(),
        omar_keung(),
        topan(),
    ]
}
