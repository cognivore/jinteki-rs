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

16 distinct cards, 12 to write.

- [ ] **Anoetic Void** ◆ ×3 — upgrade
      "Whenever the Runner approaches this server, you may pay 2[credit] and trash 2 cards from HQ. If you do, end the run."
- [ ] **Brân 1.0** ×3 — ice
      "Lose [click]: Break 1 subroutine on this ice. Only the Runner can use this ability. / [subroutine] You may install 1 piece of ice from HQ or Archives directly inward from this ice, ignoring all costs. / [subroutine] End the run. / [subroutine] End the run."
- [ ] **Charlotte Caçador** ◆ ×3 — asset
      "You can advance this asset. / When your turn begins, you may remove 1 hosted advancement counter to gain 4[credit] and draw 1 card. / [trash], hosted advancement counter: Gain 3[credit]."
- [ ] **Empiricist** ×3 — ice
      "[subroutine] Draw 1 card. You may add 1 card from HQ to the top of R&D. / [subroutine] Do 1 net damage. Give the Runner 1 tag. / [subroutine] Do 2 net damage."
- [ ] **Flyswatter** ×2 — ice
      "When you rez this ice during a run against this server, purge virus counters. / [subroutine] End the run."
- [ ] **Fujii Asset Retrieval** ×1 — agenda
      "When this agenda is scored or stolen, do 2 net damage."
- [ ] **Hansei Review** ×3 — operation
      "Gain 10[credit]. If there are any cards in HQ, trash 1 of them."
- [x] **Hedge Fund** ×3 — operation
- [ ] **Knowledge Seeker** ×3 — ice
      "Whenever an encounter with this ice ends, if it has 3 or more hosted virus counters, purge virus counters and derez this ice. / [subroutine] Place 1 virus counter on this ice. / [subroutine] Look at the top 4 cards of R&D and arrange them in any order. / [subroutine] End the run."
- [ ] **La Costa Grid** ×3 — upgrade
      "Remote server only. / When your turn begins, place 1 advancement counter on a card in the root of this server. / Limit 1 region per server."
- [ ] **Mavirus** ×2 — upgrade
      "While the Runner is accessing this upgrade in R&D, they must reveal it. / When the Runner accesses this upgrade, you may purge virus counters. If this upgrade is rezzed, do 1 net damage. / [trash]: Purge virus counters."
- [ ] **Proprionegation** ×3 — agenda
      "When you score this agenda, place 1 agenda counter on it. / Hosted agenda counter: The Runner moves to the outermost position of Archives. (They approach any ice in that position.) Use this ability only during a run."
- [x] **Seamless Launch** ×3 — operation
- [ ] **Send a Message** ×3 — agenda
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

(none recorded yet)
