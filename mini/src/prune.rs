//! Pruning: turn the parse tree (CST) into an abstract syntax tree.
//!
//! The parse tree records *how* the program was written; the AST keeps
//! only what it *means*. Pruning drops the parentheses (the tree's shape
//! already groups things), turns operator tokens into `BinOp`s, and
//! turns the text of literals into values.

use crate::ast::{BinOp, Expr};
use crate::cst::Cst;
use crate::diagnostic::Diagnostic;

/// Prune a parse tree from `yacc/mini.y` into an AST.
pub fn prune(tree: &Cst<'_>) -> Result<Expr, Diagnostic> {
    todo!("Part 4: prune")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;
    use crate::yacc::parse_cst;

    /// Parse with the generated parser, then prune.
    fn pruned(source: &str) -> Result<Expr, Diagnostic> {
        prune(&parse_cst(source).unwrap())
    }

    #[test]
    fn prune_literals_and_names() {
        assert_eq!(pruned("42"), Ok(Expr::int(42, Span::new(0, 2))));
        assert_eq!(pruned(" x"), Ok(Expr::var("x", Span::new(1, 2))));
    }

    #[test]
    fn prune_operators() {
        assert_eq!(pruned("1 + 2 * 3").unwrap().to_string(), "(+ 1 (* 2 3))");
        assert_eq!(pruned("x - y / 2").unwrap().to_string(), "(- x (/ y 2))");
        assert_eq!(pruned("-x").unwrap().to_string(), "(neg x)");
    }

    #[test]
    fn prune_drops_parentheses() {
        assert_eq!(pruned("((7))").unwrap().to_string(), "7");
        assert_eq!(pruned("(1 + 2) * 3").unwrap().to_string(), "(* (+ 1 2) 3)");
    }

    #[test]
    fn prune_keeps_the_span_of_the_parentheses() {
        let expr = pruned("(1 + 2) * 3").unwrap();
        assert_eq!(expr.span, Span::new(0, 11));
        let crate::ast::ExprKind::Binary { lhs, .. } = expr.kind else {
            panic!("expected a binary expression");
        };
        assert_eq!(
            lhs.span,
            Span::new(0, 7),
            "the span includes the parentheses"
        );
    }

    #[test]
    fn prune_rejects_literals_too_large_for_i64() {
        assert_eq!(
            pruned("1 + 99999999999999999999"),
            Err(Diagnostic::new(
                "integer literal is too large",
                Span::new(4, 24)
            ))
        );
    }

    #[test]
    fn prune_matches_the_hand_written_front_end() {
        let sources = [
            "1 + 2 * 3",
            "-(1 + x) * y",
            "x * (2 + 3) + y * 0",
            "- - 1",
            "8 / 4 / 2 - 1 - 2",
            "((a))",
            "-x - -y",
        ];
        for source in sources {
            let tokens = crate::lexer::lex(source).unwrap();
            let hand_written = crate::parser::parse(&tokens);
            assert_eq!(pruned(source), hand_written, "for `{source}`");
        }
    }
}
