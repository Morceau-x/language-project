# Learn Rust by building a compiler

You'll build **Mini**, a compiler from a small language to JavaScript
and to native code, one step at a time. Each step adds the compiler
ideas behind one feature or one layer of a real compiler, plus the
Rust it needs, explained as you go.

The code lives in [`../mini`](../mini). The full Maxim compiler in
[`../example`](../example) is a reference for how a real compiler does
the same jobs at scale.

## How each step works

Every step has two parts:

- **The Rust you need**: every Rust concept the step uses, with small
  examples. Read it first, then come back to it as needed.
- **The guide**: the compiler ideas, then a series of small tasks.
  Each task is a `todo!()` in `mini/` with tests that turn green when
  you get it right, plus folded hints and a solution. A long step's
  guide is split into one page per part.

The habit to build: **read the test, write the code, run the test.**

## Steps

| Step                                                                                                          | You build                                       | Compiler ideas                                                                              | Rust you learn                                                                                | Status   |
|---------------------------------------------------------------------------------------------------------------|-------------------------------------------------|---------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|----------|
| [0. Setup](00-setup/00-setup-rustrover.md)                                                                    | a working RustRover + Cargo workspace           | —                                                                                           | Cargo, workspaces                                                                             | ✅ ready |
| [1. A minimal compiler](01-minimal-compiler/guide.md) ([Rust concepts](01-minimal-compiler/rust-concepts.md)) | integer arithmetic → JavaScript                 | lexing, recursive descent parsing, precedence, ASTs, code generation, spans and diagnostics | structs, enums, `match`, ownership and borrowing, `Option`/`Result`/`?`, `Box`, traits, tests | ✅ ready |
| [2. Compiler layers and tooling](02-compiler-layers/guide.md) ([Rust concepts](02-compiler-layers/rust-concepts.md)) | inputs; a second front end generated from a yacc grammar; an IR, an interpreter and an optimizer; JavaScript and LLVM backends | LR parsing with Bison, parser generators, parse trees vs ASTs, three-address code and SSA, optimization passes, LLVM IR, undefined behaviour, runtime libraries, differential and property testing | lifetimes in types, build scripts, slice patterns, `HashMap`/`HashSet`, traits and trait objects, closures, `Command`, `proptest` | ✅ ready |
| 3. Control flow and types                                                                                     | `true`/`false`, comparisons, `if … then … else` | a **type checker** as its own phase; basic blocks, branches and φ nodes in the IR and in LLVM | more enums, `impl` on types, passing environments around                                      | proposed |
| 4. Variables                                                                                                  | `let` bindings, several statements per program  | keywords, scopes and symbol tables, "unknown name, did you mean…?" errors                   | scoped `HashMap`s, `Vec` of statements, string similarity                                     | proposed |
| 5. Functions                                                                                                  | `fn` definitions and calls                      | call checking (arity, argument types), recursion, calls and calling conventions in the IR and LLVM | slices, iterators in depth, lifetimes beyond the basics                                  | proposed |
| 6. Many errors at once                                                                                        | report every mistake, not just the first        | error recovery in the parser, `Vec<Diagnostic>`, cascading-error suppression, golden tests   | collecting results, `impl Iterator`, ownership of shared state, file I/O                      | proposed |
| 7. Toward Maxim                                                                                               | records, enums and pattern matching in Mini     | algebraic data types, exhaustiveness checking                                               | everything above, plus reading the code in `example/`                                         | proposed |

Steps 3 and later are **proposals**: say which one you want next, or
suggest something else (a REPL, a VS Code extension using a language
server, compiling to WebAssembly…) and the plan adapts.

## When you're stuck

- Read the failing test's message: it shows *expected* and *actual*.
- Read the compiler error **from the top**. Rust's errors are long
  because they're helpful, and the first one is usually the real one.
- Set a breakpoint in the test and step through it in RustRover's
  debugger.
- Open one hint at a time. The solution is there when you need it,
  and using it is not cheating. Re-type it rather than pasting it.
