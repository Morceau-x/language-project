//! Dead code elimination (DCE): prune the instructions whose result
//! nobody uses.
//!
//! Other passes leave such instructions behind: once `Fold` has replaced
//! every use of `%0 = add 2, 3` by `5`, the instruction is dead weight.

use std::collections::HashSet;

use crate::ir::{Function, Value};

use super::Pass;

/// Removes instructions whose result is never used and that can't fail.
pub struct Dce;

impl Pass for Dce {
    fn name(&self) -> &'static str {
        "dce"
    }

    fn run(&self, function: &mut Function) -> bool {
        todo!("Part 6d: Dce::run")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::lower::lower;
    use crate::opt::Fold;
    use crate::parser::parse;

    /// Lower `source`, then run `Fold` and `Dce` once each.
    fn fold_then_dce(source: &str) -> (Function, bool) {
        let mut function = lower(&parse(&lex(source).unwrap()).unwrap());
        Fold.run(&mut function);
        let changed = Dce.run(&mut function);
        (function, changed)
    }

    #[test]
    fn dce_removes_unused_instructions() {
        let (function, changed) = fold_then_dce("x * (2 + 3)");
        assert!(changed);
        assert_eq!(
            function.to_string(),
            "fn main(%x) {\n  %1 = mul %x, 5\n  ret %1\n}\n"
        );
    }

    #[test]
    fn dce_removes_whole_chains() {
        let (function, _) = fold_then_dce("(x + 1) * 2 * 0");
        assert_eq!(function.to_string(), "fn main(%x) {\n  ret 0\n}\n");
    }

    #[test]
    fn dce_keeps_divisions_that_could_fail() {
        let (function, _) = fold_then_dce("0 * (x / y)");
        assert_eq!(
            function.to_string(),
            "fn main(%x, %y) {\n  %0 = div %x, %y\n  ret 0\n}\n"
        );
    }

    #[test]
    fn dce_keeps_a_division_whose_value_fold_already_knows() {
        let (function, _) = fold_then_dce("x / x");
        assert_eq!(
            function.to_string(),
            "fn main(%x) {\n  %0 = div %x, %x\n  ret 1\n}\n"
        );
    }

    #[test]
    fn dce_removes_divisions_that_cannot_fail() {
        let (function, _) = fold_then_dce("0 * (x / 2)");
        assert_eq!(function.to_string(), "fn main(%x) {\n  ret 0\n}\n");
    }

    #[test]
    fn dce_reports_when_nothing_is_dead() {
        let mut function = lower(&parse(&lex("x * y + 1").unwrap()).unwrap());
        assert!(!Dce.run(&mut function));
    }
}
