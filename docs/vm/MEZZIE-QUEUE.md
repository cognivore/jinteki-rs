# Mezzie's two decks — the card queue

User-mandated, after the 150-identity campaign. Two decks, 99 cards, **37 distinct
cards left to write**. Both identities are already complete (they came out of
the identity queue), so this is cards only.

Same bar as every deck before it. CR SYS-D-12: a deck with one partial card
cannot be played at all, so there is no partial credit — a deck is finished
when every one of its cards is `is_complete()`. ARCHITECTURE §12: cards are
built out of the PUBLIC vocabulary of `jinteki-cards`, no card names in the
kernel, and a card that needs a kernel word it does not have is SKIPPED and
recorded as a blocker rather than approximated.

A ticked box means the card function exists AND carries no `.unimplemented(…)`
AND has a behaviour test asserting the printed sentence. `tools/assess-drift.sh`
checks the first two; the third is on the wave that wrote it.

## Modules

- `crates/jinteki-cards/src/decks/mezzie_asa.rs` — Asa Group: Security Through Vigilance
- `crates/jinteki-cards/src/decks/mezzie_valencia.rs` — Valencia Estevez: The Angel of Cayambe

Cards shared with a finished deck are already done — reuse the existing
function, never write a second copy of a card.

---

## Mezzie's Asa — Asa Group: Security Through Vigilance (49 cards, 24 distinct)

Identity is COMPLETE. Printed text below is from
`~/Github/jinteki/netrunner-cards-json`, which is the source of truth.

- [x] **Global Food Initiative** ×1 — agenda · Initiative · adv 5/3
      "Global Food Initiative is worth 1 fewer agenda point while in the Runner's score area."
- [x] **Luminal Transubstantiation** ×1 — agenda · Research · adv 3/2
      "When you score this agenda, gain [click][click][click]. You cannot score agendas for the remainder of the turn. / Limit 1 per deck."
- [ ] **Project Vacheron** ×3 — agenda · Research · adv 5/3
      "[interrupt] → When this agenda would be added to the Runnerʼs score area from anywhere except Archives, instead it is added to their score area with 4 hosted agenda counters. / While this agenda is in the Runnerʼs score area with 1 or more hosted agenda counters, it is worth 0 agenda points and gains “When the Runnerʼs turn begins, remove 1 hosted agenda counter.“"
- [x] **Project Vitruvius** ×3 — agenda · Research · adv 3/2
      "When you score this agenda, place 1 agenda counter on it for each hosted advancement counter past 3. / Hosted agenda counter: Add 1 card from Archives to HQ."
- [x] **Estelle Moon** ◆ ×3 — asset · Executive · cost 2, trash 3
      "Whenever you install a card in the root of a remote server, place 1 power counter on this asset. / [trash]: For each power counter on this asset, gain 2[credit] and draw 1 card."
- [x] **Jeeves Model Bioroids** ◆ ×1 — asset · Alliance · cost 2, trash 5
      "This card costs 0 influence if you have 6 or more non-alliance [haas-bioroid] cards in your deck. / The first time you spend 3[click] on the same action each turn, gain [click]."
- [x] **Lakshmi Smartfabrics** ×2 — asset · cost 1, trash 3
      "Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics. / X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn."
- [x] **Marilyn Campaign** ×1 — asset · Advertisement · cost 2, trash 3
      "When you rez this asset, load 8[credit] onto it. When it is empty, trash it. / When your turn begins, take 2[credit] from this asset. / [interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. (It is still considered trashed.)"
- [x] **MCA Austerity Policy** ◆ ×2 — asset · cost 1, trash 3
      "Once per turn → [click]: Place 1 power counter on this asset. When the Runner's next turn begins, they lose [click]. / [click], [trash], 3 hosted power counters: Gain [click][click][click][click]."
- [x] **Mumba Temple** ×3 — asset · Alliance - Facility · cost 1, trash 3
      "This card costs 0 influence if you have 15 or fewer ice in your deck. / 2[recurring-credit] / Use these credits to rez cards."
- [x] **Rashida Jaheem** ◆ ×3 — asset · Character · cost 0, trash 1
      "When your turn begins, you may trash Rashida Jaheem to gain 3[credit] and draw 3 cards."
- [x] **Spin Doctor** ◆ ×3 — asset · Character · cost 0, trash 2
      "When you rez this asset, draw 2 cards. / Remove this asset from the game: Shuffle up to 2 cards from Archives into R&D."
- [x] **Enhanced Login Protocol** ×2 — operation · Current · cost 2
      "This operation is not trashed until another current is played or an agenda is stolen. / As an additional cost to take the basic action to run a server for the first time each turn, the Runner must spend [click]."
- [x] **Flood the Market** ×1 — operation · Double · cost 3
      "As an additional cost to play this operation, spend [click]. / Choose 1 installed card you can advance. Place 1 advancement counter on that card for each remote server that has a card in its root and is protected by ice."
- [x] **Friends in High Places** ×3 — operation · Terminal · cost 2
      "After you resolve this operation, end your action phase. / Install up to 2 cards from Archives (paying all install costs)."
