//! Mini: a small compiler from Mini source code to JavaScript and to
//! native code (through LLVM).
//!
//! A compiler is a pipeline of layers. Each one has one job and hands a
//! well-defined data structure to the next:
//!
//! ```text
//! source ─▶ tokens ─▶ AST ─▶ IR ─▶ optimized IR ─▶ JavaScript, or LLVM IR ─▶ machine code
//! ```
//!
//! There are two front ends that build the same AST: the hand-written
//! one from step 1 (`lexer`, `parser`), and one generated from a yacc
//! grammar (`yacc`, `cst`, `prune`). See `driver` for the whole pipeline.
//!
//! Any layer can fail with a [`Diagnostic`] that points at the offending
//! source code.

// Scaffolding: while functions are still `todo!()`, the code they will
// use looks unused to the compiler, and Clippy misreads a few
// signatures. This line hides those warnings. Delete it when you finish
// step 2 (Part 8 of the guide).

// Front ends.
pub mod ast;
pub mod cst;
pub mod lexer;
pub mod parser;
pub mod prune;
pub mod token;
pub mod yacc;

// Middle: the IR and what works on it.
pub mod interp;
pub mod ir;
pub mod lower;
pub mod opt;

// Back ends.
pub mod backend;

// Shared.
pub mod diagnostic;
pub mod driver;
pub mod span;

pub use diagnostic::Diagnostic;
pub use driver::{Frontend, Options};

/// Compile a whole Mini program to JavaScript, with the default options.
pub fn compile(source: &str) -> Result<String, Diagnostic> {
    driver::compile_to_js(source, Options::default())
}
