# Writing cards

You do not need to program to write a card here. You need the card.

A card is a short block of Rust, but nothing about it is programming. You copy
the card's printed text in, then make one call per printed sentence saying what
that sentence does. If you can read the card out loud, you can write it.

```rust
use jinteki_cards::edsl::*;

/// Sure Gamble — Event. Cost 5.
/// "Gain 9[credit]."
fn sure_gamble() -> Card {
    card("Sure Gamble")
        .runner()
        .event()
        .cost(5)
        .text("Gain 9[credit].")
        .play([gain(Runner, 9)])
        .build()
}
```

That is a whole card. The `.text(…)` is the printed text, copied exactly — it
is what anyone later checks the behaviour against. The `.play([…])` is what
happens.

Two things are worth knowing before anything else:

- **The compiler is your proof-reader.** A sentence the vocabulary cannot say
  will not compile. That is the point: it means a card can never quietly do
  the wrong thing because a word was almost right.
- **Autocomplete is the manual.** Type `card(` and the types lead you. Type a
  dot after the builder and every fact and every kind of ability appears, each
  with the printed wording it stands for in its tooltip. You can write most
  cards without leaving the editor.

## A whole card, slowly

Here is Daily Casts, which has three printed sentences and one of them cannot
be said yet.

```rust
use jinteki_cards::edsl::*;

/// Daily Casts — Resource. Install 3.
/// "When you install this resource, load 8[credit] onto it. When it is empty,
///  trash it.
///  When your turn begins, take 2[credit] from this resource."
fn daily_casts() -> Card {
    card("Daily Casts")
        .runner()
        .resource()
        .cost(3)
        .text("When you install this resource, load 8[credit] onto it. When it is empty, trash it.")
        .text("When your turn begins, take 2[credit] from this resource.")
        .when(installed(), [load(CounterKind::Credit, 8)])
        .when(empty_of(CounterKind::Credit), [trash_self()])
        .unimplemented("When your turn begins, take 2[credit] from this resource.")
        .build()
}
```

Reading it in order:

1. `card("Daily Casts")` names it. The name is exactly as printed.
2. `.runner().resource().cost(3)` are the facts under the name.
3. `.text(…)` twice — one call per printed **line** of the text box, copied
   character for character. This is required; a card without it does not
   build.
4. `.when(installed(), …)` and `.when(empty_of(…), …)` are two of the printed
   sentences. Each reads like the card: *when installed*, load 8 credits;
   *when it is empty*, trash it.
5. `.unimplemented(…)` is the third sentence, quoted from the card, because
   nothing in the vocabulary moves credits from a card into a credit pool
   yet. The card still exists, still loads and still trashes itself — it is
   just counted honestly as partial.

The doc comment above the card carries the printed text too, for whoever is
reading the file. A test checks that the comment and the `.text(…)` calls say
the same thing, which is the only reason writing it twice is safe.

## The facts under the name

| you write | the card says |
|---|---|
| `.corp()` / `.runner()` | which side it belongs to |
| `.event()` `.operation()` `.asset()` `.upgrade()` `.hardware()` `.program()` `.resource()` `.identity()` | the card type |
| `.ice(4)` | "ICE" — and its strength |
| `.agenda(3, 2)` | "3/2" — advancement requirement and points |
| `.subtypes(&["Barrier", "Tracer"])` | the subtypes, exactly as printed |
| `.cost(3)` | play, install or rez cost |
| `.strength(1)` | icebreaker strength |
| `.trash_cost(2)` `.memory(1)` `.link(1)` `.recurring_credits(2)` | the rest of the numbers |
| `.unique()` | the ◆ |
| `.console()` | consoles |

An agenda takes its two numbers in `.agenda(…)` and has no `.cost(…)` at all,
because agendas have no cost printed on them. Ice takes its strength in
`.ice(…)`. Everything else is a separate call.

## Saying what a card does

Every printed sentence is one call. Which call depends on **when** the
sentence happens:

