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
use jinteki_cr::ability::{AbilityDef, Cost, TriggerCond};
use jinteki_cr::effects::DamageKind;
use jinteki_cr::instr::{Instruction, Quantity, TargetSpec};
use jinteki_cr::object::{CardType, CounterKind, PrintedCard, Side};

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
    let name: &'static str = Box::leak(ast.name.clone().into_boxed_str());
    let side = fact(ast, "side").map(|s| parse_side(&s)).transpose()?.unwrap_or(Side::Corp);
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
            "subtypes" => {
                c.subtypes = v
                    .split(',')
                    .map(|s| -> &'static str { Box::leak(s.trim().to_string().into_boxed_str()) })
                    .collect()
            }
            "cost" => c.cost = Some(num(file, ast, v)? as u32),
            "strength" => c.strength = Some(num(file, ast, v)? as i32),
            "trash cost" => c.trash_cost = Some(num(file, ast, v)? as u32),
            "memory" => c.memory_cost = Some(num(file, ast, v)? as u32),
            "advancement" => c.advancement_requirement = Some(num(file, ast, v)? as u32),
            "points" => c.agenda_points = Some(num(file, ast, v)? as i32),
            "unique" => c.unique = v.eq_ignore_ascii_case("yes"),
            "console" => c.console = v.eq_ignore_ascii_case("yes"),
            other => {
                return Err(err(
                    file,
                    ast.line,
                    &ast.name,
                    format!("unknown fact `{other}`"),
                    "facts are: side, type, subtypes, cost, strength, trash cost, memory, \
                     advancement, points, unique, console, faction, set, influence",
                ))
            }
        }
    }

    let mut abilities = Vec::new();
    for b in &ast.blocks {
        abilities.push(denote_block(file, ast, b)?);
    }
    c.abilities = abilities;

    Ok(DenotedCard {
        printed: c,
        oracle_text: ast.text.join("\n"),
        unimplemented: ast.unimplemented.clone(),
    })
}

fn denote_block(file: &str, ast: &CardAst, b: &Block) -> Result<AbilityDef, CardError> {
    let instrs = b
        .lines
        .iter()
        .map(|l| denote_line(file, ast, l))
        .collect::<Result<Vec<_>, _>>()?;
    let label: &'static str =
        Box::leak(format!("{}: {}", ast.name.to_lowercase(), b.header).into_boxed_str());

    let h = b.header.as_str();
    Ok(match h {
        "play" => AbilityDef::play(instrs).labeled(label),
        "subroutine" => AbilityDef::subroutine(instrs).labeled(label),
        _ if h.starts_with("paid ") => {
            AbilityDef::paid(parse_cost(file, ast, b, &h[5..])?, instrs).labeled(label)
        }
        _ if h.starts_with("when ") => {
            AbilityDef::conditional(parse_trigger(file, ast, b, &h[5..])?, instrs, false).labeled(label)
        }
        "static" => {
            return Err(err(
                file,
                b.line,
                &ast.name,
                "`static:` blocks are not written yet",
                "for now, write the sentence as `unimplemented: \"…\"` and it will be \
                 counted honestly as a gap",
            ))
        }
        other => {
            return Err(err(
                file,
                b.line,
                &ast.name,
                format!("unknown ability block `{other}:`"),
                "blocks are: play, paid <cost>, when <trigger>, subroutine, static, interrupt <trigger>",
            ))
        }
    })
}

