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
- [ ] **Lakshmi Smartfabrics** ×2 — asset · cost 1, trash 3
      "Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics. / X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn."
- [ ] **Marilyn Campaign** ×1 — asset · Advertisement · cost 2, trash 3
      "When you rez this asset, load 8[credit] onto it. When it is empty, trash it. / When your turn begins, take 2[credit] from this asset. / [interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. (It is still considered trashed.)"
- [x] **MCA Austerity Policy** ◆ ×2 — asset · cost 1, trash 3
      "Once per turn → [click]: Place 1 power counter on this asset. When the Runner's next turn begins, they lose [click]. / [click], [trash], 3 hosted power counters: Gain [click][click][click][click]."
- [ ] **Mumba Temple** ×3 — asset · Alliance - Facility · cost 1, trash 3
      "This card costs 0 influence if you have 15 or fewer ice in your deck. / 2[recurring-credit] / Use these credits to rez cards."
- [x] **Rashida Jaheem** ◆ ×3 — asset · Character · cost 0, trash 1
      "When your turn begins, you may trash Rashida Jaheem to gain 3[credit] and draw 3 cards."
- [x] **Spin Doctor** ◆ ×3 — asset · Character · cost 0, trash 2
      "When you rez this asset, draw 2 cards. / Remove this asset from the game: Shuffle up to 2 cards from Archives into R&D."
- [ ] **Enhanced Login Protocol** ×2 — operation · Current · cost 2
      "This operation is not trashed until another current is played or an agenda is stolen. / As an additional cost to take the basic action to run a server for the first time each turn, the Runner must spend [click]."
- [x] **Flood the Market** ×1 — operation · Double · cost 3
      "As an additional cost to play this operation, spend [click]. / Choose 1 installed card you can advance. Place 1 advancement counter on that card for each remote server that has a card in its root and is protected by ice."
- [x] **Friends in High Places** ×3 — operation · Terminal · cost 2
      "After you resolve this operation, end your action phase. / Install up to 2 cards from Archives (paying all install costs)."
- [x] **Fully Operational** ×3 — operation · cost 1
      "Gain 2[credit] or draw 2 cards. Repeat this process for each remote server that has a card in its root and is protected by ice."
- [x] **Ash 2X3ZB9CY** ◆ ×1 — upgrade · Bioroid · cost 2, trash 3
      "Whenever there is a successful run on this server, Trace[4]. If successful, the Runner cannot access any cards other than Ash 2X3ZB9CY for the remainder of this run."
- [ ] **Manegarm Skunkworks** ◆ ×1 — upgrade · cost 2, trash 3
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
- [ ] **Hacktivist Meeting** ×3 — event · Current · cost 1
      "This card is not trashed until another current is played or an agenda is scored. / As an additional cost to rez non-ice cards, the Corp must randomly trash a card from HQ."
- [ ] **I've Had Worse** ×3 — event · cost 1
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
- [ ] **Steelskin Scarring** ×3 — event · cost 1
      "Draw 3 cards. / When this event is trashed from your grip or stack, you may draw 2 cards."
- [ ] **Stimhack** ×1 — event · Run · cost 0
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
- [ ] **Mystic Maemi** ◆ ×3 — resource · Companion - Virtual · cost 1
      "When your turn begins and whenever you steal an agenda, place 1[credit] on this resource. / You can spend hosted credits to play events. / When your turn ends, if there are 3 or more hosted credits, you must trash 1 card from your grip at random or trash this resource."
- [x] **Same Old Thing** ×1 — resource · cost 0
      "[click], [click], [trash]: Play an event from your heap (paying its play cost)."
- [ ] **Tsakhia "Bankhar" Gantulga** ◆ ×3 — resource · Connection · cost 1
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

### The approach condition naming WHICH server (CR 6.9.4g)

`TriggerCond::ServerApproached` is a unit variant, met by 6.9.4g's step
whatever server was approached. The kernel's other two run conditions about a
server both carry it — `SuccessfulRunOnServer` compares the attacked server
against the server the source is in, and `RunEnds { on }` carries the list a
sentence names — but the approach was written for the Formicary class, whose
sentence names A server and means every one of them.

This is measured, not assumed: an upgrade rezzed in the root of a remote,
carrying nothing but a `ServerApproached` conditional, ends a run on HQ. A card
saying "this server" written with the word that exists is not a smaller card
than the printed one; it is a different and much larger one.

Wanted: the server as CONTENT on the one condition — the source's own server,
or the named list, in whatever shape `RunEnds` and `SuccessfulRunOnServer`
already agree on — so "whenever the Runner approaches this server", "…a
server" and "…HQ" are one condition with different content.