```rust
# use jinteki_cards::edsl::*;
# fn f() -> CardBuilder { card("X").corp().asset().text("t")
.play([/* … */])                      // an event or operation
.paid(credits(1), [/* … */])          // "1[credit]: …"
.paid_interface(credits(1), Some("Sentry"), [/* … */])  // "Interface → 1[credit]: …"
.paid_access(credits(1), [/* … */])   // "Access → 1[credit]: …"
.when(turn_begins(Corp), [/* … */])   // "When your turn begins, …"
.may_when(turn_begins(Corp), [/* … */])  // "When your turn begins, you may …"
.subroutine([/* … */])                // one [subroutine] line — one call each
.declares([/* … */])                  // a permanent fact, not an action
.declares_at_threat(4, [/* … */])     // "Threat 4 → …"
.interrupt(run_ends(), [/* … */])     // "[interrupt] → …"
# }
```

Write one `.subroutine([…])` per printed subroutine, in printed order. Gold
Farmer prints the same subroutine twice, so it is written twice.

### When it happens

| you write | the card says |
|---|---|
| `turn_begins(Corp)` | "When your turn begins, …" |
| `installed()` | "When you install this card…" |
| `scored()` / `stolen()` | "When you score this agenda…" |
| `empty_of(CounterKind::Credit)` | "When it is empty, …" |
| `encountered()` | "When the Runner encounters this ice…" |
| `passed()` | "When the Runner passes this ice…" |
| `accessed()` | "When the Runner accesses this card…" |
| `run_ends()` | "When this run ends…" |
| `after_this_resolves()` | "After you resolve this operation, …" |

### What it costs

Everything printed before the colon is the cost.

```rust
# use jinteki_cards::edsl::*;
let _ = free();                                  // no cost
let _ = credits(2);                              // "2[credit]:"
let _ = clicks(1);                               // "[click]:"
let _ = clicks(3).plus_cost(trash_this_card());  // "[click][click][click], [trash]:"
let _ = hosted_counters(CounterKind::Power, 1);  // "Hosted power counter:"
let _ = forfeit_agenda(1);                       // "forfeit an agenda"
```

## The sentences themselves

Grouped by what the card is talking about. Every one of these is a call that
returns one instruction; `.play([a, b])` takes as many as the sentence needs.

**Credits and cards.** `gain(Runner, 9)`, `lose(Corp, 3)`, `draw(Corp, 2)`,
`add_to_hand(…)`, `add_to_deck(…)`, `search_stack(…)`.

**Damage.** `net_damage(Corp, 3)`, `meat_damage(Corp, 7)`,
`core_damage(Corp, 1)` — the side is who is *responsible*, which the rules
care about. `prevent_all_meat_damage()`, `prevent_all_net_damage()`.

**Tags.** `give_tags(2)`, `remove_tags(1)`.

**Runs.** `end_the_run()`, `run(ServerId::Hq)`,
`run_then_if_successful(ServerId::Hq, […])`, `bypass_encountered_ice()`,
`force_encounter(…)`.

**This card.** `trash_self()`, `remove_self_from_game()`, `trash(…)`,
`purge_virus_counters()`, `end_action_phase(Corp)`, `host(…, …)`.

**Counters.** `load(CounterKind::Credit, 8)` for "load N onto it" — loading is
what a "when it is empty" ability is linked to. `place(CounterKind::Power, 1)`
for "place N counters on this card", `place_on(target, …)` for somewhere else,
`advance(target)` for "advance a card".

**Strength and subroutines.** `pump(1)` for "+1 strength",
`break_subroutines(1)`, `break_up_to(2)`, `break_all_subroutines()`.

**Installing and playing.** `install(…)`, `install_cards_from_hand(…)`,
`play_cards_from_hand(1, Corp)`, `rez(…)`,
`resolve_when_scored_ability_of(…)`.

