//! The `maxim` binary (Rust port): build / run / check. The format, lint,
//! and lsp subcommands live in the Haskell toolchain packages and are not
//! part of this port; they are listed (and refused) for CLI parity.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use maxim_compiler::diag::render::{mk_source_map, render_diagnostics};
use maxim_compiler::driver::{BuildOutcome, CompileOptions, DiskLoader, build_entry};
use maxim_compiler::panic::{catch_panic, render_panic};

const VERSION: &str = "maxim 0.1.0";

const HELP: &str = "maxim - the Maxim language toolchain

Usage: maxim [--stdlib DIR] COMMAND

  Compile, run, format, and lint Maxim programs.

Available options:
  --stdlib DIR             Standard library directory (overrides MAXIM_STDLIB)
  --version                Show version
  -h,--help                Show this help text

Available commands:
  build ENTRY.mx [--out-dir DIR]   Compile a program to ES modules
  run ENTRY.mx [ARGS...]           Compile and run under Node
  check ENTRY.mx                   Type-check without generating code
  format PATHS...                  Format Maxim sources in place (Haskell toolchain only)
  lint PATHS...                    Lint Maxim sources (Haskell toolchain only)
  lsp                              Run the language server on stdio (Haskell toolchain only)
";

fn main() -> ExitCode {
    // A compiler panic is a bug in Maxim itself; exit 2 keeps it distinct
    // from exit 1, "your program has errors". The pipeline runs on a
    // spawned thread with a large stack (deeply nested sources recurse).
    std::panic::set_hook(Box::new(|_| {}));
    let handle = std::thread::Builder::new()
        .name("maxim".to_string())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| catch_panic(real_main))
        .expect("spawn compiler thread");
    match handle.join() {
        Ok(Ok(code)) => code,
        Ok(Err(p)) => {
            eprintln!("{}", render_panic(&p));
            ExitCode::from(2)
        }
        Err(_) => ExitCode::from(2),
    }
}

fn real_main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdlib_flag: Option<PathBuf> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                println!("{VERSION}");
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            "--stdlib" => {
                i += 1;
                match args.get(i) {
                    Some(dir) => stdlib_flag = Some(PathBuf::from(dir)),
                    None => {
                        eprintln!("maxim: --stdlib needs a directory");
                        return ExitCode::FAILURE;
                    }
                }
            }
            _ => {
                rest.extend(args[i..].iter().cloned());
                break;
            }
        }
        i += 1;
    }

    let Some(cmd) = rest.first().cloned() else {
        eprint!("{HELP}");
        return ExitCode::FAILURE;
    };
    let cmd_args = &rest[1..];

    match cmd.as_str() {
        "build" => {
            let (entry, out_dir) = match parse_build_args(cmd_args) {
                Ok(v) => v,
                Err(code) => return code,
            };
            let Some(stdlib) = resolve_stdlib(stdlib_flag) else {
                return ExitCode::FAILURE;
            };
            if run_build(&stdlib, &entry, &out_dir, false).succeeded {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "check" => {
            let [entry] = cmd_args else {
                eprintln!("maxim check: expected exactly one ENTRY.mx");
                return ExitCode::FAILURE;
            };
            let Some(stdlib) = resolve_stdlib(stdlib_flag) else {
                return ExitCode::FAILURE;
            };
            if run_build(&stdlib, &PathBuf::from(entry), &PathBuf::from("build"), true).succeeded {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        "run" => {
            let Some(entry) = cmd_args.first() else {
                eprintln!("maxim run: expected ENTRY.mx");
                return ExitCode::FAILURE;
            };
            let node_args = &cmd_args[1..];
            let Some(stdlib) = resolve_stdlib(stdlib_flag) else {
                return ExitCode::FAILURE;
            };
            let outcome = run_build(&stdlib, &PathBuf::from(entry), &PathBuf::from("build"), false);
            if !outcome.succeeded {
                return ExitCode::FAILURE;
            }
            let status = std::process::Command::new("node")
                .arg(PathBuf::from("build").join("_main.mjs"))
                .args(node_args)
                .status();
            match status {
                Ok(s) => ExitCode::from(s.code().unwrap_or(1).clamp(0, 255) as u8),
                Err(e) => {
                    eprintln!("maxim run: cannot start node: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "format" | "lint" | "lsp" => {
            eprintln!(
                "maxim {cmd}: not supported by the Rust port — use the Haskell toolchain binary"
            );
            ExitCode::FAILURE
        }
        other => {
            eprintln!("maxim: unknown command `{other}`");
            eprint!("{HELP}");
            ExitCode::FAILURE
        }
    }
}

fn parse_build_args(args: &[String]) -> Result<(PathBuf, PathBuf), ExitCode> {
    let mut entry: Option<PathBuf> = None;
    let mut out_dir = PathBuf::from("build");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" => {
                i += 1;
                match args.get(i) {
                    Some(d) => out_dir = PathBuf::from(d),
                    None => {
                        eprintln!("maxim build: --out-dir needs a directory");
                        return Err(ExitCode::FAILURE);
                    }
                }
            }
            other => {
                if entry.is_some() {
                    eprintln!("maxim build: unexpected argument `{other}`");
                    return Err(ExitCode::FAILURE);
                }
                entry = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }
    match entry {
        Some(e) => Ok((e, out_dir)),
        None => {
            eprintln!("maxim build: expected ENTRY.mx");
            Err(ExitCode::FAILURE)
        }
    }
}

fn run_build(stdlib: &PathBuf, entry: &PathBuf, out_dir: &PathBuf, check_only: bool) -> BuildOutcome {
    let mut loader = DiskLoader;
    let outcome = build_entry(
        &mut loader,
        &CompileOptions {
            entry: entry.clone(),
            out_dir: out_dir.clone(),
            stdlib_dir: stdlib.clone(),
            check_only,
        },
    );
    let color = use_color();
    let srcs = mk_source_map(&outcome.sources);
    if !outcome.diags.is_empty() {
        eprintln!("{}", render_diagnostics(color, &srcs, &outcome.diags));
    }
    if outcome.succeeded && !check_only && !outcome.entry_has_main {
        eprintln!("note: the entry module has no `main`, so no main.mjs was written.");
    }
    outcome
}

fn use_color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// `--stdlib` flag, then MAXIM_STDLIB, then `<exe>/../lib/maxim/stdlib`.
fn resolve_stdlib(flag: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(dir) = flag {
        return Some(dir);
    }
    if let Some(dir) = std::env::var_os("MAXIM_STDLIB") {
        return Some(PathBuf::from(dir));
    }
    let installed = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("..").join("lib").join("maxim").join("stdlib")));
    if let Some(installed) = installed {
        if installed.is_dir() {
            return Some(installed);
        }
        eprintln!(
            "maxim: cannot find the standard library. I tried:\n  1. the --stdlib flag        (not given)\n  2. the MAXIM_STDLIB env var (not set)\n  3. {}\nIn this repository: export MAXIM_STDLIB=$PWD/../../language-project/stdlib/core",
            installed.display()
        );
    }
    None
}
