# Step 2 — The Rust you need

The Rust concepts step 2 adds to [step 1's](../01-minimal-compiler/rust-concepts.md),
in the order you'll meet them. Same format: what it is, a small
example, and **In Mini**, where it shows up.

Skim the whole page once before you start. The [guide](guide.md) links
back to the sections you need for each part.

**Contents**

1. [Lifetimes in types](#1-lifetimes-in-types)
2. [Owned vs borrowed: `String` vs `&str`](#2-owned-vs-borrowed-string-vs-str)
3. [Dependencies and build scripts](#3-dependencies-and-build-scripts)
4. [Slice patterns, and more patterns](#4-slice-patterns-and-more-patterns)
5. [`HashMap` and `HashSet`](#5-hashmap-and-hashset)
6. [Deriving `Hash`, `PartialOrd`, `Ord`](#6-deriving-hash-partialord-ord)
7. [Wrapping, checked and overflowing arithmetic](#7-wrapping-checked-and-overflowing-arithmetic)
8. [Traits as interfaces](#8-traits-as-interfaces)
9. [Trait objects: `Box<dyn Trait>`](#9-trait-objects-boxdyn-trait)
10. [Closures: `Fn`, `FnMut`, `FnOnce`](#10-closures-fn-fnmut-fnonce)
11. [Writing text: `fmt::Write` and `writeln!`](#11-writing-text-fmtwrite-and-writeln)
12. [More `Vec` methods](#12-more-vec-methods)
13. [Running other programs: `std::process::Command`](#13-running-other-programs-stdprocesscommand)
14. [Files, paths and `include_str!`](#14-files-paths-and-include_str)
15. [Raw strings and `const`](#15-raw-strings-and-const)
16. [Property-based testing with proptest](#16-property-based-testing-with-proptest)
17. [Check yourself](#17-check-yourself)

---

## 1. Lifetimes in types

[Book ch. 10.3](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)

In step 1, the lexer held a `&'src str`: a struct that borrows needs a
lifetime parameter. The same goes for any type that contains a
borrowing type, all the way up:

```rust
enum TokenKind<'src> {
    Int(i64),
    Ident(&'src str),   // borrows from the source
}

struct Token<'src> {
    kind: TokenKind<'src>,  // contains a borrow, so Token has a lifetime too
    span: Span,
}
```

`Token<'src>` reads "a token that can't outlive the text `'src`". A
`Vec<Token<'src>>` can't either.

Functions connect lifetimes between inputs and outputs. The **elision
rules** fill them in when you don't write them: with one reference
input, the output borrows from it; in a method with `&self` or
`&mut self`, the output borrows from `self`. Sometimes that's not what
you mean:

```rust
impl<'src> Lexer<'src> {
    // Elided: the token would borrow from the lexer (`&mut self`).
    // fn next_token(&mut self) -> Token { ... }

    // Explicit: the token borrows from the source, which outlives the lexer.
    fn next_token(&mut self) -> Token<'src> { ... }
}
```

`'_` means "a lifetime, elided": `fn lex(source: &str) -> Vec<Token<'_>>`
is the same as `fn lex<'a>(source: &'a str) -> Vec<Token<'a>>`. Writing
`Token<'_>` rather than just `Token` shows the reader that the type
borrows something; Clippy asks for it.

**In Mini:** `Token<'src>`, `TokenKind<'src>`, `Parser<'t>` holding
`&'t [Token<'t>]`, and `Cst<'src>` in Part 4.

---

## 2. Owned vs borrowed: `String` vs `&str`

[Book ch. 4.3](https://doc.rust-lang.org/book/ch04-03-slices.html)

`&str` borrows text that lives somewhere else; `String` owns its text.
Converting costs:

- `String` → `&str`: free (`&name`, or automatic through `Deref`);
- `&str` → `String`: a copy (`name.to_string()`, `String::from(name)`).

Rules of thumb:

| Borrow (`&str`) when                                | Own (`String`) when                                       |
|-----------------------------------------------------|-----------------------------------------------------------|
| the data is short-lived and read once               | the data lives long, moves around, or is stored           |
| the owner obviously outlives the borrower           | lifetimes would spread through many types                 |
| copying would cost a lot (many items)               | copying is cheap enough                                    |

**In Mini:** tokens borrow (`Ident(&'src str)`): thousands of them,
gone after parsing. The AST owns (`Var(String)`): it outlives the
tokens, and the IR copies the names into `Function::inputs`.

---

## 3. Dependencies and build scripts

[Cargo book: dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html),
[build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)

`Cargo.toml` has three kinds of dependencies:

```toml
[dependencies]          # used by the crate's code
lrpar = "=0.15.0"

[build-dependencies]    # used by build.rs
lrpar = "=0.15.0"

[dev-dependencies]      # used only by tests, examples and benchmarks
proptest = "1"
```

`"1"` means "any 1.x version"; `"=0.15.0"` means exactly that one.
`Cargo.lock` records the exact versions actually used, so every build
is the same.

A **build script**, `build.rs` next to `Cargo.toml`, is a program Cargo
compiles and runs before compiling the crate. Code generators run
there. It writes into the folder given by the `OUT_DIR` environment
variable, and the crate includes the generated code:

```rust
// What lrpar_mod!("yacc/mini.y") expands to, more or less:
mod mini_y {
    include!(concat!(env!("OUT_DIR"), "/yacc/mini.y.rs"));
}
```

`include!` pastes a file's contents as Rust code; `env!` reads an
environment variable *at compile time*.

**In Mini:** `build.rs` runs grmtools; `src/yacc/mod.rs` includes its
output with `lrlex_mod!` and `lrpar_mod!`.

---

## 4. Slice patterns, and more patterns

[Reference: patterns](https://doc.rust-lang.org/reference/patterns.html)

**Slice patterns** match a slice by its length and its elements:

```rust
match children.as_slice() {
    [] => "nothing",
    [only] => "one",
    [first, second] => "two",
    [first, .., last] => "at least two",   // `..` skips any number
}
```

They nest with every other pattern, including literals and struct
patterns:

```rust
match tokens {
    [Token { kind: "(", .. }, inner, Token { kind: ")", .. }] => ...,
}
```

`.as_slice()` turns a `Vec<T>` into a `&[T]`; a `match` on a `Vec`
itself doesn't accept slice patterns.

Three more pattern tools:

```rust
match op {
    // `|`: either pattern (both must bind the same names).
    Op::Binary(Add, x, Const(0)) | Op::Binary(Add, Const(0), x) => Some(x),
    // A guard: the arm matches only if the condition holds.
    Op::Binary(Sub, x, y) if x == y => Some(Const(0)),
    // `@`: bind a name to a value that matched a pattern.
    Op::Binary(op @ (Add | Mul), a, b) if b < a => Op::Binary(op, b, a),
    _ => ...,
}
```

**`let … else`** destructures or leaves:

```rust
let Cst::Node { children, .. } = tree else {
    unreachable!("a tree from the grammar is always a node")
};
// `children` is usable here
```

**In Mini:** `prune` (slice patterns), `simplify` and `canonical`
(`|`, guards, `@`).

---

## 5. `HashMap` and `HashSet`

[Book ch. 8.3](https://doc.rust-lang.org/book/ch08-03-hash-maps.html)

```rust
use std::collections::{HashMap, HashSet};

let mut temps: HashMap<Temp, i64> = HashMap::new();
temps.insert(Temp(0), 42);
temps[&Temp(0)];                 // 42; panics if missing
temps.get(&Temp(1));             // Option<&i64>: None
temps.contains_key(&Temp(0));    // true

let mut live: HashSet<Temp> = HashSet::new();
live.insert(Temp(3));            // returns false if it was already there
live.contains(&Temp(3));         // true
```

Lookups take a reference to the key (`&Temp(0)`). `get` returns a
reference to the value; for `Copy` values, `.copied()` turns
`Option<&V>` into `Option<V>`.

Iterating over a `HashMap` visits keys in an unpredictable order, which
changes from one run to the next. A compiler must produce the same
output every time, so never let that order leak into the output: Mini
only *looks up* in its maps. When order matters, use a `Vec`, or a
`BTreeMap` (sorted by key).

**In Mini:** the interpreter's temporaries, CSE's `seen`, DCE's `live`,
the LLVM backend's `slots`, `replace_uses`' `replaced`.

---

## 6. Deriving `Hash`, `PartialOrd`, `Ord`

[Book appendix C](https://doc.rust-lang.org/book/appendix-03-derivable-traits.html)

To be a `HashMap` key, a type needs `Hash` and `Eq`. To be compared
with `<` or sorted, `PartialOrd` and `Ord`. All can be derived, as long
as every field implements them:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Value {
    Const(i64),
    Input(usize),
    Temp(Temp),   // so Temp must derive them too
}
```

Derived `Ord` on an enum compares the variant first (in declaration
order: every `Const` is less than every `Input`), then the fields.

**In Mini:** `Op` is a `HashMap` key in CSE, so it derives `Hash`, and
so must `Value`, `Temp` and `BinOp` (that's why `BinOp` gained `Hash`).
CSE sorts operands with `b < a`, thanks to `Ord` on `Value`.

---

## 7. Wrapping, checked and overflowing arithmetic

[std: `i64`](https://doc.rust-lang.org/std/primitive.i64.html)

`a + b` on integers panics on overflow in debug builds, and wraps in
release builds. When you need one behaviour, ask for it:

```rust
i64::MAX.wrapping_add(1)     // i64::MIN: wraps around
i64::MAX.checked_add(1)      // None
i64::MAX.overflowing_add(1)  // (i64::MIN, true)
i64::MAX.saturating_add(1)   // i64::MAX: sticks at the limit

7i64.wrapping_div(2)         // 3
i64::MIN.wrapping_div(-1)    // i64::MIN
1i64.wrapping_div(0)         // still panics: no integer result exists
1i64.checked_div(0)          // None
```

`wrapping_neg`, `wrapping_sub` and `wrapping_mul` exist too.

**In Mini:** `eval_binary` (Mini's arithmetic), the interpreter's
negation, and the constant folder.

---

## 8. Traits as interfaces

[Book ch. 10.2](https://doc.rust-lang.org/book/ch10-02-traits.html)

A trait declares methods that types promise to provide:

```rust
pub trait Pass {
    fn name(&self) -> &'static str;
    fn run(&self, function: &mut Function) -> bool;
}

pub struct Fold;    // a "unit struct": no fields, just a type

impl Pass for Fold {
    fn name(&self) -> &'static str { "fold" }
    fn run(&self, function: &mut Function) -> bool { ... }
}
```

Unit structs are common for implementations that need no data: `Fold`
is both the type and its only value.

**In Mini:** `Pass`, implemented by `Fold`, `Cse` and `Dce`.

---

## 9. Trait objects: `Box<dyn Trait>`

[Book ch. 18.2](https://doc.rust-lang.org/book/ch18-02-trait-objects.html)

A `Vec` holds values of one type. To hold a `Fold`, a `Cse` and a `Dce`
in one list, store them as **trait objects**:

```rust
let passes: Vec<Box<dyn Pass>> = vec![Box::new(Fold), Box::new(Cse), Box::new(Dce)];
for pass in &passes {
    pass.run(&mut function);    // calls Fold's run, then Cse's, then Dce's
}
```

`dyn Pass` is "some type that implements `Pass`", unknown until
runtime. Its size isn't known either, so it lives behind a pointer:
`Box<dyn Pass>`, `&dyn Pass` or `&mut dyn Pass`. Each call looks up the
right method at runtime (**dynamic dispatch**).

The alternative is generics, `fn add(pass: impl Pass)`: the compiler
generates one copy of the function per type (**static dispatch**):
faster, but one call site can only ever handle one type.

**In Mini:** `PassManager` stores `Vec<Box<dyn Pass>>`; `with(pass: impl Pass + 'static)`
takes any pass and boxes it. (`'static` says the pass doesn't borrow
anything short-lived: the box may keep it forever.)

---

## 10. Closures: `Fn`, `FnMut`, `FnOnce`

[Book ch. 13.1](https://doc.rust-lang.org/book/ch13-01-closures.html)

A closure can use variables from where it's written. How it uses them
decides which trait it implements:

| Trait    | The closure…                         | Can be called |
|----------|--------------------------------------|---------------|
| `Fn`     | only reads what it captured          | any number of times, even at once |
| `FnMut`  | modifies what it captured            | any number of times, one at a time |
| `FnOnce` | consumes (moves out) what it captured | once         |

A function that takes a closure says which it needs; ask for the
weakest guarantee you can (`FnOnce` accepts all closures, `Fn` the
fewest):

```rust
fn replace_uses(function: &mut Function, mut replacement: impl FnMut(&Inst) -> Option<Value>) -> bool

let mut seen = HashMap::new();
replace_uses(function, |inst| {
    seen.insert(inst.op, inst.dest);   // modifies `seen`: this closure is FnMut
    None
});
```

A closure can also be passed as a trait object, `&mut dyn FnMut(&str, &Function)`,
when the function shouldn't be generic: `PassManager::run` does that
with `trace`.

**In Mini:** `replace_uses`, `Op::map_operands`, the `trace` callback
of `PassManager::run`.

---

## 11. Writing text: `fmt::Write` and `writeln!`

[std: `fmt::Write`](https://doc.rust-lang.org/std/fmt/trait.Write.html)

`write!` and `writeln!` work on a `Formatter` (in `Display`), and on a
`String` too, once the `fmt::Write` trait is in scope:

```rust
use std::fmt::{self, Write};

fn emit(out: &mut String) -> fmt::Result {
    writeln!(out, "define i64 @mini_main() {{")?;   // `{{` writes `{`
    writeln!(out, "}}")
}
```

They return a `fmt::Result`, which `?` propagates. Writing into a
`String` never fails, so at the top, one `.expect("...")` is fine.

**In Mini:** the IR printer (`Display for Function`), both backends.

---

## 12. More `Vec` methods

```rust
v.contains(&x)                   // is x in v?
v.iter().position(|y| *y == x)   // Option<usize>: where is it?
v.iter().rev()                   // iterate backwards
v.retain(|x| keep(x))            // remove, in place, every element for which keep is false
names.join(", ")                 // Vec<String> → "a, b, c"
v.first(), v.last()              // Option<&T>
```

**In Mini:** `collect_inputs` (`contains`), `Lowerer::input`
(`position`), DCE (`rev`, `retain`), the printers (`join`).

---

## 13. Running other programs: `std::process::Command`

[std: `Command`](https://doc.rust-lang.org/std/process/struct.Command.html)

```rust
use std::process::Command;

let output = Command::new("clang")
    .arg("-O2")
    .arg(&ll_file)
    .arg("-o")
    .arg(&executable)
    .output();              // run it, wait, collect stdout and stderr

match output {
    Err(err) => { /* couldn't start it: not installed? */ }
    Ok(out) if !out.status.success() => { /* it ran, and failed: read out.stderr */ }
    Ok(out) => { /* success: out.stdout */ }
}
```

`out.stdout` and `out.stderr` are bytes (`Vec<u8>`);
`String::from_utf8_lossy(&out.stdout)` turns them into text.

**In Mini:** `build_native` in `driver.rs` runs clang; the end-to-end
tests run `node`, the native executables, and `mini` itself.

---

## 14. Files, paths and `include_str!`

```rust
use std::path::{Path, PathBuf};

let dir: PathBuf = std::env::temp_dir().join("mini-123");   // an owned path
std::fs::create_dir_all(&dir)?;
std::fs::write(dir.join("program.ll"), text)?;
let stem: PathBuf = Path::new("prog.mini").with_extension("");   // "prog"
std::fs::remove_dir_all(&dir)?;
```

`PathBuf` is to `Path` what `String` is to `str`. Paths aren't
`String`s because file names don't have to be valid UTF-8; to print
one, use `path.display()`.

`include_str!("../runtime/mini_rt.c")` embeds a file's contents into
the program, as a `&'static str`, at compile time: the `mini` binary
carries the runtime with it.

In tests, `env!("CARGO_TARGET_TMPDIR")` is a folder for test files and
`env!("CARGO_BIN_EXE_mini")` the path of the compiled `mini` command.

**In Mini:** `build_native` in `driver.rs`, `main.rs`, the end-to-end
tests.

---

## 15. Raw strings and `const`

A **raw string** has no escapes: `r"\d+"` is the five characters
`\d+`. With `#`s, it can contain quotes: `r#"say "hi""#`. Handy for
embedding code in other languages.

A **`const`** is a value computed at compile time and copied wherever
it's used:

```rust
const MAX_ROUNDS: usize = 10;
const PRELUDE: &str = r#"function div(a, b) { ... }"#;
```

**In Mini:** the backends' `PRELUDE`s; `MAX_ROUNDS` in the pass
manager; `USAGE` in `main.rs`.

---

## 16. Property-based testing with proptest

[The proptest book](https://proptest-rs.github.io/proptest/)

A unit test checks one example. A **property test** states something
true for *all* inputs, and checks it on many random ones:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn reversing_twice_gives_back_the_original(v in prop::collection::vec(any::<i64>(), 0..10)) {
        let mut w = v.clone();
        w.reverse();
        w.reverse();
        prop_assert_eq!(w, v);
    }
}
```

`v in ...` takes values from a **strategy**, a recipe for random
values. Strategies combine: `prop_oneof![a, b]` picks one of several,
`prop_map` transforms, `prop_recursive` builds trees (like random
programs). When a case fails, proptest **shrinks** it: it tries smaller
and simpler values, as long as the test still fails, and reports the
smallest one. That turns "fails on this 300-character program" into
"fails on `x / x`".

Use `prop_assert!` and `prop_assert_eq!` inside, not `assert!`: they
let shrinking work.

**In Mini:** `tests/properties.rs`: random programs for the two front
ends, the IR, and the optimizer.

---

## 17. Check yourself

1. Why does `Token` need a lifetime parameter, and `Expr` doesn't?
2. In `fn next_token(&mut self) -> Token`, which lifetime would the
   token get, and why would `lex` then fail to compile?
3. What's the difference between `[dependencies]` and
   `[build-dependencies]`? Why does Mini list grmtools in both?
4. Write a slice pattern that matches a slice of exactly three
   elements whose middle one is `0`.
5. Why can `Op` be a `HashMap` key? What would you need to add to a new
   field's type to keep it that way?
6. What do `i64::MIN.wrapping_neg()`, `i64::MIN.checked_neg()` and
   `(-7i64).wrapping_div(2)` return?
7. Why does `PassManager` store `Box<dyn Pass>` rather than a generic
   `P: Pass`?
8. The closure passed to `replace_uses` in CSE inserts into `seen`.
   Which closure trait does it implement, and why?

<details>
<summary>Answers</summary>

1. A token can borrow its name from the source (`&'src str`), so it
   must not outlive it. `Expr` owns its names (`String`), so it borrows
   nothing.
2. Elision gives it the lifetime of `&mut self`, the borrow of the
   lexer. `lex` creates the lexer locally and drops it when it returns,
   so the tokens couldn't be returned.
3. `[dependencies]` are for the crate's code, `[build-dependencies]`
   for `build.rs`. grmtools runs in `build.rs` (to generate the parser)
   and its runtime is used by the generated code.
4. `[_, 0, _]`.
5. It derives `Hash` and `Eq`, and so do all its fields' types. A new
   field's type must implement `Hash` and `Eq` too.
6. `i64::MIN`, `None`, `-3`.
7. The passes have different types (`Fold`, `Cse`, `Dce`); a generic
   `Vec<P>` holds a single type.
8. `FnMut`: it modifies a captured variable, so it needs mutable access
   to it, one call at a time.

</details>