**Instead of what would have happened.** `instead_of_breaching(true, […])` is
Account Siphon's "instead of breaching HQ, you may …" — the `true` is the
printed "you may". Pair it with `run_then_if_successful(…)`, and read the
credits it actually took with `per_credit_lost_by(Corp)`, which is what makes
"lose up to 5" and "for each credit lost" agree.

### Sentences built out of other sentences

The card writes some things as one sentence that are really two. These take
the smaller sentences as arguments:

```rust
# use jinteki_cards::edsl::*;
// "Gain 4[credit] and draw 3 cards."
let _ = combined([gain(Corp, 4), draw(Corp, 3)]);

// "You may trash this card to gain 3[credit] and draw 3 cards."
let _ = may_pay(trash_this_card(), combined([gain(Corp, 3), draw(Corp, 3)]));

// "End the run unless the Runner pays 3[credit]."
let _ = unless_pays(Runner, credits(3), end_the_run());

// "You may …"
let _ = may(draw(Runner, 2));

// "Trace[4]. If successful, give the Runner 4 tags."
let _ = trace(4, [give_tags(4)]);

// "Resolve 1 of the following."
let _ = choose_one([
    ("take 1 tag", vec![give_tags(1)]),
    ("end the run", vec![end_the_run()]),
]);
```

### What a sentence acts on

"1 installed program", "the card you are accessing", "an agenda in your score
area" — a description, which the ability's controller picks from when it
resolves.

```rust
# use jinteki_cards::edsl::*;
let _ = choose(2, &[installed_runner_card()]);        // "2 installed Runner cards"
let _ = choose(1, &[in_archives()]);                  // "1 card from Archives"
let _ = choose(1, &[in_score_area_of(Corp)]);         // "an agenda in your score area"
let _ = choose(1, &[installed_corp_card(), rezzed()]);// "a rezzed card you control"
let _ = this_card();
let _ = accessed_card();
let _ = encountered_ice();
```

The descriptions stack: several of them together mean *all* of them, exactly
as the printed words do.

### Amounts that count things

"…for each tag the Runner has" is an amount, not a number:

```rust
# use jinteki_cards::edsl::*;
let _ = per_runner_tag();
let _ = per_hosted_counter(CounterKind::Advancement);
let _ = plus(amount(1), times(1, per_hosted_counter(CounterKind::Advancement)));
```

Resistor's whole strength sentence — "Resistor has +1 strength for each tag
the Runner has" — is `strength_is(plus(amount(0), times(1, per_runner_tag())))`:
its printed 0, plus 1 for each tag. Written that way it is recomputed as tags
come and go, which is what the card means.

## The things a `.declares(…)` says

Some sentences are permanently true rather than things that happen. They go in
`.declares([…])`, and they never resolve — the engine reads them continuously.

```rust
# use jinteki_cards::edsl::*;
let _ = plus_memory(1);                             // "+1[mu]"
let _ = plus_link(1);                               // "+1 link"
let _ = strength_mod(-2);                           // "This program gets −2 strength."
let _ = strength_is(per_runner_tag());              // "…+1 strength for each tag"
let _ = can_host(&[of_type(CardType::Agenda)], Some(1));
let _ = runs_not_declared_successful();
let _ = removed_from_game_instead_of_trashed();
let _ = not_trashed_until_an_agenda_is_stolen();
let _ = additional_cost_to_steal_any_agenda(credits(3));
let _ = can_be_advanced();
```

An additional cost printed on the card itself is not a declaration — it is a
fact about the card, so it goes with the other facts:

```rust
# use jinteki_cards::edsl::*;
# fn f() -> CardBuilder { card("X").corp().agenda(5, 3).text("t")
.additional_steal_cost(credits(5))   // "…to steal THIS agenda, the Runner must pay 5[credit]"
.additional_play_cost(clicks(1))     // "As an additional cost to play this operation, spend [click]"
# }
```

Note the difference between "steal **an** agenda" (every agenda, for as long as
this card is around — a declaration) and "steal **this** agenda" (printed on
the agenda — a fact). Write whichever the card writes.