- [x] **Fully Operational** ×3 — operation · cost 1
      "Gain 2[credit] or draw 2 cards. Repeat this process for each remote server that has a card in its root and is protected by ice."
- [x] **Ash 2X3ZB9CY** ◆ ×1 — upgrade · Bioroid · cost 2, trash 3
      "Whenever there is a successful run on this server, Trace[4]. If successful, the Runner cannot access any cards other than Ash 2X3ZB9CY for the remainder of this run."
- [x] **Manegarm Skunkworks** ◆ ×1 — upgrade · cost 2, trash 3
      "Whenever the Runner approaches this server, end the run unless they either spend [click][click] or pay 5[credit]."
- [x] **Tatu-Bola** ×1 — ice · Barrier · cost 2, str 1
      "When the Runner passes this ice, you may swap it with a piece of ice from HQ. If you do, gain 4[credit]. (The new ice is installed unrezzed. You do not pay an install cost.) / [subroutine] End the run."
- [x] **Vanilla** ×3 — ice · Barrier · cost 0, str 0
      "[subroutine] End the run."
- [x] **Fairchild 3.0** ×2 — ice · Code Gate - Bioroid - AP · cost 6, str 5
      "Lose [click][click][click]: Break up to 3 subroutines on this ice. Only the Runner can use this ability. / [subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards. / [subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards. / [subroutine] Do 1 core damage or end the run."
- [x] **Vertigo** ×1 — ice · Code Gate · cost 1, str 1
      "When the Runner passes this ice, if they have no [click] remaining, they cannot steal or trash Corp cards for the remainder of this run. / [subroutine] The Runner loses [click]."
- [x] **Drafter** ×2 — ice · Sentry · cost 3, str 3
      "[subroutine] You may add 1 card from Archives to HQ. / [subroutine] You may install 1 card from Archives or HQ, ignoring all costs."
- [x] **Tour Guide** ×3 — ice · Sentry · cost 2, str 0
      "This ice gains "[subroutine] End the run." for each rezzed asset."

---

## Mezzie's Valencia — Valencia Estevez: The Angel of Cayambe (50 cards, 23 distinct)

Identity is COMPLETE.

- [x] **Blackmail** ×3 — event · Run · cost 1
      "Play only if the Corp has at least 1 bad publicity. / Run any server. The Corp cannot rez ice during that run."
- [x] **Hacktivist Meeting** ×3 — event · Current · cost 1
      "This card is not trashed until another current is played or an agenda is scored. / As an additional cost to rez non-ice cards, the Corp must randomly trash a card from HQ."
- [x] **I've Had Worse** ×3 — event · cost 1
      "Draw 3 cards. / Whenever I've Had Worse is trashed by taking net or meat damage, draw 3 cards."
- [x] **Inject** ×3 — event · cost 1
      "Reveal the top 4 cards of your stack and trash all programs revealed. Gain 1[credit] for each program trashed, and add the rest of the revealed cards to your grip."
- [x] **Levy AR Lab Access** ×1 — event · cost 5
      "Shuffle your grip and heap into your stack. Draw 5 cards. Remove Levy AR Lab Access from the game instead of trashing it."
- [x] **Mad Dash** ×1 — event · Run · cost 0
      "Run any server. When that run ends, if you stole an agenda during that run, add this event to your score area as an agenda worth 1 agenda point. Otherwise, suffer 1 meat damage."
- [x] **Moshing** ×3 — event · cost 0
      "As an additional cost to play this event, trash 3 cards from your grip. / Gain 3[credit] and draw 3 cards."
- [x] **Raindrops Cut Stone** ×2 — event · Run · cost 1
      "Run any server. Whenever a subroutine resolves during that run (including a subroutine that ends the run), place 1 power counter on this event. / When that run ends, draw 1 card for each hosted power counter and gain 3[credit]."
- [x] **Rebirth** ×1 — event · cost 0
      "Switch your identity with another identity from the same faction. Remove Rebirth from the game instead of trashing it. / Limit 1 per deck."
- [x] **Steelskin Scarring** ×3 — event · cost 1
      "Draw 3 cards. / When this event is trashed from your grip or stack, you may draw 2 cards."
- [x] **Stimhack** ×1 — event · Run · cost 0
      "Place 9[credit] on this event, then run any server. During that run, hosted credits are considered to be in your credit pool. When that run ends, suffer 1 core damage. This damage cannot be prevented."
- [x] **Sure Gamble** ×2 — event · cost 5
      "Gain 9[credit]."
- [x] **Boomerang** ◆ ×2 — hardware · cost 2
      "When you install this hardware, choose 1 installed piece of ice. Use this hardware only during encounters with that ice. / [trash]: Break up to 2 subroutines. When this run ends, if it was successful, you may shuffle 1 copy of Boomerang from your heap into your stack."
- [x] **Desperado** ◆ ×2 — hardware · Console · cost 3
      "+1[mu] / Gain 1[credit] whenever you make a successful run. / Limit 1 console per player."
- [x] **Zer0** ◆ ×3 — hardware · cost 1
      "Once per turn → [click], suffer 1 net damage: Gain 1[credit] and draw 2 cards."
