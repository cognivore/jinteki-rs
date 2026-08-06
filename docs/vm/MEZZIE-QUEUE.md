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

- [ ] **Global Food Initiative** ×1 — agenda · Initiative · adv 5/3
      "Global Food Initiative is worth 1 fewer agenda point while in the Runner's score area."
- [ ] **Luminal Transubstantiation** ×1 — agenda · Research · adv 3/2
      "When you score this agenda, gain [click][click][click]. You cannot score agendas for the remainder of the turn. / Limit 1 per deck."
- [ ] **Project Vacheron** ×3 — agenda · Research · adv 5/3
      "[interrupt] → When this agenda would be added to the Runnerʼs score area from anywhere except Archives, instead it is added to their score area with 4 hosted agenda counters. / While this agenda is in the Runnerʼs score area with 1 or more hosted agenda counters, it is worth 0 agenda points and gains “When the Runnerʼs turn begins, remove 1 hosted agenda counter.“"
- [ ] **Project Vitruvius** ×3 — agenda · Research · adv 3/2
      "When you score this agenda, place 1 agenda counter on it for each hosted advancement counter past 3. / Hosted agenda counter: Add 1 card from Archives to HQ."
- [x] **Estelle Moon** ◆ ×3 — asset · Executive · cost 2, trash 3
      "Whenever you install a card in the root of a remote server, place 1 power counter on this asset. / [trash]: For each power counter on this asset, gain 2[credit] and draw 1 card."
- [ ] **Jeeves Model Bioroids** ◆ ×1 — asset · Alliance · cost 2, trash 5
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
- [ ] **Flood the Market** ×1 — operation · Double · cost 3
      "As an additional cost to play this operation, spend [click]. / Choose 1 installed card you can advance. Place 1 advancement counter on that card for each remote server that has a card in its root and is protected by ice."
- [x] **Friends in High Places** ×3 — operation · Terminal · cost 2
      "After you resolve this operation, end your action phase. / Install up to 2 cards from Archives (paying all install costs)."
- [ ] **Fully Operational** ×3 — operation · cost 1
      "Gain 2[credit] or draw 2 cards. Repeat this process for each remote server that has a card in its root and is protected by ice."
- [ ] **Ash 2X3ZB9CY** ◆ ×1 — upgrade · Bioroid · cost 2, trash 3
      "Whenever there is a successful run on this server, Trace[4]. If successful, the Runner cannot access any cards other than Ash 2X3ZB9CY for the remainder of this run."
- [ ] **Manegarm Skunkworks** ◆ ×1 — upgrade · cost 2, trash 3
      "Whenever the Runner approaches this server, end the run unless they either spend [click][click] or pay 5[credit]."
- [x] **Tatu-Bola** ×1 — ice · Barrier · cost 2, str 1
      "When the Runner passes this ice, you may swap it with a piece of ice from HQ. If you do, gain 4[credit]. (The new ice is installed unrezzed. You do not pay an install cost.) / [subroutine] End the run."
- [x] **Vanilla** ×3 — ice · Barrier · cost 0, str 0
      "[subroutine] End the run."
- [ ] **Fairchild 3.0** ×2 — ice · Code Gate - Bioroid - AP · cost 6, str 5
      "Lose [click][click][click]: Break up to 3 subroutines on this ice. Only the Runner can use this ability. / [subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards. / [subroutine] The Runner must pay 3[credit] or trash 1 of their installed cards. / [subroutine] Do 1 core damage or end the run."
- [ ] **Vertigo** ×1 — ice · Code Gate · cost 1, str 1
      "When the Runner passes this ice, if they have no [click] remaining, they cannot steal or trash Corp cards for the remainder of this run. / [subroutine] The Runner loses [click]."
- [x] **Drafter** ×2 — ice · Sentry · cost 3, str 3
      "[subroutine] You may add 1 card from Archives to HQ. / [subroutine] You may install 1 card from Archives or HQ, ignoring all costs."
- [x] **Tour Guide** ×3 — ice · Sentry · cost 2, str 0
      "This ice gains "[subroutine] End the run." for each rezzed asset."

---