## When a sentence has no words yet

The vocabulary is still growing. If a sentence of the card cannot be written
yet, say so, in the card's own words:

```rust
# use jinteki_cards::edsl::*;
# fn f() -> CardBuilder { card("X").corp().asset().text("t")
.unimplemented("If you made a successful run this turn, you may install that program.")
# }
```

The card still exists, still carries its text, and still works for everything
it *can* do — but it is counted honestly as partial, it will not be playable
in a strict game (DESIGN.md SYS-D-12), and it shows up on the gap list. Never
approximate a sentence you cannot express: write the marker instead, and add
the missing engine capability to the gap list in `docs/vm/WAVES.md` so someone
can build it.

Two rules of thumb for the marker:

- If the sentence would *do the wrong thing* rather than nothing — a
  restriction the engine cannot yet honour, say — the marker is still the
  right answer, **even when the words exist**. A card that quietly misbehaves
  is worse than one that says it is incomplete. Say why in the doc comment.
- Write the marker for one printed sentence at a time, so the gap list counts
  sentences and not paragraphs.

## Reading a compiler error as a card error

The compiler talks about types; you are thinking about cards. The translation
is short.

**"cannot find function `gain_clicks` in this scope"** — there is no such
sentence in the vocabulary. Either it is spelled differently (try typing the
first few letters and letting autocomplete finish it), or the engine genuinely
cannot say it yet, and the card wants `.unimplemented(…)`.

**"expected `Side`, found integer"** — a sentence that needs to know *who*.
`gain(9)` is not enough; the rules always name who gains, so write
`gain(Runner, 9)`.

**"expected `Cost`, found `Instruction`"** — something is on the wrong side of
the colon. Everything printed *before* the colon on the card is the cost and
goes in the first argument; everything after is what happens.

**"expected `TargetSpec`, found `TargetFilter`"** — a description without a
number. "1 installed program" is `choose(1, &[installed_runner_card()])`; the
filter on its own is only half of it.

**"mismatched types: expected `[Instruction; 1]`"** — an ability takes a
*list* of sentences, even when there is one: `.play([gain(Corp, 9)])`.

**A panic when the tests run, not a compiler error** — `.build()` checks the
three things the type system cannot: that you said which side, said the type,
and copied the printed text. It names the card and what is missing.

## Seeing where the decks stand

```text
cargo test -p jinteki-cards
```

The deck tests print the manifest:

```text
priority decks: 51 cards, 14 complete, 37 partial, 57 printed sentences still unsayable
complete: ["Sure Gamble", "Diesel", "Account Siphon", "Desperado", …]
```

That count is ratcheted: a change that makes it worse fails, so the gap list
cannot quietly grow. If a card *has* to become partial, say why in
`docs/vm/WAVES.md` and move the ratchet deliberately.

The examples in this guide are mirrored as doctests in `src/edsl.rs`, so a
call named here always exists. (They would be doctested from this file
directly, but the nix build closure keeps `docs/cards` out — one line in
`nix/package.nix` would fix that, the way it was already widened for
`docs/rules`.)

`cargo test -p jinteki-cards --test behaviour` is the other half: every card
the manifest calls complete is dealt onto a board and played through the rules
engine, and the printed sentence's effect is asserted. Compiling is not proof.
When you finish a card, add its test there.

## Rules of thumb

1. **Copy the text first, always.** Behaviour is checked against it.
2. **One call per printed sentence.** If your call is doing two things, it is
   probably two sentences on the card, and the rules agree (§9.11.3) — unless
   the card joined them with "and", which is `combined([…])`.
3. **Restrictions are not sentences you write.** "Limit 1 per deck" or "Limit
   1 region per server" belong in the facts or nowhere, never as an action.
4. **If you're unsure whether it's a cost or an effect**, look for the colon
   on the printed card: everything before it is the cost.
5. **Write the marker rather than a lie.** A partial card is fine. A card that
   quietly does the wrong thing is not.
