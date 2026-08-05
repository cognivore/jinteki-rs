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
- [ ] **Estelle Moon** ×3 — asset · Executive · cost 2, trash 3
      "Whenever you install a card in the root of a remote server, place 1 power counter on this asset. / [trash]: For each power counter on this asset, gain 2[credit] and draw 1 card."
- [ ] **Jeeves Model Bioroids** ×1 — asset · Alliance · cost 2, trash 5
      "This card costs 0 influence if you have 6 or more non-alliance [haas-bioroid] cards in your deck. / The first time you spend 3[click] on the same action each turn, gain [click]."
- [ ] **Lakshmi Smartfabrics** ×2 — asset · cost 1, trash 3
      "Whenever you rez a card, place 1 power counter on Lakshmi Smartfabrics. / X hosted power counters: Reveal an agenda worth X points from HQ. The Runner cannot steal copies of that agenda for the remainder of this turn."
- [ ] **Marilyn Campaign** ×1 — asset · Advertisement · cost 2, trash 3
      "When you rez this asset, load 8[credit] onto it. When it is empty, trash it. / When your turn begins, take 2[credit] from this asset. / [interrupt] → When this asset would be trashed, you may shuffle it into R&D instead of adding it to Archives. <em>(It is still considered trashed.)</em>"
- [ ] **MCA Austerity Policy** ×2 — asset · cost 1, trash 3
      "Once per turn → [click]: Place 1 power counter on this asset. When the Runner's next turn begins, they lose [click]. / [click], [trash], 3 hosted power counters: Gain [click][click][click][click]."
- [ ] **Mumba Temple** ×3 — asset · Alliance - Facility · cost 1, trash 3
      "This card costs 0 influence if you have 15 or fewer ice in your deck. / 2[recurring-credit] / Use these credits to rez cards."
- [x] **Rashida Jaheem** ×3 — asset · Character · cost 0, trash 1
      "When your turn begins, you may trash Rashida Jaheem to gain 3[credit] and draw 3 cards."
- [ ] **Spin Doctor** ×3 — asset · Character · cost 0, trash 2
      "When you rez this asset, draw 2 cards. / Remove this asset from the game: Shuffle up to 2 cards from Archives into R&D."
- [ ] **Enhanced Login Protocol** ×2 — operation · Current · cost 2
      "This operation is not trashed until another current is played or an agenda is stolen. / As an additional cost to take the basic action to run a server for the first time each turn, the Runner must spend [click]."
- [ ] **Flood the Market** ×1 — operation · Double · cost 3
      "As an additional cost to play this operation, spend [click]. / Choose 1 installed card you can advance. Place 1 advancement counter on that card for each remote server that has a card in its root and is protected by ice."
- [ ] **Friends in High Places** ×3 — operation · Terminal · cost 2
      "After you resolve this operation, end your action phase. / Install up to 2 cards from Archives (paying all install costs)."
- [ ] **Fully Operational** ×3 — operation · cost 1
      "Gain 2[credit] or draw 2 cards. Repeat this process for each remote server that has a card in its root and is protected by ice."
- [ ] **Ash 2X3ZB9CY** ×1 — upgrade · Bioroid · cost 2, trash 3
      "Whenever there is a successful run on this server, Trace[4]. If successful, the Runner cannot access any cards other than Ash 2X3ZB9CY for the remainder of this run."
- [ ] **Manegarm Skunkworks** ×1 — upgrade · cost 2, trash 3
      "Whenever the Runner approaches this server, end the run unless they either spend [click][click] or pay 5[credit]."
- [x] **Tatu-Bola** ×1 — ice · Barrier · cost 2, str 1
      "When the Runner passes this ice, you may swap it with a piece of ice from HQ. If you do, gain 4[credit]. <em>(The new ice is installed unrezzed. You do not pay an install cost.)</em> / [subroutine] End the run."
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
      "Run any server. Whenever a subroutine resolves during that run <em>(including a subroutine that ends the run)</em>, place 1 power counter on this event. / When that run ends, draw 1 card for each hosted power counter and gain 3[credit]."
- [x] **Rebirth** ×1 — event · cost 0
      "Switch your identity with another identity from the same faction. Remove Rebirth from the game instead of trashing it. / Limit 1 per deck."
- [ ] **Steelskin Scarring** ×3 — event · cost 1
      "Draw 3 cards. / When this event is trashed from your grip or stack, you may draw 2 cards."
- [ ] **Stimhack** ×1 — event · Run · cost 0
      "Place 9[credit] on this event, then run any server. During that run, hosted credits are considered to be in your credit pool. When that run ends, suffer 1 core damage. This damage cannot be prevented."
- [x] **Sure Gamble** ×2 — event · cost 5
      "Gain 9[credit]."
- [x] **Boomerang** ×2 — hardware · cost 2
      "When you install this hardware, choose 1 installed piece of ice. Use this hardware only during encounters with that ice. / [trash]: Break up to 2 subroutines. When this run ends, if it was successful, you may shuffle 1 copy of Boomerang from your heap into your stack."
- [x] **Desperado** ×2 — hardware · Console · cost 3
      "+1[mu] / Gain 1[credit] whenever you make a successful run. / Limit 1 console per player."
- [ ] **Zer0** ×3 — hardware · cost 1
      "Once per turn → [click], suffer 1 net damage: Gain 1[credit] and draw 2 cards."
- [ ] **Clan Vengeance** ×3 — resource · Clan · cost 3
      "Whenever you suffer any amount of damage, place 1 power counter on Clan Vengeance. / [trash]: Trash 1 card from HQ at random for each power counter on Clan Vengeance."
- [ ] **Mystic Maemi** ×3 — resource · Companion - Virtual · cost 1
      "When your turn begins and whenever you steal an agenda, place 1[credit] on this resource. / You can spend hosted credits to play events. / When your turn ends, if there are 3 or more hosted credits, you must trash 1 card from your grip at random or trash this resource."
- [ ] **Same Old Thing** ×1 — resource · cost 0
      "[click], [click], [trash]: Play an event from your heap (paying its play cost)."
- [ ] **Tsakhia "Bankhar" Gantulga** ×3 — resource · Connection · cost 1
      "When your turn begins, you may choose a server. / During the first encounter each turn with a piece of ice protecting the chosen server, whenever the Corp would resolve a subroutine, instead they resolve "[subroutine] Do 1 net damage."."
- [ ] **Black Orchestra** ×2 — program · Icebreaker - Decoder · cost 3, str 2
      "Whenever you encounter a code gate, you may install this program from your heap. / 3[credit]: +2 strength. Then, if this program can interface with the code gate you are encountering, break up to 2 subroutines."
- [ ] **MKUltra** ×2 — program · Icebreaker - Killer · cost 2, str 1
      "Whenever you encounter a sentry, you may install this program from your heap. / 3[credit]: +2 strength. Then, if this program can interface with the sentry you are encountering, break up to 2 subroutines."
- [x] **Paperclip** ×2 — program · Icebreaker - Fracter · cost 4, str 1
      "Whenever you encounter a barrier, you may install this program from your heap. / X[credit]: +X strength. Then, if this program can interface with the barrier you are encountering, break up to X subroutines."
- [ ] **Rezeki** ×1 — program · cost 2
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
remainder of this run").

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
