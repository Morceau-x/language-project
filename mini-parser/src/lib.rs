//! A lexer and a parser for Maxim, built the way rust-analyzer parses
//! Rust:
//!
//! ```text
//! text ──lexer──▶ tokens ──parser──▶ events ──builder──▶ syntax tree ──▶ typed AST
//!                 (all of them,       (start node,        (lossless:       (ast::Fn,
//!                  trivia too)         token, finish…)     every byte)      ast::Expr…)
//! ```
//!
//! The parser never gives up: any text, however broken, gives a tree
//! that contains every byte of it, plus a list of errors.

// While the step is in progress, unfinished functions leave code unused.
// Part 13 removes this attribute.
#![allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    clippy::ptr_arg
)]

pub mod ast;
pub mod lexer;
pub mod outline;
mod parser;
mod syntax_kind;
mod text_range;
pub mod tree;
mod validation;

use std::fmt;
use std::rc::Rc;

pub use syntax_kind::SyntaxKind;
pub use text_range::TextRange;
pub use tree::{SyntaxElement, SyntaxNode, SyntaxToken};

use ast::AstNode;
use tree::GreenNode;

/// A syntax error: a message and where it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    msg: String,
    range: TextRange,
}

impl SyntaxError {
    pub fn new(msg: impl Into<String>, range: TextRange) -> SyntaxError {
        SyntaxError {
            msg: msg.into(),
            range,
        }
    }

    pub fn message(&self) -> &str {
        &self.msg
    }

    pub fn range(&self) -> TextRange {
        self.range
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error {}: {}", self.range, self.msg)
    }
}

/// The result of parsing: a tree, and the errors found on the way.
pub struct Parse {
    green: Rc<GreenNode>,
    errors: Vec<SyntaxError>,
}

impl Parse {
    /// The root of the syntax tree.
    pub fn syntax_node(&self) -> SyntaxNode {
        SyntaxNode::new_root(Rc::clone(&self.green))
    }

    /// The root, seen as a typed AST node.
    pub fn tree(&self) -> ast::SourceFile {
        ast::SourceFile::cast(self.syntax_node()).expect("the root is a SOURCE_FILE")
    }

    /// The errors, in the order they appear in the text.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// The tree in rust-analyzer's text format, then the errors.
    pub fn debug_dump(&self) -> String {
        let mut out = self.syntax_node().debug_dump();
        for error in &self.errors {
            out += &format!("{error}\n");
        }
        out
    }
}

/// Parse a Maxim file.
pub fn parse(text: &str) -> Parse {
    let lexed = lexer::lex(text);
    let input = parser::Input::from_lexed(&lexed);
    let events = parser::parse(&input);
    let (green, parser_errors) = tree::build_tree(&lexed, events);

    let mut errors: Vec<SyntaxError> = lexed
        .errors()
        .iter()
        .map(|error| SyntaxError::new(error.msg.clone(), error.range))
        .collect();
    errors.extend(parser_errors);
    errors.extend(validation::validate(&SyntaxNode::new_root(Rc::clone(
        &green,
    ))));
    // One error per place: when the first goes wrong, what follows at the
    // same token usually follows from it. The sort is stable, so the
    // lexer's errors come first, then the parser's.
    errors.sort_by_key(|error| error.range.start());
    errors.dedup_by_key(|error| error.range.start());
    Parse { green, errors }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_come_in_order_one_per_place() {
        // The lexer's error for `@` and the parser's at the same place:
        // only the lexer's is kept.
        let parse = parse("module M\nlet x = @\nlet y = )");
        let errors: Vec<String> = parse.errors().iter().map(|e| e.to_string()).collect();
        assert_eq!(
            errors,
            [
                "error 17..18: unrecognized character `@`",
                "error 27..28: expected an expression",
            ]
        );
    }

    #[test]
    fn the_tree_is_the_text() {
        let text = "module M\n\n// hi\nfn f( ) { 1 +\n  2 } @ ";
        assert_eq!(parse(text).syntax_node().text(), text);
    }
}
