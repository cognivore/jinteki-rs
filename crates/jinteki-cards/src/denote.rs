//! Denotation: the card AST becomes the CR VM's own data.
//!
//! SYS-D-6 — every DSL construct denotes into the engine vocabulary and the
//! DSL has no state access of its own. SYS-D-11 — the vocabulary it denotes
//! into is the CR's §9.11 instruction taxonomy, which is why writing a card
//! is transcription: one printed sentence, one instruction.
//!
//! Adding a verb here is the *only* way to widen what designers can write,
//! and a verb must denote into an existing kernel instruction — if the kernel
//! cannot express it, the card gets an `unimplemented:` marker instead
//! (ARCHITECTURE §12: no card-shaped kernel variants).

use crate::parse::{Block, CardAst, CardError, Line};
use jinteki_cr::ability::{
    AbilityClass, AbilityDef, AbilityFlag, Cost, StaticDecl, TimingRestriction, TriggerCond,
};
use jinteki_cr::effects::DamageKind;
use jinteki_cr::instr::{
    InstallDest, InstallFilter, Instruction, Quantity, SubroutineSpec, TargetFilter, TargetSpec,
    TrashDestination,
};
use jinteki_cr::object::{CardType, CounterKind, PrintedCard, Side, Zone};

/// One card, plus what could not be said about it.
pub struct DenotedCard {
    pub printed: PrintedCard,
    pub oracle_text: String,
    pub unimplemented: Vec<String>,
}

impl DenotedCard {
    /// A card is *complete* when every printed sentence is expressed. Only
    /// complete cards are playable in a strict game (SYS-D-12).
    pub fn is_complete(&self) -> bool {
        self.unimplemented.is_empty()
    }
}

pub fn denote(file: &str, ast: &CardAst) -> Result<DenotedCard, CardError> {
    let name: &'static str = leak(&ast.name);
    let side = side_of(ast);
    let card_type = match fact(ast, "type") {
        Some(t) => parse_type(&t).ok_or_else(|| {
            err(file, ast.line, &ast.name, format!("unknown card type `{t}`"),
                "one of: agenda, asset, ice, operation, upgrade, event, hardware, program, resource, identity")
        })?,
        None => {
            return Err(err(file, ast.line, &ast.name, "this card has no type",
                           "add e.g. `type: event` — the type is printed under the card's name"))
        }
    };

    let mut c = PrintedCard::vanilla(name, side, card_type);
    for (k, v) in &ast.facts {
        match k.as_str() {
            "side" | "type" | "faction" | "set" | "influence" => {}
            "subtypes" => c.subtypes = v.split(',').map(|s| leak(s.trim())).collect(),
            "cost" => c.cost = Some(num(file, ast, v)? as u32),
            "strength" => c.strength = Some(num(file, ast, v)? as i32),
            "trash cost" => c.trash_cost = Some(num(file, ast, v)? as u32),
            "memory" => c.memory_cost = Some(num(file, ast, v)? as u32),
            "advancement" => c.advancement_requirement = Some(num(file, ast, v)? as u32),
            "points" => c.agenda_points = Some(num(file, ast, v)? as i32),
            "unique" => c.unique = v.eq_ignore_ascii_case("yes"),
            "console" => c.console = v.eq_ignore_ascii_case("yes"),
            // CR 1.20: base link is printed on the card, and the kernel reads
            // link as the sum of active declarations (`runner_link`), so the
            // printed number is a declaration of the identity.
            "link" => {
                let n = num(file, ast, v)? as i32;
                c.abilities
                    .push(AbilityDef::static_ability(vec![StaticDecl::LinkBonus(n)]).labeled("base link"));
            }
            other => {
                return Err(err(
                    file,
                    ast.line,
                    &ast.name,
                    format!("unknown fact `{other}`"),
                    "facts are: side, type, subtypes, cost, strength, trash cost, memory, \
                     advancement, points, unique, console, link, faction, set, influence",
                ))
            }
        }
    }

    for b in &ast.blocks {
        denote_block(file, ast, b, &mut c)?;
    }

    Ok(DenotedCard {
        printed: c,
        oracle_text: ast.text.join("\n"),
        unimplemented: ast.unimplemented.clone(),
    })
}

