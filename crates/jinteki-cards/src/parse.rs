//! The card file parser: text in, a card AST out.
//!
//! Deliberately boring. Indentation is two spaces per level, every construct
//! is `key: value` or `block:` followed by an indented body, and no line ever
//! means something different depending on a line far away from it. A card
//! designer reading an unfamiliar file should be able to guess what it does
//! (SYS-D-1, SYS-D-2); an error should tell them how to fix it (SYS-D-3).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAst {
    pub name: String,
    pub facts: Vec<(String, String)>,
    /// The printed oracle text, verbatim (SYS-D-10). Required.
    pub text: Vec<String>,
    pub blocks: Vec<Block>,
    /// Printed sentences with no expression in the vocabulary yet (SYS-D-9).
    pub unimplemented: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The header without its trailing colon: "play", "paid 1 credit",
    /// "when your turn begins", "subroutine", "static", "interrupt …".
    pub header: String,
    pub lines: Vec<Line>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    /// A nested list under this line ("choose one:" and its `- …` options).
    pub items: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardError {
    pub file: String,
    pub line: usize,
    pub card: String,
    pub problem: String,
    pub hint: String,
}

impl fmt::Display for CardError {
    /// The shape SYS-D-3 asks for: where, which card, what is wrong, what to
    /// do about it — in card-designer language.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)?;
        if !self.card.is_empty() {
            write!(f, " in \"{}\"", self.card)?;
        }
        write!(f, ": {}", self.problem)?;
        if !self.hint.is_empty() {
            write!(f, "\n  hint: {}", self.hint)?;
        }
        Ok(())
    }
}

impl CardError {
    fn at(file: &str, line: usize, card: &str, problem: impl Into<String>, hint: impl Into<String>) -> Self {
        CardError {
            file: file.to_string(),
            line,
            card: card.to_string(),
            problem: problem.into(),
            hint: hint.into(),
        }
    }
}

/// Indentation depth in levels; two spaces per level, tabs refused loudly.
fn depth(raw: &str, file: &str, line: usize, card: &str) -> Result<usize, CardError> {
    if raw.starts_with('\t') {
        return Err(CardError::at(
            file,
            line,
            card,
            "this line starts with a tab",
            "use two spaces per level of indentation, not tabs",
        ));
    }
    let spaces = raw.len() - raw.trim_start().len();
    if spaces % 2 != 0 {
        return Err(CardError::at(
            file,
            line,
            card,
            format!("this line is indented by {spaces} spaces"),
            "indent by two spaces per level (2, 4, 6 …)",
        ));
    }
    Ok(spaces / 2)
}

