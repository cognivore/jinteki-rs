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
//! The deck is written in the queue's printed order, and a card an earlier
//! deck already carries is reused from there rather than copied — Sure Gamble,
//! Rebirth, Boomerang, Desperado and Paperclip are all Andromeda's, and
//! Paperclip in particular is the card [`black_orchestra`] and [`mkultra`] are
//! written as siblings of. Every card of the queue's list is now written;
//! "written" is not "complete", and the tick-boxes are what count.

use crate::edsl::*;

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Blackmail — Event: Run. Cost 1.
/// "Play only if the Corp has at least 1 bad publicity.
///  Run any server. The Corp cannot rez ice during that run."
///
/// PARTIAL — two of its three sentences are expressed, and the third is not.
///
/// "Play only if …" is a PLAY RESTRICTION and not a cost, and the difference
/// decides where it goes. 9.3.3 states a restriction as a condition on
/// whether an ability may be used at all; 1.16.1 makes an unpayable cost stop
/// the play too, but a cost is something a player PAYS — 1.16.1's "an effect a
/// player must resolve" — and nothing here is paid or given up. The Corp's bad
/// publicity is a fact about the game state, so `play_only_if` (9.1.8c) is the
/// shape, and `StaticDecl::PlayOnlyIf` is the word for it.
///
/// The fact itself is 10.6.1's bad publicity — counters on the Corp, which
/// 1.9.5d makes a player-level count exactly as 1.9.5c makes tags one — read
/// as a quantity with the printed threshold on it. 9.1.8c is what keeps the
/// declaration working from HQ, where the card is otherwise inactive, and it
/// is read at the moment the play is offered rather than stamped anywhere
/// (9.12.2), so bad publicity gained or removed mid-turn is seen. It is NOT
/// 10.6.2's bad publicity fund: those are credits the Runner controls, and
/// 10.6.3c leaves them alone once a run has begun.
///
/// UNIMPLEMENTED: "The Corp cannot rez ice during that run", which is marked
/// for two reasons and neither is the acts. `ProhibitedAction::Rez` exists and
/// `WantedDuration::ThisRun` exists; what does not is a moment to state them
/// in, because the sentence is printed AFTER the sentence that runs and
/// 5.2.2b suspends the play until the run completes — so an instruction
/// written after the run would create its lingering effect when the run is
/// already over. Beside that, the prohibition would have to reach the ice by
/// DESCRIPTION and the lingering prohibition names one object fixed at
/// creation. Both capabilities are on MEZZIE-QUEUE.md's Blockers. Reordering
/// the prohibition in front of the run is not an answer either: it would
/// invent an instruction boundary the card does not print (9.11.3).
pub fn blackmail() -> Card {
    card("Blackmail")
        .runner()
        .event()
        .faction("Neutral")
        .subtypes(&["Run"])
        .cost(1)
        .text("Play only if the Corp has at least 1 bad publicity.")
        .text("Run any server. The Corp cannot rez ice during that run.")
        .declares([play_only_if(&[at_least(bad_publicity_of(Corp), 1)])])
        .named("play only if the Corp has bad publicity")
        .play([run_any_server([])])
        .unimplemented("The Corp cannot rez ice during that run.")
        .build()
}

/// Hacktivist Meeting — Event: Current. Cost 1.
/// "This card is not trashed until another current is played or an agenda is
///  scored.
///  As an additional cost to rez non-ice cards, the Corp must randomly trash
///  a card from HQ."
///
/// PARTIAL — the current's own sentence is expressed and the tax is not.
///
/// The first sentence is 8.6.6c said the way Employee Strike says it: 3.7.1b
/// prints the current EVENT's ending occurrences, and the pair — another
/// current played, or an agenda scored — is content on one declaration rather
/// than two.
///
/// The second is a 1.16.10 ADDITIONAL COST, and additional costs in the kernel
/// are either a fact printed on the card being paid for (`.additional_rez_cost`,
/// which is Archer's "to rez THIS card") or one of the three declarations that
/// tax an act by description — stealing, accessing, and the basic run action.
/// Rezzing a DESCRIBED card ("non-ice cards") is none of them, and the payment
/// this one asks for is not credits: it is a random trash out of HQ, which
/// `Instruction::TrashRandomFromHand` performs as an effect but which no `Cost`
/// component can charge. Written with the credit-cost words it would tax the
/// wrong resource by the wrong amount, so it is marked.
pub fn hacktivist_meeting() -> Card {
    card("Hacktivist Meeting")
        .runner()
        .event()
        .faction("Anarch")
        .subtypes(&["Current"])
        .cost(1)
        .text("This card is not trashed until another current is played or an agenda is scored.")
        .text("As an additional cost to rez non-ice cards, the Corp must randomly trash a card from HQ.")
        .declares([not_trashed_until_an_agenda_is_scored()])
        .unimplemented("As an additional cost to rez non-ice cards, the Corp must randomly trash a card from HQ.")
        .build()
}

