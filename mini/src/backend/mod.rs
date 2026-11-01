//! Backends: turn the IR into a program another tool can run.
//!
//! - `js`: JavaScript, for Node.js.
//! - `llvm`: LLVM IR, which clang compiles to native machine code.

pub mod js;
pub mod llvm;
