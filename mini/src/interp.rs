//! The IR interpreter: runs a program without compiling it any further.
//!
//! It defines what every program *means*: each backend, and every
//! optimization, is tested against it.

use std::collections::HashMap;

use crate::diagnostic::Diagnostic;
use crate::ir::{Function, Op, Temp, Value, eval_binary};

/// Run `function` with one value per input, in the order of
/// `function.inputs`. A division by zero is an error that points at the
/// division in the source.
pub fn run(function: &Function, inputs: &[i64]) -> Result<i64, Diagnostic> {
    assert_eq!(inputs.len(), function.inputs.len(), "one value per input");
    let mut machine = Machine {
        inputs,
        temps: HashMap::new(),
    };
    for inst in &function.body {
        let result = match inst.op {
            Op::Neg(a) => machine.value(a).wrapping_neg(),
            Op::Binary(op, a, b) => match eval_binary(op, machine.value(a), machine.value(b)) {
                Some(result) => result,
                None => return Err(Diagnostic::new("division by zero", inst.span)),
            },
        };
        machine.temps.insert(inst.dest, result);
    }
    Ok(machine.value(function.result))
}

/// The interpreter's state while it runs a function.
struct Machine<'a> {
    inputs: &'a [i64],
    /// The value of each temporary computed so far.
    temps: HashMap<Temp, i64>,
}

impl Machine<'_> {
    /// The current value of an operand.
    fn value(&self, value: Value) -> i64 {
        match value {
            Value::Const(c) => c,
            Value::Input(i) => self.inputs[i],
            Value::Temp(t) => self.temps[&t],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::lower::lower;
    use crate::parser::parse;
    use crate::span::Span;

    fn eval(source: &str, inputs: &[i64]) -> Result<i64, Diagnostic> {
        run(&lower(&parse(&lex(source).unwrap()).unwrap()), inputs)
    }

    #[test]
    fn value_reads_constants_inputs_and_temps() {
        let mut machine = Machine {
            inputs: &[10, 20],
            temps: HashMap::new(),
        };
        machine.temps.insert(Temp(3), 30);
        assert_eq!(machine.value(Value::Const(-5)), -5);
        assert_eq!(machine.value(Value::Input(1)), 20);
        assert_eq!(machine.value(Value::Temp(Temp(3))), 30);
    }

    #[test]
    fn run_constants() {
        assert_eq!(eval("42", &[]), Ok(42));
        assert_eq!(eval("(1 + 2) * 3 - 10 / 4", &[]), Ok(7));
        assert_eq!(eval("-(2 * 3) + - -1", &[]), Ok(-5));
    }

    #[test]
    fn run_with_inputs() {
        assert_eq!(eval("x * (2 + 3) + y * 0", &[4, 100]), Ok(20));
        assert_eq!(eval("total / count", &[10, 3]), Ok(3));
        assert_eq!(eval("b - a + b", &[1, 10]), Ok(-8), "b comes first");
    }

    #[test]
    fn run_wraps_on_overflow() {
        assert_eq!(eval("x + 1", &[i64::MAX]), Ok(i64::MIN));
        assert_eq!(eval("-x", &[i64::MIN]), Ok(i64::MIN));
    }

    #[test]
    fn run_reports_division_by_zero_where_it_happens() {
        assert_eq!(
            eval("1 + x / y", &[5, 0]),
            Err(Diagnostic::new("division by zero", Span::new(4, 9)))
        );
    }
}