/// I've Had Worse — Event. Cost 1.
/// "Draw 3 cards.
///  Whenever I've Had Worse is trashed by taking net or meat damage, draw 3
///  cards."
///
/// PARTIAL — the draw is expressed; the damage sentence is marked, and the
/// reason is a DAMAGE KIND rather than a missing mechanism.
///
/// Note first what this card is NOT: it neither prevents nor reduces damage.
/// 9.9.7's prevention and 9.9.6's reduction both act on an imminent effect
/// before it resolves; this sentence is met AFTER the damage has resolved and
/// taken this card with it, and what it does is draw. The card that "softens"
/// damage does so only by replacing a card the damage already took.
///
/// `TriggerCond::SelfTrashedByDamage` is the condition, and 9.1.8b is what
/// makes it reachable at all: the ability has to be active in the GRIP for the
/// trash to meet it, and the kernel keeps it active in the heap it lands in.
/// What the condition cannot carry is WHICH damage. 10.4.2a resolves meat and
/// net damage by trashing randomly-chosen cards from the grip — and 10.4.2b
/// resolves CORE damage the same way, adding only the permanent hand-size
/// reduction. So a condition that says nothing about the kind is met by core
/// damage too, and this card's "net or meat" excludes it. That is a silent
/// OVER-trigger: three cards drawn where the card promises none, from a
/// Stimhack in this very deck. Marked rather than approximated.
pub fn ive_had_worse() -> Card {
    card("I've Had Worse")
        .runner()
        .event()
        .faction("Anarch")
        .cost(1)
        .text("Draw 3 cards.")
        .text("Whenever I've Had Worse is trashed by taking net or meat damage, draw 3 cards.")
        .play([draw(Runner, 3)])
        .unimplemented("Whenever I've Had Worse is trashed by taking net or meat damage, draw 3 cards.")
        .build()
}

/// Inject — Event. Cost 1.
/// "Reveal the top 4 cards of your stack and trash all programs revealed.
///  Gain 1[credit] for each program trashed, and add the rest of the revealed
///  cards to your grip."
///
/// PARTIAL — the first sentence, which is the whole of the reveal and the
/// trash; the payout is marked.
///
/// 9.11.3 is why the first sentence is ONE instruction with two effects rather
/// than two: "reveal … **and** trash …" joins them, so one checkpoint and one
/// reaction window cover both, and `combined` is the atom for that. 1.15.2
/// announces the trash's targets before either half resolves, which is exactly
/// what lets the description name cards that are still on top of the stack —
/// 1.15.2c's play-area default lifts because "the top 4 cards of your stack"
/// names a zone (4.2.1).
///
/// The second sentence needs two things the vocabulary does not have, and they
/// are the same thing twice: a way to name THE CARDS THIS ABILITY REVEALED
/// after the instruction that revealed them has finished. 1.21.3's reveal
/// records nothing on the resolving ability — `LookedAtByThisAbility` is the
/// record 1.21.2's LOOK keeps, and 1.21.5 keeps the two words distinct — so
/// neither "for each program trashed" nor "the rest of the revealed cards" has
/// anything to refer back to. Counting the top of the stack again would count
/// different cards, because the trash already moved some of them.
pub fn inject() -> Card {
    card("Inject")
        .runner()
        .event()
        .faction("Anarch")
        .cost(1)
        .text("Reveal the top 4 cards of your stack and trash all programs revealed. Gain 1[credit] for each program trashed, and add the rest of the revealed cards to your grip.")
        .play([combined([
            reveal(top_of_stack(amount(4))),
            // 4.2.1 + 1.15.2c: the zone is named, so the description reaches
            // the stack; `TopOfDeckOf` is the criterion form of the same
            // window `top_of_stack` names as a fixed group.
            trash(all_matching(&[
                TargetFilter::TopOfDeckOf { side: Runner, n: 4 },
                of_type(CardType::Program),
            ])),
        ])])
        .unimplemented("Gain 1[credit] for each program trashed, and add the rest of the revealed cards to your grip.")
        .build()
}

/// Levy AR Lab Access — Event. Cost 5.
/// "Shuffle your grip and heap into your stack. Draw 5 cards. Remove Levy AR
///  Lab Access from the game instead of trashing it."
///
/// COMPLETE. Three printed sentences, and the third is why the other two can
/// be written in the order they are printed.
///
/// "Shuffle your grip and heap into your stack" is one instruction over one
/// description: 4.2.3's shuffle takes the cards named, and the "or" between
/// two whole zones is `any_of` — a disjunction whose every branch names a
/// zone, which is what lifts 1.15.2c's installed-cards default for the whole
/// list. 8.3 is what the shuffle then guarantees: the stack's order becomes
/// unknown to both players, and 1.12.3 makes each shuffled card a NEW object,
/// so nothing that was watching one of them can act on it afterwards.
///
/// This card is not among them, and 8.6.7a is why: a played event sits in the
/// PLAY AREA while its ability resolves, so it is in neither the grip nor the
/// heap when the description is read. 8.6.7g is what would put it in the heap
/// afterwards — and the third sentence replaces exactly that step, sending it
/// out of the game instead, which is why the card cannot come back through a
/// second Levy. The declaration is `removed_from_game_instead_of_trashed`
/// (8.2.2: it is still trashed; only where it lands changes).
///
/// The draw follows the shuffle as its own instruction, so the five cards come
/// off a stack that already holds the grip and the heap — which is the whole
/// point of the card and the reason the printed order is not decoration.
pub fn levy_ar_lab_access() -> Card {
    card("Levy AR Lab Access")
        .runner()
        .event()
        .faction("Shaper")
        .cost(5)
        .text("Shuffle your grip and heap into your stack. Draw 5 cards. Remove Levy AR Lab Access from the game instead of trashing it.")
        .play([
            shuffle_into_deck(
                all_matching(&[any_of(&[&[in_hand_of(Runner)], &[in_heap()]])]),
                Runner,
            ),
            draw(Runner, 5),
        ])
        .declares([removed_from_game_instead_of_trashed()])
        .build()
}