/// The verbs. Each is one printed sentence and denotes into exactly one
/// kernel instruction.
fn denote_line(file: &str, ast: &CardAst, l: &Line) -> Result<Instruction, CardError> {
    let t = l.text.trim().to_lowercase();
    let w: Vec<&str> = t.split_whitespace().collect();
    let side = fact(ast, "side").map(|s| parse_side(&s)).transpose()?.unwrap_or(Side::Corp);
    let other = if side == Side::Corp { Side::Runner } else { Side::Corp };

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

    Ok(match w.as_slice() {
        // credits
        ["gain", n, "credits"] | ["gain", n, "credit"] => {
            Instruction::GainCredits(side, Quantity::c(int(file, l, ast, n)?))
        }
        ["lose", n, "credits"] | ["lose", n, "credit"] => {
            Instruction::LoseCredits(side, int(file, l, ast, n)? as u32)
        }
        ["the", "runner", "loses", n, "credits"] | ["the", "runner", "loses", n, "credit"] => {
            Instruction::LoseCredits(Side::Runner, int(file, l, ast, n)? as u32)
        }
        ["the", "corp", "loses", n, "credits"] | ["the", "corp", "loses", n, "credit"] => {
            Instruction::LoseCredits(Side::Corp, int(file, l, ast, n)? as u32)
        }
        // cards
        ["draw", n, "cards"] | ["draw", n, "card"] => {
            Instruction::Draw(side, int(file, l, ast, n)? as u32)
        }
        // damage
        ["do", n, "net", "damage"] => Instruction::Damage {
            kind: DamageKind::Net,
            amount: Quantity::c(int(file, l, ast, n)?),
            responsible: side,
        },
        ["do", n, "meat", "damage"] => Instruction::Damage {
            kind: DamageKind::Meat,
            amount: Quantity::c(int(file, l, ast, n)?),
            responsible: side,
        },
        ["do", n, "core", "damage"] | ["do", n, "brain", "damage"] => Instruction::Damage {
            kind: DamageKind::Core,
            amount: Quantity::c(int(file, l, ast, n)?),
            responsible: side,
        },
        // tags
        ["give", "the", "runner", n, "tags"] | ["give", "the", "runner", n, "tag"] => {
            Instruction::GainTags(int(file, l, ast, n)? as u32)
        }
        ["take", n, "tags"] | ["take", n, "tag"] => Instruction::GainTags(int(file, l, ast, n)? as u32),
        ["remove", n, "tags"] | ["remove", n, "tag"] => {
            Instruction::RemoveTags(Quantity::c(int(file, l, ast, n)?))
        }
        // the run
        ["end", "the", "run"] => Instruction::EndTheRun,
        // self
        ["trash", "self"] => Instruction::TrashSelf,
        ["purge", "virus", "counters"] => Instruction::PurgeVirusCounters,
        // counters on self
        ["load", n, "credits", "on", "self"] | ["load", n, "credit", "on", "self"] => {
            Instruction::LoadCounters { target: TargetSpec::SelfSource, kind: CounterKind::Credit, amount: Quantity::c(int(file, l, ast, n)?) }
        }
        ["load", n, "agenda", "counters", "on", "self"] | ["load", n, "agenda", "counter", "on", "self"] => {
            Instruction::LoadCounters {
                target: TargetSpec::SelfSource,
                kind: CounterKind::Agenda,
                amount: Quantity::c(int(file, l, ast, n)?),
            }
        }
        ["load", n, "power", "counters", "on", "self"] | ["load", n, "power", "counter", "on", "self"] => {
            Instruction::LoadCounters { target: TargetSpec::SelfSource, kind: CounterKind::Power, amount: Quantity::c(int(file, l, ast, n)?) }
        }
        ["take", _n, "credits", "from", "self"] | ["take", _n, "credit", "from", "self"] => {
            Instruction::MoveSetAsideCounters {
                kind: CounterKind::Credit,
                target: TargetSpec::SelfSource,
            }
        }
        // opponent-facing shorthand used by ice and ambushes
        ["give", "the", "corp", n, "bad", "publicity"] => {
            Instruction::TakeBadPublicity { side: Side::Corp, amount: Quantity::c(int(file, l, ast, n)?) }
        }
        _ => {
            // trace N: <effect>  — one line, because the card writes it as one
            if let Some((head, tail)) = t.split_once(':') {
                if let Some(base) = head.trim().strip_prefix("trace ") {
                    let inner = Line { text: tail.trim().to_string(), items: Vec::new(), line: l.line };
                    let effect = denote_line(file, ast, &inner)?;
                    return Ok(Instruction::Trace {
                        base: Quantity::c(int(file, l, ast, base.trim())?),
                        if_successful: vec![effect],
                        if_unsuccessful: Vec::new(),
                        determined_min: None,
                    });
                }
            }
            // choose one: with its list
            if t == "choose one" && !l.items.is_empty() {
                let mut options = Vec::new();
                for item in &l.items {
                    let inner = Line { text: item.clone(), items: Vec::new(), line: l.line };
                    let label: &'static str = Box::leak(item.clone().into_boxed_str());
                    options.push((label, vec![denote_line(file, ast, &inner)?]));
                }
                return Ok(Instruction::ChooseOne { options });
            }
            let _ = other;
            return Err(unknown(&l.text));
        }
    })
}

// --- small helpers -------------------------------------------------------

fn fact(ast: &CardAst, key: &str) -> Option<String> {
    ast.facts.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

fn parse_side(s: &str) -> Result<Side, CardError> {
    Ok(if s.eq_ignore_ascii_case("runner") { Side::Runner } else { Side::Corp })
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

fn parse_cost(file: &str, ast: &CardAst, b: &Block, s: &str) -> Result<Cost, CardError> {
    let mut cost = Cost::free();
    for part in s.split(',') {
        let p = part.trim().to_lowercase();
        match p.as_str() {
            "free" => {}
            "click" => cost.clicks += 1,
            "trash" => cost.trash_self = true,
            _ => {
                if let Some(n) = p.strip_suffix(" credits").or_else(|| p.strip_suffix(" credit")) {
                    cost.credits = Quantity::c(n.trim().parse::<i64>().map_err(|_| {
                        err(file, b.line, &ast.name, format!("`{n}` is not a number"), "e.g. `paid 2 credits:`")
                    })?);
                } else {
                    return Err(err(
                        file,
                        b.line,
                        &ast.name,
                        format!("unknown cost `{p}`"),
                        "costs are: free, click, trash, N credits — combine with commas, \
                         e.g. `paid click, 1 credit:`",
                    ));
                }
            }
        }
    }
    Ok(cost)
}

fn side_of(ast: &CardAst) -> Side {
    fact(ast, "side").map(|s| if s.eq_ignore_ascii_case("runner") { Side::Runner } else { Side::Corp }).unwrap_or(Side::Corp)
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
        "encountered" => TriggerCond::SelfEncountered,
        "accessed" => TriggerCond::SelfAccessed { requires: Vec::new() },
        other => {
            return Err(err(
                file,
                b.line,
                &ast.name,
                format!("unknown trigger `when {other}`"),
                "triggers are: your turn begins, the run ends, installed, scored, stolen, \
                 empty, encountered, accessed — see docs/cards/DSL.md",
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

/// Unused import guard: `TargetSpec` is part of the vocabulary the next verbs
/// will need (install/trash targets); referencing it keeps the intent visible.
#[allow(dead_code)]
fn _vocabulary_reserved(_: TargetSpec) {}
