//! The `mini` command.
//!
//! Kept deliberately thin: all the compiling happens in the library
//! (`src/lib.rs`), where tests can reach it. This file only deals with
//! the outside world: arguments, files, printing, exit codes.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mini::driver::{self, Frontend, Options};
use mini::{Diagnostic, interp, lexer, yacc};

const USAGE: &str = "\
usage: mini [OPTIONS] FILE.mini                 compile to JavaScript (or --emit another layer)
       mini run [OPTIONS] FILE.mini [INPUT...]  run the program with the IR interpreter
       mini build [OPTIONS] FILE.mini           build a native executable with clang

options:
  --emit=LAYER        print one layer: tokens, cst, ast, ir, js (the default) or llvm
  -O                  optimize the IR
  --print-after-each  print the IR after each optimization pass that changed it (to stderr)
  --parser=yacc       parse with the generated parser instead of the hand-written one
  -o OUTPUT           where `mini build` writes the executable (default: FILE without .mini)

For `mini run`, options go before FILE: everything after it is an input.";

/// What the command line asks for.
struct Command {
    action: Action,
    file: String,
    options: Options,
    print_after_each: bool,
    emit: Emit,
    output: Option<PathBuf>,
    inputs: Vec<String>,
}

enum Action {
    Compile,
    Run,
    Build,
}

#[derive(Clone, Copy)]
enum Emit {
    Tokens,
    Cst,
    Ast,
    Ir,
    Js,
    Llvm,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let command = match parse_args(&args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let source = match std::fs::read_to_string(&command.file) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", command.file);
            return ExitCode::FAILURE;
        }
    };
    let report = |diagnostic: Diagnostic| {
        eprint!("{}", diagnostic.render(&command.file, &source));
        ExitCode::FAILURE
    };
    let print_after_each = command.print_after_each;
    let mut trace = |pass: &str, function: &mini::ir::Function| {
        if print_after_each {
            eprintln!("; after {pass}\n{function}");
        }
    };

    match command.action {
        Action::Compile => match emit(command.emit, &source, command.options, &mut trace) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(diagnostic) => report(diagnostic),
        },
        Action::Run => {
            let function = match driver::compile_to_ir(&source, command.options, &mut trace) {
                Ok(function) => function,
                Err(diagnostic) => return report(diagnostic),
            };
            if command.inputs.len() != function.inputs.len() {
                eprintln!(
                    "usage: mini run {} {}",
                    command.file,
                    function.inputs.join(" ")
                );
                return ExitCode::from(2);
            }
            let mut inputs = Vec::new();
            for (name, text) in function.inputs.iter().zip(&command.inputs) {
                match text.parse::<i64>() {
                    Ok(value) => inputs.push(value),
                    Err(_) => {
                        eprintln!("error: input `{name}` must be a 64-bit integer, got `{text}`");
                        return ExitCode::from(2);
                    }
                }
            }
            match interp::run(&function, &inputs) {
                Ok(value) => {
                    println!("{value}");
                    ExitCode::SUCCESS
                }
                Err(diagnostic) => report(diagnostic),
            }
        }
        Action::Build => {
            let llvm = match emit(Emit::Llvm, &source, command.options, &mut trace) {
                Ok(llvm) => llvm,
                Err(diagnostic) => return report(diagnostic),
            };
            let output = command
                .output
                .unwrap_or_else(|| Path::new(&command.file).with_extension(""));
            match driver::build_native(&llvm, &output) {
                Ok(()) => {
                    eprintln!("built {}", output.display());
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("error: {message}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

/// Compile `source` as far as the layer `emit`, and print that layer.
fn emit(
    emit: Emit,
    source: &str,
    options: Options,
    trace: &mut dyn FnMut(&str, &mini::ir::Function),
) -> Result<String, Diagnostic> {
    Ok(match emit {
        Emit::Tokens => {
            let mut out = String::new();
            for token in lexer::lex(source)? {
                let span = token.span;
                out += &format!("{:>3}..{:<3} {}\n", span.start, span.end, token.kind);
            }
            out
        }
        Emit::Cst => yacc::parse_cst(source)?.to_string(),
        Emit::Ast => format!("{}\n", driver::parse(source, options.frontend)?),
        Emit::Ir => driver::compile_to_ir(source, options, trace)?.to_string(),
        Emit::Js => mini::backend::js::generate(&driver::compile_to_ir(source, options, trace)?),
        Emit::Llvm => {
            mini::backend::llvm::generate(&driver::compile_to_ir(source, options, trace)?)
        }
    })
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut args = args.iter();
    let mut command = Command {
        action: Action::Compile,
        file: String::new(),
        options: Options::default(),
        print_after_each: false,
        emit: Emit::Js,
        output: None,
        inputs: Vec::new(),
    };
    let mut first = true;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "run" if first => command.action = Action::Run,
            "build" if first => command.action = Action::Build,
            "-O" => command.options.optimize = true,
            "--print-after-each" => command.print_after_each = true,
            "--parser=yacc" => command.options.frontend = Frontend::Generated,
            "--parser=hand" => command.options.frontend = Frontend::HandWritten,
            "-o" => command.output = Some(args.next().ok_or("-o needs a file name")?.into()),
            _ if arg.starts_with("--emit=") => {
                command.emit = match &arg["--emit=".len()..] {
                    "tokens" => Emit::Tokens,
                    "cst" => Emit::Cst,
                    "ast" => Emit::Ast,
                    "ir" => Emit::Ir,
                    "js" => Emit::Js,
                    "llvm" => Emit::Llvm,
                    other => return Err(format!("unknown layer `{other}`")),
                }
            }
            _ if arg.starts_with('-') => return Err(format!("unknown option `{arg}`")),
            _ if command.file.is_empty() => {
                command.file = arg.clone();
                if matches!(command.action, Action::Run) {
                    // Inputs may be negative: `-5` is an input, not an option.
                    command.inputs = args.cloned().collect();
                    break;
                }
            }
            _ => return Err(format!("unexpected argument `{arg}`")),
        }
        first = false;
    }
    if command.file.is_empty() {
        return Err("no input file".to_string());
    }
    Ok(command)
}
