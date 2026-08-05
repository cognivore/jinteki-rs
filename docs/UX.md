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

   A third colour answers a third question. Gold and green say *what you
   may do*; WHITE says *what the game is waiting on you for*, and it is
   never an affordance. It lands in two places and means one thing:

   * `.priority` on an identity — **that seat owes the next word**,
     continuously, including while they are only choosing an action on
     their own turn. Both identities are drawn, but the identity card
     column exists only where the screen is tall enough (`.identity-col`
     is hidden under 640px), so the seat rail's `.idchip` is the copy that
     must always carry it.
   * `.staged` on a card — **chosen by you, not yet done**. See §7.

   A hueless white, so neither amber nor acid green can be confused with it.

   The corollary: a prompt may only fall silent about a card **because the
   board is already showing it**. That test is the server's `on_screen`, one
   function, used by every prompt that considers keeping quiet — a zone the
   board draws as cards *and* a face this viewer may see. If it were two
   tests they would drift, and a drifted test is a question with no answer
   anywhere on screen.

3b. **Nothing that merely shows you something may hold the board hostage.**
   Every reader closes by tapping away from it — on pointer and on touch,
   via `pointerdown` (a long-press opens a reader with the pointer already
   down, so no `click` is ever synthesised) — and by Escape. A reader that
   is also a decision steps aside instead of discarding it, leaving a way
   back. The one preview that cannot be tapped through (`.hover-preview` is
   `pointer-events: none` by design) is tied to the element it belongs to
   and dies with it, because a re-render destroys hovered elements without
   ever firing `mouseleave`.

4. **Chips are the exception, and only for crowding.** When too much ice
   protects one server to show as cards, that stack — and only that stack —
   collapses to chips. Nothing else may.

5. **A chip is still a card.** Every chip keeps hover preview on a pointer
   device and long-press preview on touch. Collapsing for space must never
   cost the ability to read what a card says.

6. **An empty answer is stated, never implied — and it is always GIVABLE.**
   "Search your stack for an icebreaker" with no icebreaker left says so — a
   prompt that silently does not appear is indistinguishable from a bug, and
   players reported exactly that about Mutual Favor. The second half was
   learned the hard way: installing Boomerang with no ice on the table drew
   "Choose 0 cards. No card qualifies — there is nothing to choose", with no
   card to tap and no button to press. Stating the empty answer and then
   offering no way to give it is worse than not asking, because the game
   stops. **Every prompt carries at least one thing the player can do.** A
   sheet that renders no cards and no buttons is a bug in itself, whatever
   the server sent.

   The kernel's half of this is the stronger guarantee: a decision with
   exactly one legal answer is never asked at all (`Vm::forced_answer`, CR
   1.15.2b). The UI rule stands anyway — the client must not depend on the
   server never making a mistake.

7. **An irreversible choice is staged, then confirmed — but it is CHOSEN
   the way everything else is.** 5.5.4c cannot be taken back, and it is the
   one decision a player makes with no clicks left and their mind already on
   next turn, so the discard still accumulates in white (`.staged`), the
   sentence still changes to name what is about to happen, and a separate
   button still does it. What is NOT special is the picking: it used to have
   a verb of its own ("tap to MARK it") and therefore an affordance of its
   own to learn. Choosing a card out of a pool is §8's two taps, everywhere,
   with the same sentence printed under every one of them; the button is the
   multi-pick's *done*, not a second way to pick. Every other card choice is
   an *announcement* (CR 1.15.2) with nothing yet to undo, and those commit
   on the last pick — staging everything would be ceremony, staging nothing
   was a trap.

