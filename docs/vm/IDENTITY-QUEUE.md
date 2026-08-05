# Identity queue — every identity in the game

CR 1.5.4a lets a player bring "any number of additional Runner identity cards"
alongside their deck, and Rebirth and DJ Fenris choose from those. With six
identities implemented, "choose another identity of the same faction" is a
choice of one. This queue is the fix: implement ALL of them.

## How

One module per faction under `crates/jinteki-cards/src/decks/identities/`,
registered in `decks/mod.rs`. One function per identity, printed text from
`crates/jinteki-core/carddata/cards.json`, written in the EMBEDDED DSL.

**An identity joins a deck's CR 1.5.4a pile only when it is COMPLETE.**
`crates/jinteki-server/src/cr.rs::readiness()` holds pile cards to the same
bar as deck cards, so adding a stub would make both priority decks unplayable
again. Implement, then enlist — never the other way round.

Runner identities come first: they are the ones Rebirth and DJ Fenris reach.
Within a faction, do the mechanically simple ones first — an identity whose
whole text is a static declaration or one conditional is a handful of lines
once the vocabulary exists, and the hard ones are easier after the easy ones
have grown the vocabulary.

Double-sided identities (Nebula/Gemilang class) carry their back face with
`.flip_face(...)`; see `decks/gauntlet.rs` for the pattern.

## Known blockers — found while working the queue, never approximated

An identity is skipped rather than written wrong (ARCHITECTURE §12). Each of
these is a general kernel capability, stated with the identities that want it.
A blocker is DELETED from this list when the capability lands, not struck
through — `git log` is where the history lives.

Still wanted by identities other than the ones named, in every case: these are
kernel words, not card patches.

- **No condition for discarding down to maximum hand size.**
  `Instruction::DiscardToHandSize` is 5.7.4's step and
  `TriggerCond::DiscardPhaseEnds` is the phase around it; neither is "whenever
  you discard cards to reach your maximum hand size", which names the discard
  itself and hands the cards discarded to the next sentence.
  *(Magdalene Keino-Chemutai: Cryptarchitect.)*
- **No ability sees the score areas' agenda-point totals as a WIN
  condition.** The kernel ends a game on a flatline or an empty R&D
  (`GameResult`); nothing anywhere counts agenda points towards 7 or asks how
  many are needed. *(Harmony Medtech: Biomedical Pioneer; Issuaq Adaptics:
  Sustaining Diversity.)*

- **A sentence joined by "and" cannot refer to the card its own other half
  chose.** 9.11.3 makes such a sentence ONE instruction, and `Combined` says
  so by merging its halves' expected atoms — but a half that CHOOSES its own
  targets cannot ride that merge, so it is spliced out to announce them where
  1.15.2 puts them, which is AFTER the merged half has resolved. "Add 1
  rezzed card to HQ and gain credits equal to its rez cost" therefore gains
  the credits before the card has moved, and 1.15.4's "its" reads nothing at
  all. Writing it as two instructions is the other wrong answer: it invents
  the checkpoint 9.11.3 charges for. *(Blue Sun: Powering the Future.)*
- **Nothing modifies the NUMBER of tags the Runner is considered to have.**
  `Instruction::GainTags` and `RemoveTags` move the real count, and
  `Quantity::RunnerTags` and `TriggerRequirement::RunnerTagsAtLeast` read it —
  but "the Runner is considered to have 1 additional tag (even if they have
  0)" is a DECLARATION about that number, which every reader of
  `st.runner.tags` would have to go through. The scope is the other half:
  "during encounters with the outermost piece of ice protecting any server"
  wants both a description of the outermost ice and a window the declaration
  holds in. *(Acme Consulting: The Truth You Need.)*
- **Nothing happens BEFORE the first turn.** `PrintedCard::starting_hand_size`,
  `starting_credits` and `starting_bad_publicity` are 1.6's setup FACTS,
  settled while the game is built and never resolved by anything; "before
  taking your first turn, you may install up to 3 pieces of ice" is an
  ABILITY, and 1.7 is a window the kernel does not open, so there is nowhere
  to put one. *(NEXT Design: Guarding the Net; Cyber Bureau: Keeping the
  Peace; Jinteki Biotech: Life Imagined; Ayla "Bios" Rahim: Simulant
  Specialist.)*
- **Nothing is met by trashing a card TO PAY for damage.**
  `TriggerCond::SelfTrashedByDamage` is the trashed card's own view of
  10.4.3, and `CardTrashed` describes a trash by who did it, what type it was
  and whether it was being accessed — neither is "the Runner trashes a card
  FOR brain damage", which is a question about what the trash was for and
  hands the card's name to the next sentence. Nor can that sentence be
  written: `all_named_cards_in_discard_of` reads a MAINTAINED name and one
  zone, and "all copies of that card (installed, in the heap, stack, grip, or
  any other location)" is the triggering card's name across every zone at
  once. *(Chronos Protocol: Haas-Bioroid.)*
- **No instruction TAKES an action.** `TriggerCond::DifferentActionsThisTurn`
  already counts 5.2.5's distinct actions, so MirrorMorph's condition is
  sayable — but "take another different action, paying [click] less" is an
  ability handing a player a basic action at a reduced cost, and 5.2 offers
  actions only from the action phase's own step. *(MirrorMorph: Endless
  Iteration.)*