/// Mad Dash — Event: Run. Cost 0.
/// "Run any server. When that run ends, if you stole an agenda during that
///  run, add this event to your score area as an agenda worth 1 agenda point.
///  Otherwise, suffer 1 meat damage."
///
/// PARTIAL — the run; the pay-off is marked.
///
/// The run is 6.9.1a's announcement with no server named, so the Runner
/// declares the attacked one as the run is initiated.
///
/// Everything after it waits on ONE missing question: "if you stole an agenda
/// during that run". 7.5's steal is recorded — `TriggerCond::RunnerStealsAgenda`
/// reads the occurrence as it happens — but a "when this run ends" ability
/// asks about the run's HISTORY, and the requirement vocabulary has no term
/// counting steals inside a window the way `AccessesThisRun` counts accesses
/// or `SubroutinesBrokenThisRun` counts breaks. Without it the branch cannot
/// be chosen, and a card that guessed would either hand out an agenda point it
/// did not earn or do a meat damage it did not deserve.
///
/// (`add_to_score_area(this_card(), Runner, Some(1))` says the rest of the
/// first branch, and 1.17.3e/f is what makes it an ADD and not a steal: a card
/// added to a score area is not stolen, so nothing that watches for a steal
/// sees this. It is left out only because the condition that reaches it
/// cannot be stated.)
pub fn mad_dash() -> Card {
    card("Mad Dash")
        .runner()
        .event()
        .faction("Neutral")
        .subtypes(&["Run"])
        .cost(0)
        .text("Run any server. When that run ends, if you stole an agenda during that run, add this event to your score area as an agenda worth 1 agenda point. Otherwise, suffer 1 meat damage.")
        .play([run_any_server([])])
        .unimplemented("When that run ends, if you stole an agenda during that run, add this event to your score area as an agenda worth 1 agenda point.")
        .unimplemented("Otherwise, suffer 1 meat damage.")
        .build()
}

/// Moshing — Event. Cost 0.
/// "As an additional cost to play this event, trash 3 cards from your grip.
///  Gain 3[credit] and draw 3 cards."
///
/// COMPLETE, and the whole card is the difference between a COST and an
/// EFFECT.
///
/// "As an additional cost to play this event" is 1.16.10 verbatim, so the
/// three cards are a fact printed on the card and not an instruction:
/// `.additional_play_cost(…)`, beside the play cost, paid at step 8.6.7c
/// before anything resolves. 1.16.1b is what that buys — a Runner holding
/// fewer than three other cards cannot play it at all, and a Runner who can
/// pays before knowing what the draw brings. Written as an instruction instead
/// it would be a trash the card CAUSED, reachable by a prevention effect and
/// resolvable after the gain, and every one of those differences is wrong.
///
/// Which three cards go is the payer's announcement (1.16.10's criteria are
/// the ordinary description vocabulary), and naming the grip is what lifts
/// 1.15.2c's installed-cards default.
///
/// "Gain 3[credit] and draw 3 cards" is 9.11.3's one sentence, one
/// instruction: `combined`, so one checkpoint and one reaction window cover
/// both halves and nothing gets to act between the credits and the cards.
pub fn moshing() -> Card {
    card("Moshing")
        .runner()
        .event()
        .faction("Anarch")
        .cost(0)
        .text("As an additional cost to play this event, trash 3 cards from your grip.")
        .text("Gain 3[credit] and draw 3 cards.")
        .additional_play_cost(trash_cards_from_hand_of(Runner, 3))
        .play([combined([gain(Runner, 3), draw(Runner, 3)])])
        .build()
}

