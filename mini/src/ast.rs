//! The abstract syntax tree (AST): the structure of a program as the
//! parser understood it.
//!
//! "Abstract" because it drops what only mattered for reading the text:
//! whitespace, and parentheses — the shape of the tree already says what
//! groups with what.

use crate::span::Span;
use std::fmt;

/// An expression, with the span of source it was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// An integer literal: `42`.
    Int(i64),
    /// An input: `x`. The tree owns its copy of the name, so it can
    /// outlive the source text and the tokens.
    Var(String),
    /// Negation: `-operand`.
    Neg(Box<Expr>),
    /// A binary operation: `lhs op rhs`.
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// The binary operators. `Hash` lets the optimizer use them in hash
/// map keys (step 2, Part 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinOp {
    /// How the operator is written: `+`, `-`, `*` or `/`.
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
        }
    }
}

impl Expr {
    /// An integer literal.
    pub fn int(value: i64, span: Span) -> Expr {
        Expr {
            kind: ExprKind::Int(value),
            span,
        }
    }

    /// An input, read by name.
    pub fn var(name: &str, span: Span) -> Expr {
        Expr {
            kind: ExprKind::Var(name.to_string()),
            span,
        }
    }

    /// `-operand`, where `minus` is the span of the `-` sign.
    pub fn neg(minus: Span, operand: Expr) -> Expr {
        let span = minus.merge(operand.span);
        Expr {
            kind: ExprKind::Neg(Box::new(operand)),
            span,
        }
    }

    /// `lhs op rhs`, spanning from the start of `lhs` to the end of `rhs`.
    pub fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        let span = lhs.span.merge(rhs.span);
        Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            span,
        }
    }
}

/// Print the tree as an S-expression: `1 + 2 * 3` prints as
/// `(+ 1 (* 2 3))` and `-1` as `(neg 1)`. It shows the tree's shape at a
/// glance, which is exactly what parser tests need to check.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExprKind::Int(value) => write!(f, "{value}"),
            ExprKind::Var(name) => write!(f, "{name}"),
            ExprKind::Neg(operand) => write!(f, "(neg {operand})"),
            ExprKind::Binary { op, lhs, rhs } => write!(f, "({} {lhs} {rhs})", op.symbol()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Spans don't matter for printing, so every node gets a dummy one.
    fn int(value: i64) -> Expr {
        Expr::int(value, Span::new(0, 0))
    }

    fn neg(operand: Expr) -> Expr {
        Expr::neg(Span::new(0, 0), operand)
    }

    #[test]
    fn display_int() {
        assert_eq!(int(42).to_string(), "42");
        assert_eq!(int(-7).to_string(), "-7");
    }

    #[test]
    fn display_var() {
        assert_eq!(Expr::var("width", Span::new(0, 5)).to_string(), "width");
        let sum = Expr::binary(BinOp::Add, Expr::var("x", Span::new(0, 1)), int(1));
        assert_eq!(sum.to_string(), "(+ x 1)");
    }

    #[test]
    fn display_neg() {
        assert_eq!(neg(int(1)).to_string(), "(neg 1)");
    }

    #[test]
    fn display_binary() {
        assert_eq!(
            Expr::binary(BinOp::Add, int(1), int(2)).to_string(),
            "(+ 1 2)"
        );
        assert_eq!(
            Expr::binary(BinOp::Div, int(1), int(2)).to_string(),
            "(/ 1 2)"
        );
    }

    #[test]
    fn display_nested() {
        let sum = Expr::binary(BinOp::Add, int(1), int(2));
        let product = Expr::binary(BinOp::Mul, sum, neg(int(3)));
        assert_eq!(product.to_string(), "(* (+ 1 2) (neg 3))");
    }
}