Wants it: **Manegarm Skunkworks**.

### A nested cost with ALTERNATIVE costs (CR 1.16.11b / 9.12.3c)

`Instruction::NestedCostUnless { cost, effect, payer }` holds one `Cost`, and
`Cost` is a conjunction — every component is paid together. A sentence whose
escape is a CHOICE of costs ("unless they either spend [click][click] or pay
5[credit]") has nowhere to put the second one.

Neither workaround is honest. Writing one cost drops whichever door was not
written, and the two are not interchangeable: a Runner with 5[credit] and no
clicks escapes by one and a Runner with two clicks and no credits by the other.
Nesting one inside the other invents an instruction boundary the sentence does
not have (9.11.3), and with it a checkpoint, a reaction window and an interrupt
window between the two halves of a single choice.

Wanted: the costs as a LIST on the one instruction, filtered by 1.16.1b's
payability where it is offered — which is 9.12.3c's rule about a choice among
options said for costs: the payer picks among the costs they can actually pay,
and a payer who can pay none faces no choice and the effect resolves.

Wants it: **Manegarm Skunkworks** (its only sentence, which also wants the
condition above).

### A trash whose destination an ABILITY redirects (CR 9.9.8a-b / 8.2.2)

The kernel replaces a trash's destination in exactly one shape:
`StaticDecl::ReplaceTrashDestination`, read where the movement happens,
mandatory, with `TrashDestination` naming two places — removed from game (4.9)
and turned facedown in play (8.1.4d). A card printing "[interrupt] → when this
card would be trashed, you may put it somewhere else instead of adding it to
<the discard pile>" has neither the optionality nor the destination, and
writing it with the static would make every trash of that card a redirect
whether its controller wanted one or not.

Wanted: the destination as CONTENT on the one atom — a player's deck (shuffled
in, 4.2.3), a hand, the set-aside zone — added as variants of
`TrashDestination` rather than as new replacement atoms; and the same
replacement expressible from a card's own optional interrupt (9.9.8a) as well
as from a static (9.9.8b), so the printed "you may" is one flag on the existing
word and not a second mechanism. 8.2.2 is what every shape of it must keep: the
card is still trashed and conditions about being trashed are still met — only
where it lands changes, which is exactly what these cards' parenthetical says.