/// Raindrops Cut Stone — Event: Run. Cost 1.
/// "Run any server. Whenever a subroutine resolves during that run (including
///  a subroutine that ends the run), place 1 power counter on this event.
///  When that run ends, draw 1 card for each hosted power counter and gain
///  3[credit]."
///
/// PARTIAL — the run and the pay-off are expressed; the counter sentence is
/// marked, which makes the draw half of the pay-off read zero until it lands.
///
/// The pay-off is a CONDITIONAL ability of the card and not a delayed one
/// (9.6.13): 4.6.4e keeps a played event active in the play area for the whole
/// of its resolution, and 5.2.2b suspends that resolution until the run ends,
/// so the event is still there — with its counters on it — when "when that run
/// ends" is met. 9.11.3 again makes "draw … and gain …" one instruction.
///
/// The counter sentence needs a trigger condition met when a SUBROUTINE
/// RESOLVES. 9.8.10's occurrence is recorded — `Quantity::SubroutinesResolvedThisRun`
/// counts it from the change log — but no condition is met BY it, and the
/// count is not a substitute for the counters: 1.12.1 makes a counter an
/// object, so a power counter on this event is a thing other cards can see,
/// count and remove, and a number recomputed from history is not. The
/// parenthetical is 6.10's reminder that a subroutine ending the run still
/// resolved, which is the same occurrence and needs no separate word.
pub fn raindrops_cut_stone() -> Card {
    card("Raindrops Cut Stone")
        .runner()
        .event()
        .faction("Anarch")
        .subtypes(&["Run"])
        .cost(1)
        .text("Run any server. Whenever a subroutine resolves during that run (including a subroutine that ends the run), place 1 power counter on this event.")
        .text("When that run ends, draw 1 card for each hosted power counter and gain 3[credit].")
        .play([run_any_server([])])
        .when(
            run_ends(),
            [combined([draw_q(Runner, per_hosted_counter(CounterKind::Power)), gain(Runner, 3)])],
        )
        .named("when that run ends")
        .unimplemented("Whenever a subroutine resolves during that run (including a subroutine that ends the run), place 1 power counter on this event.")
        .build()
}

/// Steelskin Scarring — Event. Cost 1.
/// "Draw 3 cards.
///  When this event is trashed from your grip or stack, you may draw 2 cards."
///
/// PARTIAL — the draw is expressed; the second sentence is marked.
///
/// Like [`ive_had_worse`], this card prevents and reduces nothing: 9.9.7 and
/// 9.9.6 act on an imminent effect, and this sentence is met after a trash has
/// already happened. It replaces cards; it does not save them.
///
/// What it wants is the twin of `TriggerCond::SelfTrashedByDamage`: THIS
/// card's own trash, with the zone it was trashed FROM as content. The kernel
/// has the zone-carrying condition (`CardTrashed { from_zone, … }`) and the
/// self-scoped one, and no condition that is both — and the scope is not
/// cosmetic, because 9.1.8b decides where the ability is ACTIVE from the
/// condition alone. A grip and a stack are 4.3 and 4.2's hidden zones, where
/// 4.4.4 leaves everything inactive unless the ability says otherwise, so a
/// condition that cannot name them is a condition that can never be met.
pub fn steelskin_scarring() -> Card {
    card("Steelskin Scarring")
        .runner()
        .event()
        .faction("Anarch")
        .cost(1)
        .text("Draw 3 cards.")
        .text("When this event is trashed from your grip or stack, you may draw 2 cards.")
        .play([draw(Runner, 3)])
        .unimplemented("When this event is trashed from your grip or stack, you may draw 2 cards.")
        .build()
}

/// Stimhack — Event: Run. Cost 0.
/// "Place 9[credit] on this event, then run any server. During that run,
///  hosted credits are considered to be in your credit pool. When that run
///  ends, suffer 1 core damage. This damage cannot be prevented."
///
/// PARTIAL — three of its four sentences; the credit pool is marked.
///
/// "Place 9[credit] on this event, then run any server" joins its two effects
/// with "THEN", and that word is why they are two instructions rather than
/// `Combined`'s one. 9.11.3's single-instruction reading is what an "and"
/// sentence gets, because "and" says nothing about order and a boundary would
/// invent a checkpoint the card does not print; "then" prints the order
/// itself, and 6.9's run is a timing structure that suspends the ability
/// resolving it (5.2.2b) — so the credits must be on the card before the run
/// is announced, which is the only order that makes the rest of the card mean
/// anything. (The `an_and_sentence_is_one_instruction_not_two` guard pins the
/// "and" shape, which this is not.)
///
/// The last two sentences are one instruction as well, and that is the point
/// of the second of them: 9.9.7's prevention acts on an imminent damage
/// effect, and `Instruction::DamageUnpreventable` is the same damage with
/// `EffectAtom::unpreventable` set, so "suffer 1 core damage" and "this damage
/// cannot be prevented" are one effect with a flag and not a damage followed
/// by a rule about it. 10.4.2b is what the core damage then does: a random
/// card out of the grip AND a permanent hand-size reduction. It rides on a
/// conditional ability rather than on a later instruction of the play, because
/// 4.6.4e keeps the event active in the play area for its whole resolution and
/// "when that run ends" names 6.10's moment rather than "afterwards".
///
/// "Hosted credits are considered to be in your credit pool" is marked, and
/// the near miss is worth naming: `CreditUse::AnyPayment` would let the nine
/// credits pay for anything, which is most of what the sentence is FOR. It is
/// not what the sentence SAYS. 1.13.3 keeps hosted credits out of the pool
/// entirely — they are never "on" the player — and this card waives exactly
/// that, so the credits are read by anything that reads the pool: a forced
/// 1.10.3b loss during the run takes them, and a quantity asking how many
/// credits the Runner has counts them. `CreditUse` cannot say so, and neither
/// can any lingering effect, so writing the permission alone would be a
/// silent UNDER-reach in every one of those places.
pub fn stimhack() -> Card {
    card("Stimhack")
        .runner()
        .event()
        .faction("Anarch")
        .subtypes(&["Run"])
        .cost(0)
        .text("Place 9[credit] on this event, then run any server. During that run, hosted credits are considered to be in your credit pool. When that run ends, suffer 1 core damage. This damage cannot be prevented.")
        .play([place(CounterKind::Credit, 9), run_any_server([])])
        .when(
            run_ends(),
            [Instruction::DamageUnpreventable {
                kind: DamageKind::Core,
                amount: amount(1),
                responsible: Runner,
            }],
        )
        .named("when that run ends")
        .unimplemented("During that run, hosted credits are considered to be in your credit pool.")
        .build()
}