/// Parse a whole card file.
pub fn parse_file(file: &str, src: &str) -> Result<Vec<CardAst>, CardError> {
    let mut cards: Vec<CardAst> = Vec::new();
    let mut mode = Mode::Top;
    let lines: Vec<(usize, &str)> = src.lines().enumerate().map(|(i, l)| (i + 1, l)).collect();

    for (no, raw) in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let cur_name = cards.last().map(|c| c.name.clone()).unwrap_or_default();
        let d = depth(raw, file, no, &cur_name)?;

        // A new card always starts at column 0.
        if d == 0 {
            let rest = trimmed.strip_prefix("card ").ok_or_else(|| {
                CardError::at(
                    file,
                    no,
                    &cur_name,
                    format!("expected a new card here, found: {trimmed}"),
                    "a card starts with: card \"Card Name\"",
                )
            })?;
            let name = rest.trim().trim_matches('"').to_string();
            if name.is_empty() {
                return Err(CardError::at(file, no, "", "this card has no name", "write: card \"Card Name\""));
            }
            cards.push(CardAst {
                name,
                facts: Vec::new(),
                text: Vec::new(),
                blocks: Vec::new(),
                unimplemented: Vec::new(),
                line: no,
            });
            mode = Mode::Card;
            continue;
        }

        let card = cards.last_mut().ok_or_else(|| {
            CardError::at(
                file,
                no,
                "",
                "this line is indented but no card has started",
                "begin the file with: card \"Card Name\"",
            )
        })?;

        // Depth 1: facts, the text block, ability blocks, unimplemented notes.
        if d == 1 {
            mode = Mode::Card;
            if let Some(v) = trimmed.strip_prefix("unimplemented:") {
                let s = v.trim().trim_matches('"').to_string();
                if s.is_empty() {
                    return Err(CardError::at(
                        file,
                        no,
                        &card.name,
                        "an unimplemented note with no sentence",
                        "quote the printed sentence you cannot write yet, e.g. \
                         unimplemented: \"If you made a successful run this turn…\"",
                    ));
                }
                card.unimplemented.push(s);
                continue;
            }
            if trimmed == "text:" {
                mode = Mode::Text;
                continue;
            }
            if let Some(h) = trimmed.strip_suffix(':') {
                if is_block_header(h) {
                    card.blocks.push(Block { header: h.trim().to_string(), lines: Vec::new(), line: no });
                    mode = Mode::Block;
                    continue;
                }
            }
            // Otherwise it is a fact: `key: value`.
            let (k, v) = trimmed.split_once(':').ok_or_else(|| {
                CardError::at(
                    file,
                    no,
                    &card.name,
                    format!("cannot tell what this line is: {trimmed}"),
                    "facts look like `cost: 3`; abilities look like `play:` or \
                     `when your turn begins:` and have their sentences indented under them",
                )
            })?;
            card.facts.push((k.trim().to_lowercase(), v.trim().to_string()));
            continue;
        }

        // Depth ≥ 2: the body of whatever is open.
        match mode {
            Mode::Text => card.text.push(trimmed.to_string()),
            Mode::Block => {
                let block = card.blocks.last_mut().expect("a block is open");
                if let Some(item) = trimmed.strip_prefix("- ") {
                    let last = block.lines.last_mut().ok_or_else(|| {
                        CardError::at(
                            file,
                            no,
                            &card.name,
                            "a list item with nothing above it",
                            "put the list under a line that introduces it, e.g. `choose one:`",
                        )
                    })?;
                    last.items.push(item.trim().to_string());
                } else {
                    block.lines.push(Line {
                        text: trimmed.trim_end_matches(':').to_string(),
                        items: Vec::new(),
                        line: no,
                    });
                }
            }
            _ => {
                return Err(CardError::at(
                    file,
                    no,
                    &card.name,
                    format!("this line is indented further than anything it could belong to: {trimmed}"),
                    "indent sentences exactly one level under their ability block",
                ))
            }
        }
    }

    for c in &cards {
        if c.text.is_empty() {
            return Err(CardError::at(
                file,
                c.line,
                &c.name,
                "this card has no printed text",
                "add a `text:` block and copy the card's printed text into it — \
                 behaviour is checked against that text (SYS-D-10)",
            ));
        }
    }
    Ok(cards)
}

enum Mode {
    Top,
    Card,
    Text,
    Block,
}

fn is_block_header(h: &str) -> bool {
    let h = h.trim();
    h == "play"
        || h == "static"
        || h == "subroutine"
        || h.starts_with("static ")
        || h.starts_with("paid ")
        || h.starts_with("when ")
        || h.starts_with("interrupt ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURE_GAMBLE: &str = r#"
card "Sure Gamble"
  side: runner
  type: event
  cost: 5
  text:
    Gain 9[credit].
  play:
    gain 9 credits
"#;

    #[test]
    fn a_whole_card_parses() {
        let cards = parse_file("t.cards", SURE_GAMBLE).expect("parses");
        assert_eq!(cards.len(), 1);
        let c = &cards[0];
        assert_eq!(c.name, "Sure Gamble");
        assert_eq!(c.text, vec!["Gain 9[credit]."]);
        assert_eq!(c.blocks.len(), 1);
        assert_eq!(c.blocks[0].header, "play");
        assert_eq!(c.blocks[0].lines[0].text, "gain 9 credits");
    }

    #[test]
    fn a_card_without_its_printed_text_is_refused() {
        let e = parse_file("t.cards", "card \"X\"\n  side: runner\n").unwrap_err();
        assert!(e.problem.contains("no printed text"), "{e}");
        assert!(e.hint.contains("text:"), "{e}");
        assert!(e.to_string().contains("in \"X\""), "the error names the card: {e}");
    }

    #[test]
    fn an_error_names_file_line_card_and_a_fix() {
        let e = parse_file("t.cards", "card \"Y\"\n  this is not a fact\n").unwrap_err();
        let s = e.to_string();
        assert!(s.starts_with("t.cards:2 in \"Y\":"), "{s}");
        assert!(s.contains("hint:"), "{s}");
    }

    #[test]
    fn lists_attach_to_the_line_that_introduces_them() {
        let src = "card \"Z\"\n  text:\n    x\n  play:\n    choose one:\n      - gain 3 credits\n      - draw 3 cards\n";
        let cards = parse_file("t.cards", src).expect("parses");
        let l = &cards[0].blocks[0].lines[0];
        assert_eq!(l.text, "choose one");
        assert_eq!(l.items, vec!["gain 3 credits", "draw 3 cards"]);
    }
}
