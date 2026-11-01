//! The optimizer: passes that rewrite the IR into a smaller program that
//! computes exactly the same thing, errors included.
//!
//! Each pass does one small job and reports whether it changed anything.
//! The pass manager runs them in a loop until a whole round changes
//! nothing, because one pass often creates work for another.

use std::collections::HashMap;

use crate::ir::{Function, Inst, Temp, Value};

mod cse;
mod dce;
mod fold;

pub use cse::Cse;
pub use dce::Dce;
pub use fold::{Fold, simplify};

/// One optimization pass.
pub trait Pass {
    /// A short name, shown by `--print-after-each`.
    fn name(&self) -> &'static str;

    /// Rewrite `function`, and say whether anything changed.
    fn run(&self, function: &mut Function) -> bool;
}

/// Runs a list of passes until they stop changing the function.
pub struct PassManager {
    passes: Vec<Box<dyn Pass>>,
}

/// Safety net: a buggy pair of passes could undo each other forever.
const MAX_ROUNDS: usize = 10;

impl PassManager {
    /// A pass manager with no passes.
    pub fn new() -> PassManager {
        PassManager { passes: Vec::new() }
    }

    /// Add a pass at the end of the list.
    pub fn with(mut self, pass: impl Pass + 'static) -> PassManager {
        self.passes.push(Box::new(pass));
        self
    }

    /// The passes `-O` runs.
    pub fn standard() -> PassManager {
        PassManager::new().with(Fold).with(Cse).with(Dce)
    }

    /// Run every pass in order, and again, until a whole round changes
    /// nothing (or `MAX_ROUNDS` is reached). After each pass that changed
    /// the function, call `trace` with the pass's name and the function.
    pub fn run(&self, function: &mut Function, trace: &mut dyn FnMut(&str, &Function)) {
        todo!("Part 6e: PassManager::run")
    }
}

impl Default for PassManager {
    fn default() -> PassManager {
        PassManager::new()
    }
}

/// Optimize `function` with the standard passes.
pub fn optimize(function: &mut Function) {
    PassManager::standard().run(function, &mut |_, _| {});
}

/// The engine behind `Fold` and `Cse`: "replace all uses with".
///
/// Walks the instructions in order. First, each instruction's operands
/// are rewritten: a temporary that was replaced becomes its replacement.
/// Then `replacement` says whether the instruction itself can be
/// replaced by a value; if so, every later use of its result will be.
/// The instruction itself stays: removing dead code is `Dce`'s job.
///
/// Returns whether any operand (or the result) changed.
fn replace_uses(
    function: &mut Function,
    mut replacement: impl FnMut(&Inst) -> Option<Value>,
) -> bool {
    let mut replaced: HashMap<Temp, Value> = HashMap::new();
    let resolve = |replaced: &HashMap<Temp, Value>, value: Value| match value {
        Value::Temp(temp) => replaced.get(&temp).copied().unwrap_or(value),
        _ => value,
    };
    let mut changed = false;
    for inst in &mut function.body {
        let before = inst.op;
        inst.op.map_operands(|value| resolve(&replaced, value));
        changed |= inst.op != before;
        if let Some(value) = replacement(inst) {
            replaced.insert(inst.dest, value);
        }
    }
    let result = resolve(&replaced, function.result);
    changed |= result != function.result;
    function.result = result;
    changed
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
    fn run_repeats_until_nothing_changes() {
        // CSE makes `%0 - %0`, which only a second round of folding sees.
        let mut function = lowered("(x + 1) - (x + 1)");
        PassManager::standard().run(&mut function, &mut |_, _| {});
        assert_eq!(function.to_string(), "fn main(%x) {\n  ret 0\n}\n");
    }

    #[test]
    fn run_traces_each_pass_that_changed_something() {
        let mut function = lowered("(x + 1) - (x + 1)");
        let mut names = Vec::new();
        PassManager::standard().run(&mut function, &mut |name, _| names.push(name.to_string()));
        assert_eq!(names, ["cse", "dce", "fold", "dce"]);
    }

    #[test]
    fn run_shows_the_function_after_each_pass() {
        let mut function = lowered("x * (2 + 3)");
        let mut log = String::new();
        PassManager::standard().run(&mut function, &mut |name, function| {
            log += &format!("; after {name}\n{function}");
        });
        assert_eq!(
            log,
            "\
; after fold
fn main(%x) {
  %0 = add 2, 3
  %1 = mul %x, 5
  ret %1
}
; after dce
fn main(%x) {
  %1 = mul %x, 5
  ret %1
}
"
        );
    }

    #[test]
    fn run_stops_when_there_is_nothing_to_do() {
        let mut function = lowered("x");
        let mut calls = 0;
        PassManager::standard().run(&mut function, &mut |_, _| calls += 1);
        assert_eq!(calls, 0);
        assert_eq!(function, lowered("x"));
    }

    #[test]
    fn run_gives_up_after_max_rounds() {
        /// A broken pass that claims to change something every time.
        struct Liar;
        impl Pass for Liar {
            fn name(&self) -> &'static str {
                "liar"
            }
            fn run(&self, _: &mut Function) -> bool {
                true
            }
        }
        let mut calls = 0;
        PassManager::new()
            .with(Liar)
            .run(&mut lowered("x"), &mut |_, _| calls += 1);
        assert_eq!(calls, MAX_ROUNDS);
    }
}
