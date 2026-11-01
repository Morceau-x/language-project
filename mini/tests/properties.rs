//! Property tests: instead of checking a few examples, generate hundreds
//! of random programs and check a property that must hold for all of
//! them. When one fails, proptest *shrinks* it: it searches for the
//! smallest program that still fails, and shows you that one.
//!
//! Run with `cargo test -p mini --test properties`.

use mini::ast::{BinOp, Expr, ExprKind};
use mini::driver::{self, Frontend};
use mini::{interp, lower, opt};
use proptest::prelude::*;

/// Random syntactically valid programs. Their meaning doesn't matter:
/// any `a op b`, `-a` or `(a)` of valid programs is a valid program.
fn program() -> impl Strategy<Value = String> {
    // Small numbers and few names, so that interesting coincidences
    // (`x - x`, a zero divisor, the same subexpression twice) are common.
    let leaf = prop_oneof![
        3 => (0i64..3).prop_map(|n| n.to_string()),
        1 => (0i64..100).prop_map(|n| n.to_string()),
        1 => Just(i64::MAX.to_string()),
        4 => prop::sample::select(vec!["x", "y"]).prop_map(String::from),
    ];
    leaf.prop_recursive(5, 32, 3, |inner| {
        prop_oneof![
            4 => (
                inner.clone(),
                prop::sample::select(vec!["+", "-", "*", "/"]),
                inner.clone()
            )
                .prop_map(|(a, op, b)| format!("{a} {op} {b}")),
            1 => inner.clone().prop_map(|a| format!("-{a}")),
            1 => inner.prop_map(|a| format!("({a})")),
        ]
    })
}

/// Random sequences of tokens: mostly not valid programs.
fn token_soup() -> impl Strategy<Value = String> {
    let token = prop::sample::select(vec!["1", "42", "x", "y", "+", "-", "*", "/", "(", ")"]);
    prop::collection::vec(token, 0..8).prop_map(|tokens| tokens.join(" "))
}

/// Inputs worth trying: small numbers, and the edges where arithmetic
/// wraps around or fails.
fn inputs() -> impl Strategy<Value = Vec<i64>> {
    let input = prop_oneof![
        4 => -1i64..=1,
        1 => Just(i64::MIN),
        1 => Just(i64::MAX),
        1 => any::<i64>(),
    ];
    prop::collection::vec(input, 2)
}

/// A reference evaluator, written directly on the AST and as simply as
/// possible. `inputs` follow the order of first appearance.
fn evaluate(expr: &Expr, names: &[String], inputs: &[i64]) -> Option<i64> {
    Some(match &expr.kind {
        ExprKind::Int(value) => *value,
        ExprKind::Var(name) => inputs[names.iter().position(|n| n == name).unwrap()],
        ExprKind::Neg(operand) => evaluate(operand, names, inputs)?.wrapping_neg(),
        ExprKind::Binary { op, lhs, rhs } => {
            let a = evaluate(lhs, names, inputs)?;
            let b = evaluate(rhs, names, inputs)?;
            match op {
                BinOp::Add => a.wrapping_add(b),
                BinOp::Sub => a.wrapping_sub(b),
                BinOp::Mul => a.wrapping_mul(b),
                BinOp::Div => a.checked_div(b).or((b == -1).then_some(a.wrapping_neg()))?,
            }
        }
    })
}

proptest! {
    /// The generated parser and the hand-written one agree on every valid
    /// program, down to the spans.
    #[test]
    fn both_front_ends_agree_on_programs(source in program()) {
        let hand_written = driver::parse(&source, Frontend::HandWritten);
        prop_assert!(hand_written.is_ok(), "{hand_written:?}");
        prop_assert_eq!(driver::parse(&source, Frontend::Generated), hand_written);
    }

    /// On random tokens, they agree on what's valid, and on its tree.
    #[test]
    fn both_front_ends_agree_on_token_soup(source in token_soup()) {
        let hand_written = driver::parse(&source, Frontend::HandWritten);
        let generated = driver::parse(&source, Frontend::Generated);
        prop_assert_eq!(hand_written.is_ok(), generated.is_ok(), "{:?} vs {:?}", hand_written, generated);
        if let (Ok(a), Ok(b)) = (hand_written, generated) {
            prop_assert_eq!(a, b);
        }
    }

    /// Lowering then interpreting computes what the AST means.
    #[test]
    fn the_ir_means_what_the_ast_means(source in program(), inputs in inputs()) {
        let expr = driver::parse(&source, Frontend::HandWritten).unwrap();
        let function = lower::lower(&expr);
        let inputs = &inputs[..function.inputs.len()];
        let expected = evaluate(&expr, &function.inputs, inputs);
        let actual = interp::run(&function, inputs).ok();
        prop_assert_eq!(actual, expected, "{}", function);
    }

    /// The optimizer never changes what a program computes, nor where it
    /// fails.
    #[test]
    fn optimizing_preserves_meaning(source in program(), inputs in inputs()) {
        let expr = driver::parse(&source, Frontend::HandWritten).unwrap();
        let original = lower::lower(&expr);
        let mut optimized = original.clone();
        opt::optimize(&mut optimized);
        let inputs = &inputs[..original.inputs.len()];
        prop_assert_eq!(
            interp::run(&optimized, inputs),
            interp::run(&original, inputs),
            "before:\n{}after:\n{}", original, optimized
        );
    }

    /// The optimizer never makes a program longer.
    #[test]
    fn optimizing_never_adds_instructions(source in program()) {
        let original = lower::lower(&driver::parse(&source, Frontend::HandWritten).unwrap());
        let mut optimized = original.clone();
        opt::optimize(&mut optimized);
        prop_assert!(optimized.body.len() <= original.body.len());
    }
}
