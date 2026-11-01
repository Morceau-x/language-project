//! The JavaScript backend: IR → JavaScript, run by Node.js.
//!
//! Three-address code maps almost line for line onto JavaScript: each
//! instruction becomes a `const`. No parentheses to get right anymore:
//! every intermediate value has a name.

use std::fmt::{self, Write};

use crate::ast::BinOp;
use crate::ir::{Function, Inst, Op, Value};

/// The whole JavaScript program for `function`.
pub fn generate(function: &Function) -> String {
    let mut out = String::new();
    out.push_str(PRELUDE);
    emit_main(function, &mut out).expect("writing to a String cannot fail");
    emit_epilogue(function, &mut out).expect("writing to a String cannot fail");
    out
}

/// Support code every program needs.
const PRELUDE: &str = r#"// Compiled from Mini by `mini`. Run it with: node FILE.js INPUT...
"use strict";

// Mini integers are 64-bit and wrap around on overflow. BigInt is exact,
// so each result is wrapped back to 64 bits.
const wrap = (n) => BigInt.asIntN(64, n);

// Mini's division rounds toward zero (BigInt's `/` already does) and
// fails on a zero divisor.
function div(a, b) {
  if (b === 0n) {
    fail("division by zero", 1);
  }
  return wrap(a / b);
}

function fail(message, status) {
  console.error(`error: ${message}`);
  process.exit(status);
}

"#;

/// The program itself, as a function of its inputs:
///
/// ```js
/// function main($x, $y) {
///   const $0 = wrap($x * 5n);
///   const $1 = div($0, $y);
///   return $1;
/// }
/// ```
///
/// Every name gets a `$`: Mini names can't contain one, so `$x` can't
/// clash with `wrap`, `div` or a JavaScript keyword (try an input called
/// `new`), and the temporary `$0` can't clash with an input.
fn emit_main(function: &Function, out: &mut String) -> fmt::Result {
    let params: Vec<String> = function
        .inputs
        .iter()
        .map(|name| format!("${name}"))
        .collect();
    writeln!(out, "function main({}) {{", params.join(", "))?;
    for inst in &function.body {
        emit_inst(function, inst, out)?;
    }
    writeln!(out, "  return {};", value(function, function.result))?;
    writeln!(out, "}}")
}

/// One instruction, as a `const` declaration.
fn emit_inst(function: &Function, inst: &Inst, out: &mut String) -> fmt::Result {
    todo!("Part 7a: js::emit_inst")
}

/// How an operand is written in JavaScript: `5n`, `(-5n)`, `$x` or `$3`.
fn value(function: &Function, value: Value) -> String {
    todo!("Part 7a: js::value")
}

/// Read the inputs from the command line, run `main`, print the result.
fn emit_epilogue(function: &Function, out: &mut String) -> fmt::Result {
    let names: Vec<String> = function
        .inputs
        .iter()
        .map(|name| format!("{name:?}"))
        .collect();
    write!(
        out,
        r#"
// Read the inputs from the command line, run the program, print the result.
const names = [{}];
const args = process.argv.slice(2);
if (args.length !== names.length) {{
  console.error(`usage: node ${{process.argv[1]}} ${{names.join(" ")}}`);
  process.exit(2);
}}
const inputs = args.map((text, i) => {{
  const n = /^[+-]?[0-9]+$/.test(text) ? BigInt(text) : null;
  if (n === null || n !== wrap(n)) {{
    fail(`input \`${{names[i]}}\` must be a 64-bit integer, got \`${{text}}\``, 2);
  }}
  return n;
}});
console.log(String(main(...inputs)));
"#,
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Temp;
    use crate::lexer::lex;
    use crate::lower::lower;
    use crate::parser::parse;

    fn main_function(source: &str) -> String {
        let function = lower(&parse(&lex(source).unwrap()).unwrap());
        let mut out = String::new();
        emit_main(&function, &mut out).unwrap();
        out
    }

    #[test]
    fn value_writes_bigints_and_names() {
        let function = Function {
            inputs: vec!["x".to_string()],
            body: vec![],
            result: Value::Const(0),
        };
        assert_eq!(value(&function, Value::Const(5)), "5n");
        assert_eq!(value(&function, Value::Const(-5)), "(-5n)");
        assert_eq!(value(&function, Value::Input(0)), "$x");
        assert_eq!(value(&function, Value::Temp(Temp(3))), "$3");
    }

    #[test]
    fn emit_main_without_instructions() {
        assert_eq!(main_function("42"), "function main() {\n  return 42n;\n}\n");
        assert_eq!(main_function("x"), "function main($x) {\n  return $x;\n}\n");
    }

    #[test]
    fn emit_inst_wraps_arithmetic() {
        assert_eq!(
            main_function("-x * (y + 1) - 2"),
            "\
function main($x, $y) {
  const $0 = wrap(-$x);
  const $1 = wrap($y + 1n);
  const $2 = wrap($0 * $1);
  const $3 = wrap($2 - 2n);
  return $3;
}
"
        );
    }

    #[test]
    fn emit_inst_divides_with_the_div_helper() {
        assert_eq!(
            main_function("x / 2"),
            "function main($x) {\n  const $0 = div($x, 2n);\n  return $0;\n}\n"
        );
    }
}