- [x] **Clan Vengeance** ×3 — resource · Clan · cost 3
      "Whenever you suffer any amount of damage, place 1 power counter on Clan Vengeance. / [trash]: Trash 1 card from HQ at random for each power counter on Clan Vengeance."
- [x] **Mystic Maemi** ◆ ×3 — resource · Companion - Virtual · cost 1
      "When your turn begins and whenever you steal an agenda, place 1[credit] on this resource. / You can spend hosted credits to play events. / When your turn ends, if there are 3 or more hosted credits, you must trash 1 card from your grip at random or trash this resource."
- [x] **Same Old Thing** ×1 — resource · cost 0
      "[click], [click], [trash]: Play an event from your heap (paying its play cost)."
- [x] **Tsakhia "Bankhar" Gantulga** ◆ ×3 — resource · Connection · cost 1
      "When your turn begins, you may choose a server. / During the first encounter each turn with a piece of ice protecting the chosen server, whenever the Corp would resolve a subroutine, instead they resolve "[subroutine] Do 1 net damage."."
- [x] **Black Orchestra** ×2 — program · Icebreaker - Decoder · cost 3, str 2
      "Whenever you encounter a code gate, you may install this program from your heap. / 3[credit]: +2 strength. Then, if this program can interface with the code gate you are encountering, break up to 2 subroutines."
- [x] **MKUltra** ×2 — program · Icebreaker - Killer · cost 2, str 1
      "Whenever you encounter a sentry, you may install this program from your heap. / 3[credit]: +2 strength. Then, if this program can interface with the sentry you are encountering, break up to 2 subroutines."
- [x] **Paperclip** ×2 — program · Icebreaker - Fracter · cost 4, str 1
      "Whenever you encounter a barrier, you may install this program from your heap. / X[credit]: +X strength. Then, if this program can interface with the barrier you are encountering, break up to X subroutines."
- [x] **Rezeki** ×1 — program · cost 2
      "When your turn begins, gain 1[credit]."

---

## Blockers — kernel words these cards want, found while working the queue

**State after the `work/finish` wave: 46 of 47 ticked, 1 printed
sentence still unsayable, 1 card unticked.** Nine kernel words landed and
two cards turned out never to have been blocked at all. Entries marked
LANDED are kept, with what was built and what the entry got wrong, because
the standing lesson of this queue is that a blocker is a claim about the
kernel and claims have to be checked against it.

The one that remains, and what it is waiting on:

| Card | Waiting on |
|---|---|
| Project Vacheron | three: a CONJUNCTIVE stated condition, agenda points SET, a declaration granting a stated ability |


Never approximated. A card that needs one of these is left unticked with the
word it wants named here, exactly as the identity queue did it.

Each entry is a GENERAL capability (ARCHITECTURE §12): thresholds, polarity,
scope and windows are content on one atom, never a new atom per card. The
"wants it" line names the cards that ran into it, for whoever picks it up.

A blocker that is no longer true costs more than no blocker at all, because it
tells the next wave to skip a card it could finish. Two entries were deleted on
that ground when this list was re-read against the kernel:

- **An ability that names its controller (CR 1.14.4)** — landed as
  `AbilityDef.controller` / `used_only_by` / `paid_used_only_by` (CR 1.14.4b).
  **Fairchild 3.0** is written and ticked.
- **A trigger condition over the CLICKS spent on one action** — never actually
  missing. `TriggerCond::ClicksSpentOnAction { side, count }` has existed since
  the W13e wave under CR 1.16.4d's citation rather than 5.2.1's; it is read by
  the checkpoint scan, `tk::jeeves_like` is its shape, and
  `example_rule_inherent_cost_aggregates_1` drives it on a board. **Jeeves
  Model Bioroids** is written and ticked, as
  `when_first_each_turn(spends_clicks_on_one_action(Corp, 3), …)`.
  Writing it did turn up one defect in the word rather than a gap in the
  vocabulary, fixed in the same wave: 9.6.5c's ordinal was read off the bare
  `ClickSpent` records, so the FIRST click of a three-click action spent it and
  the third — the one the sentence is about — was refused as a repeat. The
  scan now asks "how many clicks had been spent on the action in progress AT
  THAT POINT", of the log, exactly as `same_action_run_at` already did for
  5.2.5's neighbour.
- **The two halves of Vertigo's first sentence** — both landed, from two
  kernel waves that ran in parallel and each recorded the other as the
  blocker. `at_most(clicks_of(Runner), 0)` is the number, and
  `ProhibitionScope::Matching` is the act, the player, the description and the
  duration. **Vertigo** is written and ticked. The "if" is CR 9.6.5**d** —
  Underworld Contact's word order, not Quantum Predictive Model's — so it is
  `if_met` inside the instruction, and 9.11.3 keeps the whole sentence one
  instruction.

### A stated condition that asks about the SOURCE and the game state at once (CR 9.3.7a)

`AbilityDef::condition` holds one `Condition`, and `StaticCond` is a flat list
of alternatives: an ability can state that it is active in a score area
(`SourceInScoreAreaOf`, which 9.1.8b also reads to keep it alive there) or that
some requirement about the game holds (`StateRequirement`), and never both. A
printed "while this agenda is in the Runner's score area **with 1 or more
hosted agenda counters**" is one stated condition with two clauses, and there
is no position for the second.

