//! The Maxim compiler, translated from the Haskell reference implementation
//! in `compiler/`. The public surface mirrors `Maxim.Compiler.Api`.

pub mod ast;
pub mod builtin;
pub mod core;
pub mod cst;
pub mod diag;
pub mod driver;
pub mod index;
pub mod intrinsics;
pub mod js;
pub mod lexer;
pub mod match_analysis;
pub mod names;
pub mod panic;
pub mod parser;
pub mod rename;
pub mod source;
pub mod span;
pub mod types;
pub mod unique;
