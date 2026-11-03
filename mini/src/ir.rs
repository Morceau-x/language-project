//! The intermediate representation (IR): the program as a flat list of
//! simple instructions, halfway between the AST and machine code.
//!
//! ```text
//! x * (2 + 3)            fn main(%x) {
//!                          %0 = add 2, 3
//!                          %1 = mul %x, %0
//!                          ret %1
//!                        }
//! ```
//!
//! Two ideas shape it, and LLVM's IR is built on the same two:
//!
//! - **Three-address code**: an instruction applies one operator to at
//!   most two operands and names its result. No nesting: every
//!   intermediate value has a name.
//! - **SSA form** (static single assignment): every temporary is
//!   assigned exactly once. `%1` means the same value everywhere, so a
//!   pass can replace it, or reason about it, without looking at the
//!   order of anything.

use std::fmt;

use crate::ast::BinOp;
use crate::span::Span;

/// A compiled Mini program: one function, from its inputs to a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The input names, in the order the program takes them.
    /// `Value::Input(i)` reads `inputs[i]`.
    pub inputs: Vec<String>,
    /// The instructions, in the order they run.
    pub body: Vec<Inst>,
    /// What the program returns.
    pub result: Value,
}

/// One instruction: `dest = op`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inst {
    pub dest: Temp,
    pub op: Op,
    /// The source this instruction was lowered from, for runtime errors.
    pub span: Span,
}

/// What an instruction computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    /// `neg a`: `-a`.
    Neg(Value),
    /// `add a, b`, `sub a, b`, `mul a, b`, `div a, b`.
    Binary(BinOp, Value, Value),
}

/// An operand: what an instruction reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Value {
    /// A constant: `42`.
    Const(i64),
    /// One of the program's inputs: `%x`.
    Input(usize),
    /// The result of an earlier instruction: `%3`.
    Temp(Temp),
}

/// A temporary: the name of one instruction's result, printed `%3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Temp(pub u32);

impl fmt::Display for Temp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

/// The IR name of a binary operator.
pub fn mnemonic(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
    }
}

/// Mini's arithmetic on 64-bit integers, in one place: the interpreter
/// runs it and the constant folder runs it at compile time, so they
/// can't disagree.
///
/// Overflow wraps around (two's complement). Division rounds toward
/// zero; `i64::MIN / -1` wraps to `i64::MIN`. Division by zero has no
/// result: `None`.
pub fn eval_binary(op: BinOp, a: i64, b: i64) -> Option<i64> {
    match op {
        BinOp::Add => Some(a.wrapping_add(b)),
        BinOp::Sub => Some(a.wrapping_sub(b)),
        BinOp::Mul => Some(a.wrapping_mul(b)),
        BinOp::Div if b == 0 => None,
        BinOp::Div => Some(a.wrapping_div(b)),
    }
}

impl Op {
    /// The operands, in order.
    pub fn operands(self) -> Vec<Value> {
        match self {
            Op::Neg(a) => vec![a],
            Op::Binary(_, a, b) => vec![a, b],
        }
    }

    /// Replace every operand `v` with `f(v)`.
    pub fn map_operands(&mut self, mut f: impl FnMut(Value) -> Value) {
        match self {
            Op::Neg(a) => *a = f(*a),
            Op::Binary(_, a, b) => {
                *a = f(*a);
                *b = f(*b);
            }
        }
    }

    /// Can running this instruction fail? Only a division can, when its
    /// divisor is zero. Such an instruction must run even if nothing
    /// uses its result: removing it would remove the error.
    pub fn can_trap(self) -> bool {
        match self {
            Op::Binary(BinOp::Div, _, Value::Const(divisor)) => divisor == 0,
            Op::Binary(BinOp::Div, _, _) => true,
            _ => false,
        }
    }
}

impl Function {
    /// How `value` is written in the IR: `42`, `%x` or `%3`.
    pub fn show(&self, value: Value) -> String {
        match value {
            Value::Const(i) => format!("{}", i),
            Value::Input(i) => format!("%{}", self.inputs[i]),
            Value::Temp(i) => format!("{}", i),
        }
    }
}