8. **In a fan, the first tap focuses and the second acts — and the list
   never moves.** A crowded fan draws a resting card as a strip far below the
   48px a tap target has to be, so the first tap brings a card to focus,
   lifted and scaled clear of its neighbours, and the second is the one that
   plays, discards or picks it. On a pointer device the focus follows the
   mouse (MTGA), so the two taps are only ever two on touch. A tap is a press
   and a release in the same place: a pointer that has travelled is not a tap
   on whatever it happens to be over when it lifts.

   The row itself is STATIC. It does not slide, scrub, spring or reflow under
   a pointer, because a list that moves while you are reading it cannot be
   read — the carousel that used to do this was reported as a wobble and has
   been deleted, not tuned. Cards outside the window are reached by the
   rail's chevrons, by the peeks, or on a pointer by hovering the fan's outer
   edge. A DRAG means exactly one thing, in exactly one place: in a zone you
   may rearrange (CR 8.3.3) it carries the one card you picked up, and every
   other card stands still. Anywhere else a drag is not a gesture at all. A
   press that wanders a little is still a press — a thumb is not a mouse.

8b. **A fan uses the width it has.** The window is not a constant: the row
   measures the free band its neighbours leave it and lays the cards out to
   fill it — five cards on a wide band stand side by side at full size, and
   the tight nine-in-212px packing is the WORST case, not the layout. The
   step shrinks continuously as the count grows, so one card more is a
   slightly tighter row and never a jump into a different mode; only when the
   band genuinely cannot hold the list do the peeks and the rail appear. The
   arc flattens as the row spreads: overlapping cards fan, cards standing
   clear of each other lie flat.

9. **What a rule entitles you to see, you are SHOWN.** CR 7.1.2 lets the
   Runner look at a card they are accessing; most accesses ask them nothing,
   so for most accesses the card existed only as a line in the log drawer.
   An entitlement discharged into a log is not discharged. The card is
   snapshot when the entitlement is live (`state.accessed`) — by the time a
   state is pushed the access is over and `vm.st.accessed` is already null —
   and carried until the player has dismissed it.

10. **The hand and every "choose one of these" are ONE widget.** Not two that
   look alike — one function draws both, and a caller cannot answer a single
   question about how the fan behaves. It supplies what is in the row and
   what each slot is captioned; the fan supplies the size, the arc, the
   focus, the gestures and the two taps. Where they must differ it is one
   parameter, never a second code path. This is not tidiness: when the prompt
   built its own slots it forgot to pass them the fan's index, so a single
   tap on a 16px strip answered a question the hand would have asked twice
   for — and the prompt's cards were, for the whole life of that code, not
   clickable at all, because the sheet they live in is `pointer-events: none`
   and only the hand's copy had ever opted back in.