- **An ordinal cannot be SHARED between two conditions.**
  `AbilityDef::ordinal` belongs to one ability, and a sentence with two
  conditions is written as two abilities (Leela Patel class) — which is right
  until the sentence also says "the first time each turn", because then each
  ability spends its own ordinal and the pair fires twice.
  *(Epiphany Analytica: Nations Undivided.)*
- **The basic ADVANCE action does not pay through a payment, so nothing about
  it can carry a purpose.** `CreditPurpose` now says what a payment is for —
  trashing a card, using a card's abilities (9.1.6a), a trace attempt's spend
  steps — but 5.2.6f's 1[credit] is taken straight out of the Corp's pool by
  `Vm::take_action`, so there is no `PaymentCont` to read a purpose from and
  no 1.10.3c division for the payer to make. Routing it through
  `Vm::begin_payment` is not a new word but a change to 5.2's shape, and it
  would also make `GameChange::CostPaid` name the advanced card as the source
  that caused the payment (9.1.4) — which a basic action has none of, so a
  "whenever a Corp card ability causes…" condition would start meeting it.
  *(Weyland Consortium: Because We Built It.)*
- **A declaration cannot modify the strength of cards it DESCRIBES.**
  `StaticDecl::StrengthMod` reaches the source or its host, and the
  characteristic pipeline that would read a criteria-scoped one is the same
  pipeline `has_subtype` goes through — so "all **bioroid** ice has +1
  strength" needs the loop broken before it can be said.
  *(Haas-Bioroid: Stronger Together.)*
- **Nothing names the card an ability INSTALLED.**
  `TargetFilter::{LookedAtByThisAbility, SetAsideByThisAbility, DrawnCards}`
  each name a set an ability made, and there is no "the card this ability
  installed" beside them — and a 9.6.13 delayed conditional cannot read the
  targets of the ability that created it either, so "when that run ends, trash
  THAT PROGRAM" has nothing to point at. *(Arissana Rocha Nahu: Street Artist;
  Kabonesa Wu: Netspace Thrillseeker; Topan: Ormas Leader; Mti Mwekundu: Life
  Improved, whose "the Runner moves to THAT ICE and approaches it" is the same
  words about the piece its own first sentence installed — `MoveRunnerToIce`
  is 6.2.8a and would carry it, and the description of the ice is what is
  missing.)*
- **A choice between SERVERS cannot be written when the servers do not exist
  yet.** 1.15.1b lists a server among the things a player can be told to
  choose, and `ChoiceSpec::Server` names one — the choice BETWEEN them being
  9.11.4g's option choice, an `Instruction::ChooseOne` whose branches each
  maintain a different server. That works for the three centrals and for
  nothing else: 4.6.8's remote servers are created during play, so "choose a
  server other than the attacked server" has no set of branches to write when
  the card is written. Two further words the same sentence wants: no
  `TargetFilter` describes the piece of ice the Runner is APPROACHING (the
  cost trashes it), and none describes the outermost ice of a server the
  ability just chose. *(AgInfusion: New Miracles for a New World.)*
- **A trash records neither that the card was REZZED nor that it happened
  during an installation.** `GameChange::CardTrashed` carries the zone the
  card left and whether it was being accessed — both facts about the MOMENT
  of the trash, recorded there because the card has moved by the time any
  condition is scanned — and "when you trash a **rezzed** card, **except
  during installation**" is two more of exactly that kind: the second is
  8.5.11a's like-card trash, which the install procedure performs and this
  sentence excludes. The same card wants two more words: `TargetFilter` is
  `Copy`, so "a card with a printed rez cost exactly 1[credit] less than the
  trashed card's printed rez cost" has to be a relational atom beside
  `SameNameAsTriggeringCard` and there is none; and `InstallCard::
  ignore_costs` is 1.16.5c's "ignoring ALL costs", which the kernel already
  reads as the INHERENT ones only (an additional rez cost is still paid — the
  Ob/Archer case is written into `InstallRezPayCost`), while "ignoring
  **credit** costs" selects costs by their KIND and cuts across 1.16.4's
  inherent/additional split. *(Ob Superheavy Logistics: Extract. Export.
  Excel.)*
- **A counter cost comes off the SOURCE, in a printed number.**
  `Cost::spend_counters` is 1.9.2's "spend N counters hosted on this card"
  (Imp), which is why an empty card's ability is unusable rather than free —
  and "**any** X virus counters" is neither half of that: the count is
  announced under 1.16.2c and the counters come from any of the payer's cards,
  which needs a division put to the payer the way 1.10.3c's credits already
  are. Two more words in the same sentence: `Cost::x_restriction` states the
  bound as "X must be equal to **or less than**", and "X must be equal to" is
  a different relation; and `Quantity` reads the rez cost of the ice being
  ENCOUNTERED (Nasir) but not the printed rez-or-play cost of a card a
  description names. *(Freedom Khumalo: Crypto-Anarchist.)*
- **Nothing installs a card at the game's SETUP.**
  `PrintedCard::starting_hand_size`, `starting_credits` and
  `starting_bad_publicity` are 1.6's setup facts, and "you start the game with
  3 different **directive** cards installed (these cards are not considered
  part of your deck)" is a fourth of the same kind — except that its cards
  come from outside the deck, so it also wants somewhere for a player to bring
  them from. `TargetFilter::InIdentityPileOf` is 1.5.4a's pile and is the
  nearest thing; nothing says a card is brought alongside the deck for any
  other reason. *(Adam: Compulsive Hacker.)*