/// One ability block. Most blocks add an [`AbilityDef`]; a `static:` block may
/// instead state a printed additional cost (1.16.10), which the kernel keeps
/// on the card rather than as a declaration — so the block is given the card.
fn denote_block(
    file: &str,
    ast: &CardAst,
    b: &Block,
    card: &mut PrintedCard,
) -> Result<(), CardError> {
    let label: &'static str = leak(&format!("{}: {}", ast.name.to_lowercase(), b.header));
    let h = b.header.as_str();

    // `static:` / `static threat 4:` — declarations, which never resolve.
    if h == "static" || h.starts_with("static ") {
        let mut decls = Vec::new();
        for l in &b.lines {
            if let Some(d) = denote_static(file, ast, l, card)? {
                decls.push(d);
            }
        }
        if decls.is_empty() {
            return Ok(());
        }
        let mut a = AbilityDef::static_ability(decls).labeled(label);
        if let Some(rest) = h.strip_prefix("static ") {
            let t = rest.trim();
            let n = t.strip_prefix("threat ").ok_or_else(|| {
                err(file, b.line, &ast.name, format!("unknown static qualifier `{t}`"),
                    "the only qualifier is `static threat N:` — the [threat N] flag")
            })?;
            let n: u8 = n.trim().parse().map_err(|_| {
                err(file, b.line, &ast.name, format!("`{n}` is not a number"), "e.g. `static threat 4:`")
            })?;
            a = a.with_flag(AbilityFlag::Threat(n));
        }
        card.abilities.push(a);
        return Ok(());
    }

    let instrs = b
        .lines
        .iter()
        .map(|l| denote_line(file, ast, l))
        .collect::<Result<Vec<_>, _>>()?;

    let a = match h {
        "play" => AbilityDef::play(instrs).labeled(label),
        "subroutine" => AbilityDef::subroutine(instrs).labeled(label),
        _ if h.starts_with("paid ") => {
            let (cost, flags) = parse_cost(file, ast, b, &h[5..])?;
            let mut a = AbilityDef::paid(cost, instrs).labeled(label);
            for f in flags {
                a = a.with_flag(f);
            }
            // CR 9.5.6a/c: an ability that refers to the ENCOUNTERED ice is
            // usable only during an encounter — and only with ice matching
            // any stipulation it used in referring to it. The break line IS
            // that reference, so the restriction is read off it rather than
            // written out again.
            if let Some(t) = encounter_timing(&b.lines) {
                a = a.with_timing(t);
            }
            a
        }
        _ if h.starts_with("when ") => {
            AbilityDef::conditional(parse_trigger(file, ast, b, &h[5..])?, instrs, false).labeled(label)
        }
        // CR 9.3.6d / 9.9.1: an `[interrupt] →` ability — the same conditional
        // with the flag that confines it to the interrupt window.
        _ if h.starts_with("interrupt ") => {
            AbilityDef::conditional(parse_trigger(file, ast, b, &h[10..])?, instrs, false)
                .with_flag(AbilityFlag::Interrupt)
                .labeled(label)
        }
        other => {
            return Err(err(
                file,
                b.line,
                &ast.name,
                format!("unknown ability block `{other}:`"),
                "blocks are: play, paid <cost>, when <trigger>, interrupt <trigger>, \
                 subroutine, static",
            ))
        }
    };
    card.abilities.push(a);
    Ok(())
}

