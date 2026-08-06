# Deck of the week — NetrunnerDB, week of 2026-07-31 … 2026-08-06

Pulled from netrunnerdb.com's public v2 API, `decklists/by_date`. Both carry
NRDB's tournament badge. Same bar as every deck before: a card counts only
when every printed sentence resolves in the VM and a behaviour test drives it
on a board (SYS-D-12 — one partial card and the deck is unplayable).

Cards already implemented are REUSED, never rewritten. The repository has 272
card definitions; check `jinteki_cards::find` before writing anything.

---

## Boring.dec: King of Swiss, 12th overall at Cascadia, 4-0-1  (NRDB #97714)

**Corp** — identity **Jinteki: Restoring Humanity**  ✅ already implemented
Module: `crates/jinteki-cards/src/decks/notw_restoring_humanity.rs`

16 distinct cards, 12 to write. Written in
`crates/jinteki-cards/src/decks/notw_restoring_humanity.rs`; nine complete,
three blocked below. Behaviour tests are in
`crates/jinteki-cards/tests/behaviour.rs` under "Deck of the week".

- [ ] **Anoetic Void** ◆ ×3 — upgrade
      "Whenever the Runner approaches this server, you may pay 2[credit] and trash 2 cards from HQ. If you do, end the run."
- [x] **Brân 1.0** ×3 — ice
      "Lose [click]: Break 1 subroutine on this ice. Only the Runner can use this ability. / [subroutine] You may install 1 piece of ice from HQ or Archives directly inward from this ice, ignoring all costs. / [subroutine] End the run. / [subroutine] End the run."
- [x] **Charlotte Caçador** ◆ ×3 — asset
      "You can advance this asset. / When your turn begins, you may remove 1 hosted advancement counter to gain 4[credit] and draw 1 card. / [trash], hosted advancement counter: Gain 3[credit]."
- [x] **Empiricist** ×3 — ice
      "[subroutine] Draw 1 card. You may add 1 card from HQ to the top of R&D. / [subroutine] Do 1 net damage. Give the Runner 1 tag. / [subroutine] Do 2 net damage."
- [ ] **Flyswatter** ×2 — ice
      "When you rez this ice during a run against this server, purge virus counters. / [subroutine] End the run."
- [x] **Fujii Asset Retrieval** ×1 — agenda
      "When this agenda is scored or stolen, do 2 net damage."
- [x] **Hansei Review** ×3 — operation
      "Gain 10[credit]. If there are any cards in HQ, trash 1 of them."
- [x] **Hedge Fund** ×3 — operation
- [x] **Knowledge Seeker** ×3 — ice
      "Whenever an encounter with this ice ends, if it has 3 or more hosted virus counters, purge virus counters and derez this ice. / [subroutine] Place 1 virus counter on this ice. / [subroutine] Look at the top 4 cards of R&D and arrange them in any order. / [subroutine] End the run."
- [ ] **La Costa Grid** ×3 — upgrade
      "Remote server only. / When your turn begins, place 1 advancement counter on a card in the root of this server. / Limit 1 region per server."
- [x] **Mavirus** ×2 — upgrade
      "While the Runner is accessing this upgrade in R&D, they must reveal it. / When the Runner accesses this upgrade, you may purge virus counters. If this upgrade is rezzed, do 1 net damage. / [trash]: Purge virus counters."
- [x] **Proprionegation** ×3 — agenda
      "When you score this agenda, place 1 agenda counter on it. / Hosted agenda counter: The Runner moves to the outermost position of Archives. (They approach any ice in that position.) Use this ability only during a run."
- [x] **Seamless Launch** ×3 — operation
- [x] **Send a Message** ×3 — agenda
      "When this agenda is scored or stolen, you may rez 1 installed piece of ice, ignoring all costs."
- [x] **Spin Doctor** ◆ ×3 — asset
- [x] **Tatu-Bola** ×3 — ice

---

## kit costume party  (NRDB #97727)

**Runner** — identity **Nyusha "Sable" Sintashta: Symphonic Prodigy**  ✅ already implemented
Module: `crates/jinteki-cards/src/decks/notw_sable.rs`

