//! The driver: chains the phases into a pipeline. Each layer can also be
//! reached on its own, which is what `mini --emit=...` shows.
//!
//! ```text
//!            hand-written: lexer → parser ─┐
//! source ──┤                               ├─▶ AST ─lower─▶ IR ─optimize─▶ IR ─┬─ js ───▶ JavaScript
//!            generated: yacc → CST → prune ┘                                   ├─ llvm ─▶ LLVM IR ─clang─▶ executable
//!                                                                              └─ interp ─▶ result
//! ```

use std::path::Path;
use std::process::Command;

use crate::ast::Expr;
use crate::diagnostic::Diagnostic;
use crate::ir::Function;
use crate::opt::PassManager;
use crate::{backend, lexer, lower, parser, yacc};

/// Which parser turns the source into an AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frontend {
    /// Step 1's lexer and recursive descent parser.
    #[default]
    HandWritten,
    /// The lexer and LALR parser generated from `yacc/mini.l` and `yacc/mini.y`.
    Generated,
}

/// How to compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    pub frontend: Frontend,
    /// Run the optimizer on the IR (`-O`).
    pub optimize: bool,
}

/// Source → AST, with the chosen front end.
pub fn parse(source: &str, frontend: Frontend) -> Result<Expr, Diagnostic> {
    match frontend {
        Frontend::HandWritten => parser::parse(&lexer::lex(source)?),
        Frontend::Generated => yacc::parse(source),
    }
}

/// Source → IR, optimized if asked. `trace` is called after each
/// optimization pass that changed the IR.
pub fn compile_to_ir(
    source: &str,
    options: Options,
    trace: &mut dyn FnMut(&str, &Function),
) -> Result<Function, Diagnostic> {
    let expr = parse(source, options.frontend)?;
    let mut function = lower::lower(&expr);
    if options.optimize {
        PassManager::standard().run(&mut function, trace);
    }
    Ok(function)
}

/// Source → JavaScript.
pub fn compile_to_js(source: &str, options: Options) -> Result<String, Diagnostic> {
    let function = compile_to_ir(source, options, &mut |_, _| {})?;
    Ok(backend::js::generate(&function))
}

/// Source → LLVM IR.
pub fn compile_to_llvm(source: &str, options: Options) -> Result<String, Diagnostic> {
    let function = compile_to_ir(source, options, &mut |_, _| {})?;
    Ok(backend::llvm::generate(&function))
}

/// The runtime every native program is linked with.
const RUNTIME: &str = include_str!("../runtime/mini_rt.c");

/// Build a native executable at `output` from LLVM IR: write the IR and
/// the runtime to a temporary folder, and let clang compile and link
/// them.
pub fn build_native(llvm: &str, output: &Path) -> Result<(), String> {
    let unique = format!(
        "mini-{}-{}",
        std::process::id(),
        output
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("out")
    );
    let dir = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("cannot create {}: {err}", dir.display()))?;
    let ll = dir.join("program.ll");
    let runtime = dir.join("mini_rt.c");
    std::fs::write(&ll, llvm).map_err(|err| format!("cannot write {}: {err}", ll.display()))?;
    std::fs::write(&runtime, RUNTIME)
        .map_err(|err| format!("cannot write {}: {err}", runtime.display()))?;

    let result = Command::new("clang")
        .arg("-O2")
        // The IR doesn't say which CPU it's for; clang picks yours, and
        // would warn that it "overrides" the (missing) target triple.
        .arg("-Wno-override-module")
        .arg(&ll)
        .arg(&runtime)
        .arg("-o")
        .arg(output)
        .output();
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Err(err) => Err(format!("cannot run clang ({err}); is it installed?")),
        Ok(out) if !out.status.success() => Err(format!(
            "clang failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )),
        Ok(_) => Ok(()),
    }
}