## Mezzie's Valencia — Valencia Estevez: The Angel of Cayambe (50 cards, 23 distinct)

Identity is COMPLETE.

- [ ] **Blackmail** ×3 — event · Run · cost 1
      "Play only if the Corp has at least 1 bad publicity. / Run any server. The Corp cannot rez ice during that run."
- [ ] **Hacktivist Meeting** ×3 — event · Current · cost 1
      "This card is not trashed until another current is played or an agenda is scored. / As an additional cost to rez non-ice cards, the Corp must randomly trash a card from HQ."
- [ ] **I've Had Worse** ×3 — event · cost 1
      "Draw 3 cards. / Whenever I've Had Worse is trashed by taking net or meat damage, draw 3 cards."
- [ ] **Inject** ×3 — event · cost 1
      "Reveal the top 4 cards of your stack and trash all programs revealed. Gain 1[credit] for each program trashed, and add the rest of the revealed cards to your grip."
- [ ] **Levy AR Lab Access** ×1 — event · cost 5
      "Shuffle your grip and heap into your stack. Draw 5 cards. Remove Levy AR Lab Access from the game instead of trashing it."
- [ ] **Mad Dash** ×1 — event · Run · cost 0
      "Run any server. When that run ends, if you stole an agenda during that run, add this event to your score area as an agenda worth 1 agenda point. Otherwise, suffer 1 meat damage."
- [ ] **Moshing** ×3 — event · cost 0
      "As an additional cost to play this event, trash 3 cards from your grip. / Gain 3[credit] and draw 3 cards."
- [ ] **Raindrops Cut Stone** ×2 — event · Run · cost 1
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
- [ ] **Clan Vengeance** ×3 — resource · Clan · cost 3
      "Whenever you suffer any amount of damage, place 1 power counter on Clan Vengeance. / [trash]: Trash 1 card from HQ at random for each power counter on Clan Vengeance."
- [ ] **Mystic Maemi** ◆ ×3 — resource · Companion - Virtual · cost 1
      "When your turn begins and whenever you steal an agenda, place 1[credit] on this resource. / You can spend hosted credits to play events. / When your turn ends, if there are 3 or more hosted credits, you must trash 1 card from your grip at random or trash this resource."
- [ ] **Same Old Thing** ×1 — resource · cost 0
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

### An ability that names its controller (CR 1.14.4)

1.14.4 says the controller of an ability is "**by default**" the controller of
its source, and that a player can only use abilities they control. The kernel
has no way to depart from that default: `Vm::paid_window_options` offers a
card's paid abilities to its controller and to nobody else, so a Corp card
whose ability says "Only the Runner can use this ability" would hand the
Runner's ability to the Corp.

