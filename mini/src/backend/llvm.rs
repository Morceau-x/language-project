//! The LLVM backend: IR → LLVM IR, as text (a `.ll` file).
//!
//! From there, clang (built on LLVM) optimizes it and compiles it to
//! machine code for your CPU. `mini build` links it with the runtime in
//! `runtime/mini_rt.c`, which reads the inputs and prints the result.
//!
//! Our IR was designed after LLVM's, so most instructions translate one
//! to one. The exceptions are where Mini and LLVM disagree on what an
//! operation means: see `mini_div` in the prelude.

use std::collections::HashMap;
use std::fmt::{self, Write};

use crate::ast::BinOp;
use crate::ir::{Function, Op, Temp, Value};

/// The whole LLVM module for `function`.
pub fn generate(function: &Function) -> String {
    let mut out = String::new();
    out.push_str(PRELUDE);
    emit_main(function, &mut out).expect("writing to a String cannot fail");
    emit_glue(function, &mut out).expect("writing to a String cannot fail");
    out
}

/// Declarations and helpers every program needs.
const PRELUDE: &str = r#"; Compiled from Mini by `mini`.
; Build it with the runtime: clang FILE.ll mini_rt.c -o FILE

; Provided by the runtime: prints "error: division by zero" and exits.
declare void @mini_division_by_zero() noreturn nounwind

; Mini's division. LLVM's `sdiv` has *undefined behaviour* when the
; divisor is 0, and for i64::MIN / -1: the optimizer may assume they never
; happen. Mini defines both cases (an error, and wrapping), so every
; division checks first.
define internal i64 @mini_div(i64 %a, i64 %b) {
entry:
  %is_zero = icmp eq i64 %b, 0
  br i1 %is_zero, label %zero, label %nonzero

zero:
  call void @mini_division_by_zero()
  unreachable

nonzero:
  %is_minus_one = icmp eq i64 %b, -1
  br i1 %is_minus_one, label %minus_one, label %divide

minus_one:
  ; a / -1 is -a, and `sub` wraps: i64::MIN stays i64::MIN.
  %negated = sub i64 0, %a
  ret i64 %negated

divide:
  %quotient = sdiv i64 %a, %b
  ret i64 %quotient
}

"#;

/// The program itself, as an LLVM function:
///
/// ```llvm
/// define i64 @mini_main(i64 %x, i64 %y) {
/// entry:
///   %0 = mul i64 %x, 5
///   %1 = call i64 @mini_div(i64 %0, i64 %y)
///   ret i64 %1
/// }
/// ```
///
/// LLVM numbers unnamed values in order, `%0`, `%1`… with no gaps, but
/// our temporaries have gaps once dead code is gone. So each temporary
/// gets the next free number, its *slot*, when its instruction is written.
fn emit_main(function: &Function, out: &mut String) -> fmt::Result {
    let params: Vec<String> = function
        .inputs
        .iter()
        .map(|name| format!("i64 %{name}"))
        .collect();
    writeln!(out, "define i64 @mini_main({}) {{", params.join(", "))?;
    writeln!(out, "entry:")?;
    let mut slots: HashMap<Temp, usize> = HashMap::new();
    for inst in &function.body {
        let slot = slots.len();
        let instruction = match inst.op {
            Op::Neg(a) => format!("sub i64 0, {}", value(function, &slots, a)),
            Op::Binary(BinOp::Div, a, b) => format!(
                "call i64 @mini_div(i64 {}, i64 {})",
                value(function, &slots, a),
                value(function, &slots, b)
            ),
            Op::Binary(op, a, b) => format!(
                "{} i64 {}, {}",
                instruction_name(op),
                value(function, &slots, a),
                value(function, &slots, b)
            ),
        };
        writeln!(out, "  %{slot} = {instruction}")?;
        slots.insert(inst.dest, slot);
    }
    writeln!(
        out,
        "  ret i64 {}",
        value(function, &slots, function.result)
    )?;
    writeln!(out, "}}")
}

/// The LLVM instruction for `add`, `sub` and `mul`. They wrap on overflow,
/// exactly like Mini. (`add nsw` would instead promise LLVM that it never
/// overflows.)
fn instruction_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => unreachable!("divisions call @mini_div"),
    }
}

/// How an operand is written in LLVM IR: `5`, `%x`, or `%` and the
/// temporary's slot.
fn value(function: &Function, slots: &HashMap<Temp, usize>, value: Value) -> String {
    match value {
        Value::Const(c) => c.to_string(),
        Value::Input(i) => format!("%{}", function.inputs[i]),
        Value::Temp(t) => format!("%{}", slots[&t]),
    }
}