- **A trash cannot be replaced by SETTING THE CARDS ASIDE.**
  `StaticDecl::ReplaceTrashDestination` is 9.9.8b's replacement and its
  destinations are the removed-from-game zone and 8.1.4d's facedown-in-play —
  neither is 4.8's set-aside zone, and none of them is stated about "1 or more
  cards" as one group the ability then looks through, removes one of and
  returns. "Ignore this ability if you have already removed a card from the
  game with it this turn" is 9.3.6g's once-per-turn flag said about a static
  ability, which never resolves (9.4.1) and so never spends one.
  *(Skorpios Defense Systems: Persuasive Power.)*
- **"The other card" cannot be said.** `TargetSpec::EarlierTargets` is 1.15.4's
  plural and `EarlierTarget { nth }` is one by position; neither is "the ones
  an earlier instruction chose, EXCEPT the one a later instruction chose",
  which is what a sentence handing one card to each player needs.
  *(Steve Cambridge: Master Grifter; AU Co.: The Gold Standard in Clones,
  whose "trash 1 of those cards and add THE REST to HQ" is the same words
  about the cards an earlier instruction looked at — and, being one sentence
  joined by "and", it wants the Blue Sun entry above settled as well.)*
- **Nothing records a psi game's reveal.** `Instruction::PsiGame` resolves
  10.14.6's construction whole and writes no change for 10.14.6c's reveal, so
  "whenever you and the Runner reveal secretly spent credits" has no
  occurrence to be met by. *(Nisei Division: The Next Generation.)*
- **A basic action's cost cannot depend on the card it acts on.** 5.2.6g's
  trash-a-resource action pays its click and its 2[credit] and only THEN
  announces the resource, so an additional cost stated about *which* resource
  (1.16.10) has nowhere to be paid: by the time the card is known the payment
  is over. The card is not the problem — the action is, and moving the
  announcement in front of the payment is a change to 5.2's shape rather than
  a new word. *(Sebastião Souza Pessoa: Activist Organizer, second sentence.)*
- **A card cannot be INSTALLED facedown.** `TrashDestination::FacedownInPlay`
  turns an already-installed Runner card facedown (8.1.4d, Harbinger), and
  `TargetFilter::Facedown` can describe the result — but `Instruction::
  InstallCard` has no such stipulation, so nothing puts a card into the rig
  facedown in the first place. *(Apex: Invasive Predator, second sentence;
  its first needs a described install PROHIBITION as well.)*
- **Nothing is met by a run that WOULD be declared successful.** 9.9.3's
  interrupt relevance is computed from the `EffectAtom`s of the imminent
  instruction, and `Instruction::DeclareRunSuccessful` carries only a
  structural atom — so there is no expected effect for a "would" condition to
  be relevant to, and the ability that changes the attacked server one step
  before 6.9.5 has no moment to fire in. *(Omar Keung: Conspiracy
  Theorist.)*
- **Nothing is met by an ability FINISHING.** `TriggerCond::SelfPlayResolved`
  is 8.6.7h read about the card being played — the source's own view, which is
  why `GameChange::CardPlayResolved` names one object and no player — and
  there is no twin for a card OTHER than the source. The other half of the
  same sentence has no occurrence at all: "an action on an **expendable**
  card" is a card ability that is an action (5.2.4) reaching the end of its
  resolution, and `GameChange::AbilityUsed` records that one was USED (9.1.6),
  which is a different moment. One printed "or" joining them makes it one
  condition, so both halves have to be sayable before either is.
  *(Nuvem SA: Law of the Land.)*
- **A prohibition cannot name one card for a stated span, and none of them
  names a REZ.** `StaticDecl::CannotScoreMatching` describes the agendas that
  cannot be scored in the ordinary words — which is how Clot's "during the
  same turn they installed that agenda" is said — but "you cannot score or rez
  THAT card until your next turn begins" needs three things it has none of:
  a description of the one card an earlier instruction of the same ability
  installed (the entry above), a declaration about rezzing beside the one
  about scoring, and a span — `WantedDuration` stops at `ThisTurn`, and the
  rez half bites during the OPPONENT's turn, which is past the end of it.
  *(Saraswati Mnemonics: Endless Exploration; A Teia: IP Recovery, whose "you
  cannot score the second card this turn" is the same sentence with the
  shorter span and no rez half.)*
- **An install cannot be told to leave the card FACEUP.**
  `Instruction::InstallCard` stipulates 8.5.15's rez (`and_rez`) and 1.16.5c's
  costs and nothing at all about the card's face, and 8.1.2 leaves an
  installed Corp card facedown until it is rezzed — an agenda can never be
  rezzed, so "you may install agendas faceup" has no stipulation to make. It
  is the other half of the Apex entry above: one wants a Runner card installed
  facedown, this one a Corp card installed faceup, and `TrashDestination::
  FacedownInPlay` is the same word said about a card already installed.
  *(BANGUN: When Disaster Strikes, whose second sentence also wants
  `TriggerCond::RunnerAccessesCard` to describe the card accessed.)*
- **A declaration cannot be scoped to ONE occurrence, or offered.**
  `StaticDecl::SelectsDamageTrashes { by, count }` is 10.4.3a's modification of
  the damage procedure, and a static ability never resolves (9.4.1) and is
  never declined — but "for the FIRST net damage the Runner suffers each turn,
  YOU MAY look at the Runner's grip and select the card that is trashed"
  states that declaration about one damage and puts it to the Corp. 9.6.13's
  lingering effect is the shape that would carry it, and `LingeringSpec` has
  no member holding a declaration for a stated span.
  *(Chronos Protocol: Selective Mind-mapping.)*


