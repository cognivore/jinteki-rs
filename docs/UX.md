# UX.md — what we stole from MTG Arena mobile and Hearthstone mobile

Complex card games on phones are a solved problem — Blizzard and Wizards spent
nine figures solving it. This document records the lessons we assimilated and
maps each one to a concrete feature in `ui/`. The card pool is text-rendered
for now (no art assets), so all the juice budget went into interaction.

## THE LAW: cards are shown as cards, and the board does not move

This governs every prompt, and it outranks the lessons below where they
conflict.

1. **If a decision is about cards, it renders as CARDS.** Not a list of
   titles in buttons. A player recognises art and layout faster than they
   read, and half of Netrunner's decisions are "which of these do I know
   least about". A prompt naming Boomerang and Pinhole Threading must show
   Boomerang and Pinhole Threading.

2. **The board LAYOUT does not move to ask a question.** Cards do not
   reflow, servers do not resize, the rig does not reorder. A player is
   holding a mental picture of the board; rearranging it to ask something
   destroys that picture and costs them the read they had already done.
   Prompts overlay; they do not rearrange.

3. **Where the board itself can answer, ask it there.** An ability that can
   be used right now is a GREEN outline on its own card (`.usable`), tapped
   to use — not a button naming it. A legal target is a GOLD outline
   (`.selectable`). Two different questions, two colours, never merged.

4. **Chips are the exception, and only for crowding.** When too much ice
   protects one server to show as cards, that stack — and only that stack —
   collapses to chips. Nothing else may.

5. **A chip is still a card.** Every chip keeps hover preview on a pointer
   device and long-press preview on touch. Collapsing for space must never
   cost the ability to read what a card says.

6. **An empty answer is stated, never implied.** "Search your stack for an
   icebreaker" with no icebreaker left says so — a prompt that silently does
   not appear is indistinguishable from a bug, and players reported exactly
   that about Mutual Favor.

## The lessons, and where each one lives

1. **One decision on screen at a time** (HS's whole design thesis). The engine
   is prompt-driven: at any moment exactly one side has exactly one decision.
   The prompt sheet (`.prompt-sheet`) is centered, large-type, impossible to
   miss — Hearthstone's Discover pattern. Nothing else competes for attention
   while it is open.

2. **Legality is shown, not discovered** (MTGA's glow). Cards you can act
   with get the cyan `.legal` glow; select-prompt targets get the gold
   `.selectable` glow. The data comes straight from the engine's legality
   enumerator (`enumerate_actions`) — the UI never guesses, so the glow is
   never wrong. In bridge mode (reference server) there is no enumerator, so
   the UI degrades to generic affordances and lets the server be the
   authority — exactly how jinteki.net's own client behaves.

3. **Big tap targets, bottom-anchored** (both games). Every button is ≥48px.
   Prompt buttons are 48px+ chips. The hand is a fanned arc at the bottom
   center — thumb territory — with overlap that expands on touch (HS hand
   fan). Tap raises the card and opens its action sheet; tap-away lowers it.

4. **The End Turn button is sacred** (HS). Fixed right-middle circular
   button. Grey while you still have things to do, pulsing green
   (`.ready`) when your clicks are spent or it is the only sensible action.
   Start Turn reuses it — the game never advances without your consent,
   which doubles as the pacing gate for watching the bot's moves.

5. **Long-press to read** (both games). 420ms press on ANY card — hand,
   board, facedown, ice — zooms it: full title, type line, stats, rules text,
   counters, subroutine state. Release taps never misfire as plays: the
   press timer cancels the tap.

6. **Attack visualization** (HS's arrow, MTGA's red glow). A run paints the
   target server column with a red pulsing frame (`.run-target`), the
   currently-approached/encountered ice gets the danger glow
   (`.current-ice`), and the phase pill narrates: "R&D: ENCOUNTER",
   "Movement — jack out?". Continue / Jack out live bottom-right as big
   thumb buttons only while they are legal.

7. **Watching the opponent act must be legible** (both). The bot plays with
   a 350ms cadence, one state push per action, so installs and plays appear
   as discrete, followable events (cards animate in via `dealin`). The
   opponent bar shows a "thinking…" pulse while it is their move.

8. **Numbers that change should announce it** (HS juice). Credit/click chips
   bump-animate on change (`.bump`). Damage discards animate into the heap.

9. **Chrome is minimal, logs are a drawer** (MTGA). The game log (with chat
   in bridge mode) slides in from the right edge tab; the board owns the
   screen. Toasts are transient and top-center.

10. **Landscape-first, portrait-tolerant** (both games are landscape-locked;
    we soft-prefer it). Layout uses safe-area insets, horizontal-scrolling
    server row with snap — remotes grow rightward like MTGA's battlefield
    lanes.

## Deliberate deviations

- **No drag-to-play yet.** HS drags cards; we tap-then-confirm. On small
  screens with fanned hands, tap+sheet measured fewer misplays in HS's own
  later UX iterations (see mulligan flow). Drag is a later nicety.
- **No card art.** Text cards with type-colored frames. The information
  hierarchy (cost badge top-left, strength bottom-left, counters top-right)
  copies physical Netrunner card anatomy so players' eyes already know it.
- **Corp always on top.** Netrunner table convention beats HS mirroring;
  your hand is always yours at the bottom regardless of side.

## Backend enablers (per DESIGN.md I-10)

Every lesson above leans on backend properties: the legality enumerator
(SYS-F-2) powers the glow; the prompt queue with uuid choices powers the
one-decision model; per-action state pushes power legible pacing. This is
why the backend rebuild and the mobile client are one project.
