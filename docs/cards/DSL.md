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
| `unique: yes` | the ◆ cards | `unique: yes` |
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
  subroutine:                one [subroutine] line — write one block each
  static:                    a permanent fact, not an action
  interrupt <trigger>:       an [interrupt] → ability
```

Costs are written the way the card writes them: `paid 1 credit:`,
`paid click:`, `paid trash:`, `paid click, click:`, `paid free:`.

Triggers are written the way the card writes them:

| you write | the card says |
|---|---|
| `when your turn begins` | "When your turn begins, …" |
| `when installed` | "When you install this…" |
| `when scored` / `when stolen` | "When you score this agenda…" |
| `when the run ends` | "When this run ends…" |
| `when encountered` | "When the Runner encounters this ice…" |
| `when empty` | "When it is empty, …" |
| `when a successful run ends` | "Whenever you make a successful run…" |
| `when accessed` | "When the Runner accesses this card…" |

## The sentences you can write

One line per printed sentence. The words are the card's words.

```
gain 5 credits                    lose 3 credits
draw 3 cards                      the runner loses 2 credits
do 3 net damage                   do 7 meat damage
give the runner 2 tags            take 1 tag
end the run                       trash self
gain 1 click                      purge virus counters
load 8 credits on self            take 2 credits from self
load 3 power counters on self     remove 1 power counter from self
place 2 advancement counters on a card you can advance
trace 4: give the runner 4 tags
run hq                            access 2 additional cards
install 1 card from your grip     add 1 card from archives to hq
search your stack for 1 icebreaker
```

Numbers can be a count *for each* something:

```
gain 2 credits for each credit lost
```

A choice is written as a list:

```
  play:
    choose one:
      - gain 3 credits
      - draw 3 cards
```

An optional thing uses `you may`:

```
    you may trash self: gain 3 credits and draw 3 cards
```

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
  hint: this is a "whenever" clause — start an ability block with
        `when a subroutine is broken:` and put the sentence inside it
```

## Rules of thumb

1. **Copy the text first, always.** Behaviour is checked against it.
2. **One line per printed sentence.** If your line is doing two things, it is
   probably two sentences on the card, and the CR agrees (§9.11.3).
3. **Restrictions are not sentences you write.** "Limit 1 per deck" or "You
   cannot…" belong in the facts or in a `static:` block, never as an action.
4. **If you're unsure whether it's a cost or an effect**, look for the colon
   on the printed card: everything before it is the cost.
5. **Write the marker rather than a lie.** A partial card is fine. A card
   that quietly does the wrong thing is not.
