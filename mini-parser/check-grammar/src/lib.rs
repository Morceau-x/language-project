//! The grammar checker as a library: `main.rs` is its command line, and
//! `Oracle` lets the parser's tests ask a spec whether a program is valid.

pub mod earley;
pub mod ebnf;
pub mod lexical;
pub mod ll1;
pub mod samples;
pub mod tokens;

use std::collections::HashSet;
use std::path::Path;

use earley::{Bnf, recognize};
use ebnf::Grammar;
use tokens::NoTokens;

/// The token kinds the syntactic grammar can use by name.
pub const TOKENS: &[&str] = &[
    "LOWER",
    "UPPER",
    "INT",
    "FLOAT",
    "STRING",
    "STRING_START",
    "STRING_MIDDLE",
    "STRING_END",
    "NEWLINE",
];

/// The rules used from `start`, if they're all defined; or the first
/// that isn't.
pub fn bnf_for(grammar: &Grammar, start: &str) -> Result<Bnf, String> {
    if !grammar.rules.iter().any(|r| r.name == start) {
        return Err(format!("`{start}` is not defined yet"));
    }
    let mut seen = HashSet::new();
    let mut todo = vec![start.to_string()];
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) || TOKENS.contains(&name.as_str()) {
            continue;
        }
        let Some(rule) = grammar.rules.iter().find(|r| r.name == name) else {
            return Err(format!("`{name}` is not defined yet"));
        };
        let mut refs = Vec::new();
        rule.expr.refs(&mut refs);
        todo.extend(refs.into_iter().map(|(name, _)| name));
    }
    Ok(Bnf::new(grammar, &[start], TOKENS))
}

/// A spec, ready to judge programs: is this text a valid module?
pub struct Oracle {
    bnf: Bnf,
    start: usize,
}

impl Oracle {
    /// Read the spec in `path` (a Markdown file with ` ```ebnf ` blocks).
    pub fn from_spec(path: &Path) -> Result<Oracle, String> {
        let markdown = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let grammar = ebnf::parse_spec(&markdown).map_err(|errors| {
            let errors: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            format!("{} doesn't parse:\n{}", path.display(), errors.join("\n"))
        })?;
        let bnf = bnf_for(&grammar, "Module")?;
        let start = bnf.nt("Module").expect("Module is in the BNF");
        Ok(Oracle { bnf, start })
    }

    /// Is `text` a valid module according to the spec? `None` if it has
    /// HTML in it, which the spec leaves out.
    pub fn is_valid(&self, text: &str) -> Option<bool> {
        match tokens::lex(text) {
            Ok(tokens) => {
                let ids: Vec<Option<usize>> = tokens
                    .names
                    .iter()
                    .map(|name| self.bnf.term(name))
                    .collect();
                Some(recognize(&self.bnf, self.start, &ids).accepts(self.start))
            }
            Err(NoTokens::LexicalError) => Some(false),
            Err(NoTokens::Html) => None,
        }
    }
}
