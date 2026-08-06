# UX.md — what we stole from MTG Arena mobile and Hearthstone mobile

Complex card games on phones are a solved problem — Blizzard and Wizards spent
nine figures solving it. This document records the lessons we assimilated and
maps each one to a concrete feature in `ui/`. Cards carry their printed ART,
served from our own box (`/img/card/<code>.jpg`, a local cache the server
pre-warms with the whole catalog), over a text scaffold that shows through
only for a card whose art we genuinely do not have.

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
   never an affordance. It lands in three places and means one thing:

   * `.priority` on an identity — **that seat owes the next word**,
     continuously, including while they are only choosing an action on
     their own turn. Both identities are drawn, but the identity card
     column exists only where the screen is tall enough (`.identity-col`
     is hidden under 640px), so the seat rail's `.idchip` is the copy that
     must always carry it.
   * `.staged` on a card — **chosen by you, not yet done**. See §7.
   * `.armed` (a ring OUTSIDE the border, on a card, an ice sliver, an
     identity chip or a server column) — **the next tap on this exact
     thing commits**. The first tap on any candidate arms it; a tap on a
     different candidate re-arms to that one; Escape or bare board
     disarms; only the second tap on the armed thing answers. The ring is
     outside the border so the gold candidate outline survives under it —
     an armed target still reads as a target. See lesson 16.

   A hueless white, so neither amber nor acid green can be confused with it.

   The corollary: a prompt may only fall silent about a card **because the
   board is already showing it**. That test is the server's `on_screen`, one
   function, used by every prompt that considers keeping quiet — a zone the
   board draws as tappable nodes. The face is deliberately NOT part of the
   test: a facedown card the board draws (unrezzed ice, an unadvanced
   ambush) is still a place with an outline and a tap, and §10.2 blanks a
   sheet's copy exactly as it blanks the board's — so requiring the face
   only replaced three tappable slivers with three identical blank
   thumbnails. Choosing among facedown cards by where they lie is how the
   physical game does it. If this were two tests they would drift, and a
   drifted test is a question with no answer anywhere on screen.

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

14. **What the engine offers, the board draws — even out of Archives.**
    THE LAW §3's "where the board itself can answer, ask it there" has a
    quiet precondition: the board must be DRAWING the card the answer lives
    on. An ability can act from a zone the board draws only as a count —
    "[click]: Play this operation from Archives" (Petty Cash, CR 9.3.3c)
    put a legal action on a card whose only pixels were "Archives 1", so
    the glow had nowhere to land and the play existed only for a player who
    thought to open the pile reader. Now any card the engine offers an
    action on that is drawn nowhere joins the top-right play rail
    (`renderPlayRail`), tagged with the zone it acts from ("archives",
    "heap", "scored"), wearing the same glow ladder and answering the same
    tap as every other card — one dispatch path, so the rail's copy can
    never do less than the hand's would. The drawn-set the client checks
    mirrors the server's `on_screen`: piles and score areas are counts you
    tap to open, so a card in one is nowhere an outline could land.

