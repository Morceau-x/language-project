//! Common subexpression elimination (CSE).
//!
//! In `(x + 1) * (x + 1)`, `x + 1` is computed twice. In SSA form, two
//! instructions with the same operator and the same operands always
//! compute the same value, so the second one can reuse the first one's
//! result.

use std::collections::HashMap;

use crate::ast::BinOp;
use crate::ir::{Function, Op, Temp, Value};

use super::{Pass, replace_uses};

/// Replaces each instruction that repeats an earlier one.
pub struct Cse;

impl Pass for Cse {
    fn name(&self) -> &'static str {
        "cse"
    }

    fn run(&self, function: &mut Function) -> bool {
        todo!("Part 6c: Cse::run")
    }
}

/// `op` written in a standard form, so that `add %x, %y` and
/// `add %y, %x` look the same: the operands of `add` and `mul` (whose
/// order doesn't matter) are sorted.
fn canonical(op: Op) -> Op {
    todo!("Part 6c: canonical")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::lower::lower;
    use crate::parser::parse;

    fn lowered(source: &str) -> Function {
        lower(&parse(&lex(source).unwrap()).unwrap())
    }

    #[test]
    fn canonical_sorts_operands_when_order_does_not_matter() {
        let (x, y) = (Value::Input(0), Value::Input(1));
        assert_eq!(
            canonical(Op::Binary(BinOp::Add, y, x)),
            canonical(Op::Binary(BinOp::Add, x, y))
        );
        assert_eq!(
            canonical(Op::Binary(BinOp::Mul, y, x)),
            canonical(Op::Binary(BinOp::Mul, x, y))
        );
        assert_ne!(
            canonical(Op::Binary(BinOp::Sub, y, x)),
            canonical(Op::Binary(BinOp::Sub, x, y))
        );
    }

    #[test]
    fn cse_reuses_an_earlier_result() {
        let mut function = lowered("(x + 1) * (x + 1)");
        assert!(Cse.run(&mut function));
        assert_eq!(
            function.to_string(),
            "\
fn main(%x) {
  %0 = add %x, 1
  %1 = add %x, 1
  %2 = mul %0, %0
  ret %2
}
"
        );
    }

    #[test]
    fn cse_knows_that_order_does_not_matter_for_add_and_mul() {
        let mut function = lowered("x * y - y * x");
        assert!(Cse.run(&mut function));
        assert!(
            function.to_string().contains("%2 = sub %0, %0"),
            "{function}"
        );
    }

    #[test]
    fn cse_knows_that_order_matters_for_sub_and_div() {
        let mut function = lowered("(x - y) * (y - x) + x / y * (y / x)");
        assert!(!Cse.run(&mut function), "{function}");
    }

    #[test]
    fn cse_follows_chains_in_one_run() {
        // The second `%2` repeats `%0` only once its operand `%1` is
        // known to be `%0`.
        let mut function = lowered("(x + 1) * 2 - (x + 1) * 2");
        Cse.run(&mut function);
        assert!(
            function.to_string().contains("%4 = sub %1, %1"),
            "{function}"
        );
    }
}