/// The verbs. Each is one printed sentence and denotes into exactly one
/// kernel instruction.
fn denote_line(file: &str, ast: &CardAst, l: &Line) -> Result<Instruction, CardError> {
    let t = l.text.trim().to_lowercase();
    let w: Vec<&str> = t.split_whitespace().collect();
    let side = side_of(ast);

    let unknown = |what: &str| {
        err(
            file,
            l.line,
            &ast.name,
            format!("unknown sentence: \"{what}\""),
            "see docs/cards/DSL.md for the sentences you can write; if the card says \
             something the vocabulary cannot yet say, write it as \
             `unimplemented: \"<the printed sentence>\"` instead of approximating it",
        )
    };
    let n = |s: &str| int(file, l, ast, s);

    Ok(match w.as_slice() {
        // ---- credits ----------------------------------------------------
        ["gain", x, "credits"] | ["gain", x, "credit"] => {
            Instruction::GainCredits(side, Quantity::c(n(x)?))
        }
        ["lose", x, "credits"] | ["lose", x, "credit"] => {
            Instruction::LoseCredits(side, n(x)? as u32)
        }
        ["the", "runner", "loses", x, "credits"] | ["the", "runner", "loses", x, "credit"] => {
            Instruction::LoseCredits(Side::Runner, n(x)? as u32)
        }
        ["the", "corp", "loses", x, "credits"] | ["the", "corp", "loses", x, "credit"] => {
            Instruction::LoseCredits(Side::Corp, n(x)? as u32)
        }
        // ---- cards -------------------------------------------------------
        ["draw", x, "cards"] | ["draw", x, "card"] => Instruction::Draw(side, n(x)? as u32),
        // ---- damage ------------------------------------------------------
        ["do", x, "net", "damage"] => Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(n(x)?),
            responsible: side,
        },
        ["do", x, "meat", "damage"] => Instruction::Damage {
            kind: DamageKind::Meat,
            amount: Quantity::c(n(x)?),
            responsible: side,
        },
        ["do", x, "core", "damage"] | ["do", x, "brain", "damage"] => Instruction::Damage {
            kind: DamageKind::Core,
            amount: Quantity::c(n(x)?),
            responsible: side,
        },
        ["prevent", "all", "meat", "damage"] => {
            Instruction::PreventAllDamage { kind: DamageKind::Meat }
        }
        ["prevent", "all", "net", "damage"] => {
            Instruction::PreventAllDamage { kind: DamageKind::Net }
        }
        // ---- tags --------------------------------------------------------
        ["give", "the", "runner", x, "tags"] | ["give", "the", "runner", x, "tag"] => {
            Instruction::GainTags(n(x)? as u32)
        }
        ["take", x, "tags"] | ["take", x, "tag"] => Instruction::GainTags(n(x)? as u32),
        ["remove", x, "tags"] | ["remove", x, "tag"] => Instruction::RemoveTags(Quantity::c(n(x)?)),
        ["the", "runner", "removes", x, "tags"] | ["the", "runner", "removes", x, "tag"] => {
            Instruction::RemoveTags(Quantity::c(n(x)?))
        }
        // ---- the run -----------------------------------------------------
        ["end", "the", "run"] => Instruction::EndTheRun,
        // ---- self --------------------------------------------------------
        ["trash", "self"] => Instruction::TrashSelf,
        ["remove", "self", "from", "the", "game"] => Instruction::RemoveSelfFromGame,
        ["purge", "virus", "counters"] => Instruction::PurgeVirusCounters,
        // CR 5.6.2b: "your action phase ends" (Oppo Research / Terminal class).
        ["your", "action", "phase", "ends"] => Instruction::EndActionPhase(side),
        // ---- counters ----------------------------------------------------
        // CR 10.9.1: LOADING is a placement that also marks the kind loaded,
        // which is what an "empty" ability on the same card is linked to.
        ["load", x, "credits", "on", "self"] | ["load", x, "credit", "on", "self"] => {
            Instruction::LoadCounters {
                target: TargetSpec::SelfSource,
                kind: CounterKind::Credit,
                amount: Quantity::c(n(x)?),
            }
        }
        ["load", x, "power", "counters", "on", "self"]
        | ["load", x, "power", "counter", "on", "self"] => Instruction::LoadCounters {
            target: TargetSpec::SelfSource,
            kind: CounterKind::Power,
            amount: Quantity::c(n(x)?),
        },
        // CR 1.18.2: PLACING a counter is not advancing, and not loading.
        ["place", x, kind, "counters", "on", "self"] | ["place", x, kind, "counter", "on", "self"] => {
            Instruction::PlaceCounters {
                target: TargetSpec::SelfSource,
                kind: counter_kind(file, l, ast, kind)?,
                amount: Quantity::c(n(x)?),
            }
        }
        ["place", x, kind, "counters", "on", "this", "ice"]
        | ["place", x, kind, "counter", "on", "this", "ice"]
        | ["place", x, kind, "counters", "on", "this", "card"]
        | ["place", x, kind, "counter", "on", "this", "card"] => Instruction::PlaceCounters {
            target: TargetSpec::SelfSource,
            kind: counter_kind(file, l, ast, kind)?,
            amount: Quantity::c(n(x)?),
        },
        // ---- bad publicity -----------------------------------------------
        ["give", "the", "corp", x, "bad", "publicity"] => Instruction::TakeBadPublicity {
            side: Side::Corp,
            amount: Quantity::c(n(x)?),
        },
        // ---- moving cards ------------------------------------------------
        // CR 8.2: "Add N installed Runner cards to the grip."
        ["add", x, "installed", "runner", "cards", "to", "the", "grip"]
        | ["add", x, "installed", "runner", "card", "to", "the", "grip"] => {
            Instruction::AddCardsToHand {
                cards: TargetSpec::Choose {
                    count: Quantity::c(n(x)?),
                    criteria: vec![TargetFilter::InstalledRunnerCard],
                },
            }
        }
        ["add", x, "card", "from", "archives", "to", "hq"]
        | ["add", x, "cards", "from", "archives", "to", "hq"] => Instruction::AddCardsToHand {
            cards: TargetSpec::Choose {
                count: Quantity::c(n(x)?),
                criteria: vec![TargetFilter::InDiscardOf(Side::Corp)],
            },
        },
        // ---- strength ----------------------------------------------------
        // CR 3.9.5b: no duration stated on an icebreaker modifying its own
        // strength means "for the remainder of the current encounter".
        [amount, "strength"] if amount.starts_with('+') || amount.starts_with('-') => {
            Instruction::ModifyStrength {
                target: TargetSpec::SelfSource,
                amount: strength_delta(file, l, ast, amount)?,
                duration: None,
            }
        }
        // ---- subroutines -------------------------------------------------
        ["break", "all", "subroutines"] => {
            Instruction::BreakSubroutines { subs: SubroutineSpec::All }
        }
        ["break", "up", "to", x, "subroutines"] | ["break", "up", "to", x, "subroutine"] => {
            Instruction::BreakSubroutines {
                subs: SubroutineSpec::Chosen { count: Quantity::c(n(x)?), up_to: true },
            }
        }
        ["break", x, "subroutines"] | ["break", x, "subroutine"] => Instruction::BreakSubroutines {
            subs: SubroutineSpec::Chosen { count: Quantity::c(n(x)?), up_to: false },
        },
        // "break 1 sentry subroutine" — the subtype is the 9.5.6c stipulation
        // and becomes the ability's timing restriction, not part of the break.
        ["break", x, _sub @ .., "subroutine"] | ["break", x, _sub @ .., "subroutines"] => {
            Instruction::BreakSubroutines {
                subs: SubroutineSpec::Chosen { count: Quantity::c(n(x)?), up_to: false },
            }
        }
        // ---- installing and playing ---------------------------------------
        ["install", "up", "to", x, "cards", "from", "hq"] => Instruction::InstallCards {
            count: n(x)? as u32,
            from_hand_of: Side::Corp,
            filter: InstallFilter::Any,
            dest: InstallDest::DeclaredByInstaller,
            and_rez: false,
            and_rez_if_able: false,
            ignore_costs: false,
        },
        ["play", x, "operation", "from", "hq"] | ["play", x, "operations", "from", "hq"] => {
            Instruction::PlayCards {
                count: n(x)? as u32,
                from_hand_of: Side::Corp,
                ignore_costs: false,
            }
        }
        // ---- searching ----------------------------------------------------
        ["search", "your", "stack", "for", x, sub] => Instruction::Search {
            zone: Zone::Deck(Side::Runner),
            criteria: vec![TargetFilter::HasSubtype(leak(&title_case(sub)))],
            count: Quantity::c(n(x)?),
            may_fail: true,
        },
        _ => return denote_composite(file, ast, l, &t, unknown),
    })
}

