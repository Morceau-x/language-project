//! Cutting sample programs into tokens, named the way the spec names
//! them: `LOWER`, `"fn"`, `NEWLINE`…
//!
//! A small lexer of its own, so that the samples can be checked before
//! you write yours. It follows section 2 of the spec: longest match,
//! nested comments, strings with holes, and the rules that need context
//! (where a line break is a `NEWLINE`, where `<` would start HTML).

/// The tokens of a sample: their names in the spec, and their texts
/// (`⏎` for a `NEWLINE`).
pub struct Tokens {
    pub names: Vec<String>,
    pub texts: Vec<String>,
}

/// Why a sample has no tokens.
pub enum NoTokens {
    /// The lexical grammar rejects it.
    LexicalError,
    /// It has HTML in it, which the spec leaves out.
    Html,
}

const KEYWORDS: &[&str] = &[
    "module",
    "import",
    "type",
    "fn",
    "cast",
    "component",
    "let",
    "match",
    "if",
    "else",
];

const TWO_CHAR_PUNCTUATION: &[&str] = &[
    "::", "..", "==", "=>", "!=", "<=", ">=", "||", "|>", "&&", "->", "++", "**", "~/",
];

const ONE_CHAR_PUNCTUATION: &str = "(){}[]<>,:.=!|-+*/%?";

/// The tokens an expression can end with.
const CAN_END: &[&str] = &[
    "LOWER",
    "UPPER",
    "INT",
    "FLOAT",
    "STRING",
    "STRING_END",
    "\"_\"",
    "\")\"",
    "\"]\"",
    "\"}\"",
    "\"?\"",
];

/// The tokens a statement, a match arm or a declaration can start with.
const CAN_START: &[&str] = &[
    "LOWER",
    "UPPER",
    "INT",
    "FLOAT",
    "STRING",
    "STRING_START",
    "\"_\"",
    "\"(\"",
    "\"[\"",
    "\"{\"",
    "\"-\"",
    "\"!\"",
    "\"fn\"",
    "\"if\"",
    "\"match\"",
    "\"let\"",
    "\"import\"",
    "\"type\"",
    "\"cast\"",
    "\"component\"",
];

/// A token before `NEWLINE`s are added: its name, its text, and whether
/// a line break comes before it.
struct Raw {
    name: String,
    text: String,
    line_break: bool,
}

pub fn lex(text: &str) -> Result<Tokens, NoTokens> {
    let raw = raw_tokens(text)?;
    // Section 2.6: a line break is a `NEWLINE` when what's before can end
    // a statement, what's after can start one, and it isn't inside
    // parentheses or brackets.
    let mut names = Vec::new();
    let mut texts = Vec::new();
    let mut brackets: Vec<String> = Vec::new();
    for token in raw {
        let in_parens = brackets
            .last()
            .is_some_and(|b| b == "\"(\"" || b == "\"[\"");
        let after_end = names
            .last()
            .is_some_and(|last: &String| CAN_END.contains(&last.as_str()));
        if token.line_break && !in_parens && after_end && CAN_START.contains(&token.name.as_str()) {
            names.push("NEWLINE".to_string());
            texts.push("⏎".to_string());
        }
        match token.name.as_str() {
            "\"(\"" | "\"[\"" | "\"{\"" => brackets.push(token.name.clone()),
            "\")\"" | "\"]\"" | "\"}\"" => {
                brackets.pop();
            }
            _ => {}
        }
        names.push(token.name);
        texts.push(token.text);
    }
    Ok(Tokens { names, texts })
}