11. **A card is a BOX, and everything it carries is drawn inside it.** Cost,
   strength, every counter (CR 1.9.5's kinds), and the ✓ of a staged choice.
   Nothing hangs off a corner: in a fan the only part of a resting card you
   can see is the strip its neighbour does not cover, and a badge that
   overhangs is a badge painted across that strip. Counters fill the
   card's top-right corner right-to-left and wrap downward, and past two they
   get smaller rather than spreading — whatever a card is carrying fits, at
   every size this UI draws a card at, and nothing is ever dropped, because a
   counter the player cannot see is a counter they will misplay.

12. **Where the screen can afford it, the focused card is READABLE.** A fan
   draws a card small enough to recognise and too small to read. In landscape
   the focused card — whichever card the focus is on, in the hand or in a
   prompt — is drawn at reading size on the right, and it follows the focus.
   When the focused option is an ability rather than a card, the card shown
   is the one the ability LIVES ON, with the option's own words under it: "use
   the second ability" names nothing by itself. The panel shows and never
   asks (`pointer-events: none`, §3b), and it is bounded to clear every other
   thing pinned to that edge.

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

11. **A badge lives on the sliver that survives** (THE LAW §11, carried into
    every truncation). In an overlapping fan the only part of a resting card
    on screen is the LEFT strip its neighbour does not cover — and the
    counters sat top-right, which is the first part of the card to go under,
    so a fanned card carrying three virus counters read as bare. When a row
    overlaps (`.fanrow.overlapped`, written by `renderFan` from the same step
    arithmetic that laid the row out) a resting card's badges re-anchor down
    that strip, one disc per kind, below the cost disc; the focused card is
    whole and keeps the top-right fill, so several kinds still cluster
    without covering each other or the title. The right peek anchors left for
    the same reason; the left peek shows the card's right sliver and keeps
    the corner. And the deepest truncation of all — the ice sliver — carries
    the same discs inline (`sliverBadges`), because an advanced Ice Wall that
    reads like a bare one is a counter the player will misplay. Overlay only,
    never a reflow: the badges move within their card, and the board does not
    move at all (THE LAW §2).

12. **Accesses arrive one at a time** (CR 7.5, kept visible). The kernel
    resolves a breach access by access — each its own decision round-trip —
    and the client now presents exactly that shape: every reveal is ONE card,
    whole, oldest first ("You accessed — from R&D · 2 more to see"),
    acknowledged before the next appears, so it is never ambiguous which card
    is in front of the player. A decision the machine stopped on (steal,
    trash) waits behind the reveals that predate it — every snapshot in
    `state.accessed` was taken before the stop, so the reveals are always the
    earlier beat — and "access A, then steal B" reads as A, then B, exactly
    as it resolved. "You accessed 3 cards" over a grid of thumbnails was a
    summary of a sequence; Archives, where a whole pile arrives at once, was
    the worst of it, and facedown cards turning faceup ride the same
    one-per-beat path. Presentation only: the kernel's sequence is untouched,
    the tap that acknowledges is the same tap every reader takes (§3b), and
    nothing is ever trapped — Escape and tap-away advance, and the log keeps
    every line.

13. **Every stat of each side fits, at every width** (the fan's elastic step,
    applied to the seat rails). The rail's box is fixed — the board's inset
    reserves it, so it can never reflow the board (THE LAW §2) — but its
    content is not: a 43-card R&D, twelve credits, a tag, and an identity
    name are wider than the box on a narrow screen, and the old answer was
    clipping chips, folding "AP · TAG · MU" into ragged lines, and cutting
    the name to "Nebula T…" at a fixed 70px — every one of which is a number
    the player then plays without. Everything in the rail is now sized in em
    off the bar's own font, and the font carries `--sscale`: `fitSideStats`
    measures the stack against the stylesheet's own max-width/max-height
    (the box IS the budget, read rather than duplicated), and writes the one
    number that makes it all fit — the same measure-then-write-one-property
    shape as the fan's `fit.step`, re-run on every state push (digit counts
    change width) and on resize (the box is in vh). Down-scale only, floored
    at 9px: below the floor the rail switches to `.cram` — the tightest
    packing, chips keeping every stat, the identity name folding to two
    lines before it may truncate at all — because a smear nobody can read
    drops information exactly as surely as clipping it did. The floor holds
    against the NAME (its two-line ellipsis is the deal the floor struck)
    but yields, last of all, to the BOX: on a viewport too short even for
    the cram at 9px, the scale keeps going, because a 8px stat is squinted
    at and a clipped one is played without. The faction colours and the
    dotted tap affordances ride through untouched; nothing is merged away,
    nothing is dropped.

## Deliberate deviations

- **No drag-to-play, and no drag-to-navigate.** HS drags cards; we
  tap-then-act. On small screens with fanned hands, tap+sheet measured fewer
  misplays in HS's own later UX iterations (see mulligan flow). Dragging a
  fan to move it through a window was tried and REMOVED: see §8. The one
  drag that survives carries a card in a rearrangement.
- **A fan may draw cards below the 48px tap target, and only then.** The
  elastic layout (§8b) keeps cards at full size while the band allows it, so
  the sub-48px strip now happens only when the list genuinely outgrows the
  room. It is still a deviation, and §8's two taps are still what pays for
  it: the strip is never the tap target, the focused card is.
- **A prompt sheet steps aside for the reading panel.** In landscape the
  sheet is centred on what is left of the screen beside §11's panel rather
  than on the screen itself. It is a fixed offset per orientation, decided by
  a media query and not by any game state, so nothing moves while the game is
  being played (THE LAW §2).
- **The right-hand panel overlays the board's right edge.** §11's preview has
  to be somewhere, and every edge of a landscape phone is already spoken for.
  It overlays rather than reflows — the board's layout never moves (§2) — and
  it is `pointer-events: none`, so the cards under it are still reachable if
  the row it covers is ever full.
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
