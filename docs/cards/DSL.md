# Writing cards

You do not need to program to write a card here. You need the card.

A card file is a plain text file. You copy the card's printed text into it,
then write one line per sentence saying what that sentence does. If you can
read the card out loud, you can write it.

```
card "Sure Gamble"
  side: runner
  type: event
  cost: 5
  faction: neutral
  text:
    Gain 9[credit].
  play:
    gain 9 credits
```

That is a whole card. The `text:` block is the printed text, copied exactly —
it is what anyone later checks the behaviour against. The `play:` block is
what happens.

## The shape of a card

Every card starts with `card "<name>"` and then a few facts, one per line:

| line | meaning | example |
|---|---|---|
| `side:` | `corp` or `runner` | `side: corp` |
| `type:` | agenda, asset, ice, operation, upgrade, event, hardware, program, resource, identity | `type: ice` |
| `subtypes:` | comma-separated, exactly as printed | `subtypes: Barrier, Tracer` |
| `cost:` | play/install/rez cost | `cost: 3` |
| `strength:` | ice and icebreakers | `strength: 4` |
| `trash cost:` | what the Runner pays to trash it | `trash cost: 3` |
| `memory:` | program MU | `memory: 1` |
| `advancement:` / `points:` | agendas | `advancement: 3` |
| `link:` | printed base link | `link: 1` |
| `unique: yes` | the ◆ cards | `unique: yes` |
| `console: yes` | consoles | `console: yes` |
| `text:` | the printed text, copied | see below |

`text:` is followed by indented lines and is **required**. Copy the oracle
text exactly, line breaks and all. Nothing else in the file is allowed to
contradict it, and a test refuses any card without it.

## Saying what a card does

After the facts come **ability blocks**. Each one starts with when it
happens, and contains one line per sentence of the card.

```
  play:                      the card is an event or operation
  paid <cost>:               "1[credit]: do something" — a paid ability
  when <trigger>:            "When your turn begins, …"
  interrupt <trigger>:       an "[interrupt] →" ability
  subroutine:                one [subroutine] line — write one block each
  static:                    a permanent fact, not an action
  static threat 4:           the same, active only at threat 4 ("Threat 4 →")
```

### Costs

Costs are written the way the card writes them, separated by commas:

```
  paid free:                 paid click:              paid click, click:
  paid 1 credit:             paid 2 credits:          paid click, click, click, trash:
  paid trash:                paid hosted power counter:
  paid hosted agenda counter:
```

A printed `Interface →` or `Access →` in front of the cost is written the
same way, first:

```
  paid interface 1 credit:   "Interface → 1[credit]: …"
  paid access 1 credit:      "Access → 1[credit]: …"
```

### Triggers

Triggers are written the way the card writes them:

| you write | the card says |
|---|---|
| `when your turn begins` | "When your turn begins, …" |
| `when installed` | "When you install this…" |
| `when scored` / `when stolen` | "When you score this agenda…" |
| `when the run ends` | "When this run ends…" |
| `when a successful run ends` | "Whenever you make a successful run…" |
| `when encountered` | "When the Runner encounters this ice…" |
| `when passed` | "When the Runner passes this ice…" |
| `when empty` | "When it is empty, …" |
| `when accessed` | "When the Runner accesses this card…" |
| `when this operation resolves` | "After you resolve this operation, …" |

## The sentences you can write

One line per printed sentence. The words are the card's words.

```
gain 5 credits                     lose 3 credits
the runner loses 2 credits         the corp loses 2 credits
draw 3 cards
do 3 net damage                    do 7 meat damage
do 1 core damage                   prevent all meat damage
give the runner 2 tags             take 1 tag
remove 1 tag                       the runner removes 2 tags
end the run                        trash self
remove self from the game          purge virus counters
your action phase ends
give the corp 1 bad publicity

load 8 credits on self             load 3 power counters on self
place 1 agenda counter on self     place 1 power counter on this ice

add 1 card from archives to hq
add 2 installed runner cards to the grip

+1 strength                        break 1 sentry subroutine
break up to 2 subroutines          break all subroutines

install up to 2 cards from hq      you may play 1 operation from hq
search your stack for 1 icebreaker
resolve the "when scored" ability on an agenda in your score area
```