fn raw_tokens(text: &str) -> Result<Vec<Raw>, NoTokens> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens: Vec<Raw> = Vec::new();
    let mut line_break = false;
    // The interpolation holes open, innermost last: the `{` open in each.
    let mut holes: Vec<u32> = Vec::new();
    let mut i = 0;
    let push =
        |tokens: &mut Vec<Raw>, line_break: &mut bool, name: String, from: usize, to: usize| {
            tokens.push(Raw {
                name,
                text: chars[from..to].iter().collect(),
                line_break: *line_break,
            });
            *line_break = false;
        };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        // The `}` that closes a hole: the string goes on.
        if c == '}' && holes.last() == Some(&0) {
            holes.pop();
            let (end, opens_hole) = string_chunk(&chars, i + 1)?;
            let name = if opens_hole {
                holes.push(0);
                "STRING_MIDDLE"
            } else {
                "STRING_END"
            };
            push(&mut tokens, &mut line_break, name.to_string(), i, end);
            i = end;
            continue;
        }
        match c {
            ' ' | '\t' => i += 1,
            '\n' | '\r' => {
                if c == '\r' && next != Some('\n') {
                    return Err(NoTokens::LexicalError);
                }
                if !holes.is_empty() {
                    return Err(NoTokens::LexicalError);
                }
                line_break = true;
                i += if c == '\r' { 2 } else { 1 };
            }
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i] != '\n' && chars[i] != '\r' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                let start = i;
                i += 2;
                let mut depth = 1;
                while depth > 0 {
                    match (chars.get(i), chars.get(i + 1)) {
                        (None, _) => return Err(NoTokens::LexicalError),
                        (Some('/'), Some('*')) => {
                            depth += 1;
                            i += 2;
                        }
                        (Some('*'), Some('/')) => {
                            depth -= 1;
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
                if chars[start..i].contains(&'\n') {
                    if !holes.is_empty() {
                        return Err(NoTokens::LexicalError);
                    }
                    line_break = true;
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let name = if word == "_" || KEYWORDS.contains(&word.as_str()) {
                    format!("\"{word}\"")
                } else if c.is_ascii_uppercase() {
                    "UPPER".to_string()
                } else {
                    "LOWER".to_string()
                };
                push(&mut tokens, &mut line_break, name, start, i);
            }
            '0'..='9' => {
                let start = i;
                let digits = |i: &mut usize| {
                    while *i < chars.len() && (chars[*i].is_ascii_digit() || chars[*i] == '_') {
                        *i += 1;
                    }
                };
                digits(&mut i);
                let mut name = "INT";
                if chars.get(i) == Some(&'.')
                    && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit())
                {
                    i += 1;
                    digits(&mut i);
                    name = "FLOAT";
                }
                push(&mut tokens, &mut line_break, name.to_string(), start, i);
            }
            '"' => {
                let (end, opens_hole) = string_chunk(&chars, i + 1)?;
                let name = if opens_hole {
                    holes.push(0);
                    "STRING_START"
                } else {
                    "STRING"
                };
                push(&mut tokens, &mut line_break, name.to_string(), i, end);
                i = end;
            }
            _ => {
                let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
                let len = if TWO_CHAR_PUNCTUATION.contains(&two.as_str()) {
                    2
                } else if ONE_CHAR_PUNCTUATION.contains(c) {
                    1
                } else {
                    return Err(NoTokens::LexicalError);
                };
                // Section 2.6, rule 3: `<` and a letter, where an
                // expression can start, would be HTML.
                if len == 1 && c == '<' && next.is_some_and(|c| c.is_ascii_alphabetic()) {
                    let can_start_here = line_break
                        || tokens
                            .last()
                            .is_none_or(|last| !CAN_END.contains(&last.name.as_str()));
                    if can_start_here {
                        return Err(NoTokens::Html);
                    }
                }
                if let Some(depth) = holes.last_mut() {
                    match c {
                        '{' => *depth += 1,
                        '}' => *depth -= 1,
                        _ => {}
                    }
                }
                let punct: String = chars[i..i + len].iter().collect();
                push(
                    &mut tokens,
                    &mut line_break,
                    format!("\"{punct}\""),
                    i,
                    i + len,
                );
                i += len;
            }
        }
    }
    if holes.is_empty() {
        Ok(tokens)
    } else {
        Err(NoTokens::LexicalError)
    }
}

/// Read a piece of string from `i`, after its opening `"` or `}`, up to
/// and including its end: `"`, or the `${` of a hole. Returns where it
/// ends, and whether it opens a hole.
fn string_chunk(chars: &[char], mut i: usize) -> Result<(usize, bool), NoTokens> {
    loop {
        match chars.get(i) {
            None | Some('\n') | Some('\r') => return Err(NoTokens::LexicalError),
            Some('"') => return Ok((i + 1, false)),
            Some('$') if chars.get(i + 1) == Some(&'{') => return Ok((i + 2, true)),
            Some('\\') => i = escape(chars, i + 1)?,
            Some(_) => i += 1,
        }
    }
}

/// Check the escape after a `\` at `i - 1`; return where it ends.
fn escape(chars: &[char], i: usize) -> Result<usize, NoTokens> {
    match chars.get(i) {
        Some('n' | 't' | '"' | '\\' | '$') => Ok(i + 1),
        Some('u') if chars.get(i + 1) == Some(&'{') => {
            let mut j = i + 2;
            let mut digits = String::new();
            while let Some(c) = chars.get(j).filter(|c| c.is_ascii_hexdigit()) {
                digits.push(*c);
                j += 1;
            }
            let valid = chars.get(j) == Some(&'}')
                && (1..=6).contains(&digits.len())
                && u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .is_some();
            if valid {
                Ok(j + 1)
            } else {
                Err(NoTokens::LexicalError)
            }
        }
        _ => Err(NoTokens::LexicalError),
    }
}