## Not a kernel gap: the back faces have no printed text here

`crates/jinteki-core/carddata/cards.json` is NSG's card data and carries FRONT
faces only, so a double-sided identity's back face has no oracle text in this
repo to copy from. SYS-D-10 forbids writing one from memory, so those
identities wait on the data and not on the kernel. *(Hoshiko Shiro: Untold
Protagonist; Dewi Subrotoputri: Pedagogical Dhalang; Jinteki Biotech: Life
Imagined; Méliès U: Only the Brightest; SYNC: Everything, Everywhere; Earth
Station: SEA Headquarters; Cyber Bureau: Keeping the Peace. Gemilang Arena,
Nebula Talent Management's back face, was sourced before this was noticed.)*

## Progress

- Implemented: **111 / 150**  (the count of ticked boxes below — `grep -c "^- \[x\]"`)

Enlisted in CR 1.5.4a's Andromeda pile (`jinteki-server`'s `cr::ANDROMEDA_PILE`),
so Rebirth reaches them at the table: every COMPLETE Criminal — Ken Tenma, 419,
Armand "Geist" Walker, Barry "Baz" Wong, Iain Stirling, Silhouette, Gabriel
Santiago, Los, Liza Talking Thunder, Laramy Fisk, Leela Patel, Nyusha "Sable"
Sintashta, Virtual Intelligence, P.I., Mercury, MuslihaT, Zahya Sadeghi,
Az McCaffrey, Khan, Nero Severn, Boris "Syfr" Kovac. (Boris prints "Draft format only." and
Andromeda is not a draft deck; 1.4.2 settles a format restriction before the
game begins and nothing reads it afterwards, so it changes no play — a pile
that wanted to honour it would filter on that printed line.)


## Runner — Criminal (21/22)

Module: `decks/identities/runner_criminal.rs`

- [x] **419: Amoral Scammer** — The first time the Corp installs a card each turn, you may expose that card unless the Corp pays 1[credit].
- [x] **Andromeda: Dispossessed Ristie** — You draw a starting hand of 9 cards.
- [x] **Armand "Geist" Walker: Tech Lord** — Whenever you use a [trash] ability, draw 1 card.
- [x] **Az McCaffrey: Mechanical Prodigy** — The first job resource, connection resource, or piece of hardware you install each turn costs 1[credit] less to install.
- [x] **Barry "Baz" Wong: Tri-Maf Veteran** — Whenever the Corp rezzes a piece of ice, you may install 1 resource or piece of hardware from your grip.
- [x] **Boris "Syfr" Kovac: Crafty Veteran** — Draft format only. If you have more [criminal] cards installed than any other faction, when your turn begins, remove 1 tag.
- [x] **Gabriel Santiago: Consummate Professional** — The first time you make a successful run on HQ each turn, gain 2[credit].
- [x] **Iain Stirling: Retired Spook** — When your turn begins, gain 2[credit] if the Corp has more scored agenda points than you.
- [x] **Ken "Express" Tenma: Disappeared Clone** — The first time each turn you play a run event, gain 1[credit].
- [x] **Khan: Savvy Skiptracer** — The first time you pass a piece of ice each turn, you may install an icebreaker from your hand, lowering the install cost by 1.
- [x] **Laramy Fisk: Savvy Investor** — The first time you make a successful run on a central server each turn, you may force the Corp to draw 1 card.
- [x] **Leela Patel: Trained Pragmatist** — Whenever an agenda is scored or stolen, add 1 unrezzed card to HQ.
- [x] **Liza Talking Thunder: Prominent Legislator** — The first time you make a successful run on a central server each turn, draw 2 cards and take 1 tag.
- [x] **Los: Data Hijacker** — The first time the Corp rezzes a piece of ice each turn, gain 2[credit].
- [x] **Mercury: Chrome Libertador** — Once per turn → When you breach HQ or R&D during a run, if you did not break any subroutines during that run, you may access 1 additional card.
- [x] **MuslihaT: Multifarious Marketeer** — When your turn begins, look at the top card of your stack. If that card is an icebreaker or a run event, you may reveal it and add it to your grip.
- [x] **Nero Severn: Information Broker** — Once per turn → When you encounter a sentry, you may jack out.
- [x] **Nyusha "Sable" Sintashta: Symphonic Prodigy** — When your turn begins, identify your mark. (If you don’t have a mark, a random central server becomes your mark for this turn.) The first time each turn you make a successful run on your mark, gain [click].
- [x] **Silhouette: Stealth Operative** — The first time you make a successful run on HQ each turn, you may expose 1 card.
- [ ] **Steve Cambridge: Master Grifter** — The first time each turn you make a successful run on HQ, you may choose 2 cards in your heap. If you do, the Corp removes 1 of those cards from the game, then you add the other card to your grip.
- [x] **Virtual Intelligence, P.I.: "You Can Call Me Vic"** — Once per turn → [click], 1[credit]: Draw 1 card and remove 1 tag.
- [x] **Zahya Sadeghi: Versatile Smuggler** — Once per turn → When a run on HQ or R&D ends, you may gain 1[credit] for each time you accessed a card during that run.

## Runner — Shaper (16/21)

Module: `decks/identities/runner_shaper.rs`