/// The sentence forms that are built out of other sentences: the printed
/// wording puts a choice, a cost or a second effect inside one sentence, and
/// the CR gives each its own instruction shape (9.11.4f/g, 1.16.11, 9.12.2).
fn denote_composite(
    file: &str,
    ast: &CardAst,
    l: &Line,
    t: &str,
    unknown: impl Fn(&str) -> CardError,
) -> Result<Instruction, CardError> {
    let inner = |text: &str| Line { text: text.to_string(), items: Vec::new(), line: l.line };

    // "Trace[N]. If successful, <effect>." — one line, because the card
    // writes it as one.
    if let Some((head, tail)) = t.split_once(':') {
        if let Some(base) = head.trim().strip_prefix("trace ") {
            let effect = denote_line(file, ast, &inner(tail.trim()))?;
            return Ok(Instruction::Trace {
                base: Quantity::c(int(file, l, ast, base.trim())?),
                if_successful: vec![effect],
                if_unsuccessful: Vec::new(),
                determined_min: None,
            });
        }
    }

    // CR 9.11.4g: "Resolve 1 of the following." — a choice among options.
    if t == "choose one" && !l.items.is_empty() {
        let mut options = Vec::new();
        for item in &l.items {
            let label: &'static str = leak(item);
            options.push((label, vec![denote_line(file, ast, &inner(item))?]));
        }
        return Ok(Instruction::ChooseOne { options });
    }

    // CR 1.16.11b: "<effect> unless <player> pays <cost>" — paying suppresses
    // the effect; declining makes it the next instruction.
    if let Some((effect, cost)) = t.split_once(" unless the runner pays ") {
        return Ok(Instruction::NestedCostUnless {
            cost: credit_cost(file, l, ast, cost)?,
            effect: Box::new(denote_line(file, ast, &inner(effect))?),
            payer: Some(Side::Runner),
        });
    }
    if let Some((effect, cost)) = t.split_once(" unless the corp pays ") {
        return Ok(Instruction::NestedCostUnless {
            cost: credit_cost(file, l, ast, cost)?,
            effect: Box::new(denote_line(file, ast, &inner(effect))?),
            payer: Some(Side::Corp),
        });
    }

    // CR 1.16.11a / 9.11.4f: "you may <pay something> to <effect>", and
    // 9.6.9c's bare "you may <effect>".
    if let Some(rest) = t.strip_prefix("you may ") {
        for (phrase, cost) in [
            ("trash self to ", Cost::trash_self()),
            ("trash this card to ", Cost::trash_self()),
        ] {
            if let Some(effect) = rest.strip_prefix(phrase) {
                return Ok(Instruction::NestedCostThen {
                    cost,
                    effect: Box::new(denote_line(file, ast, &inner(effect))?),
                    payer: None,
                });
            }
        }
        if let Some((cost, effect)) = rest.strip_prefix("pay ").and_then(|r| r.split_once(" to ")) {
            return Ok(Instruction::NestedCostThen {
                cost: credit_cost(file, l, ast, cost)?,
                effect: Box::new(denote_line(file, ast, &inner(effect))?),
                payer: None,
            });
        }
        let effect = denote_line(file, ast, &inner(rest))?;
        // CR 8.5.5 / 8.6.3: a multi-install or multi-play already chooses its
        // cards "up to" the stated number, so the printed "you may" is that
        // choice — wrapping it again would ask twice.
        if matches!(effect, Instruction::PlayCards { .. } | Instruction::InstallCards { .. }) {
            return Ok(effect);
        }
        return Ok(Instruction::DeclineableChoice(Box::new(effect)));
    }

    // CR 9.6.14d: "Resolve the 'when scored' ability on an agenda in your
    // score area."
    for lead in ["resolve the \"when scored\" ability on ", "resolve the \"when scored\" ability of "] {
        if let Some(rest) = t.strip_prefix(lead) {
            if rest == "an agenda in your score area" {
                return Ok(Instruction::ResolveAbilityOf {
                    source: TargetSpec::Choose {
                        count: Quantity::c(1),
                        criteria: vec![TargetFilter::InScoreAreaOf(side_of(ast))],
                    },
                    which: AbilityClass::WhenScored,
                });
            }
        }
    }

    // CR 9.11.4a / 9.12.2: a printed sentence that does several things is ONE
    // instruction ("Gain 4[credit] and draw 3 cards.").
    if let Some((a, b)) = split_and(t) {
        let first = denote_line(file, ast, &inner(&a))?;
        let second = denote_line(file, ast, &inner(&b))?;
        return Ok(Instruction::Combined(vec![first, second]));
    }

    Err(unknown(&l.text))
}