// ---------------------------------------------------------------------------
// Hardware
// ---------------------------------------------------------------------------

/// Zer0 — Hardware. Install 1. ◆
/// "Once per turn → [click], suffer 1 net damage: Gain 1[credit] and draw 2
///  cards."
///
/// COMPLETE. One printed sentence, and every hard call in it is about the
/// COLON: everything to the left is cost, everything to the right is effect.
///
/// 1.16.1 is what makes "suffer 1 net damage" a cost at all — a cost may be
/// "an effect a player must resolve", and a player who cannot pay the full
/// cost cannot use the ability. That is not the same card as one that DOES
/// the Runner 1 net damage: the damage here is paid before anything is
/// gained, its trash is not something the ability caused, and 1.16.1a keeps
/// an optional interrupt from cancelling the act of paying it. `Cost` is
/// therefore where it goes, beside the [click], as one cost paid all at once.
///
/// 1.11.3c is why nobody has to say where this ability is offered: a paid
/// ability that begins with [click] IS an action, so the Runner is offered it
/// in an action window and never in a paid one.
///
/// "Once per turn →" is 9.3.6g's flag, and the distinction that matters is
/// what spends it. 9.3.6g points at 9.1.6, and 9.1.6 is about USING: a player
/// who resolves an optional ability has used it. That is a fact about this
/// ability's own history, not about the turn's — which is what
/// `when_first_each_turn`'s 9.6.5c stipulation would have said instead, and
/// 9.6.5c is about an OCCURRENCE the turn has already seen, so a second copy
/// of the card would inherit it. Two copies of Zer0 could not both be
/// installed (2.2.1), but the difference is real for the whole class and
/// `crates/jinteki-cr/tests/using_abilities.rs` is where it is pinned.
///
/// 9.11.3: "Gain 1[credit] and draw 2 cards" is ONE sentence, so it is one
/// instruction with two effects — one checkpoint and one reaction window
/// cover both halves, and nothing gets to react between the credit and the
/// cards.
pub fn zer0() -> Card {
    card("Zer0")
        .runner()
        .hardware()
        .faction("Anarch")
        .cost(1)
        .unique()
        .text("Once per turn → [click], suffer 1 net damage: Gain 1[credit] and draw 2 cards.")
        .paid_once_per_turn(
            clicks(1).plus_cost(suffer_net_damage(1)),
            [combined([gain(Runner, 1), draw(Runner, 2)])],
        )
        .named("bleed for a credit and two cards")
        .build()
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Clan Vengeance — Resource: Clan. Install 3.
/// "Whenever you suffer any amount of damage, place 1 power counter on Clan
///  Vengeance.
///  [trash]: Trash 1 card from HQ at random for each power counter on Clan
///  Vengeance."
///
/// COMPLETE, and both sentences turn on 1.12.1's "counters are objects".
///
/// "Any amount of damage" is the sentence naming no kind and no size, which is
/// `suffers_any_damage()` — 10.4.1's occurrence with both stipulations empty.
/// One counter per OCCURRENCE and not per point: 9.12.2c aggregates a damage
/// instruction into a single effect, so three net damage is one thing
/// suffered and places one counter, which is exactly what "any amount" says.
///
/// The paid ability is where the counters have to be objects. [trash] is a
/// 1.16.1 trigger cost, paid before the ability resolves, so by the time the
/// trash instruction reads "for each power counter on Clan Vengeance" the card
/// is in the heap. 9.5.5 is the rule that saves it: counters set aside by a
/// [trash] trigger cost are still counted, and `Quantity::CountersOnSource`
/// implements exactly that. The threshold is re-read at resolution rather than
/// remembered from any earlier moment (9.6.5d), so a counter placed by damage
/// suffered while the ability was already offered still counts.
///
/// "At random" is 1.15.2b: a card taken at random is not announced, so nobody
/// chooses and no interrupt can act on the choice — `Instruction::TrashRandomFromHand`
/// is that movement, and the count is the calculated quantity above.
pub fn clan_vengeance() -> Card {
    card("Clan Vengeance")
        .runner()
        .resource()
        .faction("Anarch")
        .subtypes(&["Clan"])
        .cost(3)
        .text("Whenever you suffer any amount of damage, place 1 power counter on Clan Vengeance.")
        .text("[trash]: Trash 1 card from HQ at random for each power counter on Clan Vengeance.")
        .when(suffers_any_damage(), [place(CounterKind::Power, 1)])
        .named("a counter for the damage")
        .paid(
            trash_this_card(),
            // 1.15.2b: taken at random, so there is no announcement at all.
            [Instruction::TrashRandomFromHand {
                side: Corp,
                count: per_hosted_counter(CounterKind::Power),
            }],
        )
        .named("trash HQ at random, one card per counter")
        .build()
}

/// Mystic Maemi — Resource: Companion - Virtual. Install 1. ◆
/// "When your turn begins and whenever you steal an agenda, place 1[credit] on
///  this resource.
///  You can spend hosted credits to play events.
///  When your turn ends, if there are 3 or more hosted credits, you must trash
///  1 card from your grip at random or trash this resource."
///
/// PARTIAL — the first and third sentences; the spend permission is marked.
///
/// The first sentence names TWO occurrences and one ordinal-free instruction,
/// so it is one conditional ability whose condition is a disjunction
/// (`either_of`) and not two abilities: written as two, a printed "the first
/// time each turn" in front of it would be spent twice, and the shape has to
/// be right whether or not this card prints one.
///
/// The third is 5.6.3's formal end of the turn carrying 9.6.5c's requirement
/// ("if there are 3 or more hosted credits"), and the choice after it is
/// 9.11.4g's optioned effect — the one exception that really does split a
/// sentence. "You MUST … or …" is not a permission: both options are real, one
/// of them happens, and 9.11.4g is how a card offers a choice between two
/// effects. The random trash is 1.15.2b's unannounced pick again.
///
/// "You can spend hosted credits to play events" is 1.10.3c: hosted credits
/// may be spent only as the hosting card's ability allows, and the allowance
/// is content on `CreditUse`. The five allowances the kernel names are any
/// payment, trashing described cards, USING described cards (9.1.6a's paid
/// ability trigger cost), a trace attempt's spend steps, and advancing — and
/// PLAYING a card is none of them. It is not the "using" allowance under
/// another name, for the same reason rezzing is not: 8.6.7c pays a play cost
/// inside the play procedure and uses no ability at all, so writing it as
/// `UsingAbilitiesOf` would let the credits pay for paid abilities they may
/// not pay for and STILL not pay for a play. Written as `AnyPayment` it would
/// pay for installs, trashes and traces the card never allowed. Both readings
/// are wrong in opposite directions, so the sentence is marked.
pub fn mystic_maemi() -> Card {
    card("Mystic Maemi")
        .runner()
        .resource()
        .faction("Anarch")
        .subtypes(&["Companion", "Virtual"])
        .cost(1)
        .unique()
        .text("When your turn begins and whenever you steal an agenda, place 1[credit] on this resource.")
        .text("You can spend hosted credits to play events.")
        .text("When your turn ends, if there are 3 or more hosted credits, you must trash 1 card from your grip at random or trash this resource.")
        .when(
            either_of(&[turn_begins(Runner), runner_steals_agenda()]),
            [place(CounterKind::Credit, 1)],
        )
        .named("a credit at the turn's start or on a steal")
        .when(
            turn_ends_if(Runner, &[hosted_counters_at_least(CounterKind::Credit, 3)]),
            [choose_one([
                (
                    "trash 1 card from your grip at random",
                    vec![Instruction::TrashRandomFromHand { side: Runner, count: amount(1) }],
                ),
                ("trash this resource", vec![trash_self()]),
            ])],
        )
        .named("pay her at the turn's end")
        .unimplemented("You can spend hosted credits to play events.")
        .build()
}

/// Same Old Thing — Resource. Install 0.
/// "[click], [click], [trash]: Play an event from your heap (paying its play
///  cost)."
///
/// COMPLETE. One printed line, and everything before the colon is the cost:
/// two clicks and this card, paid together as one 1.16.1 payment. 1.11.3c is
/// why nothing has to say where it is offered — a paid ability whose cost
/// includes [click] IS an action (5.2.1a), so it appears in an action window
/// and never in a paid one.
///
/// The effect reaches into a DISCARD PILE, and 9.1.8b is the rule that lets
/// it: 4.4.4 leaves the heap's cards inactive, but this ability is not one of
/// theirs — it belongs to an installed resource, and an ability may name cards
/// wherever it says it does. The heap is therefore a criterion on the
/// description (`in_heap()`), which is also what lifts 1.15.2c's installed-
/// cards default, and "an event" is the ordinary type criterion beside it.
///
/// The parenthetical is 8.6.7b restated rather than an extra instruction: an
/// effect that plays a card pays its play cost unless the effect says
/// otherwise, and this one does not — so a Runner who cannot afford the event
/// cannot choose it, and the two clicks and the card are spent all the same
/// if they choose nothing at all.
pub fn same_old_thing() -> Card {
    card("Same Old Thing")
        .runner()
        .resource()
        .faction("Neutral")
        .cost(0)
        .text("[click], [click], [trash]: Play an event from your heap (paying its play cost).")
        .paid(
            clicks(2).plus_cost(trash_this_card()),
            [play_card(choose(1, &[in_heap(), of_type(CardType::Event)]))],
        )
        .named("replay an event out of the heap")
        .build()
}

/// Tsakhia "Bankhar" Gantulga — Resource: Connection. Install 1. ◆
/// "When your turn begins, you may choose a server.
///  During the first encounter each turn with a piece of ice protecting the
///  chosen server, whenever the Corp would resolve a subroutine, instead they
///  resolve \"[subroutine] Do 1 net damage.\"."
///
/// PARTIAL — the choice is expressed; the replacement is marked.
///
/// "You may choose a server" is 9.10.3's MAINTAINED CHOICE — an ordinary
/// 1.15.2 announcement whose value the card remembers for as long as it is
/// active (9.10.3c) — and the thing chosen is a SERVER (1.15.1b), not a card,
/// which is why it is `ChoiceSpec::AnyServer` and not a target description.
/// "You may" is the optionality on the conditional ability, so a Runner who
/// wants last turn's server to stay chosen simply declines.
///
/// The second sentence's MECHANISM exists: `StaticDecl::ReplaceSubroutineResolution`
/// is 9.9.2's "instead of <the effect>, <these instructions>" said of a
/// subroutine, which is the right shape for a sentence that swaps what the
/// Corp resolves without touching whether they resolve it (9.8.9: the
/// replacement still resolves from the ice, so it is still a subroutine
/// resolving). What cannot be said is WHEN it applies. The declaration is
/// either always on or gated by `declares_while`'s state requirements, and
/// this card's gate is an ENCOUNTER (6.5) matching a description — with ice
/// protecting the remembered server — carrying an ORDINAL, "the first
/// encounter each turn". No requirement asks about the encounter in progress,
/// and no static ability carries an ordinal at all. An always-on declaration
/// would rewrite every subroutine on every server for the whole game, which is
/// the largest possible over-reach, so the sentence is marked.
pub fn tsakhia_bankhar_gantulga() -> Card {
    card("Tsakhia \"Bankhar\" Gantulga")
        .runner()
        .resource()
        .faction("Anarch")
        .subtypes(&["Connection"])
        .cost(1)
        .unique()
        .text("When your turn begins, you may choose a server.")
        .text("During the first encounter each turn with a piece of ice protecting the chosen server, whenever the Corp would resolve a subroutine, instead they resolve \"[subroutine] Do 1 net damage.\".")
        .may_when(
            turn_begins(Runner),
            // 9.10.3 + 1.15.1b: a server is one of the things an instruction
            // can direct a player to choose, remembered while this card is
            // active (9.10.3c). The sentence excludes nothing.
            [Instruction::MaintainChoice {
                key: "bankhar server",
                of: jinteki_cr::instr::ChoiceSpec::AnyServer { excluding: None },
                duration: WantedDuration::WhileSourceActive,
            }],
        )
        .named("choose a server for the turn")
        .unimplemented("During the first encounter each turn with a piece of ice protecting the chosen server, whenever the Corp would resolve a subroutine, instead they resolve \"[subroutine] Do 1 net damage.\".")
        .build()
}

// ---------------------------------------------------------------------------
// Programs
// ---------------------------------------------------------------------------

/// Black Orchestra — Program: Icebreaker - Decoder. Install 3, strength 2,
/// 1[mu].
/// "Whenever you encounter a code gate, you may install this program from
///  your heap.
///  3[credit]: +2 strength. Then, if this program can interface with the code
///  gate you are encountering, break up to 2 subroutines."
///
/// COMPLETE. The conspiracy breaker, written as Paperclip's sibling and
/// deliberately not as a second mechanism: the two cards differ only in the
/// subtype they name and in the fact that Paperclip announces X (1.16.2c)
/// where this one prints its numbers.
///
/// The first sentence works from the HEAP, and 9.1.8b is what puts it there:
/// "abilities stating that they are active in a particular zone are active in
/// that zone". Without the statement, 4.4.4 would leave it inactive with the
/// rest of the discard pile. The zone is written as a requirement on the
/// trigger condition (9.6.5c) rather than as a test inside the effect,
/// because that is the half of the ability 9.1.8b reads — and it is also what
/// keeps the same ability from offering an install out of the grip, where the
/// printed words do not reach.
///
/// "If this program can interface with the code gate you are encountering" is
/// deliberately NOT 9.3.6c's interface flag, even though 3.9.5g/h is exactly
/// the question it asks. The flag is checked when the ability is OFFERED;
/// this sentence is checked when the break instruction RESOLVES, which
/// 9.6.5d permits explicitly — "the condition only needs to be met when the
/// relevant instructions resolve" — and that is the whole card: a Black
/// Orchestra whose printed 2 had to match the code gate before pumping could
/// never break anything it was not already big enough for.
///
/// 9.8.6 governs what the break may take: only unbroken subroutines can be
/// chosen, and "up to 2" is the count the Runner announces within that.
pub fn black_orchestra() -> Card {
    card("Black Orchestra")
        .runner()
        .program()
        .faction("Anarch")
        .subtypes(&["Icebreaker", "Decoder"])
        .cost(3)
        .strength(2)
        .memory(1)
        .text("Whenever you encounter a code gate, you may install this program from your heap.")
        .text("3[credit]: +2 strength. Then, if this program can interface with the code gate you are encountering, break up to 2 subroutines.")
        .may_when(
            encounters_a("Code Gate", &[source_in_discard()]),
            [install_this_card()],
        )
        .named("out of the heap")
        .paid(
            credits(3),
            [
                pump(2),
                if_met(
                    &[can_interface_with_the_encountered("Code Gate")],
                    [break_up_to(2)],
                ),
            ],
        )
        .named("pump and break")
        .build()
}

/// MKUltra — Program: Icebreaker - Killer. Install 2, strength 1, 1[mu].
/// "Whenever you encounter a sentry, you may install this program from your
///  heap.
///  3[credit]: +2 strength. Then, if this program can interface with the
///  sentry you are encountering, break up to 2 subroutines."
///
/// COMPLETE. The same two sentences as [`black_orchestra`] with a different
/// subtype in both of them, which is exactly what ARCHITECTURE §12 rule 2
/// asks for: the subtype is CONTENT on `encounters_a` and on
/// `can_interface_with_the_encountered`, so the whole conspiracy class is one
/// shape and not three.
///
/// The two rules that decide it are the same two. 9.1.8b puts the install
/// ability in the heap that 4.4.4 would otherwise make inactive; 9.6.5d puts
/// the interface question after "+2 strength" instead of before it, which for
/// this card is the difference between a breaker that works and one that
/// cannot touch a sentry stronger than its printed 1. 3.9.5g is the
/// comparison being made and 3.9.5h is the subtype half of it.
pub fn mkultra() -> Card {
    card("MKUltra")
        .runner()
        .program()
        .faction("Anarch")
        .subtypes(&["Icebreaker", "Killer"])
        .cost(2)
        .strength(1)
        .memory(1)
        .text("Whenever you encounter a sentry, you may install this program from your heap.")
        .text("3[credit]: +2 strength. Then, if this program can interface with the sentry you are encountering, break up to 2 subroutines.")
        .may_when(
            encounters_a("Sentry", &[source_in_discard()]),
            [install_this_card()],
        )
        .named("out of the heap")
        .paid(
            credits(3),
            [
                pump(2),
                if_met(
                    &[can_interface_with_the_encountered("Sentry")],
                    [break_up_to(2)],
                ),
            ],
        )
        .named("pump and break")
        .build()
}

/// Rezeki — Program. Install 2, 1[mu].
/// "When your turn begins, gain 1[credit]."
///
/// COMPLETE, and a CONDITIONAL ability (9.6.1) rather than a static
/// declaration — the one call this card asks anyone to make.
///
/// 9.4.1 is what a static ability is: one that "continuously affects the game
/// as long as it is active", that can contain declarations, and that does
/// "not resolve or have associated priority windows". Rezeki's sentence does
/// none of that. It names a moment ("when your turn begins"), it happens once
/// at that moment rather than continuously, and the credit it produces is an
/// effect that resolves — so 9.6.1's definition is the one it meets: an
/// ability with a primary condition and one or more instructions, triggered
/// at a specific point in the game. 9.6.2 then makes it pending in the next
/// reaction window, where anything watching a gain can see it; 9.4.4's "no
/// durations, no lingering effects" would have been the wrong shape for a
/// one-off payout entirely.
///
/// "YOUR turn" is the stipulation (9.6.5c) that keeps it off the Corp's turn:
/// the side is content on the condition, not a second condition.
pub fn rezeki() -> Card {
    card("Rezeki")
        .runner()
        .program()
        .faction("Shaper")
        .cost(2)
        .memory(1)
        .text("When your turn begins, gain 1[credit].")
        .when(turn_begins(Runner), [gain(Runner, 1)])
        .build()
}

/// The deck so far, in the order `docs/vm/MEZZIE-QUEUE.md` lists it.
///
/// Mixed provenance on purpose: the identity came out of the identity queue,
/// and Sure Gamble, Rebirth, Boomerang, Desperado and Paperclip are
/// Andromeda's, written once and played by both decks. Everything else is this
/// module's own. All 23 distinct cards of the queue's list are now here — but
/// "written" is not "complete": a card carrying `.unimplemented(…)` is listed
/// exactly like any other, and the tick-boxes plus `cr::readiness()` are what
/// keep an unfinished one off a table (SYS-D-12).
pub fn deck() -> Vec<Card> {
    vec![
        super::identities::runner_anarch::valencia_estevez(),
        blackmail(),
        hacktivist_meeting(),
        ive_had_worse(),
        inject(),
        levy_ar_lab_access(),
        mad_dash(),
        moshing(),
        raindrops_cut_stone(),
        super::andromeda::rebirth(),
        steelskin_scarring(),
        stimhack(),
        super::andromeda::sure_gamble(),
        super::andromeda::boomerang(),
        super::andromeda::desperado(),
        zer0(),
        clan_vengeance(),
        mystic_maemi(),
        same_old_thing(),
        tsakhia_bankhar_gantulga(),
        black_orchestra(),
        mkultra(),
        super::andromeda::paperclip(),
        rezeki(),
    ]
}

/// CR 1.5.4a: the additional identities this deck brings along with it, kept
/// in a pile outside the game. This deck plays Rebirth, so it needs Anarchs
/// for Rebirth's "another identity from the same faction" (1.5.4b) to name a
/// real choice — but which identities a player brings is a decision at the
/// table, and enlisting one is a change to what this deck IS. Still empty
/// after the wave that wrote the deck's own cards, and deliberately: the deck
/// carries partial cards, so `cr::readiness()` — which holds a pile card to
/// the same bar as a deck card — would refuse it either way, and the call is
/// worth making once against a deck that is otherwise ready.
pub fn additional_identities() -> Vec<Card> {
    Vec::new()
}
