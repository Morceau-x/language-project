//! Code generation: turns the AST into JavaScript source code.
//!
//! The job is to keep Mini's *meaning*, not to copy its syntax. Two
//! places where they differ:
//!
//! - Parentheses: the AST has none, so the generator must add them back
//!   wherever JavaScript would otherwise group things differently. The
//!   simplest safe rule is to wrap every operation in parentheses.
//! - Division: Mini divides integers like Rust (`7 / 2` is `3`), while
//!   JavaScript's `/` gives `3.5`, so it is wrapped in `Math.trunc(...)`.

use crate::ast::{BinOp, Expr, ExprKind};

/// The JavaScript program for a Mini program: print its value.
pub fn generate(expr: &Expr) -> String {
    let mut out = String::new();
    out.push_str("console.log(");
    emit_expr(expr, &mut out);
    out.push_str(");\n");
    out
}

/// Append the JavaScript for `expr` to `out`.
fn emit_expr(expr: &Expr, out: &mut String) {
    todo!("Part 5: emit_expr")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    /// The JavaScript for one Mini expression (parsed with your parser).
    fn js(source: &str) -> String {
        let expr = parse(&lex(source).unwrap()).unwrap();
        let mut out = String::new();
        emit_expr(&expr, &mut out);
        out
    }

    #[test]
    fn emit_int() {
        assert_eq!(js("42"), "42");
    }

    #[test]
    fn emit_neg() {
        assert_eq!(js("-1"), "(-1)");
    }

    #[test]
    fn emit_neg_twice_never_writes_the_decrement_operator() {
        // `--1` would be JavaScript's decrement operator, a syntax error here.
        assert_eq!(js("- -1"), "(-(-1))");
    }

    #[test]
    fn emit_binary() {
        assert_eq!(js("1 + 2"), "(1 + 2)");
        assert_eq!(js("1 - 2"), "(1 - 2)");
        assert_eq!(js("1 * 2"), "(1 * 2)");
    }

    #[test]
    fn emit_division_truncates() {
        assert_eq!(js("7 / 2"), "Math.trunc(7 / 2)");
    }

    #[test]
    fn emit_keeps_the_grouping_of_the_tree() {
        assert_eq!(js("(1 + 2) * 3"), "((1 + 2) * 3)");
        assert_eq!(js("1 - (2 - 3)"), "(1 - (2 - 3))");
        assert_eq!(js("-(4 / 2)"), "(-Math.trunc(4 / 2))");
    }

    #[test]
    fn generate_prints_the_result() {
        let expr = parse(&lex("1 + 2").unwrap()).unwrap();
        assert_eq!(generate(&expr), "console.log((1 + 2));\n");
    }
}