Wanted: the stated condition as a CONJUNCTION — one list, whose members are
the existing alternatives — so a sentence naming a zone and a state is one
condition with two clauses and not two abilities. 9.1.8b must go on reading the
zone clause wherever it appears in the list, or the ability is inactive in the
one zone it is about.

Wants it: **Project Vacheron** (its second sentence, which also wants the two
below).

### Agenda points SET rather than modified (CR 2.5 / 9.12.1a)

`StaticDecl::SelfAgendaPointsMod(Quantity)` is 9.12.1a's third stage — it adds
to the value, which is what Merger, Global Food Initiative and Project Beale
print. A card printing "it **is worth 0** agenda points" states the second
stage instead, and subtracting the printed value only lands on 0 while nothing
else is modifying it.

Wanted: the set as content beside the modification on the one declaration —
one position for the value with the STAGE as its content — so "worth 1 more",
"worth 1 fewer" and "worth 0" are one declaration with different content, and
9.12.1a's ordering does the rest.

Wants it: **Project Vacheron** ("it is worth 0 agenda points").

### A declaration that grants a STATED conditional ability (CR 9.1.9 / 9.10.2)

`StaticDecl::GainSubroutines { sub, count }` grants a stated SUBROUTINE and
`StaticDecl::GainAbilitiesOf { criteria }` copies another card's whole text,
and between them there is no way to write the commonest form of all: a card
that gains one ability the sentence spells out in quotation marks. The kernel
has the payload (`Payload::GrantedAbility { to, def }`, 9.10.2) and no
declaration reaches it.

Wanted: the stated ability as content on one declaration, the way
`GainSubroutines` already carries a stated subroutine — the ability being an
`AbilityDef` of any class, so "gains '[subroutine] End the run.'", "gains 'When
your turn begins, gain 1[credit].'" and "gains 'Hosted agenda counter: …'" are
one declaration with different content.