/// What the runtime needs to call the program: how many inputs it takes,
/// their names (for error messages), and `mini_entry`, which takes the
/// inputs as an array and passes them to `mini_main` one by one.
fn emit_glue(function: &Function, out: &mut String) -> fmt::Result {
    let count = function.inputs.len();
    writeln!(out)?;
    writeln!(out, "; Glue for the runtime (runtime/mini_rt.c).")?;
    writeln!(out, "@mini_input_count = constant i64 {count}")?;
    for (i, name) in function.inputs.iter().enumerate() {
        writeln!(
            out,
            "@mini_input_name.{i} = private unnamed_addr constant [{} x i8] c\"{name}\\00\"",
            name.len() + 1
        )?;
    }
    let names: Vec<String> = (0..count)
        .map(|i| format!("ptr @mini_input_name.{i}"))
        .collect();
    if count == 0 {
        writeln!(
            out,
            "@mini_input_names = constant [0 x ptr] zeroinitializer"
        )?;
    } else {
        writeln!(
            out,
            "@mini_input_names = constant [{count} x ptr] [{}]",
            names.join(", ")
        )?;
    }
    writeln!(out)?;
    writeln!(out, "define i64 @mini_entry(ptr %inputs) {{")?;
    writeln!(out, "entry:")?;
    let mut args = Vec::new();
    for (i, name) in function.inputs.iter().enumerate() {
        // Mini names can't contain dots, so `%x.ptr` can't clash with one.
        writeln!(
            out,
            "  %{name}.ptr = getelementptr inbounds i64, ptr %inputs, i64 {i}"
        )?;
        writeln!(out, "  %{name} = load i64, ptr %{name}.ptr")?;
        args.push(format!("i64 %{name}"));
    }
    writeln!(
        out,
        "  %mini.result = call i64 @mini_main({})",
        args.join(", ")
    )?;
    writeln!(out, "  ret i64 %mini.result")?;
    writeln!(out, "}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Inst;
    use crate::lexer::lex;
    use crate::lower::lower;
    use crate::parser::parse;
    use crate::span::Span;

    fn main_function(function: &Function) -> String {
        let mut out = String::new();
        emit_main(function, &mut out).unwrap();
        out
    }

    fn lowered(source: &str) -> Function {
        lower(&parse(&lex(source).unwrap()).unwrap())
    }

    #[test]
    fn emit_main_without_instructions() {
        assert_eq!(
            main_function(&lowered("x")),
            "define i64 @mini_main(i64 %x) {\nentry:\n  ret i64 %x\n}\n"
        );
    }

    #[test]
    fn emit_main_translates_each_instruction() {
        assert_eq!(
            main_function(&lowered("-x * (y + 1) - 2 / y")),
            "\
define i64 @mini_main(i64 %x, i64 %y) {
entry:
  %0 = sub i64 0, %x
  %1 = add i64 %y, 1
  %2 = mul i64 %0, %1
  %3 = call i64 @mini_div(i64 2, i64 %y)
  %4 = sub i64 %2, %3
  ret i64 %4
}
"
        );
    }

    #[test]
    fn emit_main_numbers_temporaries_without_gaps() {
        // As if dead code elimination had removed `%0` and `%2`.
        let function = Function {
            inputs: vec!["x".to_string()],
            body: vec![
                Inst {
                    dest: Temp(1),
                    op: Op::Binary(BinOp::Mul, Value::Input(0), Value::Const(5)),
                    span: Span::new(0, 0),
                },
                Inst {
                    dest: Temp(3),
                    op: Op::Binary(BinOp::Sub, Value::Temp(Temp(1)), Value::Const(-1)),
                    span: Span::new(0, 0),
                },
            ],
            result: Value::Temp(Temp(3)),
        };
        assert_eq!(
            main_function(&function),
            "\
define i64 @mini_main(i64 %x) {
entry:
  %0 = mul i64 %x, 5
  %1 = sub i64 %0, -1
  ret i64 %1
}
"
        );
    }

    #[test]
    fn glue_describes_the_inputs() {
        let llvm = generate(&lowered("b - a"));
        assert!(
            llvm.contains("@mini_input_count = constant i64 2\n"),
            "{llvm}"
        );
        assert!(
            llvm.contains("@mini_input_name.1 = private unnamed_addr constant [2 x i8] c\"a\\00\""),
            "{llvm}"
        );
        assert!(
            llvm.contains("%mini.result = call i64 @mini_main(i64 %b, i64 %a)"),
            "{llvm}"
        );
    }
}