Wants it: **Marilyn Campaign** ("you may shuffle it into R&D instead of adding
it to Archives").

### A description stipulating agenda points, or the X announced for the cost (CR 2.4.2 / 1.16.2c / 1.15.2)

The description vocabulary can say a card's type, its subtypes, its name, its
printed cost at most N, the counters on it, and its rez cost relative to a
triggering card. It cannot say a card's AGENDA POINTS, and nothing in it can be
compared against the X a player announced for the ability's own trigger cost
(1.16.2c) — the announced X is readable as a *quantity* and never as a
stipulation a described card has to satisfy.

Wanted: agenda points as one more characteristic the shared filter vocabulary
reads, and one comparison position whose two sides are quantities — so "an
agenda worth X points", "a card with printed cost X" and "ice with strength X
or lower" are one word with different content, and not a filter apiece.

Wants it: **Lakshmi Smartfabrics** ("Reveal an agenda worth X points from HQ").
Its prohibition is no longer a blocker — CR 1.2.2's "cannot" now names
stealing and takes a description with a duration — but the sentence still
waits on this word and on the revealed-cards word below, so the card stays
unticked. Both remaining halves are about the SAME reveal: which agenda may be
revealed, and which cards "copies of that agenda" then means.

### The printed ORDINAL on an additional-cost declaration (CR 1.16.10 / 5.2.5a)

`StaticDecl::AdditionalRunActionCost { cost, on }` says WHAT the cost is and
WHICH servers it reaches, and nothing about WHICH TAKINGS of the action it
attaches to. A sentence that charges only some of them — "…**for the first
time each turn**" — has no position to say so, and written without it the cost
is charged every time. That is not a small error: 1.16.1b makes an additional
cost a gate on the action, so an over-broad one forbids actions the player is
entitled to take for free.

A conditional ability met by the first run each turn is not a substitute and
must not be offered as one. An additional cost is paid at 6.9.1a to INITIATE
the action and gates it; a conditional resolves after it and gates nothing.
(The CR's own Heinlein Grid example turns on exactly that distinction: the
click this cost charges is spent to initiate the run and is not spent during
it.)

Wanted: the ordinal as CONTENT on the one declaration, the way
`StaticDecl::InherentCostMod` already carries `first_each_turn` — one
position, read from the change log, covering "the first N times each turn" for
every additional cost a card can print. That is the whole of what is missing.

(An earlier wave also wanted the "names no server" set here, claiming a
sentence naming no server could not be written at all. That was never true:
`RunServerSet` is among `crate::edsl`'s re-exports, so
`StaticDecl::AdditionalRunActionCost { cost, on: RunServerSet::Any }` is
written directly, and `additional_cost_to_run`'s own doc comment names
`RunServerSet::Any` as how to say it. What the two named helpers do not have
is a THIRD helper for the empty set — a shorthand, not a capability.)

Wants it: **Enhanced Login Protocol** (its second sentence; the current's own
"not trashed until…" is done and tested). Note the kernel's doc comment on
`AdditionalRunActionCost` names this card as its class exemplar — it is
describing the ORIGINAL printing, which charged every run; the printed text in
`netrunner-cards-json` is the revised one and it has the ordinal.


### Hosted credits spendable on REZZING (CR 1.10.3c / 8.1.2)

1.10.3c is the whole of what hosted credits are: they may be spent only as the
hosting card's ability allows. `CreditUse` names five allowances — any
payment, trashing described cards, USING described cards (9.1.6a's paid-ability
trigger cost), a trace attempt's two spend steps, and advancing described
cards — and rezzing is none of them.

It is not `UsingAbilitiesOf` under another name, for the reason
`AdvancingCards` is not either: 8.1.2's rez procedure pays a card's rez cost
and uses no ability at all, so writing a rez permission as the "using"
allowance would let the credits pay for paid abilities they may not pay for
and STILL not pay for a rez. A card whose only permission is unsayable has
credits placed on it that no payment can reach.

Wanted: rezzing as one more purpose on `CreditUse`, with the cards described in
the same filter vocabulary the other purposes use (so "to rez cards", "to rez
ice" and "to rez bioroids" are one word with different content), paired with
the matching `CreditPurpose` read at the one place 8.1.2d's rez cost is paid.

The same position is what a sentence naming PLAYING wants, and for the same
reason: 8.6.7c pays a play cost inside the play procedure and uses no ability
at all, so `UsingAbilitiesOf` would let the credits pay for paid abilities
they may not pay for and STILL not pay for a play.

Wants it: **Mumba Temple** ("Use these credits to rez cards" — the
2[recurring-credit] itself is done and tested, placed at the rez and refilled
without accumulating); **Mystic Maemi** ("You can spend hosted credits to play
events" — the credits arrive and the turn-end demand is done and tested; only
the permission that would let them be spent is missing).

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

### An additional cost to REZ the cards a description reaches (CR 1.16.10 / 8.1.2)

1.16.10's additional costs come in two shapes here: a fact printed on the card
being paid for (`additional_rez_cost`, Archer's "to rez THIS card"), and a
declaration taxing an ACT by description — of which there are three
(`AdditionalStealCost`, `AdditionalAccessCost`, `AdditionalRunActionCost`).
Rezzing the cards a description reaches ("non-ice cards") is neither.

Beside it, the same sentence wants a COST component that exists only as an
effect: `Instruction::TrashRandomFromHand` performs 1.15.2b's unannounced trash
out of a hand, and no `Cost` field charges one. `Cost::trash_matching` is the
announced trash and not this — 1.15.2b is explicit that a card taken at random
is not announced by anyone, which is exactly the difference the sentence turns
on. `Cost::trash_from_hand` is not it either, and it is the near miss worth
naming: it charges an UNANNOUNCED trash, but its own doc records the
approximation that makes it the wrong word — it takes the front of the hand,
and the front of a hand is not a card taken at random.

Wanted: the act as CONTENT on one additional-cost declaration — rezzing beside
stealing, accessing and the basic actions, with the cards described in the
shared filter vocabulary — and a random-trash component on `Cost`, so a
sentence charging one is a cost and not an effect that fires afterwards.

Wants it: **Hacktivist Meeting** ("As an additional cost to rez non-ice cards,
the Corp must randomly trash a card from HQ"; the current's own "not trashed
until…" is done and tested).

### A card's OWN trash, with the occurrence's stipulations as content (CR 10.4.2 / 9.1.8b)

Two conditions describe a trash and neither is both halves.
`TriggerCond::SelfTrashedByDamage` is scoped to the source and says nothing
about WHICH damage; `TriggerCond::CardTrashed { from_zone, … }` carries the
zone and every other stipulation and cannot be scoped to the source at all —
its `requires` vocabulary has no term for "the trashed card is this card".

Both halves are load-bearing, in opposite directions.

- The KIND is not decoration: 10.4.2a resolves meat and net damage by trashing
  randomly-chosen cards from the grip, and 10.4.2b resolves CORE damage the
  same way, adding only the hand-size reduction. A condition silent about the
  kind is met by core damage, so a card printing "net or meat" over-triggers —
  and Stimhack, in this very deck, is what would trigger it.
- The SCOPE decides where the ability is ACTIVE. 9.1.8b keeps an ability alive
  in the zone its condition names, and the kernel derives that zone from the
  condition alone (`SelfTrashedByDamage` → the discard pile). A grip or a stack
  is 4.3/4.2's hidden zone where 4.4.4 leaves everything inactive, so a
  condition that cannot name them can never be met at all.

Wanted: ONE condition for "this card is trashed", with the damage kinds, the
zone it was trashed FROM, and who trashed it all as content on it (§12 rule 2),
and the 9.1.8b zone derived from that content — so "trashed by net or meat
damage" and "trashed from your grip or stack" are the same word with different
contents rather than two atoms, neither of which exists.

Wants it: **I've Had Worse** ("Whenever I've Had Worse is trashed by taking net
or meat damage, draw 3 cards" — the kind half); **Steelskin Scarring** ("When
this event is trashed from your grip or stack, you may draw 2 cards" — the zone
half and the scope). Both cards' "Draw 3 cards" is done and tested.

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

**Lakshmi Smartfabrics** keeps this as one of its two blockers, and it is the
NAME half rather than the identity half: "copies of that agenda" is every card
sharing a characteristic with the revealed one, and the filter vocabulary can
say "the card this ability revealed" (now) and "a card with this printed name"
(`HasName`) and not "a card with the same name as that one".

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

### Hosted credits TREATED AS pool credits, for a duration (CR 1.13.3 / 1.10.1)

Distinct from the `CreditUse` entry above, and the distinction is the whole of
it. `CreditUse` says what hosted credits may be SPENT on (1.10.3c). 1.13.3 says
something else — hosted credits are not "on" the player at all — and a card
saying they are considered to be in the credit pool waives that: the credits
are then read by everything that reads the pool, so a forced 1.10.3b loss
during the named window takes them, and a quantity asking how many credits the
Runner has counts them.

Writing the permission alone (`CreditUse::AnyPayment`) reaches every payment
and none of those reads — a silent UNDER-reach wherever the pool is counted
rather than spent, which is why it is not offered as the near-enough answer.

Wanted: one lingering effect treating a described card's hosted credits as pool
credits, with the duration as content — the one place 1.13.3's separation is
read, so "during that run" and any other span are the same word.

Wants it: **Stimhack** ("During that run, hosted credits are considered to be
in your credit pool"; the placement, the run, and the unpreventable core
damage are done and tested).

### A static declaration scoped to an ENCOUNTER, carrying an ordinal (CR 6.5 / 9.3.7a)

`StaticDecl::ReplaceSubroutineResolution` already says 9.9.2's "instead of the
subroutine they would resolve, these instructions". What cannot be said is
WHEN it is on. A static ability is either always active or gated by
`declares_while`'s state requirements, and neither reaches an ENCOUNTER. The
nearest requirement is `CanInterfaceWithEncounteredIce { required_subtype }`,
which is 9.3.6c's strength gate wearing a subtype and not a question about the
encounter at all; nothing asks whether one is under way, nor whether its ice
matches a description. `AbilityDef` does carry an `ordinal`, but only
`first_imminence_of` ever reads it — it stipulates which IMMINENCE a
conditional may be relevant to, and no static ability is read through it — so
"the first encounter each turn" still has no position.

Both gaps have to close together. An always-on declaration rewrites every
subroutine on every server for the whole game, which is the largest over-reach
a card of this class can have, so the mechanism being present buys nothing on
its own.

Wanted: the encounter as a state requirement in the shared vocabulary — one
under way, with its ice described the way every other card is described,
including "protecting the chosen server" against a 9.10.3 maintained choice —
and the ordinal as content on a static ability the way `InherentCostMod`
already carries `first_each_turn`.

Wants it: **Tsakhia "Bankhar" Gantulga** ("During the first encounter each turn
with a piece of ice protecting the chosen server, whenever the Corp would
resolve a subroutine, instead they resolve …"; the turn-begin server choice is
done and tested, and 9.10.3 remembers it).
