# Deck queue — the order the decks are implemented in

User-mandated order. Deck 1 is done; the rest are worked one at a time, top
to bottom, and a deck is finished only when every card of it is
`is_complete()` — CR SYS-D-12 means a deck with one partial card cannot be
played at all, so there is no partial credit.

Each deck becomes:

1. a module `crates/jinteki-cards/src/decks/<key>.rs`, one function per
   distinct card in printed order, registered in `decks/mod.rs`;
2. a `DeckSpec` in `crates/jinteki-server/src/cr.rs` with its list, copy
   counts and CR 1.5.4a pile (empty unless the deck runs Rebirth or DJ
   Fenris);
3. cards implemented in the EMBEDDED DSL until the deck's own odometer reads
   complete.

Cards shared with an already-finished deck are already done — reuse the
existing function, never write a second copy of a card.

---

## 1. estrike Regular Andromeda vs Gauntlet — **DONE** (51/51, live)

`andromeda` (Andromeda: Dispossessed Ristie) vs `gauntlet`
(Nebula Talent Management: Making Stars).

---

## 2. World Forest Wu — Kabonesa Wu: Netspace Thrillseeker

225 cards, Eternal, 15/15 influence. Runner. **The big one** — work it in
type-order batches and commit per batch.

Event (50): 2 "Freedom Through Equality" · 3 Burner · 3 Creative Commission ·
3 Diesel · 3 Government Investigations · 3 Indexing · 3 Interdiction ·
3 Levy AR Lab Access · 3 Marathon · 3 Net Celebrity · 3 Overclock · 3 Rejig ·
3 Scavenge · 3 Spec Work · 3 Sure Gamble · 3 System Seizure · 3 Trick Shot

Hardware (45): 3 AirbladeX (JSRF Ed.) · 3 Akamatsu Mem Chip · 2 Astrolabe ·
2 Cataloguer · 3 Clone Chip · 1 Comet · 2 Cyberdelia · 2 CyberSolutions Mem
Chip · 2 DZMZ Optimizer · 1 Endurance · 2 Feedback Filter · 3 Flame-out ·
2 LilyPAD · 1 LLDS Memory Diamond · 2 Plascrete Carapace · 3 Rabbit Hole ·
3 Record Reconstructor · 1 Replicator · 3 Simulchip · 1 Sports Hopper ·
3 Top Hat

Resource (65): 3 Aesop's Pawnshop · 3 All-nighter · 3 Artist Colony ·
1 Beach Party · 1 Beth Kilrain-Chang · 1 Biometric Spoofing · 2 Bloo Moose ·
1 Borrowed Satellite · 1 Citadel Sanctuary · 3 Councilman · 3 Daily Casts ·
3 Dr. Nuka Vrolyck · 1 DreamNet · 1 Dummy Box · 3 Environmental Testing ·
2 Film Critic · 3 Ghost Runner · 1 Hades Shard · 1 John Masanori ·
1 Laguna Velasco District · 1 Miss Bones · 2 New Angeles City Hall ·
1 No One Home · 2 Officer Frank · 2 Patron · 1 Political Operative ·
1 Public Sympathy · 3 Reclaim · 3 Sacrificial Construct · 3 Same Old Thing ·
3 Stoneship Chart Room · 3 Telework Contract · 2 The Shadow Net

Icebreaker (17): 1 Ankusa · 1 Aumakua · 1 Echelon · 2 Euler · 1 Ika ·
1 Inversificator · 2 Laamb · 3 Mayfly · 1 Propeller · 3 Slap Vandal · 1 Yog.0

Program (48): 1 Clot · 3 Coalescence · 1 Conduit · 1 Cupellation ·
3 Dhegdheer · 1 Hush · 3 Hyperdriver · 3 K2CP Turbine · 3 Kyuban ·
3 Leprechaun · 1 Magnum Opus · 2 Misdirection · 3 Muse · 1 Net Shield ·
3 Paricia · 2 Pelangi · 3 Pichação · 3 Rezeki · 1 Scheherazade ·
3 Self-modifying Code · 1 Snitch · 3 World Tree