- [x] **Akiko Nisei: Head Case** — Whenever you breach R&D, you and the Corp secretly spend 0[credit], 1[credit], or 2[credit]. Reveal spent credits. If you and the Corp spent the same number of credits, access 1 additional card.
- [ ] **Arissana Rocha Nahu: Street Artist** — Once per turn → 0[credit]: Install 1 program from your grip (paying its install cost). Use this ability only during a run. When that run ends, trash that program if it is not a trojan.
- [ ] **Ayla "Bios" Rahim: Simulant Specialist** — Before drawing your starting hand, set aside the top 6 cards of your stack facedown. (You may look at those cards at any time.) Shuffle 2 of those cards into your stack. [click]: Add 1 card set aside with this identity to your grip.
- [x] **Captain Padma Isbister: Intrepid Explorer** — The first time each turn a run on R&D begins, you may charge 1 of your installed cards. (Add 1 power counter to a card that already has one.)
- [x] **Chaos Theory: Wünderkind** — +1[mu]
- [ ] **Dewi Subrotoputri: Pedagogical Dhalang** — Whenever you make a successful run, if your [mu] is full, you may flip this identity and gain 1[credit].
- [x] **Ele "Smoke" Scovak: Cynosure of the Net** — 1[recurring-credit] Use this credit to pay for using icebreakers.
- [x] **Exile: Streethawk** — Whenever you install a program from your heap, draw 1 card.
- [x] **Hayley Kaplan: Universal Scholar** — The first time you install a card each turn, you may install another card of the same type from your grip (paying its install cost).
- [x] **Hiram "0mission" Svensson: Shadow of the Past** — Whenever you install or trash a piece of hardware (from any location), look at the top card of R&D.
- [x] **Jamie "Bzzz" Micken: Techno Savant** — Draft format only. If you have more [shaper] cards installed than any other faction, when you install a card the first time each turn, draw 1 card.
- [x] **Jesminder Sareen: Girl Behind the Curtain** — [interrupt] → The first time each run you would take 1 or more tags, prevent 1 tag.
- [ ] **Kabonesa Wu: Netspace Thrillseeker** — [click]: Search your stack for a non-virus program and install it, lowering its install cost by 1[credit], then shuffle your stack. If that program is still installed when your turn ends, remove it from the game.
- [x] **Kate "Mac" McCaffrey: Digital Tinker** — Lower the install cost of the first program or piece of hardware you install each turn by 1.
- [x] **Lat: Ethical Freelancer** — When your discard phase ends, if you have the same number of cards in your grip as the Corp has in HQ, you may draw 1 card.
- [ ] **Magdalene Keino-Chemutai: Cryptarchitect** — Whenever you discard cards to reach your maximum hand size, you may install 1 program or piece of hardware from among those cards.
- [x] **Nasir Meidan: Cyber Explorer** — Whenever you encounter a piece of ice after an approach during which that ice was rezzed, lose all credits in your credit pool. Gain credits equal to the rez cost of that ice.
- [x] **Rielle "Kit" Peddler: Transhuman** — The first time each turn you encounter a piece of ice, it gains code gate for the remainder of this run.
- [x] **The Collective: Williams, Wu, et al.** — The first time you perform the same action three times in a row each turn, gain [click].
- [x] **The Professor: Keeper of Knowledge** — The first copy of each program in this deck does not count against your influence limit.
- [x] **Tāo Salonga: Telepresence Magician** — Whenever an agenda is scored or stolen, you may swap 2 installed pieces of ice.

## Runner — Anarch (14/19)

Module: `decks/identities/runner_anarch.rs`

- [x] **Alice Merchant: Clan Agitator** — The first time you make a successful run on Archives each turn, the Corp must trash 1 card from HQ.
- [x] **Edward Kim: Humanity's Hammer** — Trash the first operation you access each turn at no cost.
- [x] **Esâ Afontov: Eco-Insurrectionist** — The first time each turn you suffer core damage, you may draw 1 card and sabotage 2. (The Corp trashes 2 cards of their choice from HQ and/or the top of R&D.)
- [ ] **Freedom Khumalo: Crypto-Anarchist** — Access, once per turn → Any X virus counters: Trash the non-agenda card you are accessing. X must be equal to that card's rez or play cost.
- [ ] **Hoshiko Shiro: Untold Protagonist** — When your turn ends, if you accessed a card this turn, gain 2[credit] and flip this identity.
- [x] **MaxX: Maximum Punk Rock** — When your turn begins, trash the top 2 cards of your stack. Draw 1 card.
- [x] **Nathaniel "Gnat" Hall: One-of-a-Kind** — When your turn begins, gain 1[credit] if you have 2 or fewer cards in your grip.
- [x] **Noise: Hacker Extraordinaire** — Whenever you install a virus program, the Corp trashes the top card of R&D.
- [x] **Null: Whistleblower** — Once per turn → When you encounter a piece of ice, you may trash 1 card from your grip. If you do, that ice gets –2 strength for the remainder of this run.
- [ ] **Omar Keung: Conspiracy Theorist** — Once per turn → [click]: Run Archives. If that run would be declared successful, change the attacked server to HQ or R&D for the remainder of that run.
- [x] **Quetzal: Free Spirit** — Once per turn → 0[credit]: Break 1 barrier subroutine.
- [x] **Reina Roja: Freedom Fighter** — The first piece of ice the Corp rezzes each turn costs 1[credit] more to rez.
- [x] **René "Loup" Arcemont: Party Animal** — The first time each turn you trash a card you are accessing, gain 1[credit] and draw 1 card.
- [x] **Ryō "Phoenix" Ōno: Out of the Ashes** — The first time each turn a run becomes successful after a subroutine resolved during that run, gain 1[credit] and the Corp trashes 1 card from HQ.
- [ ] **Sebastião Souza Pessoa: Activist Organizer** — Whenever you take 1 or more tags, if you had no tags, you may install 1 connection resource from your grip, paying 2[credit] less. As an additional cost to trash a connection resource with the basic action, the Corp must trash 1 card from HQ.
- [ ] **Topan: Ormas Leader** — Once per turn → [click]: Install 1 card from your grip, paying 2[credit] less. When you install that card, suffer 1 meat damage.
- [x] **Valencia Estevez: The Angel of Cayambe** — The Corp starts the game with 1 bad publicity.
- [x] **Whizzard: Master Gamer** — 3[recurring-credit] Use these credits to trash cards.
- [x] **Wyvern: Chemically Enhanced** — Draft format only. You must maintain the order of your heap. Whenever you trash a Corp card, if you have more [anarch] cards installed than any other faction, shuffle the top card of your heap into your stack.

