# Step 0 — Set up RustRover

About 10 minutes. At the end, RustRover understands the whole workspace,
formats your code on save, flags mistakes as you type, and runs any
test with one click.

## What you already have

Checked on this Mac (29 Oct 2026):

| Piece | Where | Status |
|---|---|---|
| RustRover 2026.2.4 | `~/Applications/RustRover.app` (installed by JetBrains Toolbox) | ✅ |
| Rust 1.96.1 (`rustc`, `cargo`) | Homebrew, `/opt/homebrew/bin` | ✅ |
| Standard library sources | `/opt/homebrew/opt/rust/lib/rustlib/src/rust` | ✅ |
| `rustfmt` (formatter), `clippy` (linter) | Homebrew, `/opt/homebrew/bin` | ✅ |
| Node.js 24 (to run the JavaScript you compile) | `node` on your `PATH` | ✅ |

Nothing to install. The steps below only point RustRover at these tools.

> **Homebrew Rust or rustup?** The official way to install Rust is
> [rustup](https://rustup.rs), which makes switching Rust versions easy.
> Your Homebrew install has everything this tutorial needs, so keep it.
> If you ever switch, only the paths in step 2 change.

## 1. Open the workspace

The folder to open is **`language-project-rust`**, the one with the
top-level `Cargo.toml`. That file makes the folder a *Cargo workspace*:
it lists every crate in it (`example` and `mini`).

1. **File → Open…** → choose `~/Desktop/personal/language-project-rust` → **Open**.
2. Open the **Cargo** tool window (**View → Tool Windows → Cargo**). It
   should list two packages, **`maxim-compiler`** (the `example/` folder)
   and **`mini`**.

If the Cargo window is empty, or says no Cargo project was found (you
opened this folder before it had a `Cargo.toml`), do one of these:

- right-click the root `Cargo.toml` in the Project view → **Attach Cargo Project**, or
- **File → Close Project**, then open the folder again.

RustRover then indexes the project. The progress bar at the bottom
takes a minute the first time.

## 2. Point RustRover at the toolchain

Open **Settings** (`⌘,`) and type `Rust` in the search box to find the
**Rust** page.

| Field | Value |
|---|---|
| Toolchain location | `/opt/homebrew/bin` |
| Standard library | `/opt/homebrew/opt/rust/lib/rustlib/src/rust` |

RustRover usually detects both and shows the version (`1.96.1`). If it
fills in a path containing `Cellar/rust/1.96.1`, replace it with the
`opt` path above. Homebrew keeps `/opt/homebrew/opt/rust` pointing at
the current version, so the setting survives `brew upgrade rust`.

The standard library sources are what make **⌘-click on `String`,
`Option`, `Vec`…** open the real implementation. That is one of the best
ways to learn Rust.

> RustRover may say *rustup not found*. You can ignore that: only
> rustup-specific features (switching or downloading toolchains) are
> unavailable.

## 3. Format on save with rustfmt

`rustfmt` is the standard Rust formatter, so every Rust project looks
the same. In Settings, search for **`rustfmt`**:

- ✅ **Use rustfmt instead of the built-in formatter**
- ✅ **Run rustfmt on Save**

From the terminal, the same thing is `cargo fmt`.

## 4. Lint as you type with Clippy

Clippy is Rust's official linter, with hundreds of checks for
non-idiomatic or suspicious code. Its messages explain *why*, so they
teach you idiomatic Rust as you go. In Settings, search for
**`External Linters`**:

- **External tool**: `Clippy`
- ✅ **Run external linter on the fly** (the label may differ slightly between versions)

From the terminal, the same thing is `cargo clippy -p mini`.

## 5. Run tests from the editor

Open `mini/src/span.rs` and scroll to `mod tests`. Green ▶ icons appear
in the gutter:

- next to **`mod tests`**: runs every test in the module
- next to each **`#[test] fn`**: runs that one test

Click one and choose **Run**. The **Run** tool window shows each test
pass (green) or fail (red), with the failure message. Choose **Debug**
instead to stop at a breakpoint: click in the gutter next to a line
number to set one. Stepping through the lexer as it walks a string is a
great way to *see* the code work.

Right now **almost every test fails, on purpose**: you write the code
that makes them pass in [step 1](01-minimal-compiler/guide.md).

## 6. Run the compiler

Open RustRover's terminal (**View → Tool Windows → Terminal**). It
starts in the workspace folder. Useful commands:

```bash
cargo test -p mini
```

```bash
cargo run -p mini -- mini/programs/arithmetic.mini
```

```bash
cargo run -q -p mini -- mini/programs/arithmetic.mini | node
```

`-p mini` picks the package. Everything after `--` is passed to your
program instead of to Cargo. `-q` hides Cargo's own output, so only the
JavaScript reaches `node`.

To get a ▶ button for that command: **Run → Edit Configurations… → + →
Cargo**, set **Command** to
`run -p mini -- mini/programs/arithmetic.mini`, and leave the working
directory as the workspace folder.

## 7. Features worth knowing while you learn

| Feature | How | Why it helps |
|---|---|---|
| Inlay type hints | on by default (grey text after `let`) | shows the type Rust inferred for each variable |
| Quick documentation | hover a name, or `F1` | the docs for any function or type, including std |
| Go to definition | `⌘`-click | read how `Option::is_some_and` is really written |
| Expand macro | right-click a macro (`format!`, `assert_eq!`) → *Show Context Actions* → *Show recursive macro expansion* | macros stop being magic |
| Find usages | `⌥F7` | everywhere a function or type is used |
| Quick fixes | `⌥⏎` on a red or yellow squiggle | read the suggestion; often a lesson in itself |

Shortcuts are for the default macOS keymap. If you changed keymaps, use
**Help → Find Action** (`⌘⇧A`) and type the feature's name.

## Check that it works

In the terminal:

```bash
cargo test -p maxim-compiler
```

The reference compiler in `example/` should pass all its tests: 267
unit tests and 2 golden suites. It reads the shared fixtures and the
stdlib from `../language-project`, so keep both folders side by side.

```bash
cargo test -p mini
```

This shows **3 passed, 41 failed**, each failure saying
`not yet implemented: Part …`. That's your starting line.
Continue with **[Step 1 — A minimal compiler](01-minimal-compiler/guide.md)**.
