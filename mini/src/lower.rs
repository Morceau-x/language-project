//! Lowering: translate the AST into the IR.
//!
//! A tree becomes a list: each operation node becomes one instruction,
//! emitted *after* the instructions for its operands, because they must
//! run first. Literals and inputs need no instruction: they are operands.

use crate::ast::{Expr, ExprKind};
use crate::ir::{Function, Inst, Op, Temp, Value};
use crate::span::Span;

/// Lower a whole program to an IR function.
pub fn lower(expr: &Expr) -> Function {
    let mut inputs = Vec::new();
    collect_inputs(expr, &mut inputs);
    let mut lowerer = Lowerer {
        inputs,
        body: Vec::new(),
    };
    let result = lowerer.lower_expr(expr);
    Function {
        inputs: lowerer.inputs,
        body: lowerer.body,
        result,
    }
}

/// Add the name of every input used in `expr` to `names`, in the order
/// they first appear in the source, without duplicates. That order is the
/// order the program takes its inputs on the command line.
fn collect_inputs(expr: &Expr, names: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Int(_) => (),
        ExprKind::Neg(operand) => collect_inputs(operand, names),
        ExprKind::Var(name) => {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_inputs(lhs, names);
            collect_inputs(rhs, names);
        }
    }
}

/// The function being built.
struct Lowerer {
    /// The input names, in order of first appearance.
    inputs: Vec<String>,
    /// The instructions emitted so far.
    body: Vec<Inst>,
}

impl Lowerer {
    /// Append the instruction `op` and return the temporary holding its
    /// result. Temporaries are numbered in order: `%0`, `%1`…
    fn emit(&mut self, op: Op, span: Span) -> Value {
        let dest = Temp(self.body.len() as u32);
        self.body.push(Inst { dest, op, span });
        Value::Temp(dest)
    }

    /// The `Value::Input` for the input called `name`.
    fn input(&self, name: &str) -> Value {
        let index = self
            .inputs
            .iter()
            .position(|input| input == name)
            .expect("collect_inputs found every input");
        Value::Input(index)
    }

    /// Emit the instructions that compute `expr`, and return the operand
    /// that holds its value.
    fn lower_expr(&mut self, expr: &Expr) -> Value {
        match &expr.kind {
            ExprKind::Int(value) => Value::Const(*value),
            ExprKind::Var(name) => self.input(name),
            ExprKind::Neg(operand) => {
                let operand = self.lower_expr(operand);
                self.emit(Op::Neg(operand), expr.span)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                self.emit(Op::Binary(*op, lhs, rhs), expr.span)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::BinOp;
    use crate::lexer::lex;
    use crate::parser::parse;

    fn lowered(source: &str) -> Function {
        lower(&parse(&lex(source).unwrap()).unwrap())
    }

    fn ir(source: &str) -> String {
        lowered(source).to_string()
    }

    #[test]
    fn collect_inputs_in_order_of_first_appearance() {
        let expr = parse(&lex("y * x + y - (b / x)").unwrap()).unwrap();
        let mut names = Vec::new();
        collect_inputs(&expr, &mut names);
        assert_eq!(names, ["y", "x", "b"]);
    }

    #[test]
    fn lower_a_literal_needs_no_instruction() {
        assert_eq!(ir("42"), "fn main() {\n  ret 42\n}\n");
    }

    #[test]
    fn lower_an_input_needs_no_instruction() {
        assert_eq!(ir("x"), "fn main(%x) {\n  ret %x\n}\n");
    }

    #[test]
    fn lower_an_operation() {
        assert_eq!(
            ir("x + 1"),
            "fn main(%x) {\n  %0 = add %x, 1\n  ret %0\n}\n"
        );
        assert_eq!(ir("-x"), "fn main(%x) {\n  %0 = neg %x\n  ret %0\n}\n");
    }

    #[test]
    fn lower_operands_before_the_operation_left_first() {
        assert_eq!(
            ir("x * (2 + 3) + y * 0"),
            "\
fn main(%x, %y) {
  %0 = add 2, 3
  %1 = mul %x, %0
  %2 = mul %y, 0
  %3 = add %1, %2
  ret %3
}
"
        );
    }

    #[test]
    fn lower_keeps_the_source_span() {
        let function = lowered("1 + x / y");
        assert_eq!(function.body[0].span, Span::new(4, 9), "x / y");
        assert_eq!(function.body[1].span, Span::new(0, 9), "1 + x / y");
        assert_eq!(
            function.body[0].op,
            Op::Binary(BinOp::Div, Value::Input(0), Value::Input(1))
        );
    }
}
