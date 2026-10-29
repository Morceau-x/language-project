# Step 1 — The Rust you need

Every Rust concept used in step 1, in the order you'll meet them. Each
section is short: what it is, a small example, and **In Mini**, where it
shows up in the code you're about to write.

Skim the whole page once before you start. The [guide](guide.md) then
links back to the sections you need for each part.

For depth, the official book [*The Rust Programming
Language*](https://doc.rust-lang.org/book/) ("the Book") is excellent
and free. Section titles below link to the matching chapter.

**Contents**

1. [Cargo: packages, crates, workspaces](#1-cargo-packages-crates-workspaces)
2. [Modules and visibility](#2-modules-and-visibility)
3. [Variables and mutability](#3-variables-and-mutability)
4. [Numbers, `char`, `bool`](#4-numbers-char-bool)
5. [Strings: `String`, `&str` and UTF-8](#5-strings-string-str-and-utf-8)
6. [Structs and methods](#6-structs-and-methods)
7. [Enums](#7-enums)
8. [Pattern matching](#8-pattern-matching)
9. [Ownership and moves](#9-ownership-and-moves)
10. [Borrowing: `&` and `&mut`](#10-borrowing--and-mut)
11. [Lifetimes, just enough](#11-lifetimes-just-enough)
12. [`derive`, `Copy` and `Clone`](#12-derive-copy-and-clone)
13. [`Option`](#13-option)
14. [`Result` and the `?` operator](#14-result-and-the--operator)
15. [`Box` and recursive types](#15-box-and-recursive-types)
16. [Traits: `Display` and `Into`](#16-traits-display-and-into)
17. [Closures and iterators](#17-closures-and-iterators)
18. [Loops, and everything is an expression](#18-loops-and-everything-is-an-expression)
19. [Macros you'll meet](#19-macros-youll-meet)
20. [Tests](#20-tests)
21. [Panics vs errors](#21-panics-vs-errors)
22. [The command-line bits](#22-the-command-line-bits)
23. [Check yourself](#23-check-yourself)

---

## 1. Cargo: packages, crates, workspaces

[Book ch. 1.3](https://doc.rust-lang.org/book/ch01-03-hello-cargo.html), [ch. 14.3](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)

Cargo is Rust's build tool and package manager.

- A **package** is a folder with a `Cargo.toml`.
- A **crate** is one unit of compilation: either a **library**
  (`src/lib.rs`) or a **binary**, i.e. an executable (`src/main.rs`). A
  package can have one of each.
- A **workspace** groups packages so they share one `Cargo.lock` (exact
  dependency versions) and one `target/` folder (build output).

```text
language-project-rust/         workspace: the root Cargo.toml lists the members
├── Cargo.toml
├── example/                   package `maxim-compiler` (the reference compiler)
└── mini/                      package `mini`
    ├── Cargo.toml
    ├── src/lib.rs             library crate: the compiler itself
    ├── src/main.rs            binary crate: the `mini` command, using the library
    └── tests/end_to_end.rs    integration tests
```

| Command | Does |
|---|---|
| `cargo build -p mini` | compile |
| `cargo run -p mini -- ARGS` | compile and run the binary with `ARGS` |
| `cargo test -p mini [FILTER]` | compile and run tests (only those whose name contains `FILTER`) |
| `cargo clippy -p mini` | lint |
| `cargo fmt` | format everything |

**In Mini:** splitting the code into a library (`lib.rs`) and a thin
binary (`main.rs`) is standard practice. Tests and other programs can
use the compiler without going through the command line.

## 2. Modules and visibility

[Book ch. 7](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)

`pub mod lexer;` in `lib.rs` tells Rust: there is a module named `lexer`,
and its code is in `src/lexer.rs`. Then:

```rust
use crate::span::Span;          // `crate` = the root of this crate
use crate::token::{Token, TokenKind};
use super::*;                   // `super` = the parent module (used by tests)
```

Everything is **private by default**: visible only inside its own
module and that module's children. `pub` makes an item (function,
struct, field, module) visible outside.

```rust
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> { /* ... */ }  // public
struct Lexer<'src> { /* ... */ }                                         // private
```

**In Mini:** each phase exposes one public entry point (`lexer::lex`,
`parser::parse`, `codegen::generate`) and keeps its helpers private. A
phase can then change its insides without breaking the others.
`pub use diagnostic::Diagnostic;` in `lib.rs` *re-exports* the type, so
users can write `mini::Diagnostic`.

## 3. Variables and mutability

[Book ch. 3.1](https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html)

```rust
let x = 5;           // immutable: x can never change
let mut count = 0;   // mutable
count += 1;
let tokens: Vec<Token> = Vec::new();   // explicit type annotation
```

Rust **infers** most types, and RustRover shows the inferred type in
grey. Annotate when inference needs help (often with `collect()` and
`parse()`), or when it makes the code clearer.

**In Mini:** `let mut lexer = Lexer::new(source);` needs `mut` because
reading tokens changes the lexer's position.

## 4. Numbers, `char`, `bool`

[Book ch. 3.2](https://doc.rust-lang.org/book/ch03-02-data-types.html)

| Type | What | In Mini |
|---|---|---|
| `i64` | signed 64-bit integer | the value of an integer literal |
| `usize` | unsigned, pointer-sized: *the* type for lengths and indexes | byte offsets in `Span`, `pos` |
| `char` | one Unicode character (`'a'`, `'é'`, `'+'`), always 4 bytes | what the lexer reads |
| `bool` | `true` / `false` | conditions |

```rust
let digit = '7';
digit.is_ascii_digit();     // true
' '.is_whitespace();        // true, and so are '\n' and '\t'
'é'.len_utf8();             // 2: bytes needed to store it in UTF-8
"42".parse::<i64>();        // Ok(42)
"99999999999999999999".parse::<i64>();   // Err(..): too big for i64
```

In a debug build, arithmetic that overflows **panics** instead of
silently wrapping around. That's one of Rust's safety nets.

`'0'..='9'` is an *inclusive range* of chars; you can use it as a
pattern in `match`.

## 5. Strings: `String`, `&str` and UTF-8

[Book ch. 8.2](https://doc.rust-lang.org/book/ch08-02-strings.html), [ch. 4.3](https://doc.rust-lang.org/book/ch04-03-slices.html)

Rust has two main string types:

| | `String` | `&str` |
|---|---|---|
| What | owns its text, can grow | a *view* into text owned by someone else |
| Typical use | building output | reading input, function parameters |
| Example | `String::from("hi")`, `format!(...)` | `"hi"` (a literal), `&source[2..5]` |

Strings are **UTF-8**: one character can take 1 to 4 bytes. All
offsets and slices are in **bytes**:

```rust
let s = "é1";
s.len();                    // 3 bytes (é is 2 bytes)
s.chars().count();          // 2 characters
&s[2..];                    // "1": slicing by byte range
// &s[1..]                  // PANIC: byte 1 is in the middle of 'é'
s[0..].chars().next();      // Some('é'): the first char at an offset
```

Building strings:

```rust
let mut out = String::new();
out.push('(');              // a char
out.push_str("1 + 2");      // a &str
out.push(')');
out += &format!(" = {}", 3);   // += takes a &str, hence the &
let n = 42.to_string();     // any Display type can become a String
```

**In Mini:** the lexer reads a `&str` and tracks a byte offset `pos`.
Spans are byte offsets too, so they can slice the source directly. The
code generator builds its output in a `String`.

**Rule of thumb:** parameters take `&str` (works for both string types);
functions that create text return `String`.

## 6. Structs and methods

[Book ch. 5](https://doc.rust-lang.org/book/ch05-00-structs.html)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    // An associated function (no `self`), called as `Span::new(0, 3)`.
    // `new` is just a convention, not a keyword.
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }   // shorthand for `Span { start: start, end: end }`
    }

    // A method, called as `span.len()`.
    pub fn len(&self) -> usize {
        self.end - self.start
    }
}
```

The first parameter of a method says how it uses the value:

| Receiver | Meaning | In Mini |
|---|---|---|
| `&self` | reads it | `Lexer::peek`: look without moving |
| `&mut self` | changes it | `Lexer::bump`: advance `pos` |
| `self` | takes it (or a copy, if the type is `Copy`) | `Span::merge(self, other)` |

**Struct update syntax** copies every field you don't mention from
another value:

```rust
let wider = Expr { span: new_span, ..inner };   // all fields of `inner` except `span`
```

**In Mini:** `Span`, `Token`, `Expr`, `Lexer`, `Parser`, `Diagnostic`.
The parser uses struct update syntax to widen a parenthesized
expression's span.

## 7. Enums

[Book ch. 6.1](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)

A Rust `enum` is a type that is *one of* several variants, and each
variant can carry its own data. This is the most useful tool for
writing compilers.

```rust
pub enum TokenKind {
    Int(i64),        // tuple-like variant: carries an i64
    Plus,            // unit variant: no data
    Eof,
}

pub enum ExprKind {
    Int(i64),
    Neg(Box<Expr>),
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },   // struct-like variant
}

let t = TokenKind::Int(42);
let e = ExprKind::Binary { op: BinOp::Add, lhs, rhs };
```

**In Mini:** `TokenKind` (what a token is), `ExprKind` (what an
expression is), `BinOp` (which operator).

## 8. Pattern matching

[Book ch. 6.2](https://doc.rust-lang.org/book/ch06-02-match.html), [ch. 19](https://doc.rust-lang.org/book/ch19-00-patterns.html)

`match` compares a value against patterns, top to bottom, and runs the
first arm that matches. It must be **exhaustive**: every possible value
must be handled, or the code doesn't compile. Add a variant to an enum
and the compiler points at every `match` you need to update.

```rust
let kind = match c {
    '+' => TokenKind::Plus,
    '0'..='9' => self.int(start)?,       // a range of chars
    other => return Err(/* ... */),      // a name matches anything, and binds it
};
```

`match` is an **expression**: it produces a value (`kind` above). Other
patterns you'll use:

```rust
match &expr.kind {
    ExprKind::Int(value) => { /* value: &i64 */ }
    // A nested pattern: only Binary whose op is Div.
    ExprKind::Binary { op: BinOp::Div, lhs, rhs } => { /* ... */ }
    // Any other Binary. Order matters: this arm comes after the Div one.
    ExprKind::Binary { op, lhs, rhs } => { /* ... */ }
    _ => { /* `_` matches anything and binds nothing */ }
}
```

Matching on a reference (`&expr.kind`) binds the fields *by reference*,
so nothing is moved out of `expr`.

**`let ... else`** handles the one pattern you expect and bails out on
everything else:

```rust
let Some(c) = self.bump() else {
    return Ok(eof_token);   // must leave the function (return, break, panic...)
};
// here `c` is a plain char
```

## 9. Ownership and moves

[Book ch. 4.1](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html)

Rust's core idea: **every value has exactly one owner**. When the owner
goes away, the value is freed, with no garbage collector. Assigning or
passing a value **moves** ownership; the old variable can't be used
anymore:

```rust
let a = String::from("hi");
let b = a;              // moved: b owns the string now
// println!("{a}");     // compile error: `a` was moved
```

(Small `Copy` types like integers are copied instead; see
[section 12](#12-derive-copy-and-clone).)

A pattern you'll write in the parser **moves a value out of a variable
and puts a new one back**:

```rust
let mut lhs = self.term()?;
loop {
    // ...
    let rhs = self.term()?;
    lhs = Expr::binary(op, lhs, rhs);   // old `lhs` moves into the new node,
}                                       // and `lhs` now holds the bigger tree
```

That's legal because `lhs` gets a new value right away.

## 10. Borrowing: `&` and `&mut`

[Book ch. 4.2](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)

Instead of moving, you can **borrow**, i.e. lend access:

- `&T` is a **shared** borrow: read-only, and many can exist at once.
- `&mut T` is a **mutable** borrow: read and write, and *only one* can
  exist at a time, with no shared ones alongside.

These rules, checked at compile time, are what make Rust memory-safe
and free of data races.

```rust
fn emit_expr(expr: &Expr, out: &mut String) {
    // reads the tree, appends to the output
}

let mut out = String::new();
emit_expr(&tree, &mut out);
```

Two conveniences you'll rely on:

- Method calls borrow automatically: `self.peek()` passes `&self`
  without you writing `&`.
- `&Vec<T>` turns into `&[T]` (a *slice*, a view of a sequence) where
  needed, so `parse(&tokens)` works with `fn parse(tokens: &[Token])`.

**In Mini:** the parser borrows the tokens (`&[Token]`), and the code
generator borrows the tree and mutably borrows the output.

## 11. Lifetimes, just enough

[Book ch. 10.3](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)

A struct that *holds a borrow* must say how long that borrow lives:

```rust
struct Lexer<'src> {
    source: &'src str,   // a view into text owned elsewhere
    pos: usize,
}
```

`'src` is a **lifetime parameter**. It means "a `Lexer` cannot outlive
the source text it reads". You don't choose how long anything lives;
you *name* a relationship, and the compiler checks it.

```rust
impl<'src> Lexer<'src> { /* ... */ }   // methods for a Lexer of any lifetime
impl Parser<'_> { /* ... */ }          // `'_`: "some lifetime, I don't need its name"
fn symbol(self) -> &'static str        // `'static`: lives for the whole program
                                        // (string literals do)
```

Most of the time lifetimes are inferred, and you only write them on
structs that hold references. When they're wrong, the compiler error
tells you what to add.

## 12. `derive`, `Copy` and `Clone`

[Book appendix C](https://doc.rust-lang.org/book/appendix-03-derivable-traits.html)

`#[derive(...)]` asks the compiler to write standard trait
implementations for you:

| Derive | Gives you | Needed for |
|---|---|---|
| `Debug` | `{:?}` formatting | `assert_eq!` failure messages, debugging |
| `Clone` | `.clone()`: an explicit copy | copying owned data on purpose |
| `Copy` | implicit copies instead of moves | small plain data |
| `PartialEq` | `==` and `!=` | comparisons, `assert_eq!` |
| `Eq` | promises `==` is a full equality (unlike floats, where `NaN != NaN`) | hash maps, later |

`Copy` is only allowed when every field is `Copy` (numbers, `char`,
`bool`, shared references, and structs or enums made only of those).

**In Mini:** `Span`, `Token` and `TokenKind` are `Copy`, so
`parser.peek()` can hand you a `Token` *by value* and you never worry
about moving it. `Expr` is not `Copy`: it owns `Box`es, so copying it
means copying a whole tree. That has to be explicit (`.clone()`), and
Mini never needs it.

## 13. `Option`

[Book ch. 6.1](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html#the-option-enum-and-its-advantages-over-null-values)

Rust has no `null`. A value that may be missing is an `Option<T>`:

```rust
enum Option<T> { Some(T), None }   // defined in the standard library
```

```rust
let next: Option<char> = "abc".chars().next();     // Some('a')
let none: Option<char> = "".chars().next();        // None

next.is_some_and(|c| c.is_ascii_digit());          // false: Some but not a digit
next.unwrap_or(' ');                               // 'a', or ' ' if None
text.rfind('\n').map_or(0, |i| i + 1);             // map Some(i) to i + 1, None to 0
```

The `?` operator works on `Option` too. Inside a function returning
`Option`, `let c = self.peek()?;` means "if `peek()` is `None`, return
`None` right now; otherwise give me the `char`".

**In Mini:** `Lexer::peek` and `Lexer::bump` return `Option<char>`:
`None` means end of input.

## 14. `Result` and the `?` operator

[Book ch. 9.2](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)

An operation that can fail returns a `Result`:

```rust
enum Result<T, E> { Ok(T), Err(E) }
```

`?` is the heart of error handling in Rust: **on `Err`, return the
error from the current function; on `Ok`, unwrap the value**.

```rust
pub fn compile(source: &str) -> Result<String, Diagnostic> {
    let tokens = lexer::lex(source)?;      // a lexer error stops here...
    let expr = parser::parse(&tokens)?;    // ...a parser error here...
    Ok(codegen::generate(&expr))           // ...otherwise wrap the success in Ok
}
```

Without `?`, each line would be a `match` with an early `return`. Use
`?` everywhere in the compiler. Use `.unwrap()` (panic on `Err`) and
`.unwrap_err()` only in tests, where a panic is just a failing test.

**In Mini:** every phase returns `Result<_, Diagnostic>`, and `?`
passes the first error up to `main`, which prints it.

## 15. `Box` and recursive types

[Book ch. 15.1](https://doc.rust-lang.org/book/ch15-01-box.html)

Rust must know the size of every type at compile time. This can't
work:

```rust
enum ExprKind {
    Neg(Expr),   // error: recursive type has infinite size
}
```

An `Expr` containing an `Expr` containing an `Expr`… would be infinitely
big. `Box<T>` stores the `T` on the heap and is itself just a pointer,
of fixed size:

```rust
enum ExprKind {
    Neg(Box<Expr>),   // fine: a pointer to an Expr
}

let node = ExprKind::Neg(Box::new(operand));   // put the operand in a box
```

A `Box<Expr>` behaves almost like an `Expr`: you can call `Expr`
methods on it and print it with `{}`. When a function wants an `&Expr`
and you have an `&Box<Expr>`, pass it as is: Rust looks through the box
for you (this is called *deref coercion*).

**In Mini:** the AST is a tree, so its nodes hold their children in
`Box`es.

## 16. Traits: `Display` and `Into`

[Book ch. 10.2](https://doc.rust-lang.org/book/ch10-02-traits.html)

A **trait** is a set of methods a type can implement, like an
interface. Implementing the standard trait `fmt::Display` decides how a
type prints with `{}`:

```rust
use std::fmt;

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => f.write_str("+"),          // write a fixed string
            // ...
        }
    }
}

// or, with formatting:
write!(f, "({} {lhs} {rhs})", op.symbol())   // like format!, but into `f`
```

Implementing `Display` also gives you `.to_string()` for free.
(`{:?}` uses `Debug` instead, which `#[derive(Debug)]` writes for you.)

Traits also let functions accept *any type with some ability*:

```rust
pub fn new(message: impl Into<String>, span: Span) -> Diagnostic {
    Diagnostic { message: message.into(), span }
}

Diagnostic::new("fixed text", span);                 // a &str
Diagnostic::new(format!("found {kind}"), span);      // a String
```

**In Mini:** `TokenKind` implements `Display` for error messages, and
you implement it for `Expr` (Part 3).

## 17. Closures and iterators

[Book ch. 13](https://doc.rust-lang.org/book/ch13-00-functional-features.html)

A **closure** is an anonymous function, written `|params| body`:

```rust
let is_digit = |c: char| c.is_ascii_digit();
self.peek().is_some_and(|c| c.is_ascii_digit())   // passed straight to a method
```

**Iterators** produce a sequence of values. You chain adapters and
finish with a consumer:

```rust
let kinds: Vec<TokenKind> = tokens
    .iter()                    // borrow each token: &Token
    .map(|token| token.kind)   // transform each one
    .collect();                // gather into a collection (the type says which)

for (source, expected) in cases { /* `for` consumes any iterator */ }
```

`.iter()` borrows the elements, while `.into_iter()` takes ownership
of them. Iterators are lazy: nothing happens until a consumer
(`collect`, `for`, `count`…) pulls the values.

**In Mini:** mostly in tests, plus `chars()` in the lexer.

## 18. Loops, and everything is an expression

[Book ch. 3.5](https://doc.rust-lang.org/book/ch03-05-control-flow.html)

```rust
loop { /* forever, until `break` or `return` */ }
while self.peek() == Some(' ') { self.bump(); }
for token in &tokens { /* ... */ }
```

Almost everything in Rust is an **expression** with a value: `if`,
`match`, blocks. A block's value is its last expression, *without* a
semicolon:

```rust
fn double(x: i64) -> i64 {
    x * 2          // no `;`: this is the return value
}

let sign = if n < 0 { "-" } else { "" };
```

`return` is for leaving *early*. The last expression is the normal way
to return.

## 19. Macros you'll meet

[Book ch. 20.5](https://doc.rust-lang.org/book/ch20-05-macros.html)

A name followed by `!` is a **macro**: code that writes code at compile
time. They're used where a function can't do the job, for example
taking a variable number of arguments or checking a format string at
compile time.

| Macro | Does |
|---|---|
| `println!` / `print!` | print to stdout (with / without a newline) |
| `eprintln!` / `eprint!` | print to stderr |
| `format!` | build a `String` |
| `write!` | format into a `Formatter` (or a `String`) |
| `vec![a, b]` | build a `Vec` |
| `assert_eq!(a, b)` / `assert!(cond)` | fail the test if not equal / not true |
| `todo!("...")` | panic with "not yet implemented: ..." (placeholder code) |
| `panic!("...")` | crash on purpose (bugs only) |

In a format string, `{}` uses `Display`, `{:?}` uses `Debug`, and
variables can go inline: `format!("expected {kind}")`.

## 20. Tests

[Book ch. 11](https://doc.rust-lang.org/book/ch11-00-testing.html)

**Unit tests** live next to the code, in a module compiled only for
`cargo test`:

```rust
#[cfg(test)]           // only compiled when testing
mod tests {
    use super::*;      // bring the parent module's items into scope

    #[test]
    fn merge_covers_both_spans() {
        assert_eq!(Span::new(0, 1).merge(Span::new(4, 5)), Span::new(0, 5));
    }
}
```

The `tests` module is a *child* of the module it tests, so it can call
private functions too (that's how the tests reach `Lexer::bump`).

**Integration tests** are files in `tests/`. Each is compiled as a
separate crate that sees only the public API, just like a real user.

```bash
cargo test -p mini                         # everything
cargo test -p mini lexer                   # tests whose path contains "lexer"
cargo test -p mini lexer::tests::bump      # just the bump tests
```

**In Mini:** the scaffold ships with the tests already written, and
your job is to make them pass. Read a test before implementing the
function it checks: tests are the most precise specification you'll
get.

## 21. Panics vs errors

[Book ch. 9.3](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html)

Rust separates two kinds of failure:

- **Panics** are bugs: `todo!()`, indexing out of bounds, `.unwrap()`
  on `None`. The program stops with a message and a location.
- **`Result` errors** are expected failures that the caller handles,
  like a file that doesn't exist.

For a compiler this gives a firm rule: **a user's mistake must never
crash the compiler**. Every bad program gets a `Diagnostic`, a `Result`
error with a message and a span. A panic in a compiler means a bug in
the compiler.

**In Mini:** `self.tokens[self.pos]` in the parser could panic if `pos`
were past the end. It can't be: the lexer always ends the list with
`Eof` and the parser never moves past it. That's an *invariant*,
written down in the doc comments, and a panic there would mean a
compiler bug.

## 22. The command-line bits

[Book ch. 12](https://doc.rust-lang.org/book/ch12-00-an-io-project.html)

Only `main.rs` deals with the outside world. It's already written;
here's what it uses:

```rust
use std::process::ExitCode;

fn main() -> ExitCode {                        // main can return an exit code
    let args: Vec<String> = std::env::args().collect();   // program name + arguments
    let [_, path] = args.as_slice() else {     // slice pattern: exactly 2 elements
        eprintln!("usage: mini FILE.mini");
        return ExitCode::FAILURE;              // exit status 1
    };
    let source = std::fs::read_to_string(path);   // Result<String, std::io::Error>
    // ...
}
```

Compiled JavaScript goes to **stdout** (`print!`) and errors go to
**stderr** (`eprint!`). That way `mini file.mini | node` sends only
JavaScript to Node, and errors still show in your terminal.

## 23. Check yourself

Try to answer before opening each one.

<details>
<summary>1. Why does <code>Lexer::bump</code> do <code>self.pos += c.len_utf8()</code> instead of <code>self.pos += 1</code>?</summary>

`pos` is a byte offset, and a character can be up to 4 bytes in UTF-8.
Adding 1 after reading `é` (2 bytes) would leave `pos` in the middle of
the character, and the next slice `&source[pos..]` would panic.

</details>

<details>
<summary>2. <code>Token</code> is <code>Copy</code> but <code>Expr</code> isn't. Why?</summary>

`Copy` requires every field to be `Copy`. `Token` holds a `TokenKind`
and a `Span`, which are plain numbers. `Expr` owns its children through
`Box<Expr>`, and a `Box` can't be copied implicitly: copying it means
copying the whole subtree.

</details>

<details>
<summary>3. What does <code>?</code> do in <code>let tokens = lexer::lex(source)?;</code>?</summary>

If `lex` returns `Err(diagnostic)`, the current function returns
`Err(diagnostic)` immediately. If it returns `Ok(tokens)`, the
expression evaluates to `tokens`.

</details>

<details>
<summary>4. Why must <code>ExprKind::Neg</code> hold a <code>Box&lt;Expr&gt;</code> and not an <code>Expr</code>?</summary>

A type that directly contains itself would have infinite size. `Box`
puts the child on the heap and stores a fixed-size pointer.

</details>

<details>
<summary>5. <code>emit_expr</code> takes <code>expr: &amp;Expr</code> and <code>out: &amp;mut String</code>. What would go wrong if it took <code>expr: Expr</code>?</summary>

It would take ownership of the tree and consume it. The caller couldn't
use the tree afterwards, and the recursive calls would have to move
children out of their boxes. Code generation only needs to *read* the
tree, so a shared borrow is the right choice.

</details>

<details>
<summary>6. Your <code>match</code> on <code>ExprKind</code> doesn't compile: "non-exhaustive patterns: <code>ExprKind::Neg(_)</code> not covered". What happened?</summary>

The `match` has no arm for `Neg`. Rust requires every variant to be
handled. Add the arm (or a `_` catch-all, but an explicit arm is better
in a compiler, so new variants can't slip through unnoticed).

</details>

<details>
<summary>7. Why can the <code>tests</code> module call the private method <code>Lexer::bump</code>?</summary>

Private items are visible in the module where they're defined *and its
child modules*. `mod tests` is declared inside `lexer.rs`, so it's a
child of `lexer`.

</details>

<details>
<summary>8. When is it OK to call <code>.unwrap()</code>?</summary>

In tests, and when an invariant guarantees the value is there (better:
use `.expect("why it can't fail")` to document it). Never on anything
that depends on user input. That's what `Result` and `?` are for.

</details>
