//! End-to-end tests: Mini source in, a running program out.
//!
//! Integration tests like these live in `tests/` and can only use the
//! crate's public API, exactly like a real user would.
//!
//! The main test is *differential*: the same programs run through every
//! road the compiler offers (the interpreter, JavaScript on Node.js,
//! native code through LLVM, each with and without the optimizer) and
//! every road must give the same answers. Node.js and clang are optional:
//! without them, their part is skipped.

use std::path::PathBuf;
use std::process::{Command, Output};

use mini::driver::{self, Frontend, Options};
use mini::interp;

/// What running a program gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Value(i64),
    DivisionByZero,
}

use Outcome::{DivisionByZero, Value};

/// A program, and what it must give for some inputs.
type Case = (&'static str, Vec<(Vec<i64>, Outcome)>);

fn cases() -> Vec<Case> {
    vec![
        ("1 + 2 * 3", vec![(vec![], Value(7))]),
        ("(1 + 2) * 3 - 10 / 4", vec![(vec![], Value(7))]),
        ("-7 / 2", vec![(vec![], Value(-3))]),
        ("- -5", vec![(vec![], Value(5))]),
        (
            "x * (2 + 3) + y * 0",
            vec![(vec![4, 100], Value(20)), (vec![-3, 0], Value(-15))],
        ),
        (
            "total / count",
            vec![
                (vec![10, 3], Value(3)),
                (vec![-7, 2], Value(-3)),
                (vec![1, 0], DivisionByZero),
                (vec![i64::MIN, -1], Value(i64::MIN)),
            ],
        ),
        // Overflow wraps around.
        ("x + 1", vec![(vec![i64::MAX], Value(i64::MIN))]),
        ("x * x", vec![(vec![1 << 32], Value(0))]),
        ("-x", vec![(vec![i64::MIN], Value(i64::MIN))]),
        // The optimizer knows the result is 1, but the division must
        // still run, and fail on zero.
        (
            "x / x",
            vec![(vec![5], Value(1)), (vec![0], DivisionByZero)],
        ),
        // The division's result is unused, but it must still fail.
        (
            "0 * (x / y)",
            vec![(vec![1, 0], DivisionByZero), (vec![1, 2], Value(0))],
        ),
        (
            "(x + 1) * (x + 1) - (x + 1) * (x + 1) + x / 2",
            vec![(vec![9], Value(4))],
        ),
        // Too big for a JavaScript number: BigInt keeps it exact.
        (
            "9007199254740993 + x",
            vec![(vec![0], Value(9007199254740993))],
        ),
    ]
}

#[test]
fn every_backend_computes_the_same_answers() {
    let node = Command::new("node").arg("--version").output().is_ok();
    let clang = Command::new("clang").arg("--version").output().is_ok();
    if !node {
        eprintln!("node not found: skipping the JavaScript backend");
    }
    if !clang {
        eprintln!("clang not found: skipping the LLVM backend");
    }
    for (source, runs) in cases() {
        for optimize in [false, true] {
            let options = Options {
                optimize,
                ..Options::default()
            };
            let function = driver::compile_to_ir(source, options, &mut |_, _| {}).unwrap();
            let js = driver::compile_to_js(source, options).unwrap();
            let js_file = file_for(source, options, ".js");
            std::fs::write(&js_file, &js).unwrap();
            let exe = clang.then(|| build(source, options));
            for (inputs, expected) in &runs {
                let context = format!("`{source}` with inputs {inputs:?}, optimize: {optimize}");
                let interpreted = match interp::run(&function, inputs) {
                    Ok(value) => Value(value),
                    Err(diagnostic) if diagnostic.message == "division by zero" => DivisionByZero,
                    Err(diagnostic) => panic!("{context}: {}", diagnostic.message),
                };
                assert_eq!(interpreted, *expected, "interpreter, {context}");
                if node {
                    let output = Command::new("node")
                        .arg(&js_file)
                        .args(inputs.iter().map(i64::to_string))
                        .output()
                        .unwrap();
                    assert_eq!(
                        outcome(&output, &context),
                        *expected,
                        "JavaScript, {context}\n{js}"
                    );
                }
                if let Some(exe) = &exe {
                    let output = Command::new(exe)
                        .args(inputs.iter().map(i64::to_string))
                        .output()
                        .unwrap();
                    assert_eq!(outcome(&output, &context), *expected, "native, {context}");
                }
            }
        }
    }
}

/// A file name for `source` in Cargo's folder for test files.
fn file_for(source: &str, options: Options, extension: &str) -> PathBuf {
    let name: String = source
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let optimized = if options.optimize { "_O" } else { "" };
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}{optimized}{extension}"))
}

/// Build a native executable for `source`.
fn build(source: &str, options: Options) -> PathBuf {
    let llvm = driver::compile_to_llvm(source, options).unwrap();
    let exe = file_for(source, options, "");
    driver::build_native(&llvm, &exe).unwrap();
    exe
}

/// Read what a compiled program printed, and how it exited.
fn outcome(output: &Output, context: &str) -> Outcome {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match output.status.code() {
        Some(0) => Value(stdout.trim().parse().unwrap()),
        Some(1) if stderr.trim() == "error: division by zero" => DivisionByZero,
        status => panic!("{context}: exit status {status:?}, stdout {stdout:?}, stderr {stderr:?}"),
    }
}

#[test]
fn both_front_ends_build_the_same_tree() {
    for (source, _) in cases() {
        assert_eq!(
            driver::parse(source, Frontend::Generated),
            driver::parse(source, Frontend::HandWritten),
            "`{source}`"
        );
    }
}

#[test]
fn reports_errors_with_the_offending_line() {
    let source = "(1 + 2";
    let diagnostic = mini::compile(source).unwrap_err();
    assert_eq!(
        diagnostic.render("test.mini", source),
        "\
error: expected `)`, found end of input
 --> test.mini:1:7
  |
1 | (1 + 2
  |       ^
"
    );
}

/// Run the `mini` command itself.
fn mini(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mini"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn the_command_prints_each_layer() {
    let output = mini(&["--emit=ir", "-O", "programs/inputs.mini"]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "fn main(%x, %y) {\n  %1 = mul %x, 5\n  ret %1\n}\n"
    );
    let output = mini(&["--emit=ast", "--parser=yacc", "programs/inputs.mini"]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "(+ (* x (+ 2 3)) (* y 0))\n"
    );
}

#[test]
fn the_command_runs_programs() {
    let output = mini(&["run", "programs/division.mini", "-7", "2"]);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "-3\n");
    let output = mini(&["run", "programs/division.mini", "1", "0"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "\
error: division by zero
 --> programs/division.mini:1:1
  |
1 | total / count
  | ^^^^^^^^^^^^^
"
    );
}
