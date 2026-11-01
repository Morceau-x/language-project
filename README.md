# language-project-rust

Rust compilers for the Maxim language family, in one Cargo workspace.

| Folder                  | What                                                                                                                                |
|-------------------------|-------------------------------------------------------------------------------------------------------------------------------------|
| [`mini/`](mini)         | **Mini**, the compiler you're building step by step to learn Rust                                                                   |
| [`tutorial/`](tutorial) | the tutorial that guides you through `mini/` ([start here](tutorial/README.md))                                                     |
| [`example/`](example)   | the full Maxim compiler (package `maxim-compiler`), a Rust translation of the Haskell reference implementation, kept as a reference |

## Getting started

1. [Set up RustRover](tutorial/00-setup/00-setup-rustrover.md)
2. [Step 1 — A minimal compiler](tutorial/01-minimal-compiler/guide.md)
3. [Step 2 — Compiler layers and tooling](tutorial/02-compiler-layers/guide.md)
   (needs `bison` and `clang` installed)

## Commands

```bash
cargo test --workspace
```

```bash
cargo test -p mini
```

```bash
cargo run -q -p mini -- mini/programs/arithmetic.mini | node
```

```bash
cargo run -q -p mini -- run mini/programs/inputs.mini 4 100
```

```bash
cargo run -q -p mini -- build -O mini/programs/inputs.mini -o target/inputs && ./target/inputs 4 100
```

## Depends on `../language-project`

`example/` reads the Maxim standard library, the example programs and
the golden test fixtures (all shared with the Haskell compiler) from
the sibling `language-project` folder. Keep the two folders side by
side:

```text
personal/
├── language-project/        Haskell compiler, stdlib, examples, shared fixtures
└── language-project-rust/   this workspace
```

`mini/` has no such dependency. It uses `node` to run the JavaScript it
generates, and since step 2, `bison` (in Part 2) and `clang` (for
native executables). Tests that need a missing tool skip that part.