18 distinct cards, 13 to write.

- [ ] **Always Have a Backup Plan** ×3 — event
      "Run any server. When that run ends, if it was unsuccessful, you may run the attacked server again, ignoring any additional costs to run. During the second run, whenever you encounter the last piece of ice you encountered during the first run, bypass it."
- [x] **Asmund Pudlat** ◆ ×3 — resource
- [ ] **Backstitching** ×3 — resource
      "When your turn begins, identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.) / Whenever you encounter a piece of ice during a run on your mark, you may trash this resource to bypass that ice."
- [x] **Boomerang** ◆ ×3 — hardware
- [ ] **Buffer Drive** ◆ ×3 — hardware
      "The first time each turn 1 or more cards are trashed from your grip or stack, you may add 1 of those cards to the bottom of your stack. / Remove this hardware from the game: Add 1 card from your heap to the top of your stack."
- [ ] **Carmen** ×1 — program
      "If you made a successful run this turn, this program costs 2[credit] less to install. / Interface → 1[credit]: Break 1 sentry subroutine. / 2[credit]: +3 strength."
- [ ] **Carpe Diem** ×3 — event
      "Identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.) / Gain 4[credit]. You may run your mark."
- [x] **Clean Getaway** ×3 — event
- [ ] **Curupira** ×1 — program
      "Whenever you encounter a barrier, you may spend 3 hosted power counters to bypass it. / Whenever this program fully breaks a piece of ice, place 1 power counter on this program. / Interface → 1[credit]: Break 1 barrier subroutine. / 1[credit]: +1 strength."
- [ ] **Hyperbaric** ×1 — program
      "When you install this program, place 1 power counter on it. / This program gets +1 strength for each hosted power counter. / Interface → 1[credit]: Break 1 code gate subroutine. / 2[credit]: Place 1 power counter on this program."
- [ ] **Jeitinho** ◆ ×3 — hardware
      "When your turn ends, if you made a successful run on HQ, R&D, and Archives this turn, you may add this hardware to your score area as an assassination agenda worth 0 agenda points. Then, if you have 3 assassination agendas in your score area, you win the game. / Threat 3 → Whenever you bypass a piece of ice, you may spend [click] to install this hardware from your heap."
- [x] **Mutual Favor** ×3 — event
- [ ] **S-Dobrado** ×3 — event
      "Run a central server. The first time you encounter a piece of ice during that run, bypass it. / Threat 4 → The second time you encounter a piece of ice during that run, you may spend [click] to bypass it. (This ability is active if any player has 4 or more agenda points.)"
- [ ] **Swift** ◆ ×2 — hardware
      "+1[mu] / The first time each turn you play a run event, gain [click]. / Limit 1 console per player."
- [ ] **The Back** ◆ ×2 — resource
      "The first time each turn you use a piece of hardware during a run, place 1 power counter on this resource. / [click], remove this resource from the game: For each hosted power counter, choose up to 2 cards in your heap with [trash] abilities. Shuffle the chosen cards into your stack."
- [x] **The Class Act** ◆ ×3 — resource
- [ ] **The Wizard’s Chest** ◆ ×2 — hardware
      "Use this hardware only if you made a successful run on HQ, R&D, and Archives this turn. / [trash]: Choose hardware, program, or resource. Set aside cards from the top of your stack faceup until you set aside 2 cards of the chosen type. You may install 1 of those 2 cards, ignoring all costs. Shuffle the rest of the set-aside cards into your stack."
- [ ] **Verbal Plasticity** ◆ ×3 — resource
      "The first time each turn you take the basic action to draw 1 card, instead draw 2 cards."

---

## Blockers

Never approximated. A sentence the vocabulary cannot say leaves the card
unticked with the GENERAL kernel word it wants named here (ARCHITECTURE §12).
Each is a CAPABILITY, stated over the class it belongs to and never as "make
card X work".

### 1. "This server" as content on the approach condition (CR 6.9.4g / 4.6.6i)