## Runner — Neutral (3/3)

Module: `decks/identities/runner_neutral.rs`

- [x] **Nova Initiumia: Catalyst & Impetus** — Your deck cannot include more than 1 copy of any card.
- [x] **The Catalyst: Convention Breaker** — Starter game only.
- [x] **The Masque: Cyber General** — Draft format only.

## Runner — Adam (0/1)

Module: `decks/identities/runner_adam.rs`

- [ ] **Adam: Compulsive Hacker** — You start the game with 3 different directive cards installed (these cards are not considered part of your deck).

## Runner — Apex (0/1)

Module: `decks/identities/runner_apex.rs`

- [ ] **Apex: Invasive Predator** — You cannot install non-virtual resources. When your turn begins, you may install 1 card from your grip facedown.

## Runner — Sunny Lebeau (1/1)

Module: `decks/identities/runner_sunny.rs`

- [x] **Sunny Lebeau: Security Specialist** — 

## Corp — Haas-Bioroid (15/19)

Module: `decks/identities/corp_haas_bioroid.rs`

- [x] **Asa Group: Security Through Vigilance** — The first time each turn you install a card, you may install 1 non-agenda card from HQ in the root of or protecting the same server.
- [x] **Cerebral Imaging: Infinite Frontiers** — Your maximum hand size is equal to the number of credits in your credit pool.
- [ ] **Chronos Protocol: Haas-Bioroid** — Whenever the Runner trashes a card for brain damage, they remove all copies of that card from the game (installed, in the heap, stack, grip, or any other location). Then, they shuffle their stack.
- [x] **Custom Biotics: Engineered for Success** — You cannot include Jinteki cards in this deck.
- [x] **Cybernetics Division: Humanity Upgraded** — Each player's maximum hand size is reduced by 1.
- [x] **Haas-Bioroid: Architects of Tomorrow** — The first time each turn the Runner passes a rezzed piece of bioroid ice, you may rez 1 bioroid card, paying 4[credit] less.
- [x] **Haas-Bioroid: Engineering the Future** — The first time you install a card each turn, gain 1[credit].
- [x] **Haas-Bioroid: Precision Design** — You get +1 maximum hand size. Whenever you score an agenda, you may add 1 card from Archives to HQ.
- [ ] **Haas-Bioroid: Stronger Together** — All bioroid ice has +1 strength.
- [x] **LEO Construction: Labor Solutions** — Once per turn → Trash 1 rezzed bioroid card in the root of or protecting the attacked server: End the run.
- [ ] **MirrorMorph: Endless Iteration** — If the first, second, and third actions you take on your turn are each different from one another, when the third action completes, you may gain 1[credit] or take another different action, paying [click] less.
- [ ] **NEXT Design: Guarding the Net** — Before taking your first turn, you may install up to 3 pieces of ice, with no more than a single piece of ice per server. Draw until you have 5 cards in HQ.
- [x] **Poétrï Luxury Brands: All the Rage** — Whenever you score an agenda, look at the top 3 cards of R&D. You may install 1 non-agenda card from among them. Whenever an agenda is stolen, you may install 1 non-agenda card from HQ.
- [x] **Seidr Laboratories: Destiny Defined** — The first time each turn the Runner loses or spends [click] during a run, you may add 1 card from Archives to the top of R&D.
- [x] **Sportsmetal: Go Big or Go Home** — Whenever an agenda is scored or stolen, gain 2[credit] or draw 2 cards.
- [x] **Strategic Innovations: Future Forward** — Draft format only. If you have more [haas-bioroid] cards rezzed than any other faction, when the Runner's turn ends, shuffle 1 card in Archives into R&D.
- [x] **The Foundry: Refining the Process** — The first time you rez a piece of ice each turn, you may search R&D for another copy of that ice, reveal it, and add it to HQ. Shuffle R&D.
- [x] **Thule Subsea: Safety Below** — Whenever the Runner steals an agenda, do 1 core damage unless they spend [click] and 2[credit].
- [x] **Thunderbolt Armaments: Peace Through Power** — Whenever you rez a piece of AP or destroyer ice during a run, that ice gets +1 strength and gains “[subroutine] End the run unless the Runner trashes 1 of their installed cards.” after its other subroutines for the remainder of that run.

## Corp — Jinteki (10/21)

Module: `decks/identities/corp_jinteki.rs`