15. **A row wider than the screen scrolls, and says so.** More remotes than
    the viewport is not a layout problem to solve by reflowing (THE LAW §2)
    — it is a window the player's own hand moves over a board that stands
    still. `overflow-x: auto` was already true and already insufficient:
    touch and trackpads could pan the server row, but nothing SAID so — no
    scrollbar until mid-scroll, no cue at the cut edge — and a mouse had no
    way in at all. The clipped edge now carries a chevron (the fan rail's
    own chip, floating over the row's end, gone the moment the row fits),
    tap for a viewport-width of servers; a mouse wheel over the row scrolls
    the only axis the row has; shift-wheel, trackpads and touch keep their
    native pan. An affordance that appears only when there is somewhere to
    go — the same rule as the fan's rail (§8), for the same reason: a
    control for a journey of zero servers is a lie.

16. **A facedown card is a card back for everyone — its owner included.**
    CR 1.21.1 orients a facedown card so its face is not visible, as a fact
    of the table and not of the viewer; CR 4.6.6f will not even let a
    remote's root give away what kind of card sits in it. An unrezzed
    agenda that renders faceup to the Corp is therefore wrong twice: it
    breaks the table (the owner's board and the opponent's board disagree
    about what the table looks like), and it wastes the one signal a back
    carries — "this is hidden, and hover is how you look". The owner IS
    entitled to the face: CR 1.21.2a lets a player look at facedown cards
    they control at any time, for both players symmetrically — and that
    look is the READER (hover, long-press, the prompt fan), never the
    board. The wire enforces the other half: CR 4.6.3 makes a facedown
    card in the play area secret information, so the opponent's state
    never carries its face at all (`card_json` sends presence and
    orientation only — no title, no text, no art id), and no amount of
    devtools spelunking can read what was never sent. Counters, outlines
    and the armed ring still overlay the back: WHERE a counter sits is
    open information (§11), and a back the game is waiting on still glows.
16. **A board question is asked ON the board, and the log carries the
    sentence.** When every candidate of a decision is something the board
    is already drawing — installed cards, cards in your own hand, servers —
    NO sheet appears at all. The popup that used to float over the table
    saying "Choose a host" was a reminder, and it covered the very cards it
    was asking about: three copies of the same icebreaker under a dialog
    that names the icebreaker is the question at its least answerable. Now
    the candidates wear the gold (a server column wears it on the column;
    "a new remote" gets a placeholder column to wear it on), the first tap
    arms one WHITE, and the second tap on the armed one commits — the same
    two taps as everywhere else, and the tap is on the physical node, so
    same-named copies are inherently told apart. What is not a place on the
    board docks in the bottom action rail as chips (Pass, "Your rig", a
    discard's Done, the Cancel that un-arms) — chrome the player already
    owns, never over a card. And the sentence the popup used to carry is a
    LOG LINE, written once when the decision is put, to both logs:
    "{identity} choosing target for {ability} ({source})", the parenthesis
    only when the source card is not the ability's own name — so a player
    who feels stuck can always open the log and read what the game is
    waiting for, and their opponent can see them thinking, exactly as at a
    physical table. Bare "waiting for them" sheets died with the rest of
    the reminders: the seat rail's pulse and the log line carry it. The
    prompts that keep a panel are the ones whose candidates the board
    CANNOT draw — hidden-zone picks, option lists, arrangements, divisions,
    numbers — and a panel that is real UI is not a reminder.

17. **A window of cards is answered from the RAIL, never from a modal —
    because a card the board draws as a BACK is not shown.** Lesson 16 sent
    a decision to the board whenever the board was drawing its cards, and
    lesson 16's twin (the facedown law) then made "drawing it" and "showing
    it" different things: the Corp's own installed agenda is facedown until
    it scores, so the paid ability window that offers "Score AstroScript
    Pilot Program" was pointing at a blank rectangle. The old answer was a
    modal with the agenda inside it, over the table, with a Pass button —
    THE LAW §1 satisfied by breaking §2.

    The right-hand rail (`#play-rail`, `renderPlayRail`) is the answer. It
    already existed for the two other cases where the board cannot show a
    card — the play area mid-resolution, and a card playable out of a pile
    (lesson 14) — and a §9.2 window is the third: its offers go there as
    real cards, grouped under the VERB each one is ("Score", "Rez", "Play",
    "Install", "Use", "Trash", "Resolving"), in a fixed order so a player
    learns where "Score" appears and stops reading the rail. The server says
    which decisions qualify (`window-cards`: a paid/reaction/interrupt/
    mid-access window whose every offer but the pass carries a card), so the
    client never has to guess — a target announcement over three cards in
    the stack also carries cards, and THAT one keeps its panel, because
    there the question is which card and not what to do with one.

    A card the board is already showing FACE UP is not copied into the rail:
    the board answers where the board can (§3), and two copies of one card
    is the defect the rail exists to avoid. The pass docks in the bottom
    action rail beside "Gain 1⬡", labelled for the window it ends ("Pass the
    paid window") because a bare "Pass" among the turn's chips is a button
    with no sentence. The window's own sentence — including 5.6.2a's last
    call, which was reported from a real game as five advancement counters
    lost to one tap — is a LOG LINE, exactly as lesson 16 does it.

    The two-tap gate survives, and got sharper: with one option offered, the
    armed hint NAMES it ("Score AstroScript Pilot Program — tap again to
    confirm") instead of the sheet doing so, and the hint wraps rather than
    ellipsing, because that line is now the whole gate. A card offering
    SEVERAL options still opens a sheet naming each: one ring cannot name
    two acts, and 9.2.7f makes whichever is taken resolve to the end.

18. **A server's root is one tight stack, newest on top.** The root was a
    column of separated cards, which said something false about the game —
    that these are four places rather than one pile — and spent the vertical
    budget four times over, on the axis that runs out first. Now the cards
    tuck: each new one slides onto the stack (`renderServers` → `rootStack`,
    `.root-stack.tucked`), covering all but a sliver of the one before, so
    the stack has visible depth and can be counted at a glance.

    The sliver is sized to a JOB and not to taste: 26px on a 64×88 card
    (22px on a phone's 52×72) clears the 1.9em cost disc, the 1.7em counter
    badges and the first line of the name, so every member of the stack is
    still identifiable and tappable. Below that it would be depth without
    identity. Later siblings paint over earlier ones by DOM order, so
    "newest on top" needs no z-index — only the ARMED card is lifted, since
    that is the one the next tap acts on and a gate you can only half see is
    not a gate.

    §11's law follows the same rule it always did: the badges ride the edge
    that SURVIVES. Here the tuck covers each card's bottom, so counters stay
    in their top-right corner — and on the flipped board, where they had
    moved to the bottom because the columns hang from the bottom edge, they
    come back to the top for the covered cards alone. Facedown cards stay
    card backs (lesson 16's twin); their faces are read where every other
    hidden face is read — the reader, and the rail when the game is actually
    asking about them.

19. **Compress to slivers first; then SCROLL, never squeeze.** Ice collapses
    to slivers because that is the right answer to depth (§4/§5: a chip is
    still a card). But a glacier deck stacks five or six on one server, and
    six legible slivers are taller than the Corp half however small the rest
    of the board gets. The sliver has a FLOOR — 92px wide on a landscape
    phone, sized so "Tollbooth" reads as a name and not "Tol…" — and past
    that floor the honest answer is to pan the region, not to keep shrinking
    until the stack is a striped bar.

    So `.servers` owns both axes: across for more remotes than the screen is
    wide (lesson 15), down for a column deeper than the half. Both carry the
    same edge affordance, a chevron that appears only while there is
    somewhere to go, laid out so the two pairs never overlap. This is the
    player's hand moving a window over the board, not the board moving
    (§2): no card changes place, and at ordinary depths nothing appears.

    Two details are load-bearing. `align-items: safe flex-end` on the
    flipped board: a flex container that aligns to the END and then
    overflows puts the overflow off the START edge, where no browser will
    scroll to it — without `safe`, the deepest ice would be unreachable in
    the Corp's own seat and in no other. And `touch-action` stays at the
    global `manipulation`, so the BROWSER owns both pans: a pan it claims
    fires `pointercancel`, which is how a drag over the ice cancels a
    pending long-press instead of opening a reader under the player's thumb
    (the other two routes — the 14px movement threshold and the
    capture-phase `scroll` listener — cover the rest).

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
- **Art is ours, and the scaffold is the fallback.** The card box is the
  physical card's anatomy (cost badge top-left, strength bottom-left,
  counters top-right) with the printed art filling it. The art is fetched
  ONCE per printing into a cache under the server's data dir and served from
  `/img/card/<code>.jpg` — never hot-linked. A CDN that rate-limits or
  refuses a request does not blank a card in this UI, because by the time a
  player opens the builder the image is already on our disk; a card with no
  art anywhere still reads, as the type-coloured text scaffold.
- **Corp always on top.** Netrunner table convention beats HS mirroring;
  your hand is always yours at the bottom regardless of side.

## Backend enablers (per DESIGN.md I-10)

Every lesson above leans on backend properties: the legality enumerator
(SYS-F-2) powers the glow; the prompt queue with uuid choices powers the
one-decision model; per-action state pushes power legible pacing. This is
why the backend rebuild and the mobile client are one project.