The kernel scopes three run occurrences to the SOURCE's server — a successful
run on it (`SuccessfulRunOnServer`), a run on it ending (`RunOnThisServerEnds`)
and it being breached (`ThisServerBreached`), each comparing the attacked
server against `server_of_source`. The Runner APPROACHING a server carries no
such scoping: `TriggerCond::ServerApproached` was written for the Formicary
class, whose sentence names *a* server and means every one of them. The
capability wanted is the same stipulation on that fourth occurrence, as
content on the one atom rather than a fourth atom beside the three.

Waiting on it: **Anoetic Void** (this deck), **Manegarm Skunkworks**
(MEZZIE-QUEUE.md). Written with the word that exists, either would end runs on
every central as well as on its own server.

### 2. A run-in-progress REQUIREMENT scoped to the source's server (CR 6.1.1)

`TriggerRequirement::RunInProgress { on: Vec<ServerId> }` names a fixed list of
servers, which a card printed before the game cannot use for a remote created
during it. What is wanted is the same requirement able to name *this* server —
the requirement-side twin of blocker 1, so that any condition at all can carry
"…during a run against this server".

Waiting on it: **Flyswatter**'s "When you rez this ice during a run against
this server, purge virus counters."

### 3. A description reaching the ROOT of the source's own server (CR 4.6.6e)

4.6.6e gives every server a root and 4.6.6d gives it ice; 4.6.6i is the phrase
"this server" that a card in one uses to name it.
`TargetFilter::IceProtectingSourceServer` is the ice half of that idea and has
no root half. No other description is the same set: `InRemoteServer` is every
remote, `InAttackedServer` reads a run, and `HostedOnSource` is hosting rather
than a shared server. A general form — the source's server as a stipulation in
the shared description vocabulary, over the root, over what protects it, or
over both — would carry the whole class.

Waiting on it: **La Costa Grid**'s "place 1 advancement counter on a card in
the root of this server."

### 4. An install restriction stating WHERE a card may be installed (CR 8.5.12)

8.5.12 is a rule the kernel does not mention anywhere: "some upgrades have an
ability that specifies 'central server', 'remote server', or 1 or more
particular central servers followed by the word 'only'. This is a restriction
on the locations an upgrade can occupy that applies at all times, even if the
upgrade is [unrezzed]." 4.6.6h points at it from the server's side. The kernel
carries the HOSTED case of the same shape (`StaticDecl::InstallOnlyHostedOn`,
CR 8.5.1a) and nothing over servers. Without it a card carrying such a line
installs anywhere, which is a larger card than the printed one.

Waiting on it: **La Costa Grid**'s "Remote server only."

### 5. A disjunctive condition that states a zone (CR 9.1.8b)

`ability_active`'s `condition_only_met_in_zone` reads one condition at a time,
so `SelfStolen` alone keeps an ability active in the Runner's score area (4.5.4
would otherwise make it inactive) while `AnyOf { [SelfScored, SelfStolen] }`
names no single zone and reaches none. The consequence is that every "when this
agenda is scored **or** stolen" card must be written as two abilities for one
printed sentence — which is what Tomorrow's Headline, **Fujii Asset Retrieval**
and **Send a Message** all do. The capability wanted is for the zone test to
read a disjunction the way `trigger_matches` and `trigger_per_event` already do
(the zones its alternatives state, taken together), so one printed sentence can
be one ability.

Not blocking any card: all three work. It is a §12 rule-2 shape defect, and it
is recorded because the workaround is invisible in the card file.

### 6. Requirements on a condition that carries no `requires` slot (CR 9.6.5c)

`TriggerCond::EncounterEnds` — and several of its neighbours — have no
`requires` field, so a 9.6.5c requirement can only ride through
`AnyOf { alternatives: [that condition], requires }`. That is exact (9.6.5
makes a one-alternative disjunction that alternative, and both `trigger_matches`
and `trigger_per_event` recurse into it), and it is what `edsl`'s
`encounter_with_this_ice_ends_if` does for **Knowledge Seeker** — but it means
the slot is really a property of the CONDITION WRAPPER and not of each variant.
Hoisting `requires` out of the variants onto the condition itself would make
every condition able to carry one.

Not blocking any card.