/// Split a sentence on the LAST top-level " and " — "gain 3 credits and draw
/// 3 cards" is two effects, "an installed card you can advance" is not a
/// sentence at all and never reaches here.
fn split_and(t: &str) -> Option<(String, String)> {
    let i = t.rfind(" and ")?;
    Some((t[..i].to_string(), t[i + 5..].to_string()))
}

/// The declarations a `static:` block can state (9.3.5: a declaration applies
/// continuously). A line that states a printed ADDITIONAL COST is kept on the
/// card instead — 1.16.10's costs are inherent properties, not declarations —
/// and reports `None`.
fn denote_static(
    file: &str,
    ast: &CardAst,
    l: &Line,
    card: &mut PrintedCard,
) -> Result<Option<StaticDecl>, CardError> {
    let t = l.text.trim().to_lowercase();
    let w: Vec<&str> = t.split_whitespace().collect();
    let n = |s: &str| int(file, l, ast, s);

    // CR 1.16.10: "As an additional cost to steal THIS agenda, …" is printed
    // on the agenda (Obokata class); "…to steal AN agenda" is a declaration
    // reaching every steal (Ben Musashi class).
    for lead in [
        "as an additional cost to steal this agenda, the runner must pay ",
        "as an additional cost to steal this agenda, the runner pays ",
    ] {
        if let Some(cost) = t.strip_prefix(lead) {
            card.additional_steal_cost = Some(credit_cost(file, l, ast, cost)?);
            return Ok(None);
        }
    }
    for lead in [
        "as an additional cost to steal an agenda, the runner must pay ",
        "as an additional cost to steal an agenda, you must pay ",
    ] {
        if let Some(cost) = t.strip_prefix(lead) {
            return Ok(Some(StaticDecl::AdditionalStealCost(credit_cost(file, l, ast, cost)?)));
        }
    }
    // CR 1.16.10: "As an additional cost to play this operation/event, …".
    for lead in [
        "as an additional cost to play this operation, ",
        "as an additional cost to play this event, ",
        "as an additional cost to play this card, ",
    ] {
        if let Some(rest) = t.strip_prefix(lead) {
            card.additional_play_cost = Some(play_cost(file, l, ast, rest)?);
            return Ok(None);
        }
    }

    Ok(Some(match w.as_slice() {
        // CR 2.8 / 1.19: "+N[mu]".
        [x, "memory"] | [x, "mu"] => StaticDecl::MemoryLimitMod(strength_delta(file, l, ast, x)?),
        // CR 1.20: "+N link".
        [x, "link"] => StaticDecl::LinkBonus(strength_delta(file, l, ast, x)?),
        // "This program gets −N strength." (a declaration, unlike a paid
        // ability's `+N strength`, which is an instruction with a duration.)
        ["this", _kind, "gets", x, "strength"] => {
            StaticDecl::StrengthMod { target_self: true, delta: strength_delta(file, l, ast, x)? }
        }
        // CR 6.7.2 / 6.9.5a: "Runs against this server cannot be declared
        // successful."
        ["runs", "against", "this", "server", "cannot", "be", "declared", "successful"]
        | ["runs", "on", "this", "server", "cannot", "be", "declared", "successful"] => {
            StaticDecl::RunsNotDeclaredSuccessful
        }
        // CR 9.9.8b / 8.2.2: "Remove this card from the game instead of
        // trashing it."
        ["remove", "this", "card", "from", "the", "game", "instead", "of", "trashing", "it"] => {
            StaticDecl::ReplaceTrashDestination {
                criteria: vec![TargetFilter::IsSource],
                to: TrashDestination::RemovedFromGame,
            }
        }
        // CR 8.6.6c: a current stays in the play area until the stated effect.
        ["this", "card", "is", "not", "trashed", "until", "another", "current", "is", "played", "or", "an", "agenda", "is", "stolen"] => {
            StaticDecl::PlayedNotTrashedUntilAgendaSteal
        }
        // CR 1.13.5 / 1.13.6a: "…can host a single agenda", "Limit 1 hosted
        // card."
        ["this", "card", "can", "host", "a", "single", ty] | ["this", "card", "can", "host", "1", ty] => {
            StaticDecl::CanHost {
                criteria: vec![TargetFilter::CardTypeIs(parse_type(ty).ok_or_else(|| {
                    err(file, l.line, &ast.name, format!("unknown card type `{ty}`"),
                        "e.g. `this card can host a single agenda`")
                })?)],
                capacity: Some(Quantity::c(1)),
            }
        }
        ["limit", x, "hosted", "card"] | ["limit", x, "hosted", "cards"] => StaticDecl::CanHost {
            criteria: Vec::new(),
            capacity: Some(Quantity::c(n(x)?)),
        },
        _ => {
            return Err(err(
                file,
                l.line,
                &ast.name,
                format!("unknown declaration: \"{}\"", l.text),
                "see docs/cards/DSL.md for the declarations a `static:` block can state; \
                 if the card declares something the vocabulary cannot yet say, write it as \
                 `unimplemented: \"<the printed sentence>\"` instead of approximating it",
            ))
        }
    }))
}

