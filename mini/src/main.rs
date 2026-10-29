//! The `mini` command: `mini FILE.mini` prints the compiled JavaScript.
//!
//! Kept deliberately thin: all the compiling happens in the library
//! (`src/lib.rs`), where tests can reach it. This file only deals with
//! the outside world — arguments, files, printing, exit codes.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let [_, path] = args.as_slice() else {
        eprintln!("usage: mini FILE.mini");
        return ExitCode::FAILURE;
    };

    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            return ExitCode::FAILURE;
        }
    };

    match mini::compile(&source) {
        Ok(javascript) => {
            print!("{javascript}");
            ExitCode::SUCCESS
        }
        Err(diagnostic) => {
            eprint!("{}", diagnostic.render(path, &source));
            ExitCode::FAILURE
        }
    }
}