Wants it: **Project Vacheron** (gains "When the Runnerʼs turn begins, remove 1
hosted agenda counter.").

### The approach condition naming WHICH server — LANDED (CR 6.9.4g)

`TriggerCond::ServerApproached { this_server, on }` — the same two-position
shape `IcePassed` uses for the same distinction, so "this server" (Manegarm
Skunkworks), a named list, and Formicary's "a server" are one condition with
different content. Both positions empty is what every existing declaration
said, so nothing moved. Measured rather than assumed: the kernel test
`an_approach_condition_scoped_to_this_server_ignores_every_other` runs the
same upgrade against its own remote and against HQ.

**Manegarm Skunkworks** was left blocked on ONE word rather than two — the
alternative-cost entry below, which has since landed too.

### A nested cost with ALTERNATIVE costs — LANDED (CR 1.16.11b / 1.16.1)

`Instruction::NestedCostUnless { costs: Vec<Cost>, effect, payer }` — the ways
out as a LIST, exactly as the entry asked. One element is every existing site
(1.16.11b's ordinary "unless they pay 3[credit]"), so nothing moved; several
are "unless they **either** spend [click][click] **or** pay 5[credit]", one
instruction with two doors. `Cost` stays a conjunction and the list is the
disjunction over it, which is how the two nest in the printed words.

The filter is **1.16.1** rather than the 1.16.1b the entry named — "if a player
cannot pay the full cost … they cannot use the effect associated with that
cost" — asked of each alternative where the choice is OFFERED. 9.12.3c is the
analogy and not the rule: it governs a choice among *effects* in a "must"
ability, and the shape it describes ("must choose an effect that can be fully
resolved; if none can, the ability does nothing") is what the cost list does.
Filing it under 9.12.3c alone would have pointed the next reader at the wrong
sentence.

What the entry did not mention, and it is the half that costs the most:
`DecisionSpec::NestedCost` and `DecisionAnswer::PayNestedCost` both had to
learn WHICH cost. They are now `NestedCost { costs: Vec<Cost> }` — the payable
ones, in printed order — and `PayNestedCost(Option<usize>)`, an index into
that same list, with `None` for declining. The offered list is rebuilt from
one helper (`Vm::payable_nested_costs`) at the ask and at the answer, so the
two are the same list by construction. The ~20 driver call sites did NOT move:
`Reply::PayCost(true)` still means "pay", and now means `Some(0)` — the only
door wherever a sentence states one — with `Reply::PayCostWith(i)` for the
two-door case. The server's prompt renders one button per door, named by what
it costs, and a bare "Pay" when there is only one.

Measured on a real board by
`a_nested_cost_offers_only_the_alternatives_the_payer_can_pay`: five arms over
the same upgrade, holding one resource payable and starving the other, so that
which doors were offered, which was walked through and what it cost are all
visible. The arm with 1[click] and 4[credit] is the one no single-cost writing
could reach — neither door payable, no decision put, run ended.

**Manegarm Skunkworks** is written and ticked.

### A trash whose destination an ABILITY redirects — LANDED (CR 9.9.8a-b / 8.2.2)

`TrashDestination::ShuffledIntoOwnersDeck` (4.2.3 + 8.7.3 — a deck is ordered,
so a card entering it with no stated position goes in by a shuffle, and 1.12.3
then makes it a new object) and `Instruction::RedirectImminentTrash { cards,
to }`, the interrupt-effect that says where the cards of the trash ALREADY
IMMINENT go. Both halves the entry asked for, in two positions rather than one.

The optionality is NOT "one flag on the existing word", and that is the
entry's mistake. It cannot be: `StaticDecl::ReplaceTrashDestination` is read
inside `Vm::trash_card`, in the middle of a movement, where the kernel cannot
put a question to a player — `Vm::ask` records a pending decision and does not
unwind, so the movement would complete before the answer arrived. The printed
"you may" therefore lives where every other optional conditional keeps it: on
the INTERRUPT that carries the redirection (9.6.9c), triggered from the 9.9.4
window like any other. That is also what the card prints — "[interrupt] →" —
and what 9.9.8a/9.9.10 describe: an interrupt introducing a replacement for
the instruction already imminent, applied the moment it resolves.

So the two shapes divide by what they can reach, not by a flag. The 9.9.8b
static stays mandatory and stays read at the movement, which is what lets it
reach the trashes no instruction makes imminent (10.4.2's damage, 1.16.1a's
cost trashes). The 9.9.8a interrupt reaches exactly the trashes that HAVE an
imminence — which is all an interrupt can ever act on. The destination rides
the imminent atom (`EffectAtom::trash_to`), so a trash that is then PREVENTED
takes the redirection with it, where a note kept beside the atom would have
survived to redirect some later trash of the same card.

TWO DEFECTS FIXED, both found by writing the card, and the first is the one
that would have made it look implemented and do nothing:

- The basic trash ability trashed the card DIRECTLY from its payment
  continuation, with no imminence and so no interrupt window. CR 7.1.5 makes
  it an ability and 9.1.1g makes a non-static ability's text instructions, so
  its trash becomes imminent like every other; its own sibling,
  `PaymentCont::BasicTrashResourceAction`, already ran the basic action's
  effect through a rules-ability frame. Without this the Runner paying
  Marilyn's trash cost on access — the trash the card is really about — could
  never be redirected.
- The mid-access window asked "is this card in Archives?" as its way of
  saying "has this access already trashed it?". With the card redirected into
  R&D the proxy failed and the Runner was offered a second trash of a card
  that was no longer there. It now asks the trash RECORD, which 8.2.2
  guarantees exists wherever the card landed.

Measured by `marilyn_campaign_may_be_shuffled_into_rnd_instead_of_going_to_archives`:
the Runner trashes the rezzed asset on access, and taking the interrupt adds a
card to R&D while declining leaves it in Archives — with the trash recorded on
both arms, which is what the printed parenthetical promises.

**Marilyn Campaign** is written and ticked.

### A description stipulating agenda points, or the X announced for the cost — LANDED (CR 2.3 / 2.5 / 2.7 / 1.16.2c)

`TargetFilter::CharacteristicIs { of, cmp, value }`, exactly the word the entry
asked for: WHICH characteristic (`CardCharacteristic::PrintedCost |
AgendaPoints | Strength`), WHICH comparison (`NumericCmp::AtMost | AtLeast |
Exactly`) and the other side as a `Quantity`, which is what lets a description
be compared against a number the game state produced — 1.16.2c's announced X
above all. `PrintedCostAtMost(u32)` was folded into it rather than left beside
it, so the vocabulary has one numeric criterion and not three. The quantity is
a `&'static` reference so the filter vocabulary stays `Copy`, the way
`AnyOf` and `Not` already are.

The entry counted TWO words for this card. It is three, and the one it missed
is the one everything else hangs off: **"X hosted power counters" was not
sayable either.** `Cost::spend_counters` held a printed `u32`, and 1.16.2c's X
has to be ANNOUNCED — `spend_counters_any_source` is the announced one and is
the wrong word (Freedom Khumalo's counters come from any of the payer's cards;
these come off THIS one). The amount is now a `Quantity` position like
`Cost::credits`, `Cost::spend_x_counters` is the shorthand, and `Vm::x_bound`
learned that an X in a counter component is bounded by what the SOURCE hosts
and not by the payer's credit pool. Without it the card could not even be
paid for, and no amount of description vocabulary would have helped.

### The printed ORDINAL on an additional-cost declaration — LANDED (CR 1.16.10)

`StaticDecl::AdditionalRunActionCost` gains `first_each_turn: bool`, the field
`InherentCostMod` already carries and read the same way: from the change log,
the cost applying while no EARLIER basic run action has been taken this turn
(5.2.5a's action identity, asked of the turn). `false` at every existing site,
which is what Service Outage and this card's ORIGINAL printing say. **Enhanced
Login Protocol** is written and ticked.

### Hosted credits spendable on REZZING and on PLAYING — LANDED (CR 1.10.3c)

`CreditUse::Rezzing(criteria)` and `CreditUse::PlayingCards(criteria)`, with
`CreditPurpose::Rezzing`/`Playing` read at the one place each cost is paid.
Neither is `UsingAbilitiesOf` under another name, for the reason
`AdvancingCards` is not: 8.1.2's rez procedure and 8.6's play procedure each
pay a card's cost and use no ability at all. **Mumba Temple** and **Mystic
Maemi** are written and ticked.

One thing the word needed that the blocker did not mention, and it is the part
that would have made the card look implemented and do nothing: the purpose has
to be stated at the OFFER site as well as at the payment. `rez_affordable` and
the basic play action asked `cost_payable` with no purpose, so a card whose
credits are allowed "to rez cards" answered NO to affordability and the rez was
never offered at all — the payment would have worked and nothing ever reached
it. `paid_ability_cost_payable` had already solved exactly this for 9.1.6a and
says so in its doc; the two new sites now do the same.

### The install LOCATION as content on the install condition (CR 4.6.6e / 4.6.9d)

NOT blocking any card, but found here and wanted.
`TriggerCond::CardInstalledBy` carries one location word as a bool,
`into_remote_server`, and 4.6.6b makes that word "in the root of **or**
protecting" — one location, deliberately. A card saying only "in the root of a
remote server" is narrower, and there is no value to give the position that
says so.

Wanted: the location as content — root only, protecting only, root or
protecting, and central as readily as remote — one position with a value,
rather than a second flag beside the first.

Until then the narrower sentence is written long-hand, as the wider location
with the three card types a root can hold (4.6.6e allows a remote root exactly
"1 asset or agenda and any number of upgrades"; 4.6.9d puts every installed
piece of ice in a position protecting a server). That reaches the same installs
and is not an approximation — it is the same set said the other way round — and
**Estelle Moon** is ticked and tested on that shape, annotated in its doc
comment and in `installs_a_card_in_the_root_of_a_remote_server`. The behaviour
test runs the same two install actions into roots and into ice positions and
asserts the payout differs.

### A swap whose two sides are described SEPARATELY (CR 8.8.1 / 8.8.2 / 1.15.4)

NOT blocking any card, but wanted, and found here. `Instruction::SwapCards`
takes a target spec per side, but its 8.8.2 candidate filter ("only cards each
of which may occupy the other's location") looks for the partner inside the
*same* description — so it can express "swap 2 installed pieces of ice" and
cannot express "swap **it** with a piece of ice from HQ", where one side is
fixed by 1.15.4 and the other is chosen from a different zone. Written the
short way the swap finds no partner and silently does nothing.

Wanted: the 8.8.2 filter reading a side that announces nothing from the
position itself, so fixed/chosen and fixed/fixed swaps work like
chosen/chosen. Until then the sentence is written long-hand as one instruction
that announces the partner and then exchanges it with this card — same
instruction, same announcement, same windows — and **Tatu-Bola** is ticked and
tested on that shape, annotated in its doc comment.

### Two card-layer shorthands the EDSL does not name (CR 9.3.7a / 9.1.8a-b)

NOT blocking any card, and NOT a kernel gap: both kernel words exist and both
are correct. What is missing is a name for them in `crate::edsl`, whose
re-exports are otherwise everything a deck file is built out of. Neither
`AbilityDef` nor `Condition`/`StaticCond`/`AbilityFlag` is among them, so the
two abilities below cannot be spelled with a `use crate::edsl::*` line alone —
even though `CardBuilder::ability`, the documented escape hatch, exists to take
exactly them ("reach for it when a card wants a combination the shorthands do
not name … a static condition").

Wanted, as two calls beside `declares_while`:

- `declares_while_in_the_score_area_of(side, decls)` — a static ability whose
  stated condition is `StaticCond::SourceInScoreAreaOf`. It is not
  interchangeable with `declares_while`: 9.1.8b's first sentence reads THIS
  condition as the statement that keeps the ability active in the Runner's
  score area, and 4.5.4 leaves an agenda there inactive without it, so a
  declaration written with a `StateRequirement` about the same zone would never
  be read at all.
- an interrupt (and a conditional) carrying `AbilityFlag::Access` — 9.1.8a's
  "active while its source is the card being accessed", which is the only
  moment a card in R&D, in HQ or unrezzed in a remote root has to act in.

Until then both are assembled long-hand in `decks/mezzie_asa.rs`, in two
private helpers annotated there — the same `AbilityDef` the shorthand would
build, so nothing about the semantics is long-hand. **Global Food Initiative**
is ticked and tested on the first (scored for 3, stolen for 2, on one board);
**Project Vacheron**'s interrupt is written and tested on the second (stolen
with four agenda counters out of a remote, and without them out of Archives),
and it stays unticked for its second sentence alone.

### What a run-initiating sentence states about THAT run — LANDED (CR 6.9.1c)

One more position on `Instruction::InitiateRun`, beside `if_successful`
(6.7.4) and `if_would_be_successful` (9.9.1): `during`, the effects the
sentence states about the run it initiates, gated on nothing. It travels on the
run itself, exactly as the other two do, and is pended at 6.9.1c — the run has
formally begun, so 9.10.4 has a run to bind a "this run" duration to, and
6.9.1e's rez window has not opened yet. **Blackmail** is written and ticked.

The entry's second claim was WRONG and is deleted: "'ice' is a description and
`LingeringSpec::Prohibit` names one object fixed when the effect was created".
`ProhibitionSpec::Matching` takes a description and has since the CR 1.2.2
wave; the queue's own text already corrected itself two entries down and the
card's doc comment did not.

### An additional cost to REZ the cards a description reaches — LANDED (CR 1.16.10 / 8.1.2)

`StaticDecl::AdditionalRezCost { criteria, cost }` and
`Cost::trash_random_from_hand`, plus one reader for both halves of an 8.1.2
rez's additional costs.

The entry asked for "the act as CONTENT on ONE additional-cost declaration".
That was re-read against the kernel and NOT done, deliberately: the four
existing declarations are not four variants of one word, they are one per
PROCEDURE, and each carries scope vocabulary the others have no position for —
`AdditionalRunActionCost` has 6.3.4's server set and its printed ordinal,
`AdditionalBasicActionCost` has 5.2.5a's basic-action identity and a
description of the acted-on card, `AdditionalAccessCost` has 7.4.3's remote
root. Merging them would make one atom with a union of fields irrelevant to
each other, which is what §12 rule 2 exists to prevent, not to require. What
IS shared — and what the entry was right about — is the description
vocabulary: `criteria` is the ordinary filter list, so "non-ice cards" and a
sentence naming no cards at all are the same declaration with different
content.

The random-trash component is `Cost::trash_random_from_hand`, beside
`trash_from_hand` rather than instead of it: a random pick is not an
announcement (1.15.2b puts the choice to a player and this sentence takes it
away from both), which is the same distinction
`Instruction::RevealRandomFromHand` is already written on. 1.16.1 makes an
empty hand unable to pay it, which is where the card's teeth are.

What the entry did not mention, and what was found while wiring it: the
kernel read a card's PRINTED additional rez cost on the install-and-rez path
only. `Vm::rez_card_inner` — the ordinary (R) paid-window rez — never added it
and `Vm::rez_affordable` never asked about it, so an Archer rezzed from a paid
window forfeited nothing and a rez whose additional cost was unpayable was
still offered. Both now go through one reader, `Vm::additional_rez_cost_of`,
which combines the printed cost with every declaration reaching the card
(1.16.10b: they are ONE payment, so they are one affordability question).

Measured by `hacktivist_meeting_taxes_every_non_ice_rez_and_stops_it_with_an_empty_hq`:
an asset rez costs HQ a card, an ice rez costs it nothing, and with HQ empty
the asset cannot be rezzed at all. **Hacktivist Meeting** is written and
ticked.

### A card's OWN trash, with the occurrence's stipulations as content — LANDED

`TriggerCond::SelfTrashed { by_damage, from_zones }` replaces the contentless
`SelfTrashedByDamage`. Both stipulations are load-bearing and in opposite
directions: 10.4.2b resolves CORE damage by trashing from the grip exactly as
10.4.2a resolves meat and net, so a sentence silent about the kind
over-triggers; and 9.1.8b derives the zone the ability is ACTIVE in from the
condition, which is what lets a sentence name the grip or the stack at all
(4.4.4 leaves both inactive).

An EMPTY `by_damage` is itself a stipulation and not a wildcard: the condition
then reads the TRASH record (8.2.2) rather than the damage one, so a card taken
out of the grip by damage meets such a sentence exactly once. **I've Had
Worse** and **Steelskin Scarring** are written and ticked.

`by` (who trashed it) is deliberately NOT on the atom. `CardTrashed` already
carries it for the non-self case, no card in either deck asks for it, and an
unread field is untested code; it belongs on this atom the day a card prints
it.

### A card with the same NAME as one this ability revealed — LANDED (CR 2.1.4 / 1.21.6 / 9.10.1)

`TargetFilter::SameNameAsRevealedByThisAbility`, the characteristic reading of
the record `RevealedByThisAbility` reads as an identity. Both are needed and
2.1.4 is why: a copy in R&D is a different card with the same name, and the
copy is what the sentence forbids stealing.

What the entry did not see is that the criterion alone is not enough, and the
card would have compiled and done nothing. 1.21.6 keeps a revealed card
visible only "until the entire ability is finished resolving", and the
prohibition this sentence creates lasts the TURN — so a description re-read
where the steal is offered reaches nothing at all. The frame-scoped part is
therefore BOUND when the lingering effect is created, which is what
`Payload::DelayedConditional`'s `bound_targets` already does for 1.15.4 across
the same gap: `ProhibitionScope::Matching` gained a `copies_of` position
holding the cards, compared by NAME (2.1.4) and conjoined with the re-read
description. Empty at every existing site.

### The cards THIS ABILITY revealed — LANDED (CR 1.21.6)

`TargetFilter::RevealedByThisAbility`, the twin of `LookedAtByThisAbility`:
1.21.6 is ONE rule over two verbs — "if a resolving ability directs one or both
players to look at **or reveal** a card or set of cards, each such card remains
visible … until the entire ability is finished resolving" — so the reveal keeps
its cards on the resolving ability's frame exactly as the look does. Kept as a
second criterion rather than a polarity on the first because 1.21.5 says the
two "are not the same", which is the split the kernel already makes everywhere
else (one instruction and one `GameChange` per verb). **Inject** is written and
ticked.

The other half of this entry was never missing, and finding that out is what
made Inject a one-word card. "Gain 1[credit] for each program **trashed**" does
NOT want a new quantity: it is the cards this ability ANNOUNCED (1.15.4, which
the trash's own announcement fills) that are now in the heap —
`per_card_matching(&[among_earlier_choices(), in_heap()])`. That is exact where
a count of the revealed programs would not be: 9.9.7's prevention is what makes
"asked for" and "actually trashed" differ, and the heap is where the difference
shows. **Embezzle** (`unlisted.rs`) still wants the random-reveal half —
`Instruction::RevealRandomFromHand` announces nothing and takes no `TargetSpec`,
so neither 1.15.4's record nor 1.21.6's reaches its cards.

**Lakshmi Smartfabrics** kept this as one of its blockers, and it was the
NAME half rather than the identity half — see the entry above, where it
landed.

### A count of the agendas STOLEN inside a window — LANDED (CR 7.5 / 1.12.6)

`Quantity::AgendasStolen(HistoryWindow)`, read from the change log where
`AccessesThisRun` and `SubroutinesBrokenThisRun` are read from it. The window
is CONTENT on the one quantity rather than a variant per span — which is what
those two neighbours are the older spelling of — so "during that run" and "this
turn" are one count. Only the Runner steals (7.5), so no side rides on it.
`HistoryWindow` is 1.12.6's span as a named thing, and it is the position any
later history count should take. **Mad Dash** is written and ticked.

### A trigger condition met when a SUBROUTINE RESOLVES — LANDED (CR 9.8.10)

`TriggerCond::SubroutineResolved { criteria }`, met at step 9.8.10e once per
subroutine. The criteria describe the ICE it resolved from in the shared filter
vocabulary, so "a subroutine", "a subroutine on this ice" and "a subroutine on
a bioroid" are one condition with different content. Two cases the rules
already settle and neither got a word: 9.8.9's replaced subroutine "is treated
as having the same source as the original imminent subroutine", so it still
resolves from the ice; and 6.10's run-ending subroutine resolved before it
ended the run. **Raindrops Cut Stone** is written and ticked, with the
run-ending arm as its own board.

### Hosted credits TREATED AS pool credits — LANDED (CR 1.13.3)

`LingeringSpec::HostedCreditsAsPool { cards }` with the duration as content,
and `Vm::pool_credits` as the ONE place the pool is read. That is the whole
difference from a `CreditUse` allowance, which the card's own doc comment had
already argued: an allowance is read where credits are SPENT, and this is read
where they are COUNTED — by `Quantity::CreditsInPoolOf`, by 1.10.3b's forced
loss, and by every affordability question. **Stimhack** is written and ticked.

### A static declaration scoped to an ENCOUNTER, carrying an ordinal — LANDED (CR 6.5 / 9.3.7a)

`TriggerRequirement::EncounterUnderWay { criteria, first_each_turn }` and
`TargetFilter::ProtectingServer(ServerRef)`. Two words rather than the entry's
two, but not the same two — one of the entry's placements was wrong.

The requirement lives in the SHARED vocabulary, which is what the entry asked
for and what makes it usable from `declares_while` (9.3.7a) and from a trigger
condition's `requires` (9.6.5c) with the same words. `criteria` describes the
encountered ice in the ordinary filter vocabulary; an empty list is a sentence
saying plain "during an encounter".

The ordinal did NOT go on the static ability, and the entry's "the way
`InherentCostMod` already carries `first_each_turn`" is right about the
spelling and wrong about the position. A static ability never resolves (9.4.1),
so an ordinal on the ABILITY could only mean "the first time it applies" —
which is not what the sentence says. The sentence counts ENCOUNTERS: it holds
while no EARLIER encounter this turn was with ice the description reaches. So
the ordinal is content on the REQUIREMENT, beside the description it qualifies,
and it is read from the change log (10.2.1) the way every other printed ordinal
in the kernel is. The encounter's own id stops the walk, so a piece of ice
encountered twice in one turn does not answer for both.

"Protecting the chosen server" needed a description word of its own:
`MatchesMaintainedChoice` compares a maintained NAME, TYPE or SUBTYPE against a
card and its own doc says a maintained SERVER "describes no card, so nothing
matches". `TargetFilter::ProtectingServer` is that missing half — 4.6.9a puts
every installed piece of ice in a position in front of the server it protects,
and WHICH server is a `ServerRef`, so "protecting HQ" and "protecting the
server this card chose" are one criterion with different content. It is the ICE
half of 4.6.6b's "in the server" and deliberately not the whole of it: a root
is not a position protecting anything.

Measured by
`bankhar_replaces_the_first_subroutine_each_turn_on_the_chosen_server`: four
arms over the same remote behind "end the run" ice, so what the Corp resolved
is visible in whether the run survived — one ice on the chosen server (the run
goes on), two (the second encounter is out of scope and ends it), a different
server chosen, and nothing chosen at all.

**Tsakhia "Bankhar" Gantulga** is written and ticked.
