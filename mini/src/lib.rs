//! Mini: a small compiler from Mini source code to JavaScript.
//!
//! A compiler is a pipeline of phases. Each phase has one job and hands
//! a well-defined data structure to the next:
//!
//! ```text
//! source text ──lexer──▶ tokens ──parser──▶ AST ──codegen──▶ JavaScript
//! ```
//!
//! Any phase can fail with a [`Diagnostic`] that points at the offending
//! source code.

// Scaffolding: while functions are still `todo!()`, the code they will
// use looks unused to the compiler. This line hides those warnings.
// Delete it when you finish step 1 (Part 7 of the guide).
#![allow(dead_code, unused_imports, unused_variables)]

pub mod ast;
pub mod codegen;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod token;

pub use diagnostic::Diagnostic;

/// Compile a whole Mini program to JavaScript source code.
pub fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = lexer::lex(source)?;
    let expr = parser::parse(&tokens)?;
    Ok(codegen::generate(&expr))
}
