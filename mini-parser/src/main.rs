//! `mini-parser`: parse a Maxim file and show what the parser sees.
//! Run it as `cargo parse ARGS...` (an alias in `.cargo/config.toml`).

use std::process::ExitCode;

use mini_parser::{SyntaxError, lexer, outline, parse};

const USAGE: &str = "\
usage: mini-parser [--emit=WHAT] FILE

Parse a Maxim file. Without --emit, print its syntax errors, if any.

  --emit=tokens    the tokens, trivia included, then the lexical errors
  --emit=tree      the syntax tree, then the errors
  --emit=outline   one line per declaration, from the typed AST";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (emit, path) = match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [path] => (None, path),
        [flag, path] if flag.starts_with("--emit=") => (Some(&flag["--emit=".len()..]), path),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("error: cannot read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    // Deeply nested code recurses deeply: give the parser a big stack.
    let (emit, path) = (emit.map(str::to_string), path.clone());
    let thread = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(move || run(emit.as_deref(), &path, &text));
    match thread.expect("spawn the parser thread").join() {
        Ok(code) => code,
        Err(_) => ExitCode::from(2),
    }
}

fn run(emit: Option<&str>, path: &str, text: &str) -> ExitCode {
    match emit {
        Some("tokens") => {
            let lexed = lexer::lex(text);
            let mut out = String::new();
            for i in 0..lexed.len() {
                let (kind, range, text) = (lexed.kind(i), lexed.text_range(i), lexed.text(i));
                out += &format!("{kind:?}@{range} {text:?}\n");
            }
            for error in lexed.errors() {
                out += &format!("error {}: {}\n", error.range, error.msg);
            }
            print_out(&out);
            exit_code(lexed.errors().is_empty())
        }
        Some("tree") => {
            let parse = parse(text);
            print_out(&parse.debug_dump());
            exit_code(parse.errors().is_empty())
        }
        Some("outline") => {
            let parse = parse(text);
            print_out(&outline::outline(&parse.tree()));
            report(path, text, parse.errors())
        }
        None => report(path, text, parse(text).errors()),
        Some(other) => {
            eprintln!("error: unknown --emit={other}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// Print to stdout. If the reader goes away, as `head` does once it has
/// its lines, stop quietly: `print!` would panic.
fn print_out(text: &str) {
    use std::io::Write;
    let _ = std::io::stdout().lock().write_all(text.as_bytes());
}

/// Print each error with the line it's on, and exit with 1 if any.
fn report(path: &str, text: &str, errors: &[SyntaxError]) -> ExitCode {
    for error in errors {
        eprintln!("{}", render(path, text, error));
    }
    exit_code(errors.is_empty())
}

fn exit_code(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// ```text
/// error: expected an expression
///  --> main.mx:3:9
///   |
/// 3 | let x = )
///   |         ^
/// ```
fn render(path: &str, text: &str, error: &SyntaxError) -> String {
    let start = error.range().start() as usize;
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    let line_text = text[line_start..line_end].trim_end_matches('\r');
    let line_number = text[..start].matches('\n').count() + 1;
    let column = text[line_start..start].chars().count() + 1;
    let end = (error.range().end() as usize).min(line_end).max(start);
    let width = text[start..end].chars().count().max(1);
    let gutter = " ".repeat(line_number.to_string().len());
    let pad: String = text[line_start..start]
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    format!(
        "error: {}\n{gutter}--> {path}:{line_number}:{column}\n{gutter} |\n{line_number} | {line_text}\n{gutter} | {pad}{}",
        error.message(),
        "^".repeat(width)
    )
}
