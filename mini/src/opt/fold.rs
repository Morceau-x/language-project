//! Constant folding and algebraic simplification.
//!
//! `add 2, 3` is `5`, whatever the inputs: compute it now, once, instead
//! of every time the program runs. And `mul %x, 1` is just `%x`.

use super::{Pass, replace_uses};
use crate::ast::BinOp;
use crate::ir::{Function, Op, Value, eval_binary};

/// Replaces every instruction that `simplify` can simplify.
pub struct Fold;

impl Pass for Fold {
    fn name(&self) -> &'static str {
        "fold"
    }

    fn run(&self, function: &mut Function) -> bool {
        replace_uses(function, |inst| simplify(inst.op))
    }
}

/// The value `op` computes, if it can be known without running the
/// program: a constant, or one of its own operands.
///
/// A division may fail, but that's not this function's concern: `Fold`
/// only replaces the *uses* of an instruction's result, and `Dce` never
/// removes an instruction that can fail. So the value returned for a
/// division only has to be right when the division succeeds.
pub fn simplify(op: Op) -> Option<Value> {
    use BinOp::{Add, Div, Mul, Sub};
    use Value::Const;
    match op {
        // Constant folding, with the same arithmetic as the interpreter.
        Op::Neg(Const(a)) => Some(Const(a.wrapping_neg())),
        Op::Binary(op, Const(a), Const(b)) => eval_binary(op, a, b).map(Const),
        // Algebraic identities.
        Op::Binary(Add, x, Const(0)) | Op::Binary(Add, Const(0), x) => Some(x),
        Op::Binary(Sub, x, Const(0)) => Some(x),
        Op::Binary(Sub, x, y) if x == y => Some(Const(0)),
        Op::Binary(Mul, x, Const(1)) | Op::Binary(Mul, Const(1), x) => Some(x),
        Op::Binary(Mul, _, Const(0)) | Op::Binary(Mul, Const(0), _) => Some(Const(0)),
        Op::Binary(Div, x, Const(1)) => Some(x),
        // Only right when the division succeeds, which is all we need.
        Op::Binary(Div, x, y) if x == y => Some(Const(1)),
        Op::Binary(Div, Const(0), _) => Some(Const(0)),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Temp;
    use crate::lexer::lex;
    use crate::lower::lower;
    use crate::parser::parse;

    const X: Value = Value::Input(0);
    const Y: Value = Value::Temp(Temp(7));

    fn c(value: i64) -> Value {
        Value::Const(value)
    }

    fn bin(op: BinOp, a: Value, b: Value) -> Op {
        Op::Binary(op, a, b)
    }

    #[test]
    fn simplify_folds_constants() {
        assert_eq!(simplify(bin(BinOp::Add, c(2), c(3))), Some(c(5)));
        assert_eq!(simplify(bin(BinOp::Sub, c(2), c(3))), Some(c(-1)));
        assert_eq!(simplify(bin(BinOp::Mul, c(4), c(-3))), Some(c(-12)));
        assert_eq!(simplify(bin(BinOp::Div, c(-7), c(2))), Some(c(-3)));
        assert_eq!(simplify(Op::Neg(c(5))), Some(c(-5)));
    }

    #[test]
    fn simplify_folds_with_wrapping_arithmetic() {
        assert_eq!(
            simplify(bin(BinOp::Add, c(i64::MAX), c(1))),
            Some(c(i64::MIN))
        );
        assert_eq!(simplify(Op::Neg(c(i64::MIN))), Some(c(i64::MIN)));
    }

    #[test]
    fn simplify_never_folds_a_division_by_zero() {
        assert_eq!(simplify(bin(BinOp::Div, c(1), c(0))), None);
    }

    #[test]
    fn simplify_applies_identities() {
        assert_eq!(simplify(bin(BinOp::Add, X, c(0))), Some(X));
        assert_eq!(simplify(bin(BinOp::Add, c(0), Y)), Some(Y));
        assert_eq!(simplify(bin(BinOp::Sub, X, c(0))), Some(X));
        assert_eq!(simplify(bin(BinOp::Sub, Y, Y)), Some(c(0)));
        assert_eq!(simplify(bin(BinOp::Mul, X, c(1))), Some(X));
        assert_eq!(simplify(bin(BinOp::Mul, c(1), Y)), Some(Y));
        assert_eq!(simplify(bin(BinOp::Mul, X, c(0))), Some(c(0)));
        assert_eq!(simplify(bin(BinOp::Mul, c(0), Y)), Some(c(0)));
        assert_eq!(simplify(bin(BinOp::Div, X, c(1))), Some(X));
    }

    #[test]
    fn simplify_assumes_divisions_succeed() {
        // When x is 0, these divisions fail, and the program with them:
        // `Dce` keeps them. Otherwise, their values are 1 and 0.
        assert_eq!(simplify(bin(BinOp::Div, X, X)), Some(c(1)));
        assert_eq!(simplify(bin(BinOp::Div, Y, Y)), Some(c(1)));
        assert_eq!(simplify(bin(BinOp::Div, c(0), X)), Some(c(0)));
    }

    #[test]
    fn simplify_leaves_everything_else() {
        assert_eq!(simplify(bin(BinOp::Add, X, c(1))), None);
        assert_eq!(simplify(bin(BinOp::Sub, c(0), X)), None);
        assert_eq!(simplify(bin(BinOp::Mul, X, Y)), None);
        assert_eq!(simplify(bin(BinOp::Div, X, Y)), None);
        assert_eq!(simplify(bin(BinOp::Div, c(1), X)), None);
        assert_eq!(simplify(Op::Neg(X)), None);
    }

    #[test]
    fn fold_replaces_uses_and_leaves_dead_instructions() {
        let mut function = lower(&parse(&lex("x * (2 + 3)").unwrap()).unwrap());
        assert!(Fold.run(&mut function));
        assert_eq!(
            function.to_string(),
            "fn main(%x) {\n  %0 = add 2, 3\n  %1 = mul %x, 5\n  ret %1\n}\n"
        );
        assert!(!Fold.run(&mut function), "a second run has nothing to do");
    }

    #[test]
    fn fold_follows_chains_in_one_run() {
        let mut function = lower(&parse(&lex("(1 + 2) * 3 - 10 / 4").unwrap()).unwrap());
        Fold.run(&mut function);
        assert_eq!(function.result, Value::Const(7));
    }
}