- [ ] **A Teia: IP Recovery** — Limit 2 remote servers. The first time each turn you install a card in the root of or protecting a remote server, you may install 1 card from HQ in the root of or protecting another remote server, ignoring all costs. You cannot score the second card this turn.
- [ ] **AU Co.: The Gold Standard in Clones** — Whenever you do damage or trash 1 or more cards from HQ, place 1 power counter on this identity. When your turn begins, you may remove 2 hosted power counters to look at the top 3 cards of R&D. Trash 1 of those cards and add the rest to HQ.
- [ ] **AgInfusion: New Miracles for a New World** — Once per turn → Trash the unrezzed piece of ice the Runner is approaching: Choose a server other than the attacked server. The Runner moves to the outermost position of that server and encounters any ice there.
- [ ] **Chronos Protocol: Selective Mind-mapping** — For the first net damage the Runner suffers each turn, you may look at the Runner's grip and select the card that is trashed.
- [ ] **Harmony Medtech: Biomedical Pioneer** — Each player needs 1 fewer agenda point to win the game.
- [x] **Hyoubu Institute: Absolute Clarity** — The first time each turn you reveal a card, gain 1[credit]. [click]: Reveal 1 card from the grip at random or the top card of the stack.
- [x] **Industrial Genomics: Growing Solutions** — The trash cost of each card is increased by 1 for each facedown card in Archives.
- [ ] **Issuaq Adaptics: Sustaining Diversity** — Whenever you score an agenda that you did not install or advance this turn, place 1 power counter on this identity. For each hosted power counter, you need 1 less agenda point to win the game.
- [ ] **Jinteki Biotech: Life Imagined** — Before taking your first turn, you may switch this identity with any copy of Jinteki Biotech. [click][click][click]: Flip this identity.
- [x] **Jinteki: Personal Evolution** — Whenever an agenda is scored or stolen, do 1 net damage.
- [x] **Jinteki: Potential Unleashed** — Whenever the Runner takes at least 1 net damage, trash the top card of the stack.
- [x] **Jinteki: Replicating Perfection** — The Runner cannot run on remote servers. Ignore this ability until the end of the turn whenever the Runner runs on a central server.
- [x] **Jinteki: Restoring Humanity** — When your discard phase ends, if there is a facedown card in Archives, gain 1[credit].
- [ ] **Mti Mwekundu: Life Improved** — Once per turn → When the Runner approaches a server, you may install 1 piece of ice from HQ in the innermost position protecting that server, ignoring all costs. The Runner moves to that ice and approaches it. If this is not the first time they have approached ice this run, they may jack out.
- [ ] **Méliès U: Only the Brightest** — When your discard phase ends, secretly set your identity to any copy of Méliès U: Only the Brightest. When the Runner makes a successful run on a central server, flip this identity. When the Runner’s action phase ends, gain 1[credit].
- [ ] **Nisei Division: The Next Generation** — Whenever you and the Runner reveal secretly spent credits, gain 1[credit].
- [x] **PT Untaian: Life's Building Blocks** — When your discard phase ends, if there are 3 or fewer cards in HQ, you may pay 1[credit] to place 1 advancement counter on an unrezzed card you can advance. (You cannot score that card this turn.)
- [x] **Pālanā Foods: Sustainable Growth** — The first time each turn the Runner draws a card, gain 1[credit].
- [ ] **Saraswati Mnemonics: Endless Exploration** — [click], 1[credit]: Install 1 card from HQ in the root of a remote server, then place 1 advancement counter on it. You cannot score or rez that card until your next turn begins.
- [x] **Synthetic Systems: The World Re-imagined** — Draft format only. If you have more [jinteki] cards rezzed than any other faction, when your turn begins, you may swap 2 pieces of installed ice.
- [x] **Tennin Institute: The Secrets Within** — When your turn begins, if the Runner did not make a successful run during their last turn, you may place 1 advancement counter on an installed card.

## Corp — NBN (16/19)

Module: `decks/identities/corp_nbn.rs`

- [ ] **Acme Consulting: The Truth You Need** — The Runner is considered to have 1 additional tag (even if they have 0) during encounters with the outermost piece of ice protecting any server.
- [x] **Azmari EdTech: Shaping the Future** — When your turn ends, you may name a card type. Gain 2[credit] the first time each turn the Runner plays or installs a card that has the type you last named this way.
- [x] **Editorial Division: Ad Nihilum** — The first time each turn you take bad publicity, you may search R&D for 1 non-agenda black ops, gray ops, or liability card and reveal it. (Shuffle R&D after searching it.) Add that card to HQ.
- [ ] **Epiphany Analytica: Nations Undivided** — The first time each turn the Runner steals or trashes a Corp card, place 1 power counter on this identity. [click], hosted power counter: Look at the top 3 cards of R&D. You may install 1 of those cards.
- [x] **GameNET: Where Dreams are Real** — Whenever a Corp card ability causes the Runner to spend or lose at least 1[credit] during a run, gain 1[credit].
- [x] **Haarpsichord Studios: Entertainment Unleashed** — The Runner cannot steal more than one agenda each turn.
- [x] **Harishchandra Ent.: Where You're the Star** — While the Runner is tagged, they play with the grip revealed.
- [x] **Information Dynamics: All You Need To Know** — Draft format only. If you have more [nbn] cards rezzed than any other faction, whenever an agenda is scored or stolen, give the runner 1 tag.
- [x] **NBN: Controlling the Message** — The first time the Runner trashes an installed Corp card each turn, you may trace[4]. If successful, give the Runner 1 tag (cannot be avoided).
- [x] **NBN: Making News** — 2[recurring-credit] Use these credits during trace attempts.
- [x] **NBN: Reality Plus** — The first time each turn the Runner takes a tag, gain 2[credit] or draw 2 cards.
- [x] **NBN: The World is Yours*** — Your maximum hand size is increased by 1.
- [x] **Near-Earth Hub: Broadcast Center** — The first time each turn you create a remote server, draw 1 card.
- [x] **Nebula Talent Management: Making Stars** — When your action phase ends, if you played an operation this turn, gain 1[credit] and flip this identity.
- [x] **New Angeles Sol: Your News** — Whenever an agenda is scored or stolen, you may play 1 current from HQ or Archives (paying its play cost).
- [x] **Pravdivost Consulting: Political Solutions** — The first time each turn the Runner makes a successful run, you may place 1 advancement counter on an installed card you can advance.
- [ ] **SYNC: Everything, Everywhere** — [click]: Flip this identity. The Runner pays 1[credit] more when spending a [click] to remove a tag (not through a card ability).
- [x] **Spark Agency: Worldswide Reach** — The first time each turn you rez an advertisement, the Runner loses 1[credit].
- [x] **Synapse Global: Faster than Thought** — The first time each turn a tag is removed, you may reveal and install 1 card from HQ, ignoring all costs. [click], remove 1 tag: Gain 2[credit].