Wanted: the ability itself carrying WHICH player controls it, as one optional
field on `AbilityDef` (an absent value being 1.14.4's default). That is one
position with the player as content, and it covers the whole bioroid class in
both directions at once — including any future card that hands the Corp an
ability printed on a Runner card.

Wants it: **Fairchild 3.0** (the "Lose [click][click][click]: Break up to 3
subroutines on this ice" ability; the three subroutines are done).

### A "cannot" that names an act other than scoring or rezzing (CR 9.10.1 / 1.2.2)

`LingeringSpec::Prohibit` takes a list of acts, but `ProhibitedAction` has two
variants — `Score` and `Rez`. A sentence forbidding anything else has nothing
to say "cannot" about, and 1.2.2 gives a "cannot" precedence over every
permission, so getting it wrong is not a small error.

Wanted: the same one atom with the rest of the acts a card can forbid as
content — stealing (7.5), trashing (7.1.5 / 1.19.4), advancing, installing,
drawing, running — added as variants of `ProhibitedAction` rather than as new
prohibition atoms.

Wants it: **Vertigo** ("they cannot steal or trash Corp cards for the
remainder of this run"); **Lakshmi Smartfabrics** ("the Runner cannot steal
copies of that agenda for the remainder of this turn" — stealing again, with a
turn for its duration instead of a run).

### A quantity that reads a player's click pool (CR 1.11)

The `Quantity` selector language has no term for the clicks in a player's
click pool, so a requirement about how many a player has left cannot be
stated — neither "if they have no [click] remaining" nor any threshold on the
other side of it.

Wanted: one selector for the pool, read the way `CreditsInPoolOf(Side)` reads
the credit pool, with the side as content; the existing `at_least`/`at_most`
supply the threshold and the polarity.

Wants it: **Vertigo** (same sentence as above — it needs both this and the
prohibition).

### A trigger condition over the CLICKS spent on one action (CR 5.2.1 / 1.11.3b)

The trigger vocabulary counts ACTIONS — `SameActionInARow { side, count }` (The
Collective) and `DifferentActionsThisTurn` (MirrorMorph) — and has no condition
about how many [click] a player has SPENT on one action within a turn. The two
counts are not the same count and neither implies the other: 5.2.6h's basic
purge is ONE action costing three clicks, and a double operation followed by an
ordinary one is TWO actions costing three between them, and both meet a "spend
3[click] on the same action" sentence while meeting no count of repeated
actions at all. Written with the words that exist, such a card fires strictly
less often than it should — a silent under-trigger, which is the reason the
sentence is marked rather than approximated.

Wanted: one condition over clicks SPENT (5.2.1, and 1.11.3b's insistence that
spending and losing are not the same word), with the threshold and the "same
action" grouping as content on it, so "the first time you spend N[click] on the
same action each turn" is that condition paired with the ordinal
`when_first_each_turn` already supplies.

Wants it: **Jeeves Model Bioroids** (its only sentence that does anything; the
alliance line is a 1.4.5 deckbuilding restriction and denotes into nothing).

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

Wants it: **Lakshmi Smartfabrics** ("Reveal an agenda worth X points from HQ" —
which also needs the prohibition above, so the sentence waits on both).

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
every additional cost a card can print. Wanted beside it: the "names no
server" set (`RunServerSet::Any`) reachable from the card vocabulary, which
today offers helpers only for a named list and for 4.6.8's remotes — a
sentence naming no server cannot be written at all, and the servers it would
have to enumerate include remotes that do not exist when the card is written.

Wants it: **Enhanced Login Protocol** (its second sentence; the current's own
"not trashed until…" is done and tested). Note the kernel's doc comment on
`AdditionalRunActionCost` names this card as its class exemplar — it is
describing the ORIGINAL printing, which charged every run; the printed text in
`netrunner-cards-json` is the revised one and it has the ordinal.

### A quantity that counts SERVERS matching a description (CR 4.6.6)

The `Quantity` selector language counts cards (`Count(Vec<TargetFilter>)`),
counters, pools, accesses and subroutines. It has no term for a SERVER, so a
"for each" over 4.6.6's servers cannot be stated — neither the count nor any
stipulation on it.

No count of cards stands in for one. 4.6.6e lets a remote root hold an asset
or agenda AND any number of upgrades, so counting the cards in the qualifying
roots over-counts a server with an upgrade on it; counting the ice protecting
them over-counts a server behind two. Both directions hand out payouts that
were never earned, which is why these sentences are marked rather than
approximated.

Wanted: one selector for servers, with the stipulations as content in the same
filter vocabulary the card descriptions already use — the server's type
(central or remote, 4.6.6c), whether a described card is installed in its root,
whether ice protects it — so "each remote server that has a card in its root
and is protected by ice", "each central server" and "each server you have a
card installed in" are one word with different content and not a selector
apiece. (The instruction that repeats a process the resulting number of times
exists in the kernel as `Instruction::ForEach { count, effects }`; it is
unreachable from the card vocabulary, and it wants the same quantity, so the
two land together. Note the printed arithmetic: "do this, then repeat for each
X" is 1 + N resolutions, never N.)

Wants it: **Flood the Market** ("Place 1 advancement counter on that card for
each remote server that has a card in its root and is protected by ice" — the
*double*'s extra [click] is done); **Fully Operational** ("Repeat this process
for each remote server that has a card in its root and is protected by ice" —
the gain-or-draw is done).

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

Wants it: **Mumba Temple** ("Use these credits to rez cards" — the
2[recurring-credit] itself is done and tested, placed at the rez and refilled
without accumulating).

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