/// Print the function in the IR's text format:
///
/// ```text
/// fn main(%x, %y) {
///   %0 = add 2, 3
///   %1 = neg %y
///   %2 = mul %x, %0
///   ret %2
/// }
/// ```
impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inputs: Vec<String> = self.inputs.iter().map(|name| format!("%{name}")).collect();
        writeln!(f, "fn main({}) {{", inputs.join(", "))?;
        for inst in &self.body {
            match inst.op {
                Op::Neg(a) => writeln!(f, "  {} = neg {}", inst.dest, self.show(a))?,
                Op::Binary(op, a, b) => writeln!(
                    f,
                    "  {} = {} {}, {}",
                    inst.dest,
                    mnemonic(op),
                    self.show(a),
                    self.show(b)
                )?,
            }
        }
        writeln!(f, "  ret {}", self.show(self.result))?;
        writeln!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(n: u32, op: Op) -> Inst {
        Inst {
            dest: Temp(n),
            op,
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn show_values() {
        let function = Function {
            inputs: vec!["x".to_string(), "y".to_string()],
            body: vec![],
            result: Value::Const(0),
        };
        assert_eq!(function.show(Value::Const(-7)), "-7");
        assert_eq!(function.show(Value::Input(1)), "%y");
        assert_eq!(function.show(Value::Temp(Temp(3))), "%3");
    }

    #[test]
    fn display_a_function() {
        let function = Function {
            inputs: vec!["x".to_string(), "y".to_string()],
            body: vec![
                inst(0, Op::Binary(BinOp::Add, Value::Const(2), Value::Const(3))),
                inst(1, Op::Neg(Value::Input(1))),
                inst(
                    2,
                    Op::Binary(BinOp::Div, Value::Input(0), Value::Temp(Temp(0))),
                ),
            ],
            result: Value::Temp(Temp(2)),
        };
        assert_eq!(
            function.to_string(),
            "\
fn main(%x, %y) {
  %0 = add 2, 3
  %1 = neg %y
  %2 = div %x, %0
  ret %2
}
"
        );
    }

    #[test]
    fn display_a_function_without_inputs_or_instructions() {
        let function = Function {
            inputs: vec![],
            body: vec![],
            result: Value::Const(42),
        };
        assert_eq!(function.to_string(), "fn main() {\n  ret 42\n}\n");
    }

    #[test]
    fn eval_binary_computes() {
        assert_eq!(eval_binary(BinOp::Add, 2, 3), Some(5));
        assert_eq!(eval_binary(BinOp::Sub, 2, 3), Some(-1));
        assert_eq!(eval_binary(BinOp::Mul, -2, 3), Some(-6));
        assert_eq!(eval_binary(BinOp::Div, 7, 2), Some(3));
    }

    #[test]
    fn eval_binary_division_rounds_toward_zero() {
        assert_eq!(eval_binary(BinOp::Div, -7, 2), Some(-3));
        assert_eq!(eval_binary(BinOp::Div, 7, -2), Some(-3));
    }

    #[test]
    fn eval_binary_division_by_zero_has_no_result() {
        assert_eq!(eval_binary(BinOp::Div, 1, 0), None);
        assert_eq!(eval_binary(BinOp::Div, 0, 0), None);
    }

    #[test]
    fn eval_binary_overflow_wraps_around() {
        assert_eq!(eval_binary(BinOp::Add, i64::MAX, 1), Some(i64::MIN));
        assert_eq!(eval_binary(BinOp::Sub, i64::MIN, 1), Some(i64::MAX));
        assert_eq!(eval_binary(BinOp::Mul, i64::MAX, 2), Some(-2));
        assert_eq!(eval_binary(BinOp::Div, i64::MIN, -1), Some(i64::MIN));
    }

    #[test]
    fn can_trap_only_for_divisions_that_might_divide_by_zero() {
        let x = Value::Input(0);
        assert!(Op::Binary(BinOp::Div, x, x).can_trap());
        assert!(Op::Binary(BinOp::Div, x, Value::Const(0)).can_trap());
        assert!(!Op::Binary(BinOp::Div, x, Value::Const(2)).can_trap());
        assert!(!Op::Binary(BinOp::Div, x, Value::Const(-1)).can_trap());
        assert!(!Op::Binary(BinOp::Mul, x, x).can_trap());
        assert!(!Op::Neg(x).can_trap());
    }
}
