# The Ability Model — engine-kernel digest of CR v26.03

> Digest of the Null Signal Games Comprehensive Rules v26.03 (source:
> https://rules.nullsignal.games/) for implementers of the jinteki-rs engine
> kernel. Every claim cites a rule id from `CR-v26.03.md` / `cr-index.json`;
> the cited rule text is normative, this digest is not. When this document and
> the CR disagree, the CR wins and this file has a bug.
>
> **MECHANISM** marks things the engine must implement as state-machine
> behavior. **CONVENTION** marks player-facing table procedure that the engine
> replaces with a digital equivalent (e.g. RNG instead of dice).

---

## 1. The object model in one paragraph

An **ability** is an independent unit of text on a card or counter, a basic
action, or the basic trash ability (`rule_ability`). Every ability is exactly
one of: static (`sec_static_abilities`), paid (`sec_paid_abilities`),
conditional (`sec_conditional_abilities`), play (`sec_play_abilities`), or
subroutine (`sec_subroutines`) (`rule_ability_categories`). Non-static
abilities are lists of **instructions** (`rule_instruction_link`); resolving
an ability = resolving its instructions in order with a checkpoint between
consecutive instructions (`rule_resolve_ability`). An ability's **source** is
the card/counter/game rule that originated it (`rule_source`); paid abilities,
conditional abilities, and subroutines become *independent of their source* at
a defined point (cost paid / instructions imminent / first instruction
imminent — `rule_paid_ability_independent`, `rule_conditional_ability_independent`,
`rule_subroutine_independent`, umbrella `rule_abilities_resolution_independent`),
after which a source zone-change strands any self-referencing effects. Text is
classified as conditions, restrictions, instructions, declarations, and
ability flags (`rule_text_classification`); an **effect** is what the text
does to the game state (`rule_effect`), and effects that outlive their
ability's resolution become **lingering effects** owned by the engine, not the
card (`rule_effect_beyond_resolution`).

Activeness (MECHANISM): abilities work only while **active**
(`rule_ability_active`) — by default iff the source card is active — with a
closed list of exceptions in `rule_ability_active_inactive_source_card`
(a–i): access abilities, zone-scoped abilities, play/install/rez permission
and cost modifiers, advancement-requirement and steal-cost modifiers,
can-be-advanced grants, met-trigger-hangover (`rule_active_exception_conditional_move_to_inactive_zone`),
subroutines of uninstalled encountered ice, and persistent abilities.

---

## 2. Timing and priority (§9.1–9.2)

Who may act, and when, is entirely driven by **priority windows**.

- **Active player** = player whose turn it is (`rule_active_player`).
- **Timing structures** (MECHANISM — these are the state machines in
  `timing-structures.json`): the two turns and their 3 phases each
  (`rule_turn_timing_structure`), a run and its 6 phases
  (`rule_run_timing_structure`), breaching a server
  (`rule_breaching_timing_structure`), accessing a card
  (`rule_accessing_timing_structure`). Explicitly *not* timing structures:
  installing a card, resolving a trace attempt, and other step sequences
  (`rule_not_timing_structures`) — this distinction controls where
  interrupt windows and checkpoints appear (see §6 below,
  `rule_step_in_timing_structure_is_instruction` vs `rule_step_sequences`).
- **Priority** — the opportunity to act; at most one player holds it at a
  time (`rule_priority`). All windows let the holder trigger a relevant
  ability they control (`rule_triggering`); outside action windows the
  holder may **pass** (`rule_pass`); a player keeps receiving priority after
  each resolved option until they pass (`rule_keep_priority_until_pass`).
  Windows nest; the innermost window always resolves first
  (`rule_nested_priority_window`) — this is how chain reactions
  (`rule_chain_reaction`) are implemented. **Whenever a player receives
  priority, a checkpoint occurs first** (`rule_checkpoint_before_receiving_priority`,
  restated as `rule_checkpoint_before_priority`). This is the engine's
  heartbeat: checkpoint → priority → act → checkpoint → …

Five window types (`rule_priority_window_types`), each a distinct MECHANISM:

| window | who gets priority | options | closes when |
|---|---|---|---|
| **action** (`rule_action_window`) | active player only | must take an action; no pass (`rule_action_window_options`) | after one action; no re-priority (`rule_action_window_closes_after_action`). Occurs only at `step_corp_turn_action` / `step_runner_turn_action` |
| **paid ability** (`rule_paid_ability_window`) | both, active first, alternating (`rule_ability_window_priority`) | trigger paid abilities (P); some windows also rez non-ice (R), score (S), or rez approached ice (`rule_paid_ability_window_corp_rez`, `_corp_score`, `_corp_rez_ice`); any number, any order, fully resolve each before the next (`rule_paid_ability_window_multiple_options`) | a player passes on priority received after the opponent passed |
| **reaction** (`rule_reaction_window`) | both, active first (`rule_reaction_window_priority`) | trigger *pending* conditional abilities associated with this window (`rule_reaction_window_options`), any order (`rule_reaction_window_pending_abilities_unordered`) | active passes → inactive passes → closes. May not pass holding pending *mandatory* abilities (`rule_reaction_window_must_resolve_mandatory_abilities`); if the window's timing structure ends mid-window the window closes at once and all remaining pending instances die, even mandatory ones (`rule_reaction_window_closing_timing_structure`) |
| **interrupt** (`rule_interrupt_window`) | both, active first, alternating (`rule_interrupt_window_priority`) | trigger interrupts relevant to the single imminent instruction (`rule_interrupt_window_options`) | as paid-ability window; pending optionals die on close (`rule_interrupt_window_must_resolve_mandatory_abilities`) |
| **mid-access** (`rule_mid_access_window`) | Runner only (`rule_mid_access_window_priority`) | one mid-access ability or the basic trash ability, or pass (`rule_mid_access_window_options`); exactly one use, no re-priority (`rule_mid_access_window_one_ability`). Occurs only at `step_mid_access_ability` (7.2.2) |

Reaction windows are bound to a *fixed set* of instances determined when the
window opens; later-pending abilities get their own new window
(`rule_reaction_window_linked_to_pending_conditional_abilities`,
`rule_after_checkpoint_reaction_window`). Same fixed-set principle for
interrupt windows (`rule_interrupt_window_linked_to_imminent_instruction`,
`rule_pending_status_for_interrupt_windows`).

---

## 3. The five ability types

### 3.1 Static abilities (§9.4) — MECHANISM: continuous query modifiers

Apply continuously while active; never resolve, no priority windows
(`rule_static_ability`). May carry static conditions gating parts of the
ability (`rule_static_ability_with_condition`) and restrictions that are
active while the source is inactive (`rule_static_ability_with_restriction`).
They have **no durations** and cannot directly create lingering effects
(`rule_static_no_lingering_effects`) — in engine terms: static effects are
recomputed from the current board, never cached with an expiry. Value
modifications keep the original value's restrictions/specifications
(`rule_static_modification_keep_restrictions`).

### 3.2 Paid abilities (§9.5) — MECHANISM: player-initiated, cost-gated

Syntax: **trigger cost, colon, effect** (`rule_paid_ability`,
`rule_paid_abilities_link`). Always optional; used once the trigger cost is
paid (`rule_paid_ability_optional`). Subtypes by flag/cost
(`rule_trigger_paid_abilities`): cost begins with [click] → **action** (one
per action window, `rule_action`); [interrupt] flag → interrupt
(`rule_interrupt`); "access" flag → mid-access, max 1 per access
(`rule_mid_access_ability`); "interface" flag → icebreaker-strength-gated
(`rule_interface_ability`); otherwise usable in paid ability windows,
unlimited times if paid (`rule_other_paid_abilities`). Effect-based implicit
timing restrictions (`rule_paid_ability_effect_based_timing_restrictions`):
break-subroutine abilities only during an encounter
(`rule_paid_ability_breaks_subroutines`); approached-ice references only
during Approach Ice with matching ice
(`rule_paid_ability_refers_to_approached_ice`); encountered-ice references
only during a matching encounter
(`rule_paid_ability_refers_to_encountered_ice`).

If the trigger cost uninstalls/forfeits the source and the effect reads
hosted objects: set hosted cards/counters aside as the cost is paid; they
still count as "hosted" for this ability, are invisible to other abilities,
and are trashed at `step_checkpoint_hosted_on_agenda`/`_hosted_on_installed_cards`
of the next checkpoint if still set aside
(`rule_trash_ability_keeps_track_of_hosted_objects`). Street Peddler /
Fermenter depend on this.

**Steps of Using a Paid Ability** (`sec_steps_of_using_a_paid_ability`,
9.5.7 a–h): announce → pay trigger cost (used; "when used" conditions met;
cost-paid checkpoint) → targets for first instruction, instruction becomes
imminent → interrupt window → resolve instruction → checkpoint → loop to the
interrupt window for each further instruction → complete.

### 3.3 Conditional abilities (§9.6) — MECHANISM: the pending-instance queue

A conditional ability = primary condition (trigger or static) + instructions
(`rule_conditional_ability`, `rule_primary_condition`). When its condition is
met, one or more **instances** — separate, independently-resolving copies —
become **pending** in the next reaction window (`rule_pending_instances`).
Static-condition abilities: max one instance at a time
(`rule_static_condition_one_instance`); trigger-condition abilities can stack
instances, including while one is pending/imminent/resolving
(`rule_trigger_condition_multiple_instances`,
`rule_condition_met_with_pending_instances`), one instance per occurrence
between consecutive checkpoints (`rule_condition_met_multiple_times`).

The condition-detection contract (MECHANISM, the heart of the engine):

- Trigger conditions look for an **instantaneous change** in game state; the
  next checkpoint marks one pending instance per occurrence
  (`rule_trigger_condition_checked`).
- Detection happens in checkpoint step `step_checkpoint_conditional_abilities`
  (10.3.1a): "Each active conditional ability looks at the changes to the
  game state **since the beginning of the last checkpoint**" — so the engine
  must retain (or be able to diff against) the game state as of the previous
  checkpoint's step (a). "Had"-style references read exactly that snapshot
  (`rule_instruction_requirements_past_state`).
- An ability only sees the change if it was active when the checkpoint
  processed it; becoming active later — even during the associated reaction
  window — is too late (`rule_condition_only_met_while_active`).
- All stipulations inside the trigger condition must hold at the moment the
  condition would occur (`rule_condition_requirements_part_of_condition`);
  stipulations inside the instructions are checked at resolution time instead
  (`rule_condition_requirements_part_of_effect`).

Static-condition conditionals repeat while true
(`rule_conditional_ability_with_static_condition`) with anti-livelock rules:
condition must be true at the start of the checkpoint
(`rule_conditional_ability_check_start_of_checkpoint`); no new instance while
one is pending/imminent/resolving (`rule_conditional_ability_static_one_instance`);
if a resolution had no expected effects at any instruction's interrupt-window
open, no re-pend until a timing-structure step completes
(`rule_conditional_ability_static_condition_no_effect`) (Parasite/Architect).

Optional vs mandatory (`rule_optional_conditional_ability`): "may"/"allows" or
once-per-turn-style restrictions → optional; controllers may pass with
optionals pending (`rule_pass_with_optional_conditional_abilities_pending`),
must fire all pending mandatories before passing
(`rule_cannot_pass_with_mandatory_conditional_abilities_pending`); mandatory
abilities can still contain declinable optional parts
(`rule_mandatory_conditional_ability_with_optional_effects`); "used" = any
optional component carried out (`rule_optional_conditional_ability_use`).
Pending instances die if the ability goes inactive
(`rule_conditional_ability_lose_pending_when_ability_becomes_inactive`) or
their window closes (`rule_conditional_ability_lose_pending_when_priority_window_closes`).

**Delayed conditional abilities** (`rule_delayed_conditional_ability`) are
conditionals maintained by lingering effects; duration is explicit if stated
(`rule_delayed_conditional_ability_specified_duration`), otherwise
one-shot-until-resolved (`rule_delayed_conditional_ability_relevant_once`);
"when this run ends" with no run in progress → never created
(`rule_delayed_run_ends_condition_outside_run`). Classes "when
encountered"/"when installed"/"when scored" are defined positionally against
steps `step_encounter_begins`, `rule_steps_installing_installed_condition`,
and the score option (`rule_references_to_trigger_conditions` a–c); an effect
that "resolves" such an ability marks it pending as if the class stipulation
occurred, other requirements still checked
(`rule_instructed_to_resolve_conditional_ability`).

**Steps of Triggering and Resolving a Conditional Ability**
(`sec_steps_of_triggering_and_resolving_a_conditional_ability`, 9.6.15 a–g):
announce trigger of a pending instance → targets for first instruction,
imminent → interrupt window → resolve (first optional effect resolved ⇒
"when used" conditions met) → checkpoint → loop → complete.

### 3.4 Play abilities (§9.7) — MECHANISM: event/operation bodies

Any ability on an event/operation that is not paid/conditional/static
(`rule_play_ability`); they resolve during step
`rule_steps_playing_resolve_play_abilities` (8.6.7f) of playing the card, in
printed order (`rule_abilities_no_inherent_order`). **Steps of Resolving a
Play Ability** (`sec_steps_of_resolving_a_play_ability`, 9.7.2 a–f): targets
for first instruction, imminent → interrupt window → resolve → checkpoint →
loop → complete.

### 3.5 Subroutines (§9.8) — MECHANISM: ordered list on ice + break status

Only ice has subroutines; each begins with [subroutine] (`rule_subroutine`).
Ordering is total and category-based (`rule_subroutines_ordered`,
`rule_subroutine_origins` a–e): (a) externally-granted "before" subs, newest
first → (b) self-static "before" subs → (c) printed subs in printed order →
(d) self-static "after"/unspecified subs → (e) externally-granted
"after"/unspecified subs, oldest first. Special cases: TL;DR copies sit
immediately after their originals (`rule_subroutines_duplicated`); Hive loses
printed subs last-first (`rule_subroutines_static_ability_remove_subroutines`);
"in the order of your choice" and same-category simultaneous additions are
Corp-declared (`rule_gain_subroutines_in_any_order`,
`rule_corp_decides_unspecified_order`).

Per-encounter status **broken/unbroken** (`rule_subroutines_status`): all
unbroken at encounter start (`rule_subroutines_initial_status_in_encounter`);
newly gained subs arrive unbroken (`rule_new_subroutines_during_encounter`);
status remains queryable after the encounter (`rule_subroutine_status_lifetime`).
**Break** = unbroken→broken for the active encounter (`rule_break_subroutine`);
only unbroken subs are targetable by break effects
(`rule_unbroken_subroutines_target_for_break_abilities`); "break all
subroutines" targets nothing (`rule_break_all_subroutines_no_targets`).
Resolution at `step_resolve_subroutine`: mandatory, no priority window
(`rule_resolve_subroutines_mandatory`), one at a time in order
(`rule_resolve_subroutines_in_order`), encounter end stops the rest
(`rule_resolve_subroutines_run_ends`). **Steps of Resolving a Subroutine**
(`sec_steps_of_resolving_a_subroutine`, 9.8.10 a–h): subroutine itself
becomes imminent → interrupt window (may prevent the whole subroutine) →
targets for first instruction, imminent → interrupt window → resolve →
checkpoint → loop → complete. Note the *double* interrupt structure: one
window for the subroutine, then one per instruction.

---

## 4. Interrupts and replacement effects (§9.9) — MECHANISM

An **interrupt** modifies an imminent instruction's effects or context
(`rule_interrupt_definition`); flagged by [interrupt] or by
"prevent"/"avoid"/"would" (`rule_interrupt_symbol`, `rule_interrupt_keywords`).

**Expected effects** (`rule_expected_effects`) — the engine must maintain,
per imminent instruction, a continuously-updated model of what the
instruction will do: text, modified by static abilities, replacement effects,
and applied interrupts. On resolution, expected effects = what happens
(`rule_expected_effects_resolve`), except dead ≤0 values
(`rule_negative_values_resolution`).

**Relevance** (`sec_relevant_interrupts` a–d): an interrupt is triggerable
only if it could prevent/avoid part of the expected effects, could modify an
associated value, could create an applicable replacement effect, or has a
"would" trigger met by the expected effects. Relevance is re-evaluated as
expected effects change (see the Sacrificial Construct / Harbinger example
under `rule_trigger_conditional_ability_interrupt`).

**Interrupt-window lifecycle** (`sec_interrupt_window_for_imminent_instruction`):
when an instruction becomes imminent — (1) compute initial expected effects,
(2) apply active replacement effects (ordering per
`rule_order_of_replacement_effects`), (3) mark relevant *conditional* interrupts
pending (late activations never pend, `rule_pending_status_for_interrupt_windows`),
(4) give priority. Conditional interrupts require pending + still-relevant
(`rule_trigger_conditional_ability_interrupt`); paid interrupts require only
current relevance, and can be ones installed mid-window
(`rule_trigger_paid_ability_interrupt`).

**Values** (`sec_modifiable_values`, `sec_calculating_expected_effect_values`):
tag counts, damage amounts, costs, base trace strength are modifiable values.
While imminent, values may go below 0 and stay modifiable
(`rule_negative_values_imminent`); tags/damage must be > 0 at resolution to
occur at all (`rule_modifiable_value_tags`, `rule_modifiable_value_damage`,
`rule_negative_values_resolution`); "prevent all X" deletes the effect and
its value entirely (`rule_prevent_all`); modified values keep the original's
restrictions ("cannot be prevented" survives +1, `rule_modified_values_retain_properties`).
Ordinal "would" triggers count imminences, not resolutions
(`rule_ordinal_would`); non-"would" abilities never see prevented effects
(`rule_ordinal_prevented`).

**Replacement effects** (`sec_replacement_effects`): marked by "instead";
apply automatically, no trigger needed. Sources: interrupts (apply
immediately on interrupt resolution if applicable,
`rule_replace_imminent_effects`), static abilities, lingering effects created
ahead of time. Application: at window-open for pre-existing ones
(`rule_replacement_effects_apply_as_interrupt_window_opens`); each replacement
effect applies at most once per effect
(`rule_replacement_effect_only_applies_once_per_effect`); ordering chosen by
the targeted card's controller, else the base effect's controller
(`rule_order_of_replacement_effects`); a later replacement needs something
left to replace (`rule_replacement_effect_must_have_something_to_replace`) —
the Security Testing / Account Siphon exclusivity comes from this.

---

## 5. Lingering effects (§9.10) — MECHANISM: engine-owned effect records

Created by instructions; exist independently of the (possibly inactive)
source; live until their stated duration expires; expired ones are removed at
checkpoint step `step_checkpoint_duration_abilities` (10.3.1b)
(`rule_lingering_effect`). Only lingering effects have durations; duration-
extension abilities (Gebrselassie) work by keeping the lingering effect alive
and never touch static-ability output (`rule_modify_duration_of_lingering_effect`).
A duration referencing a timing structure not in progress expires immediately
(next checkpoint) (`rule_lingering_effect_inapplicable_timing_structure`);
icebreaker strength boosts have implicit duration "remainder of the current
encounter" (`rule_icebreaker_strength_increase_implicit_link`). Lingering
effects also implement remembered choices
(`rule_lingering_effect_maintain_choice`): duration tied to the referencing
lingering effect (`rule_lingering_effect_maintaining_choice_default_duration`),
or turn-end for "when your turn begins, choose"
(`rule_lingering_effect_maintaining_choice_turn_begins_duration`), else
source-inactive (`rule_lingering_effect_maintaining_choice_duration_other_cases`).

---

## 6. Identifying instructions (§9.11) — the card-text DSL specification

This section is the CR's own grammar for segmenting card text into
instructions. The card DSL/compiler must implement it exactly.

- `rule_instruction_pause` — an instruction cannot be paused once it begins
  resolving, except that a checkpoint can open mid-resolution only when:
  (a) a cost is paid (`rule_cost_paid_checkpoint`); (b) a timing structure is
  opened, whose internal checkpoints run (`rule_checkpoint_timing_structure`);
  (c) drawing cards, at `step_draw_checkpoint` (8.4.5b) (`rule_draw_checkpoint`);
  (d) playing an event/operation, at `rule_steps_playing_played_checkpoint`
  (8.6.7e) (`rule_play_checkpoint`); (e) initiating a trace, at
  `step_trace_checkpoint` (10.8.6b) (`rule_trace_checkpoint`).
- `rule_step_in_timing_structure_is_instruction` — **each step in a timing
  structure is a single instruction**, hence preceded by an interrupt window
  and followed by a checkpoint. Steps of non-timing-structure procedures
  (installing, checkpoints themselves) are *not* instruction boundaries;
  checkpoints there only where explicitly called for (`rule_step_sequences`).
- `rule_instructions_in_ability_text` — default rule: **each sentence is one
  instruction**. Between instructions: checkpoint → (reaction window if
  needed) → announce targets for next instruction → next instruction becomes
  imminent → interrupt window.
- `rule_instruction_sentence_exceptions` — the exceptions, verbatim in
  substance:
  - (a) `rule_use_restrictions` — a sentence that only clarifies, restricts,
    or conditions the ability (gives no directions to carry out) is not part
    of any instruction.
  - (b) `rule_split_up_instruction` — a sentence directing a player to play,
    install, or access more than one card splits: each play/install/access
    after the first starts a new instruction. ("Install up to 3 cards" ≡
    "You may install a card. You may install a card. You may install a card.")
  - (c) `rule_choose_instruction` — a sentence that only chooses targets and
    does not act on them fuses with the following sentence into one
    instruction (Tinkering).
  - (d) `rule_search_instruction` — older search wording: ending the search
    (and shuffling) ends an instruction; a checkpoint occurs with found cards
    set aside; the rest of the sentence is the next instruction.
  - (e) `rule_look_reveal_instruction` — older look/reveal wording: making
    the cards visible ends an instruction; checkpoint; remainder is the next
    instruction (Architect).
  - (f) `rule_nested_cost_instruction` — the choice whether to pay a nested
    cost ends an instruction; the paid-for (or default) branch is the next
    instruction (see `rule_nested_cost`).
  - (g) `rule_choice_instruction` — choosing between optioned effects ends an
    instruction; each option is its own instruction(s) (Data Raven: the
    choice is made, *then* the chosen effect becomes imminent and can be
    interrupted — `rule_mandatory_choice_effects_can_be_modified`).
- `rule_linked_abilities_during_timing_structure` — text written after an
  instruction that initiates a timing structure may be a *linked ability*
  applying during that structure, not further instructions of the same
  ability.

---

## 7. Other kernel semantics (§9.12)

- **Simultaneous value edits** (`rule_modify_value`): final value = default,
  then set-effects, then increases, then decreases. Subtype add/remove is
  count-based: present iff adds (incl. printed) > removes
  (`rule_modify_subtypes`). Conflicting once-only choices → active player
  chooses (`rule_modify_ability_with_choice`).
- **Dependency ordering** (MECHANISM — characteristic calculation):
  `rule_dependent_effects` / `rule_independent_effects`. Start from printed
  characteristics; repeatedly apply an *independent* effect (one not
  depending on any still-unapplied effect); skip effects whose static-ability
  source was removed/deactivated by an earlier application; on a dependency
  loop, effects from hosted objects ignore their dependence on their host's
  effects (Hush beats its host). This is a fixpoint/topological evaluation
  the engine needs for layered modifiers (Mother Goddess + Hush etc.).
- **Sets and aggregation** (`rule_act_on_multiple_cards`,
  `rule_calculated_quantity`, `rule_aggregated_instructions`): one effect on
  a set acts on all simultaneously; "for each/for every/plus" quantities
  aggregate into a single instance **only** for the closed list in
  `rule_aggregated_instructions` (credits gain/lose/spend; clicks; tags take/
  remove/prevent; bad publicity; look/reveal from a location; draw; trash
  from specified locations incl. damage; shuffle discard→deck); if any tied
  effect is off-list, nothing aggregates. Aggregated value ≤ 0 → that part
  does not occur. Vacuous "all of zero items" is instantly satisfied
  (`rule_vacuous_truth`); un-evaluatable X = 0 (`rule_values_defined_by_x`).
- **Must** (`subsec_must`): unqualified "must" forces enabling decisions,
  even via other cards (`rule_must_with_choice`); means-specified "must" only
  binds through that means (`rule_must_without_choice`); choose a fully
  resolvable option or nothing (`rule_mandatory_choice`); the choice is its
  own instruction and the chosen effect can still be interrupted
  (`rule_mandatory_choice_effects_can_be_modified`); "must" cannot force
  paying an additional cost, but declining one option's additional cost does
  not dodge other resolvable options (`rule_must_cannot_force_additional_cost`).
- **Repeat this process** (`subsec_repeat_this_process`): unbounded repeat
  includes the repeat instruction (`rule_repeat_this_process`); "N times" is
  fixed after the first full resolution (`rule_repeat_this_process_x_times`);
  checkpoint after each full resolution (`rule_checkpoint_after_repetition`);
  repetitions independent, choices re-made (`rule_repetition_resolve_independently`).
- **Persistent** (`subsec_persistent`): Runner trashes a rezzed accessed
  card → its persistent abilities persist via a lingering effect created
  simultaneously with the trash (`rule_persistent`, `rule_persistent_continuous`);
  persist until the reaction window after `step_run_complete` closes
  (`rule_persistent_expiration`); applicable only to that run — no new
  instances afterwards even mid-window (`rule_persistent_applicability`).
- **Modal abilities** (`subsec_modal_abilities`): bulleted lists = modes;
  the mode-selection instruction picks the next mode, any order, "up to"
  allows stopping (`rule_choose_next_mode`); return to selection after each
  mode (`rule_after_resolving_mode`); each mode at most once unless stated
  (`rule_cannot_repeat_modes`).
- **Infinite loops** (`sec_infinite_loops`) exist in the CR (10.1.4 area) —
  see the full CR; the engine's anti-livelock levers are
  `rule_conditional_ability_static_condition_no_effect` and mandatory-loop
  rules there.

---

## 8. Checkpoints (§10.3) — THE INNERMOST LOOP (MECHANISM)

`rule_checkpoints` (10.3.1): "A **checkpoint** is a process wherein objects
that have entered an illegal state are corrected, expired effects are
removed, and other important conditions are checked", performed automatically
at several timing points, with these steps **in order**:

> a. (`step_checkpoint_conditional_abilities`) Each active conditional
>    ability looks at the changes to the game state since the beginning of
>    the last checkpoint to see if its condition has been met. Any ability
>    that has met its condition creates the appropriate instances of itself
>    and marks them as pending, as described in section 9.6.
> b. (`step_checkpoint_duration_abilities`) Any ability with a duration that
>    has passed is removed from the game state.
> c. (`step_checkpoint_agenda_points`) If the agendas in either player's
>    score area total 7 or more agenda points, that player wins the game. If
>    both players would win this way simultaneously, the game ends in a draw.
> d. (`step_checkpoint_uniqueness`) If 2 or more unique (◆) cards with the
>    same name are active, for each such name, all of those cards except the
>    one that became active most recently are trashed. If 2 or more *console*
>    cards are installed under the control of the same player, for each such
>    player, all of those cards except the one that became active most
>    recently are trashed.
> e. (`step_checkpoint_card_restrictions`) If any objects break any
>    restrictions of card abilities or the game rules (such as the Runner's
>    memory limit) or are installed or hosted in an illegal location, an
>    appropriate set of those objects are trashed. [Minimal-set selection:
>    a set is appropriate if trashing it leaves all remaining installed or
>    hosted objects legal and no object can be removed from the set while
>    maintaining that property; single-owner sets → that player chooses,
>    mixed sets → active player chooses.]
> f. (`step_checkpoint_hosted_on_agenda`) Any objects that were hosted on an
>    agenda that moved from a score area to any zone other than a score area
>    are trashed.
> g. (`step_checkpoint_hosted_on_installed_cards`) Any objects that were
>    hosted on an installed card that was uninstalled are trashed, except
>    set-aside survivors per `rule_trash_ability_keeps_track_of_hosted_objects`.
>    This step is repeated until no more cards or counters are trashed.
> h. (`step_checkpoint_remote_server`) Any remote server with no cards
>    protecting it, in its root, or in the process of being installed with a
>    destination protecting it or in its root ceases to exist.
> i. (`step_checkpoint_vacant_position`) Any position protecting a server
>    that is not occupied by a piece of ice ceases to exist, except the
>    Runner's current position or a position with ice mid-install. See
>    `rule_destroy_position`.
> j. (`step_checkpoint_card_entering_root_during_breach`) If a server is
>    being breached and 1 or more cards entered its root since the previous
>    checkpoint, for each such card the Runner declares whether it becomes a
>    candidate. See `sec_determining_candidates`.
> k. (`step_checkpoint_discard_pile_cards`) Any cards in discard piles that
>    had been converted into counters or agendas return to their printed
>    characteristics.
> l. (`step_checkpoint_discard_pile_counters`) Any counters in a discard
>    pile are returned to the bank.

After a checkpoint that marked instances pending, a reaction window
immediately opens, even inside another reaction window
(`rule_after_checkpoint_reaction_window`). Checkpoint triggers: before every
priority receipt (`rule_checkpoint_before_priority`); immediately after every
cost payment, before continuing (`rule_checkpoint_after_paying_cost`);
after every instruction finishes, before the next becomes imminent
(`rule_checkpoint_after_instruction_resolution`). The checkpoint after a
timing structure's last step happens *outside* the structure, as does its
reaction window (`rule_checkpoint_after_timing_structure`) — this is why
Jesminder cannot stop AMAZE tags.

---

## 9. Costs (§1.16) — MECHANISM

- A cost is anything spent/resolved/met to use an ability or apply an effect;
  must be payable **all at once** with controlled cards/counters or it cannot
  be paid (`rule_cost`). Paying a cost cannot be modified/cancelled by
  optional interrupts (`rule_cost_no_interrupt`); if a static ability or a
  *mandatory* conditional interrupt would prevent the steps of payment, the
  cost is unpayable (`rule_cost_interrupt_static_mandatory`) (Guru Davinder /
  Jesminder gating). Payment must not break any restriction, before or after
  (`rule_cost_restrictions`). **Costs of 0 are real and explicitly paid**
  (`rule_cost_zero`) — engine: zero-costs still emit a pay event (Freedom
  Khumalo depends on it).
- Cost value pipeline (`rule_cost_calculation`): default → increases →
  decreases → clamp at 0. Quantity phrases in costs are computed at payment
  time and paid as one aggregate (`rule_cost_quantities`). X in costs is
  chosen by the payer under restrictions (`rule_cost_x`); X outside a payment
  context = 0 (`rule_cost_x_out_of_context`). Alternate-payment abilities add
  options, not value changes (`rule_alternate_payment`); "total" install+rez
  discounts are split by Corp declaration before install-cost calculation
  (`rule_install_and_rez_reducing_total`).
- **After any cost payment: checkpoint**, even for 0 (`rule_cost_checkpoint`,
  `rule_cost_checkpoint_cost_zero`, `rule_checkpoint_after_paying_cost`).
- Six main types (`rule_types_of_costs`): install (cards; ice install cost =
  number of ice already protecting the destination server,
  `rule_install_cost_ice`; assets/upgrades/agendas install free,
  `rule_no_install_cost`), play (`rule_play_cost`), rez (`rule_rez_cost`),
  paid-ability trigger (`rule_trigger_cost`), additional
  (`rule_additional_cost`), nested (`rule_nested_cost`). Inherent costs
  (install/rez/play) ride along with any effect that performs the action
  (`rule_inherent_cost`) and don't make it optional (`rule_inherent_cost_in_ability`);
  an *additional* cost on a forced effect may be declined, cancelling the
  effect (`rule_decline_additional_cost`, `rule_inherent_and_additional_cost`);
  all additional costs are paid simultaneously with the base cost — all or
  nothing (`rule_additonal_cost_simultaenous`).
- Ignoring costs (`rule_ignoring_costs`): named types removed
  (`rule_ignore_general_cost`); "credit costs" removes credit components
  (`rule_ignore_credit_cost`); "all costs" → total cost 0 incl. additional
  (`rule_ignore_all_costs`).
- **Nested costs** (`rule_nested_cost`): mid-resolution costs gating part of
  an effect; grammar "may [cost] to [effect]" / "may [cost]. If you do, …"
  (`rule_nested_cost_may`), "[effect] unless [cost]" / "If you do not, …"
  (`rule_nested_cost_unless`); "if you do" without "may" is not a nested cost
  (`rule_nested_cost_no_may`); "otherwise" attaches the complementary branch
  (`rule_nested_cost_otherwise`). The pay/decline choice ends an instruction
  (`rule_nested_cost_instruction`).

---

## 10. Runs (§6) — structural summary

State per run (MECHANISM): the **attacked server** (`rule_attacked_server`,
announced at initiation, changeable mid-run), the Runner's **position**
(`sec_position`: outermost ice at start `rule_position_initial`, moves inward
`rule_position_progression`; positions are per-server slots that can be
created/destroyed as ice comes and goes, `rule_create_position`,
`rule_destroy_position`, with a full case table for mid-approach/mid-encounter
ice changes in `rule_ice_change_current_position` a–e), the **bad publicity
fund** (`rule_bad_publicity_fund` — Runner-spendable credits during runs,
filled at `step_initiation_bad_publicity` with 1[credit] per Corp bad
publicity `rule_bad_publicity_beginning_run`, emptied at
`step_run_ends_bad_publicity`; mid-run BP changes don't retro-adjust,
`rule_bad_publicity_during_run`), and per-encounter subroutine status (§3.5
above).

The six phases (state machine in `timing-structures.json`, structure `run`,
prose `sec_steps_of_a_run` 6.9.1–6.9.6): Initiation → Approach Ice →
Encounter Ice → Movement → Success → Run Ends. Key semantics:

- Initiation: "cannot make a run" forbids initiation only
  (`rule_cannot_run_abilities`); additional run costs paid at initiation
  (`rule_additional_cost_to_run`).
- Approach: on `step_approach_paw` the Corp may rez the approached ice (and
  non-ice, and use paid abilities) (`rule_paid_ability_window_corp_rez_ice`);
  approached ice rezzed? → Encounter, else → Movement (`step_approach_complete`).
- Encounter: interface abilities and breaking happen in `step_encounter_paw`
  (`rule_encounter_break_paw`); unbroken subs resolve one at a time
  (`step_resolve_subroutine`, `rule_resolve_subroutines_in_order`). Fully
  breaking (`subsec_fully_break`), bypass (`subsec_bypass`), and forced
  encounters outside runs (`subsec_forced_encounters`, encounters can exist
  with no run: `rule_end_encounter_outside_run`).
- Movement: pass ice (`rule_pass_ice`, `step_pass_ice`) — jack-out decision
  (`step_jack_out_choice`, not before passing outermost-approach:
  `rule_jack_out_after_passing_ice`, `rule_jack_out_before_approach`) — move
  inward if possible — new position? → Approach Ice, else approach server
  (`step_approach_server`) → Success.
- Success: run declared successful (`step_run_declared_successful`,
  `rule_successful_run`); "if successful" abilities are conditionals bound to
  the attacked server (`rule_if_successful`, `rule_if_successful_tied_to_server`,
  nested breach-referencing effects apply at `step_breach` with run-end
  duration `rule_if_successful_lingering_effect`); then **breach** the
  attacked server (`step_breach`, `rule_breach_link`).
- Run Ends: close/resolve leftover priority windows
  (`step_open_priority_windows_closed`, `rule_run_ends_process_priority_windows`),
  empty BP fund, declare unsuccessful iff Success was never reached
  (`rule_unsuccessful_run`, `rule_not_unsuccessful_when_reached_success_phase`),
  complete (`step_run_complete`). "End the run" effects jump here
  (`rule_end_the_run`); ending accesses/breaches in progress is covered by
  `rule_end_run_access_or_breach_during_run`.
- Effects can restructure runs: direct approach/encounter, phase jumps, and
  "after this run's Nth phase" modifiers (`rule_modify_run_steps` a–f).

## 11. Breaching and accessing (§7) — structural summary

**Accessing** a card = the Runner viewing it with the potential to steal or
trash (`sec_accessing_cards` 7.1; the basic trash ability lives at
`rule_basic_trash_ability` 7.1.5, with forced-trash interactions
`rule_access_installed_must_trash_if_able`, `rule_access_reveal_trash_if_able`).
Access timing structure (`sec_steps_accessing_card` 7.2, structure `access`):

1. `step_card_accessed` — the card becomes accessed; "when accessed"
   conditions met.
2. `step_mid_access_ability` — the mid-access ability window (§2 table; ≤1
   ability: basic trash or an access-flagged ability).
3. `step_access_agenda` — if it is an agenda, the Runner steals it.
4. `step_access_complete` — access ends; "after access" style conditions.

**Breaching** a server = accessing a sequence of cards from it
(`sec_breaching_servers` 7.3). Candidate determination
(`sec_determining_candidates` 7.4) defines which cards are accessible per
server type; cards entering the root mid-breach become candidates only by
Runner declaration at checkpoint step
`step_checkpoint_card_entering_root_during_breach`. Breach timing structure
(`sec_breaching_steps` 7.5, structure `breach`):

1. `step_breaching_begins` — breach formally begins.
2. `step_flip_archives` — Archives only: facedown cards turn faceup.
3. `step_determine_candidates_limit` — HQ/R&D only: fix the number of
   accesses from hand/deck.
4. `step_choose_candidate` — candidates remaining? Runner chooses one, else
   go to 7.
5. `step_access_candidate` — access it (runs the 7.2 structure).
6. `step_repeat_candidate_selection` — return to 4.
7. `step_breach_complete` — breach complete.

Breaches occur inside runs at `step_breach`, but also standalone (e.g. card
effects "breach HQ") — the structure is the same, which is why it is its own
timing structure (`rule_breaching_timing_structure`).

---

## 12. Subsystems (§10.4–10.14)

- **Damage** (`sec_damage`): three types (`rule_damage_types`); meat/net:
  responsible player trashes 1 random grip card per point
  (`rule_meat_net_damage`); core: same + permanent max hand size −1 tracked
  by core damage counters (`rule_core_damage`); "brain damage" = core
  (`rule_brain_damage`). Multi-point damage trashes simultaneously,
  selected randomly (`rule_multiple_damage_taken_simultaneously`), or
  sequentially-selected-simultaneously-trashed under Chronos-style effects
  (`rule_multiple_damage_selected_sequentially`). Responsibility attribution
  (Corp "does" vs Runner "suffers") decides whose effects amplify it
  (`rule_suffer_or_take_damage`). Damage > grip ⇒ flatline
  (`rule_flatline_damage_reference`). MECHANISM: random selection = engine
  RNG over grip.
- **Tags** (`sec_tags`): tag counters on the Runner (`rule_tag`); "tagged" =
  ≥1 (`rule_tagged`); basic actions: Corp [click]+2[credit] trash a resource
  while tagged (`rule_tagged_trash_resource`), Runner [click]+2[credit]
  remove a tag (`rule_tagged_remove_tag`).
- **Bad publicity** (`sec_bad_publicity`): counters on the Corp
  (`rule_bad_publicity`); fund mechanics per §10 above.
- **Link** (`sec_link`): link value = identity base link + [link] from
  installed cards (`rule_link_value`); used chiefly in traces
  (`rule_link_contests_traces`).
- **Traces** (`sec_traces`): "Trace [N]" (`rule_trace_attempt_and_base_trace_strength`);
  Corp openly spends credits → trace strength = base + spent
  (`rule_trace_strength`); then Runner spends → link strength = link value +
  spent (`rule_link_strength`); trace > link ⇒ successful, else unsuccessful
  (`rule_compare_trace_and_link_strength`); attached "if (un)successful"
  bodies are conditionals, bare instructions get the implicit
  "when determined" condition (`rule_trace_conditional_abilities`). Steps
  (`rule_steps_of_resolving_trace_attempt` 10.8.6 a–f): initiate ("when
  initiated" conditions) → checkpoint → Corp spends → Runner spends →
  determine → complete. NOT a timing structure (`rule_not_timing_structures`).
- **Load/empty** (`sec_load_and_empty`): load = place counters
  (`rule_load_and_empty`); "when empty" only fires for counter types
  previously *loaded* (`rule_empty_requires_loading`,
  `rule_meeting_empty_condition`); loaded counters are not otherwise special
  (`rule_loading_does_not_restrict_counters`). MECHANISM: track loaded-ness
  per card+counter-type.
- **Charge** (`sec_charge`): place 1 power counter on a card already having
  ≥1 (`rule_charge`); set-charges target only valid (≥1) cards
  (`rule_charge_targets`); named-card charge requires ≥1 at imminence
  (`rule_charge_requires_hosted_counter`).
- **Mark** (`sec_mark`): a designated server (`rule_mark`), unique
  (`rule_only_one_mark`); "identify your mark": if none, a **random central
  server** becomes the mark for the remainder of the turn
  (`rule_mark_identification`); already marked → no-op, immutable this turn
  (`rule_mark_already_identified`); implemented as a turn-end lingering
  effect (`rule_mark_designation_lingering_effect`); mark-conditions only see
  events from designation onward (`rule_mark_designated_condition_check`).
  `rule_mark_identification_method` (cards/dice) is CONVENTION — engine: RNG
  uniform over the 3 centrals.
- **Sabotage** (`sec_sabotage`): "sabotage N" = Corp trashes N cards
  collectively from HQ + top of R&D (`rule_sabotage`); Corp chooses HQ part,
  remainder from top of R&D, all trashed simultaneously, facedown
  (`rule_sabotage_resolution`, `rule_sabotage_facedown`); Corp can't look at
  the R&D trashes until decisions done (`rule_sabotage_when_corp_can_look_facedown`);
  shortage rules force HQ picks / trash everything
  (`rule_sabotage_hq_first`, `rule_sabotage_all_remaining_cards`).
- **Dividends** (`sec_dividends`): "Dividends N" = when scored, place N
  agenda counters per advancement counter past the requirement
  (`rule_dividends`), evaluated **as scoring began**, pre-move
  (`rule_dividends_timing`).
- **Bidding** (`sec_bidding`): secret simultaneous credit choices
  (`rule_bidding`); secrecy method is CONVENTION (`rule_bid_secret`) —
  engine: sealed simultaneous inputs. Cannot bid unspendable amounts; 0
  always legal (`rule_bid_possible`); reveal ⇒ immediate spend, no
  checkpoint/window between reveal and spend (`rule_bid_reveal_spend`,
  `rule_bid_spent_immediately`); bid payment is a nested cost
  (`rule_bid_is_cost`); multi-source payment choice ordered active-then-
  inactive after seeing opponent's bid (`rule_bid_payment_choices`); "secretly
  spend" = bidding (`rule_bid_secretly_spend`). **Psi games**
  (`sec_psi_games`): bids ∈ {0,1,2} (`rule_psi_bid_options`); one instruction,
  no checkpoints until both paid (`rule_psi_bid_reveal`); outcomes "bids
  match"/"bids differ" (`rule_psi_outcome`).

---

## 13. Mechanism vs convention — summary table

| Area | Engine MUST implement | Player-facing CONVENTION (replace digitally) |
|---|---|---|
| Checkpoints §10.3 | the 12-step loop, last-checkpoint snapshot/diff, pending-instance creation | — |
| Priority §9.2 | all 5 window types, alternation, pass rules, nesting | table etiquette of announcing passes |
| Instances §9.6 | pending queue, per-occurrence instances, activeness gating | — |
| Interrupts §9.9 | expected-effects model, relevance, value arithmetic incl. negatives, replacement ordering | — |
| Instruction DSL §9.11 | sentence segmentation + exceptions (a)–(g) at card-compile time | — |
| Costs §1.16 | all-at-once payability check, zero-cost pay events, cost-paid checkpoints, nested-cost branching | announcing 0-cost payment verbally |
| Runs §6 | position lifecycle, BP fund, phase machine incl. jumps | — |
| Breach/access §7 | candidate sets, access limits, per-candidate access structure | physical card handling |
| Damage §10.4 | simultaneous random grip trashes (RNG) | shuffling/fanning cards |
| Mark §10.11 | uniform-random central selection (RNG), turn-scoped lingering designation | dice / three-card draw (`rule_mark_identification_method`) |
| Bidding §10.14 | sealed simultaneous bids, spendability validation, reveal-then-pay atomicity | fists/written numbers (`rule_bid_secret`) |
| Symbols §1.3 | tokens [click] [credit] [link] [mu] [subroutine] [trash] [trash-cost] [recurring] [interrupt] | printed glyphs |