// --- small helpers -------------------------------------------------------

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn fact(ast: &CardAst, key: &str) -> Option<String> {
    ast.facts.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

fn parse_type(t: &str) -> Option<CardType> {
    Some(match t.to_lowercase().as_str() {
        "agenda" => CardType::Agenda,
        "asset" => CardType::Asset,
        "ice" => CardType::Ice,
        "operation" => CardType::Operation,
        "upgrade" => CardType::Upgrade,
        "event" => CardType::Event,
        "hardware" => CardType::Hardware,
        "program" => CardType::Program,
        "resource" => CardType::Resource,
        "identity" => CardType::Identity,
        _ => return None,
    })
}

/// "Barrier", "Code Gate" — the printed capitalisation of a subtype, which is
/// how subtypes are written in the `subtypes:` fact and compared by the VM.
fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|word| {
            let mut cs = word.chars();
            match cs.next() {
                Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn counter_kind(file: &str, l: &Line, ast: &CardAst, k: &str) -> Result<CounterKind, CardError> {
    Ok(match k {
        "advancement" => CounterKind::Advancement,
        "power" => CounterKind::Power,
        "virus" => CounterKind::Virus,
        "agenda" => CounterKind::Agenda,
        "credit" => CounterKind::Credit,
        other => {
            return Err(err(
                file,
                l.line,
                &ast.name,
                format!("unknown counter kind `{other}`"),
                "counter kinds are: advancement, power, virus, agenda, credit",
            ))
        }
    })
}

/// "+2" / "−2" / "-2" — a signed printed modifier. The card prints U+2212.
fn strength_delta(file: &str, l: &Line, ast: &CardAst, s: &str) -> Result<i32, CardError> {
    let s = s.replace('\u{2212}', "-");
    let s = s.strip_prefix('+').unwrap_or(&s);
    s.parse::<i32>().map_err(|_| {
        err(file, l.line, &ast.name, format!("`{s}` is not a signed number"), "write e.g. `+1 strength` or `-2 strength`")
    })
}

/// "3 credits" / "3[credit]" as a cost (1.16.4).
fn credit_cost(file: &str, l: &Line, ast: &CardAst, s: &str) -> Result<Cost, CardError> {
    let s = s.trim().trim_end_matches('.');
    let num = s
        .strip_suffix(" credits")
        .or_else(|| s.strip_suffix(" credit"))
        .ok_or_else(|| {
            err(file, l.line, &ast.name, format!("unknown cost `{s}`"), "write e.g. `3 credits`")
        })?;
    Ok(Cost::credits(int(file, l, ast, num)? as u32))
}

/// The additional costs 1.16.10 lets a played card print.
fn play_cost(file: &str, l: &Line, ast: &CardAst, s: &str) -> Result<Cost, CardError> {
    let s = s.trim().trim_end_matches('.');
    Ok(match s {
        "spend click" | "spend 1 click" => Cost { clicks: 1, ..Cost::default() },
        "forfeit an agenda" | "forfeit 1 agenda" => Cost::forfeit_agenda(1),
        _ => {
            if let Ok(c) = credit_cost(file, l, ast, s.strip_prefix("pay ").unwrap_or(s)) {
                c
            } else {
                return Err(err(
                    file,
                    l.line,
                    &ast.name,
                    format!("unknown additional cost `{s}`"),
                    "additional play costs are: spend click, forfeit an agenda, pay N credits",
                ));
            }
        }
    })
}

/// A paid ability's trigger cost (9.5.1) plus the flags its printed prefix
/// states — "Interface →" (9.3.6c) and "Access →" (9.3.6b).
fn parse_cost(
    file: &str,
    ast: &CardAst,
    b: &Block,
    s: &str,
) -> Result<(Cost, Vec<AbilityFlag>), CardError> {
    let mut cost = Cost::free();
    let mut flags = Vec::new();
    let mut rest = s.trim().to_lowercase();
    loop {
        if let Some(r) = rest.strip_prefix("interface ") {
            flags.push(AbilityFlag::Interface);
            rest = r.trim().to_string();
        } else if let Some(r) = rest.strip_prefix("access ") {
            flags.push(AbilityFlag::Access);
            rest = r.trim().to_string();
        } else if let Some(r) = rest.strip_prefix("once per turn ") {
            flags.push(AbilityFlag::OncePerTurn);
            rest = r.trim().to_string();
        } else {
            break;
        }
    }

    for part in rest.split(',') {
        let p = part.trim();
        match p {
            "" | "free" => {}
            "click" => cost.clicks += 1,
            "trash" => cost.trash_self = true,
            _ => {
                let words: Vec<&str> = p.split_whitespace().collect();
                match words.as_slice() {
                    // CR 1.9.2: "Hosted power counter:" — spend one counter of
                    // that kind from the source.
                    ["hosted", kind, "counter"] => {
                        let l = Line { text: p.to_string(), items: Vec::new(), line: b.line };
                        cost.spend_counters = Some((counter_kind(file, &l, ast, kind)?, 1));
                    }
                    [x, "hosted", kind, "counters"] | [x, "hosted", kind, "counter"] => {
                        let l = Line { text: p.to_string(), items: Vec::new(), line: b.line };
                        let k = counter_kind(file, &l, ast, kind)?;
                        cost.spend_counters = Some((k, int(file, &l, ast, x)? as u32));
                    }
                    [x, "credits"] | [x, "credit"] => {
                        cost.credits = Quantity::c(x.parse::<i64>().map_err(|_| {
                            err(file, b.line, &ast.name, format!("`{x}` is not a number"), "e.g. `paid 2 credits:`")
                        })?);
                    }
                    [x, "clicks"] => {
                        cost.clicks += x.parse::<u32>().map_err(|_| {
                            err(file, b.line, &ast.name, format!("`{x}` is not a number"), "e.g. `paid 2 clicks:`")
                        })?;
                    }
                    _ => {
                        return Err(err(
                            file,
                            b.line,
                            &ast.name,
                            format!("unknown cost `{p}`"),
                            "costs are: free, click, trash, N credits, hosted <kind> counter — \
                             combine with commas, e.g. `paid click, 1 credit:`; a printed \
                             `Interface →` or `Access →` prefix is written the same way, \
                             e.g. `paid interface 1 credit:`",
                        ))
                    }
                }
            }
        }
    }
    Ok((cost, flags))
}

/// CR 9.5.6a/c: an ability that refers to the encountered ice is usable only
/// during an encounter, with ice matching whatever stipulation it used. The
/// break line carries both facts.
fn encounter_timing(lines: &[Line]) -> Option<TimingRestriction> {
    for l in lines {
        let t = l.text.trim().to_lowercase();
        let Some(rest) = t.strip_prefix("break ") else { continue };
        let words: Vec<&str> = rest.split_whitespace().collect();
        // "1 sentry subroutine" / "1 code gate subroutine" — everything
        // between the count and "subroutine" is the stipulated subtype.
        let subtype = match words.as_slice() {
            [_, mid @ .., last] if (*last == "subroutine" || *last == "subroutines") && !mid.is_empty() => {
                Some(leak(&title_case(&mid.join(" "))))
            }
            _ => None,
        };
        return Some(TimingRestriction::EncounterOnly { required_subtype: subtype });
    }
    None
}

fn side_of(ast: &CardAst) -> Side {
    fact(ast, "side")
        .map(|s| if s.eq_ignore_ascii_case("runner") { Side::Runner } else { Side::Corp })
        .unwrap_or(Side::Corp)
}

fn parse_trigger(file: &str, ast: &CardAst, b: &Block, s: &str) -> Result<TriggerCond, CardError> {
    Ok(match s.trim() {
        "your turn begins" | "the turn begins" => TriggerCond::TurnBegins(side_of(ast)),
        "the run ends" => TriggerCond::RunEnds { successful_only: false },
        "a successful run ends" => TriggerCond::RunEnds { successful_only: true },
        "installed" => TriggerCond::SelfInstalled,
        "scored" => TriggerCond::SelfScored { requires: Vec::new() },
        "stolen" => TriggerCond::SelfStolen,
        "empty" => TriggerCond::SelfEmpty { kind: CounterKind::Credit },
        "empty of power counters" => TriggerCond::SelfEmpty { kind: CounterKind::Power },
        "encountered" => TriggerCond::SelfEncountered,
        "accessed" => TriggerCond::SelfAccessed { requires: Vec::new() },
        "passed" => TriggerCond::SelfPassed,
        // CR 8.6.7h: "After you resolve this operation, …" (Terminal class).
        "this operation resolves" | "this event resolves" => TriggerCond::SelfPlayResolved,
        other => {
            return Err(err(
                file,
                b.line,
                &ast.name,
                format!("unknown trigger `when {other}`"),
                "triggers are: your turn begins, the run ends, a successful run ends, \
                 installed, scored, stolen, empty, encountered, accessed, passed, \
                 this operation resolves — see docs/cards/DSL.md",
            ))
        }
    })
}

fn num(file: &str, ast: &CardAst, v: &str) -> Result<i64, CardError> {
    v.trim().parse::<i64>().map_err(|_| {
        err(file, ast.line, &ast.name, format!("`{v}` is not a number"), "write a plain number, e.g. `cost: 3`")
    })
}

fn int(file: &str, l: &Line, ast: &CardAst, v: &str) -> Result<i64, CardError> {
    v.trim().parse::<i64>().map_err(|_| {
        err(file, l.line, &ast.name, format!("`{v}` is not a number"), "write a plain number, e.g. `gain 5 credits`")
    })
}

fn err(file: &str, line: usize, card: &str, problem: impl Into<String>, hint: impl Into<String>) -> CardError {
    CardError {
        file: file.to_string(),
        line,
        card: card.to_string(),
        problem: problem.into(),
        hint: hint.into(),
    }
}
