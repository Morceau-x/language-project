//! End-to-end tests: Mini source in, JavaScript (or an error) out.
//!
//! Integration tests like these live in `tests/` and can only use the
//! crate's public API (`mini::compile`), exactly like a real user would.

use std::process::Command;

#[test]
fn compiles_a_program_to_javascript() {
    assert_eq!(
        mini::compile("1 + 2 * 3").unwrap(),
        "console.log((1 + (2 * 3)));\n"
    );
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

/// Run the compiled JavaScript with Node.js and check it prints what the
/// same expression evaluates to in Rust: Mini's integers are meant to
/// behave like Rust's. Skipped when Node.js is not installed.
#[test]
fn compiled_programs_compute_the_right_answer() {
    let cases = [
        ("1 + 2 * 3", 1 + 2 * 3),
        ("(1 + 2) * 3", (1 + 2) * 3),
        ("10 - 4 - 3", 10 - 4 - 3),
        ("7 / 2", 7 / 2),
        ("-7 / 2", -7 / 2),
        ("- -5", 5),
        ("2 * -3", 2 * -3),
    ];
    for (source, expected) in cases {
        let javascript = mini::compile(source).unwrap();
        let Ok(output) = Command::new("node").arg("-e").arg(&javascript).output() else {
            eprintln!("node not found: skipping the run-the-output test");
            return;
        };
        let printed = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            printed.trim(),
            expected.to_string(),
            "`{source}` compiled to `{javascript}`"
        );
    }
}