Two effects in one printed sentence are one line, joined by `and`:

```
gain 4 credits and draw 3 cards
```

A trace is one line, because the card writes it as one:

```
trace 4: give the runner 4 tags
trace 3: place 1 power counter on this ice
```

A choice is written as a list:

```
  when encountered:
    choose one:
      - take 1 tag
      - end the run
```

An optional thing uses `you may`, and a cost inside a sentence uses the
card's own `to` or `unless`:

```
you may trash self to gain 3 credits and draw 3 cards
you may pay 2 credits to draw 2 cards
end the run unless the runner pays 3 credits
```

### Break abilities know when they can be used

Writing `break 1 sentry subroutine` also says *when* the ability can be used:
only during an encounter, and only with a sentry. You do not write that
separately — the rules already say it (CR 9.5.6), and the rest of the card
would only repeat it. `break up to 2 subroutines` names no subtype, so it is
usable during any encounter.

## The things a `static:` block can say

A `static:` block is for the sentences that are permanently true rather than
things that happen.

```
+1 memory                          "+1[mu]"
+1 link                            printed base link
this program gets −2 strength      "This program gets −2 strength."
this card can host a single agenda "Film Critic can host a single agenda."
limit 1 hosted card                "Limit 1 hosted card."
runs against this server cannot be declared successful
remove this card from the game instead of trashing it
this card is not trashed until another current is played or an agenda is stolen
as an additional cost to steal an agenda, you must pay 3 credits
as an additional cost to steal this agenda, the runner must pay 5 credits
as an additional cost to play this operation, spend click
as an additional cost to play this operation, forfeit an agenda
```

Note the difference between "steal **an** agenda" (every agenda, for as long
as this card is around) and "steal **this** agenda" (printed on the agenda
itself). Write whichever the card writes.

## When a sentence has no words yet

The vocabulary is still growing. If a sentence of the card cannot be written
yet, say so, in the card's own words:

```
  unimplemented: "If you made a successful run this turn, you may install that program."
```

The card still exists, still carries its text, and still works for everything
it *can* do — but it is counted honestly as partial, it will not be playable
in a strict game (DESIGN.md SYS-D-12), and it shows up on the gap list. Never
approximate a sentence you cannot express: write the marker instead.

Two rules of thumb for the marker:

- If the sentence would *do the wrong thing* rather than nothing — say, a
  restriction the engine cannot yet honour — the marker is still the right
  answer, even when the words exist. A card that quietly misbehaves is worse
  than one that says it is incomplete.
- Write the marker for one printed sentence at a time, so the gap list counts
  sentences and not paragraphs.

## What happens to what you wrote

The file is data. It is read into the rules engine's own instruction
vocabulary, which comes from the Comprehensive Rules' §9.11 taxonomy of
instructions — the same taxonomy that tells a judge where one instruction
ends and the next begins. That is why writing a card is transcription rather
than translation: the DSL is shaped like the rules, and the rules are shaped
like the card.

Errors name the card, the line, and what to do about it:

```
cards/gauntlet.cards:41 in "Gold Farmer": unknown sentence
  "the runner loses 1[credit] whenever they break a printed subroutine"
  hint: see docs/cards/DSL.md for the sentences you can write; if the card
        says something the vocabulary cannot yet say, write it as
        `unimplemented: "<the printed sentence>"` instead of approximating it
```

## Rules of thumb

1. **Copy the text first, always.** Behaviour is checked against it.
2. **One line per printed sentence.** If your line is doing two things, it is
   probably two sentences on the card, and the CR agrees (§9.11.3) — unless
   the card joined them with "and", which is the one sentence you write with
   `and`.
3. **Restrictions are not sentences you write.** "Limit 1 per deck" or "Limit
   1 region per server" belong in the facts or nowhere, never as an action.
4. **If you're unsure whether it's a cost or an effect**, look for the colon
   on the printed card: everything before it is the cost.
5. **Write the marker rather than a lie.** A partial card is fine. A card
   that quietly does the wrong thing is not.
