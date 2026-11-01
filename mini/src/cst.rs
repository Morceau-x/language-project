//! Concrete syntax trees (CST), also called parse trees: the tree of
//! grammar rules the parser recognized, with every token kept.
//!
//! The generated parser (`yacc/`) builds one of these. Pruning it down
//! to an AST is a separate pass (`prune.rs`).

use std::fmt;

use crate::span::Span;

/// A parse tree. It keeps *everything* the parser saw: parentheses,
/// operators, the text of each literal. Nothing is interpreted yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cst<'src> {
    /// A token, exactly as written. `kind` is its name in the grammar:
    /// `INT`, `IDENT`, `+`, `(`…
    Token {
        kind: &'static str,
        text: &'src str,
        span: Span,
    },
    /// One use of a grammar rule (`rule` is its name, such as `Expr`),
    /// and the trees it was built from, in source order.
    Node {
        rule: &'static str,
        children: Vec<Cst<'src>>,
    },
}

impl Cst<'_> {
    /// The source covered by this tree: from its first token to its last.
    pub fn span(&self) -> Span {
        match self {
            Cst::Token { span, .. } => *span,
            Cst::Node { children, .. } => {
                let first = children.first().expect("a node has children").span();
                let last = children.last().expect("a node has children").span();
                first.merge(last)
            }
        }
    }
}

/// Draw the tree, one node per line:
///
/// ```text
/// Expr
/// ├── Expr
/// │   └── INT "1"
/// ├── "+"
/// └── Expr
///     └── IDENT "x"
/// ```
impl fmt::Display for Cst<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        draw(self, "", "", f)
    }
}

/// Draw `tree` after `first` (the branch leading to it), with `rest`
/// as the prefix of the lines below it.
fn draw(tree: &Cst<'_>, first: &str, rest: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match tree {
        Cst::Token { kind, text, .. } if kind == text => writeln!(f, "{first}{text:?}"),
        Cst::Token { kind, text, .. } => writeln!(f, "{first}{kind} {text:?}"),
        Cst::Node { rule, children } => {
            writeln!(f, "{first}{rule}")?;
            for (i, child) in children.iter().enumerate() {
                if i + 1 < children.len() {
                    draw(child, &format!("{rest}├── "), &format!("{rest}│   "), f)?;
                } else {
                    draw(child, &format!("{rest}└── "), &format!("{rest}    "), f)?;
                }
            }
            Ok(())
        }
    }
}