## Corp — Weyland Consortium (12/19)

Module: `decks/identities/corp_weyland.rs`

- [x] **Argus Security: Protection Guaranteed** — Whenever the Runner steals an agenda, they must take 1 tag or suffer 2 meat damage.
- [ ] **BANGUN: When Disaster Strikes** — You may install agendas faceup. (This does not make their abilities active.) Whenever the Runner accesses a faceup installed agenda, do 2 meat damage and give the Runner 1 tag.
- [ ] **Blue Sun: Powering the Future** — When your turn begins, you may add 1 rezzed card to HQ and gain credits equal to its rez cost.
- [ ] **Earth Station: SEA Headquarters** — Limit 1 remote server. As an additional cost to run HQ, the Runner must pay 1[credit]. [click]: Flip this identity.
- [x] **Fringe Applications: Tomorrow, Today** — Draft format only. If you have more [weyland-consortium] cards rezzed than any other faction, when the Runner's turn begins, place an advancement token on a piece of ice.
- [x] **GRNDL: Power Unleashed** — You start the game with 10[credit] and 1 bad publicity.
- [x] **Gagarin Deep Space: Expanding the Horizon** — As an additional cost to access a card in the root of a remote server, the Runner must pay 1[credit].
- [x] **Jemison Astronautics: Sacrifice. Audacity. Success.** — Whenever you forfeit an agenda, place X advancement counters on 1 installed card. X is equal to the agenda point value of the forfeited agenda plus 1.
- [ ] **Nuvem SA: Law of the Land** — Whenever you finish resolving an operation or an action on an expendable card, look at the top card of R&D. You may trash that card. The first time you trash a card from R&D during each of your turns, gain 2[credit].
- [ ] **Ob Superheavy Logistics: Extract. Export. Excel.** — Once per turn → When you trash a rezzed card, except during installation, you may search R&D for 1 card with a printed rez cost exactly 1[credit] less than the trashed card's printed rez cost. Install and rez the card you found, ignoring credit costs.
- [x] **SSO Industries: Fueling Innovation** — When your turn ends, you may choose a piece of ice with no advancement tokens on it. If you do, place 1 advancement token on that piece of ice for each agenda point on all installed faceup agendas.
- [ ] **Skorpios Defense Systems: Persuasive Power** — [interrupt] → Whenever 1 or more Runner cards would be trashed (from any location), set those cards aside instead of adding them to the heap. You can look at those cards. You may remove 1 of them from the game. Then, add all of those cards that are still set aside to the heap. Ignore this ability if you have already removed a card from the game with it this turn.
- [x] **The Outfit: Family Owned and Operated** — Whenever you take 1 or more bad publicity, gain 3[credit].
- [x] **The Zwicky Group: Invisible Hands** — The first time each turn you gain credits through an ability on an agenda or operation, you may draw 1 card.
- [x] **Titan Transnational: Investing In Your Future** — Whenever you score an agenda, you may place 1 agenda counter on it.
- [ ] **Weyland Consortium: Because We Built It** — 1[recurring-credit] Use this credit to advance ice.
- [x] **Weyland Consortium: Builder of Nations** — The first time each turn an encounter with an advanced piece of ice ends, do 1 meat damage.
- [x] **Weyland Consortium: Building a Better World** — Whenever you play a transaction operation, gain 1[credit].
- [x] **Weyland Consortium: Built to Last** — Whenever you advance a card, gain 2[credit] if it had no advancement counters.

## Corp — Neutral (3/4)

Module: `decks/identities/corp_neutral.rs`

- [x] **Ampère: Cybernetics For Anyone** — Your deck cannot include more than 1 copy of any card. Your deck may include up to 2 different agenda cards from each Corp faction.
- [ ] **Cyber Bureau: Keeping the Peace** — You draw a starting hand of 10 cards. Before taking your first turn, install up to 5 cards, ignoring all install costs. Rez any number of them, lowering the total rez cost among all cards by 20. Flip this identity. Detective's Bureau: Upholding the Law The first time the Runner initiates a run each turn, force the Runner to lose 1[credit] for each agenda point in his or her score area, then you gain 1[credit] for each credit lost. [click]: Gain 3[credit] or draw 3 cards.
- [x] **The Shadow: Pulling the Strings** — Draft format only. You can use agendas from all factions in this deck.
- [x] **The Syndicate: Profit over Principle** — Starter game only.