---

## 3. "Tanzzen" NEH — Near-Earth Hub: Broadcast Center

49 cards, Eternal, 17/17 influence, 20 agenda points. Corp.

Agenda (13): 3 AstroScript Pilot Program · 3 Breaking News ·
3 Freedom of Information · 3 Post-Truth Dividend · 1 Tomorrow's Headline

Asset (16): 3 Estelle Moon · 3 Jeeves Model Bioroids · 2 Mumba Temple ·
3 Rashida Jaheem · 3 Sensie Actors Union · 2 Team Sponsorship

Operation (5): 1 BOOM! · 1 Oppo Research · 3 Shipment from MirrorMorph

Upgrade (9): 3 Arella Salvatore · 1 Cyberdex Virus Suite · 2 Hype Machine ·
3 SanSan City Grid

Ice (6): 2 IP Block · 2 Vanilla · 2 Slot Machine

---

## 4. [ETERNAL] (almost) Undefeated Jamming Nebula — Nebula Talent Management: Making Stars

49 cards, Eternal, 15/15 influence, 20 agenda points. Corp. **Heavy overlap
with Gauntlet** — most of it is already implemented.

Agenda (10): 3 AstroScript Pilot Program · 3 Bellona · 3 Breaking News ·
1 Tomorrow's Headline

Asset (6): 3 Rashida Jaheem · 3 Spin Doctor

Operation (20): 1 24/7 News Cycle · 2 BOOM! · 1 Closed Accounts ·
3 Hard-Hitting News · 3 Petty Cash · 3 Scarcity of Resources ·
2 Seamless Launch · 1 Self-Growth Program · 1 Subliminal Messaging ·
3 Your Digital Life

Upgrade (2): 2 Crisium Grid

Ice (11): 2 Gold Farmer · 3 IP Block · 3 Slot Machine · 1 Tributary ·
2 Data Raven

New vs Gauntlet: Spin Doctor, Scarcity of Resources, Your Digital Life,
Tributary.

---

## 5. Thats Numberwang — Sportsmetal: Go Big or Go Home

49 cards, Eternal, 15/15 influence, 21 agenda points. Corp.

Agenda (17): 3 Domestic Sleepers · 3 Élivágar Bifurcation ·
3 Hyperloop Extension · 3 Megaprix Qualifier · 3 Ontological Dependence ·
2 Project Vacheron

Asset (12): 3 Estelle Moon · 3 Jackson Howard · 3 Nightmare Archive ·
3 Rashida Jaheem

Operation (8): 2 Fast Break · 3 Game Changer · 3 Stock Buy-Back

Upgrade (5): 2 Arella Salvatore · 1 Cyberdex Virus Suite · 2 Djupstad Grid

Ice (7): 3 Meridian · 2 Fairchild 3.0 · 2 Gatekeeper

---

## 6. [eternal] mulch on in the next life — MaxX: Maximum Punk Rock

56 cards, Eternal, 15/15 influence. Runner. Runs **Rebirth** and
**DJ Fenris**, so this deck needs a CR 1.5.4a pile (both are implemented).

Event (23): 3 Déjà Vu · 3 Inject · 1 Levy AR Lab Access · 3 Moshing ·
1 Rebirth · 3 Rumor Mill · 3 Steelskin Scarring · 3 Strike Fund · 3 The Price

Hardware (7): 2 Clone Chip · 2 Knobkierie · 3 Simulchip

Resource (8): 3 Bloo Moose · 1 Citadel Sanctuary · 3 Cookbook · 1 DJ Fenris

Icebreaker (5): 2 Audrey v2 · 1 Black Orchestra · 1 MKUltra · 1 Paperclip

Program (13): 3 Botulus · 1 Clot · 3 Fermenter · 1 Hush · 2 Imp ·
1 Keyhole · 1 Leech · 1 Parasite
