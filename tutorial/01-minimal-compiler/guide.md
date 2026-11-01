# Step 1 — A minimal compiler

You'll finish a real compiler, small but complete. It turns arithmetic
written in **Mini** into JavaScript that Node.js can run:

```text
(1 + 2) * 3 - 10 / 4            mini/programs/arithmetic.mini
```

```js
console.log((((1 + 2) * 3) - Math.trunc(10 / 4)));
```

```text
7
```

The scaffold in `mini/` already has the structure, the data types and
the tests (47 of them). You write the 12 functions that do the work,
one at a time, and watch the tests turn green.

**Before you start:** do [Step 0](../00-setup/00-setup-rustrover.md), and skim
[the Rust you need](rust-concepts.md). Each part below links to the
sections it uses. Expect 3–5 hours, spread over as many sessions as
you like.

**Contents**

- [The big picture](#the-big-picture)
- [How to work through this guide](#how-to-work-through-this-guide)
- [Part 0 — Look around](#part-0--look-around)
- [Part 1 — Spans (warm-up)](#part-1--spans-warm-up)
- [Part 2 — The lexer](#part-2--the-lexer)
- [Part 3 — The syntax tree](#part-3--the-syntax-tree)
- [Part 4 — The parser](#part-4--the-parser)
- [Part 5 — Code generation](#part-5--code-generation)
- [Part 6 — Run it for real](#part-6--run-it-for-real)
- [Part 7 — Clean up](#part-7--clean-up)
- [Stretch exercises](#stretch-exercises)

---

## The big picture

### The language

Step 1's Mini has one kind of program: a single arithmetic expression
on integers, whose value gets printed.

```text
program = expr EOF ;
expr    = term { ( "+" | "-" ) term } ;
term    = unary { ( "*" | "/" ) unary } ;
unary   = "-" unary | primary ;
primary = INT | "(" expr ")" ;
```

This is the **grammar** in EBNF notation: `{ x }` means "zero or more
`x`", `|` means "or", quoted text is literal. It is the specification
of the language's syntax, and each rule becomes one function in the
parser.

The *meaning* (semantics): integers are 64-bit, `/` divides and rounds
toward zero, `-` negates. In short, it behaves like Rust's `i64`. So
`7 / 2` is `3` and `-7 / 2` is `-3`.

### The pipeline

A compiler is a chain of **phases**. Each phase does one job and hands
a well-defined data structure to the next:

```text
 source text ──▶ LEXER ──▶ tokens ──▶ PARSER ──▶ syntax tree ──▶ CODEGEN ──▶ JavaScript
"-(1 + 2) * 3"          [-, (, 1, +,         (* (neg (+ 1 2)) 3)       console.log(((-(1 + 2)) * 3));
                         2, ), *, 3, EOF]
```

1. The **lexer** cuts the text into **tokens**, the words of the
   language. It forgets whitespace, and knows that `42` is one integer.
2. The **parser** arranges the tokens into a **tree**, the abstract
   syntax tree or AST, according to the grammar. Precedence is decided
   here: in `1 + 2 * 3`, the `*` node sits *under* the `+` node.
3. The **code generator** walks the tree and writes the output program.

Any phase can fail. It then returns a **diagnostic**: a message plus the
exact place in the source.

### Best practices built into the scaffold

Real compilers (rustc, Go, Swift, TypeScript…) share a few habits, and
Mini follows them from day one:

| Practice                                               | Why                                                                                                                                                                                   | Where                                                  |
|--------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------|
| **Separate phases with explicit types between them**   | each phase can be understood, tested and replaced on its own                                                                                                                          | `lexer` → `Vec<Token>` → `parser` → `Expr` → `codegen` |
| **Every token and tree node carries a span**           | errors point at the exact code, and later phases (type checking!) can too                                                                                                             | `span.rs`                                              |
| **Errors are values, never crashes**                   | a user's mistake must never crash the compiler                                                                                                                                        | `Result<_, Diagnostic>` everywhere                     |
| **The grammar is written down; one function per rule** | the parser *is* the grammar, so it's easy to check and extend                                                                                                                         | top of `parser.rs`                                     |
| **A hand-written recursive descent parser**            | full control over error messages. rustc, clang, Go and Swift all hand-write theirs. (The reference compiler in `example/` uses a parser *generator* instead, another valid approach.) | `parser.rs`                                            |
| **Code generation keeps meaning, not syntax**          | JavaScript is a different language, with different rules                                                                                                                              | `codegen.rs`                                           |
| **A library with a thin command-line wrapper**         | tests and future tools (a language server, a REPL…) reuse the compiler directly                                                                                                       | `lib.rs` vs `main.rs`                                  |
| **Tests at every level**                               | unit tests per function, plus end-to-end tests that *run* the output                                                                                                                  | `#[cfg(test)]` modules, `tests/`                       |
| **No dependencies** (for now)                          | you learn by writing every piece; real projects might use crates for lexing or error reporting                                                                                        | `Cargo.toml`                                           |

---

## How to work through this guide

Each part follows the same rhythm:

1. **The idea**: what this piece of a compiler does, and why.
2. **Rust you'll use**: links into [the concepts page](rust-concepts.md).
3. **Your task**: replace a `todo!(...)` with real code. The doc comment
   above the function says what it must do.
4. **Hints**: folded. Open one at a time, only when stuck.
5. **Check**: run the tests for that function.
6. **Solution**: folded. Compare *after* your tests pass, or if you're
   stuck for good. There's always more than one correct answer.

Run tests with the ▶ in RustRover's gutter, or in the terminal:

```bash
cargo test -p mini span
```

**Read the tests before writing the code.** They sit at the bottom of
each file and are the most precise description of what's expected.
When one fails, read its message: it shows the expected and actual
values.

---

## Part 0 — Look around

Run all the tests:

```bash
cargo test -p mini
```

You should see **3 passed, 41 failed**, with messages like
`not yet implemented: Part 2: Lexer::next_token`. The 3 passing tests
cover the error formatter, which is provided.

### What's in `mini/`

| File                  | What                                               | Yours?                        |
|-----------------------|----------------------------------------------------|-------------------------------|
| `src/lib.rs`          | lists the modules; `compile()` chains the phases   | provided                      |
| `src/span.rs`         | `Span`: a location in the source                   | **Part 1**                    |
| `src/token.rs`        | `Token` and `TokenKind`: the lexer's output        | provided, read it             |
| `src/lexer.rs`        | text → tokens                                      | **Part 2**                    |
| `src/ast.rs`          | `Expr`: the syntax tree                            | **Part 3**                    |
| `src/parser.rs`       | tokens → tree                                      | **Part 4**                    |
| `src/codegen.rs`      | tree → JavaScript                                  | **Part 5**                    |
| `src/diagnostic.rs`   | `Diagnostic` and how it's printed                  | provided, explained in Part 6 |
| `src/main.rs`         | the `mini` command                                 | provided, explained in Part 6 |
| `tests/end_to_end.rs` | whole-compiler tests that run the output with Node | provided                      |
| `programs/*.mini`     | sample programs                                    | provided                      |

Open **`src/token.rs`** now and read it. It's short, and it's the
vocabulary for everything after: a `Token` is a `TokenKind` (which
word) plus a `Span` (where). `TokenKind` implements `Display` to name
tokens in error messages: `` `)` ``, `` integer `2` ``, `end of input`.

### About the `#![allow(...)]` line in `lib.rs`

While a function is still `todo!()`, the code it will use (helpers,
imports, parameters) looks unused, and Rust would warn about each
case. The `allow` line silences those warnings for now. You'll delete it
in Part 7, when nothing is `todo!()` anymore.

---

## Part 1 — Spans (warm-up)

**The idea.** A span is a range of byte offsets in the source:
`start..end`, with `end` excluded. In `12 + 3`, the token `12` has span
`0..2`, `+` has `3..4` (the space at offset 2 belongs to no token), and
`3` has `5..6`. Spans let every error point at the right code, and
bigger things get bigger spans. The expression `12 + 3` spans `0..6`:
the **merge** of its first and last parts.

**Rust you'll use:** [structs and methods](rust-concepts.md#6-structs-and-methods),
[`derive` and `Copy`](rust-concepts.md#12-derive-copy-and-clone).

**Your task:** in `src/span.rs`, implement `Span::merge`. It returns the
smallest span covering both `self` and `other`, whatever their order.

<details>
<summary>Hint</summary>

The result starts at the smaller of the two starts and ends at the
larger of the two ends. Integers have `.min(other)` and `.max(other)`
methods.

</details>

**Check:**

```bash
cargo test -p mini span
```

<details>
<summary>Solution</summary>

```rust
pub fn merge(self, other: Span) -> Span {
    Span::new(self.start.min(other.start), self.end.max(other.end))
}
```

`merge` takes `self` by value, not `&self`. That's fine because `Span`
is `Copy`: calling `a.merge(b)` copies two small numbers, and `a` and
`b` stay usable.

</details>

---

## Part 2 — The lexer

**The idea.** The lexer reads the source one character at a time and
groups characters into tokens. It works with two operations:

- **peek**: look at the next character without consuming it
- **bump**: consume the next character and move forward

The lexer also decides what doesn't matter (whitespace) and what isn't
allowed (`$`). Two classic rules:

- **Maximal munch**: a token takes as many characters as it can, so
  `123` is one integer, not three.
- **End with `Eof`**: the lexer always finishes with an end-of-input
  token, so the parser never runs off the end of the list.

Open `src/lexer.rs`. `lex()` (provided) calls `next_token()` in a loop
until it gets `Eof`. The `Lexer` struct holds the source and `pos`, the **byte** offset of the next character. `peek()`
is provided too: it
takes the text from `pos` onwards and returns its first character.

**Rust you'll use:** [strings and UTF-8](rust-concepts.md#5-strings-string-str-and-utf-8),
[`char`](rust-concepts.md#4-numbers-char-bool),
[`Option`](rust-concepts.md#13-option),
[`Result` and `?`](rust-concepts.md#14-result-and-the--operator),
[pattern matching](rust-concepts.md#8-pattern-matching),
[lifetimes](rust-concepts.md#11-lifetimes-just-enough) (for `Lexer<'src>`).

### 2a. `Lexer::bump`

Consume the next character and return it, or `None` at end of input.

<details>
<summary>Hint 1</summary>

Use `peek()` to get the character, then move `pos` forward past it.

</details>

<details>
<summary>Hint 2</summary>

`pos` counts **bytes**, and a `char` may take several. `c.len_utf8()`
tells you how many. The test `bump_counts_bytes_not_characters` checks
exactly this.

</details>

<details>
<summary>Hint 3</summary>

`let c = self.peek()?;` returns `None` from `bump` when `peek` returns
`None`. That handles end of input in one line.

</details>

```bash
cargo test -p mini lexer::tests::bump
```

<details>
<summary>Solution</summary>

```rust
fn bump(&mut self) -> Option<char> {
    let c = self.peek()?;
    self.pos += c.len_utf8();
    Some(c)
}
```

</details>

### 2b. `Lexer::skip_whitespace`

Consume characters as long as the next one is whitespace (space, tab,
newline…).

<details>
<summary>Hint 1</summary>

A `while` loop. `char` has an `is_whitespace()` method.

</details>

<details>
<summary>Hint 2</summary>

`self.peek()` is an `Option<char>`. `option.is_some_and(|c| ...)` is
`true` when the option is `Some` and the closure returns `true` for its
value.

</details>

```bash
cargo test -p mini lexer::tests::skip_whitespace
```

<details>
<summary>Solution</summary>

```rust
fn skip_whitespace(&mut self) {
    while self.peek().is_some_and(|c| c.is_whitespace()) {
        self.bump();
    }
}
```

`bump()` returns the character, which we don't need here. Ignoring an
`Option` is fine. (Ignoring a `Result` would get a warning: Rust
doesn't want errors dropped silently.)

</details>

### 2c. `Lexer::next_token`

The heart of the lexer:

1. skip whitespace
2. remember where the token starts (`self.pos`)
3. consume one character and decide what it begins:
    - nothing left → an `Eof` token with an empty span `start..start`
    - `+ - * / ( )` → the matching `TokenKind`
    - a digit `0`–`9` → an integer: call `self.int(start)` (Part 2d)
    - anything else → an error: `` unexpected character `$` ``, with the
      span of that character
4. return the token, spanning from `start` to the current `pos`

<details>
<summary>Hint 1: the shape</summary>

```rust
fn next_token(&mut self) -> Result<Token, Diagnostic> {
    self.skip_whitespace();
    let start = self.pos;
    let Some(c) = self.bump() else {
        // end of input: return the Eof token
    };
    let kind = match c {
        // one arm per kind of character
    };
    Ok(Token { kind, span: Span::new(start, self.pos) })
}
```

</details>

<details>
<summary>Hint 2: digits and errors</summary>

- `'0'..='9'` matches any digit character. `self.int(start)` returns a
  `Result<TokenKind, Diagnostic>`, so `?` turns it into a `TokenKind`
  (or returns the error).
- A name as the last pattern (`other => ...`) catches every other
  character. Build the error with
  `Diagnostic::new(format!("unexpected character `{other}`"), span)`.
  In that arm, `return Err(...)` leaves the function, so the
  `let kind = match` never needs a value from it.

</details>

```bash
cargo test -p mini lexer::tests::next_token
```

Until you do 2d, any input containing a digit hits `int`'s `todo!()`.
That's why these tests don't use digits.

<details>
<summary>Solution</summary>

```rust
fn next_token(&mut self) -> Result<Token, Diagnostic> {
    self.skip_whitespace();
    let start = self.pos;
    let Some(c) = self.bump() else {
        return Ok(Token {
            kind: TokenKind::Eof,
            span: Span::new(start, start),
        });
    };
    let kind = match c {
        '+' => TokenKind::Plus,
        '-' => TokenKind::Minus,
        '*' => TokenKind::Star,
        '/' => TokenKind::Slash,
        '(' => TokenKind::LParen,
        ')' => TokenKind::RParen,
        '0'..='9' => self.int(start)?,
        other => {
            return Err(Diagnostic::new(
                format!("unexpected character `{other}`"),
                Span::new(start, self.pos),
            ));
        }
    };
    Ok(Token {
        kind,
        span: Span::new(start, self.pos),
    })
}
```

The error span is `start..self.pos`, *after* the `bump`, so it covers
the whole character, even a multi-byte one like `é`.

</details>

### 2d. `Lexer::int`

When `next_token` sees a digit, it has already consumed it. `int`
consumes the rest of the digits (maximal munch), then turns the text
`source[start..pos]` into an `i64`. A number too big for an `i64`
(above 9223372036854775807) is an error, `integer literal is too large`,
spanning the whole literal.

<details>
<summary>Hint 1</summary>

Same loop as `skip_whitespace`, with `c.is_ascii_digit()`.

</details>

<details>
<summary>Hint 2</summary>

`&self.source[start..self.pos]` is the literal's text, a `&str`.
`text.parse::<i64>()` returns `Ok(value)` or `Err(_)`. `match` on it:
on `Ok`, return `Ok(TokenKind::Int(value))`; on `Err`, return the
diagnostic.

</details>

```bash
cargo test -p mini lexer
```

The whole lexer should now be green: 13 tests.

<details>
<summary>Solution</summary>

```rust
fn int(&mut self, start: usize) -> Result<TokenKind, Diagnostic> {
    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
        self.bump();
    }
    let text = &self.source[start..self.pos];
    match text.parse::<i64>() {
        Ok(value) => Ok(TokenKind::Int(value)),
        Err(_) => Err(Diagnostic::new(
            "integer literal is too large",
            Span::new(start, self.pos),
        )),
    }
}
```

`parse` could also fail on a non-digit, but the loop only consumed
digits, so "too large" is the only possible failure. Why parse in the
lexer rather than later? The lexer is the phase that knows the literal's
span, so it can report the error precisely.

</details>

**Try it:** add a test of your own at the bottom of `lexer.rs`. For
example, check that `"(1+2)"` lexes to
`[LParen, Int(1), Plus, Int(2), RParen, Eof]`, using the `kinds`
helper.

---

## Part 3 — The syntax tree

**The idea.** Open `src/ast.rs`. An expression is an `Expr`: a `kind`
saying what it is, plus a `span`. `ExprKind` has three variants:
`Int(42)`, `Neg(-x)`, and `Binary` with an operator and two children.

Two things are worth noticing:

- **No parentheses.** `(1 + 2) * 3` becomes a `Mul` node whose left child
  is an `Add` node. The tree's *shape* records the grouping, so
  parentheses are no longer needed. That's what "abstract" means.
- **Children are `Box<Expr>`**: a type can't contain itself directly
  ([why](rust-concepts.md#15-box-and-recursive-types)).

The constructors `Expr::int`, `Expr::neg` and `Expr::binary` (provided)
build nodes and compute their spans with your `merge`.

To test the parser, we need to *see* a tree. The standard trick is to
print it as an **S-expression**, operator first, with parentheses
around every node:

| Mini           | Tree printed as       |
|----------------|-----------------------|
| `42`           | `42`                  |
| `-1`           | `(neg 1)`             |
| `1 + 2 * 3`    | `(+ 1 (* 2 3))`       |
| `(1 + 2) * -3` | `(* (+ 1 2) (neg 3))` |

**Rust you'll use:** [enums](rust-concepts.md#7-enums),
[traits: `Display`](rust-concepts.md#16-traits-display-and-into),
[`Box`](rust-concepts.md#15-box-and-recursive-types).

**Your task:** implement `fmt` in `impl fmt::Display for Expr`.

<details>
<summary>Hint 1</summary>

`match &self.kind` with one arm per variant. For the operator's text,
use `op.symbol()`.

</details>

<details>
<summary>Hint 2</summary>

`write!(f, "...", ...)` works like `format!` but writes into the
formatter, and returns the `fmt::Result` you need. Children are `Expr`s
too, so `write!(f, "(neg {operand})")` prints them by calling your
`fmt` recursively. You get recursion without writing it yourself.

</details>

```bash
cargo test -p mini ast
```

<details>
<summary>Solution</summary>

```rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.kind {
        ExprKind::Int(value) => write!(f, "{value}"),
        ExprKind::Neg(operand) => write!(f, "(neg {operand})"),
        ExprKind::Binary { op, lhs, rhs } => write!(f, "({} {lhs} {rhs})", op.symbol()),
    }
}
```

`op.symbol()` is a method call, and format strings only accept plain
variable names inline (`{lhs}`), so the symbol goes in as a positional
argument (`{}`).

</details>

---

## Part 4 — The parser

**The idea.** The parser checks that the tokens follow the grammar, and
builds the tree. Mini's parser is **recursive descent**: one function
per grammar rule, each calling the functions for the rules it refers to.

```text
expr    = term { ( "+" | "-" ) term } ;     fn expr()
term    = unary { ( "*" | "/" ) unary } ;   fn term()
unary   = "-" unary | primary ;              fn unary()
primary = INT | "(" expr ")" ;               fn primary()
```

**Precedence comes from the layering.** `expr` builds sums out of
`term`s, and `term` builds products out of `unary`s. A product is
therefore always parsed *inside* one operand of a sum. Here's how
`1 + 2 * 3` is parsed:

```text
expr
├── term
│   └── unary → primary → 1          next is `+`: not * or /, so term returns 1
├── sees `+`, consumes it
├── term
│   ├── unary → primary → 2
│   ├── sees `*`, consumes it
│   ├── unary → primary → 3
│   └── returns (* 2 3)
└── next is EOF: returns (+ 1 (* 2 3))
```

**Associativity comes from the loop.** `1 - 2 - 3` means `(1 - 2) - 3 =
-4`, not `1 - (2 - 3) = 2`. A loop that keeps folding the next operand
into the tree built so far gives exactly that left-leaning shape:

```text
lhs = 1
lhs = (- 1 2)            after "- 2"
lhs = (- (- 1 2) 3)      after "- 3"
```

**Parentheses restart the climb.** `primary` sees `(`, calls `expr`
(the lowest precedence level) for what's inside, then expects `)`. That
recursion is how `(1 + 2) * 3` puts the sum *under* the product.

The parser walks the token list with three helpers:

- `peek()` (provided): the next token, without consuming it
- `advance()` (provided): consume and return the next token; never moves past `Eof`
- `expect(kind)` (yours): consume a token that *must* be there, or fail

**Rust you'll use:** [ownership and moves](rust-concepts.md#9-ownership-and-moves)
(the `lhs = Expr::binary(op, lhs, rhs)` pattern),
[`Result` and `?`](rust-concepts.md#14-result-and-the--operator),
[struct update syntax](rust-concepts.md#6-structs-and-methods),
[loops](rust-concepts.md#18-loops-and-everything-is-an-expression).

Build it bottom-up, in this order. Each step's tests only need the
steps before it.

### 4a. `Parser::expect`

If the next token is of the given `kind`, consume it and return it.
Otherwise return the error `expected {kind}, found {what was there}`,
with the span of the token that was found, and **consume nothing**.

<details>
<summary>Hint</summary>

`TokenKind` implements `PartialEq` (`==`) and `Display` (for the
message), so `format!("expected {kind}, found {}", token.kind)` writes
the whole message.

</details>

```bash
cargo test -p mini parser::tests::expect
```

<details>
<summary>Solution</summary>

```rust
fn expect(&mut self, kind: TokenKind) -> Result<Token, Diagnostic> {
    let token = self.peek();
    if token.kind == kind {
        Ok(self.advance())
    } else {
        Err(Diagnostic::new(
            format!("expected {kind}, found {}", token.kind),
            token.span,
        ))
    }
}
```

</details>

### 4b. `Parser::primary`

`primary = INT | "(" expr ")" ;`. Consume a token and decide:

- an `Int`: return an integer node (`Expr::int`) with the token's span
- `(`: parse an `expr`, `expect` the `)`, and return the inner
  expression **with its span widened to include both parentheses**
- anything else: `expected an expression, found {token}`

<details>
<summary>Hint 1</summary>

`let token = self.advance();` and then `match token.kind { ... }`. The
`Int(value)` pattern gives you the value.

</details>

<details>
<summary>Hint 2: the span of a parenthesized expression</summary>

The parentheses disappear from the tree, but errors about `(1 + 2)`
should underline the parentheses too. Take the inner `Expr` and replace
only its span: `Expr { span: open.span.merge(close.span), ..inner }`.

</details>

```bash
cargo test -p mini parser::tests::primary
```

The `(` branch calls `expr`, which is still `todo!()`. Parentheses
start working in 4e.

<details>
<summary>Solution</summary>

```rust
fn primary(&mut self) -> Result<Expr, Diagnostic> {
    let token = self.advance();
    match token.kind {
        TokenKind::Int(value) => Ok(Expr::int(value, token.span)),
        TokenKind::LParen => {
            let inner = self.expr()?;
            let close = self.expect(TokenKind::RParen)?;
            // The parentheses vanish from the tree, but the span keeps
            // them, so errors about this expression underline them too.
            Ok(Expr {
                span: token.span.merge(close.span),
                ..inner
            })
        }
        other => Err(Diagnostic::new(
            format!("expected an expression, found {other}"),
            token.span,
        )),
    }
}
```

</details>

### 4c. `Parser::unary`

`unary = "-" unary | primary ;`. If the next token is `-`, consume it,
parse a `unary` (recursively, so `- -1` works), and build a `Neg` node.
Otherwise, it's a `primary`.

<details>
<summary>Hint</summary>

`Expr::neg(minus_span, operand)` builds the node and its span. Check
with `self.peek().kind == TokenKind::Minus` *before* consuming: when the
token isn't a `-`, it belongs to `primary`.

</details>

```bash
cargo test -p mini parser::tests::unary
```

<details>
<summary>Solution</summary>

```rust
fn unary(&mut self) -> Result<Expr, Diagnostic> {
    if self.peek().kind == TokenKind::Minus {
        let minus = self.advance();
        let operand = self.unary()?;
        return Ok(Expr::neg(minus.span, operand));
    }
    self.primary()
}
```

</details>

### 4d. `Parser::term`

`term = unary { ( "*" | "/" ) unary } ;`. Parse a `unary`. Then, as long
as the next token is `*` or `/`, consume it, parse another `unary`, and
fold both sides into a `Binary` node. Fold to the **left**, so
`8 / 4 / 2` is `(/ (/ 8 4) 2)`.

<details>
<summary>Hint 1: the shape</summary>

```rust
let mut lhs = self .unary() ?;
loop {
// look at the next token: `*` → Mul, `/` → Div, anything else → we're done
// consume the operator, parse the right-hand side
lhs = Expr::binary(op, lhs, rhs);
}
```

</details>

<details>
<summary>Hint 2: deciding the operator</summary>

`let op = match self.peek().kind { TokenKind::Star => BinOp::Mul, ...,
_ => return Ok(lhs) };`. The `_` arm *returns from the function*, which
also ends the loop.

</details>

```bash
cargo test -p mini parser::tests::term
```

<details>
<summary>Solution</summary>

```rust
fn term(&mut self) -> Result<Expr, Diagnostic> {
    let mut lhs = self.unary()?;
    loop {
        let op = match self.peek().kind {
            TokenKind::Star => BinOp::Mul,
            TokenKind::Slash => BinOp::Div,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.unary()?;
        lhs = Expr::binary(op, lhs, rhs);
    }
}
```

`lhs = Expr::binary(op, lhs, rhs)` **moves** the old tree into the new
node and stores the new node back in `lhs`. Rust allows it because
`lhs` gets a new value right away.

</details>

### 4e. `Parser::expr`

`expr = term { ( "+" | "-" ) term } ;`. The same shape as `term`, one
level up.

```bash
cargo test -p mini parser
```

The whole parser should now be green: 14 tests, including parentheses
and the errors from `parse()`. Look at the
`parse_rejects_*` tests to see the messages the user gets.

<details>
<summary>Solution</summary>

```rust
fn expr(&mut self) -> Result<Expr, Diagnostic> {
    let mut lhs = self.term()?;
    loop {
        let op = match self.peek().kind {
            TokenKind::Plus => BinOp::Add,
            TokenKind::Minus => BinOp::Sub,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.term()?;
        lhs = Expr::binary(op, lhs, rhs);
    }
}
```

`expr` and `term` are nearly identical. With 10 precedence levels, that
would be 10 copies. Real parsers then switch to **Pratt parsing**
(precedence climbing): one loop driven by a table of operator
precedences. That's a good refactoring for a later step.

</details>

**Try it:** `parse()` (provided) calls `expr()` and then
`expect(TokenKind::Eof)`. Why is that last `expect` needed? Hint: what
would `1 2` produce without it?

---

## Part 5 — Code generation

**The idea.** Walk the tree and write JavaScript. The goal is to keep
the **meaning** of the program, not to copy its text. Mini and
JavaScript disagree in two places:

1. **Grouping.** The tree has no parentheses. If we printed
   `(* (+ 1 2) 3)` as `1 + 2 * 3`, JavaScript would compute `7` instead
   of `9`. The simplest *safe* rule: wrap every operation in
   parentheses. The output is ugly but always correct, since JavaScript
   reads it exactly the way the tree says.
2. **Division.** In JavaScript, `7 / 2` is `3.5`. Mini divides integers
   and rounds toward zero, like Rust, so a division becomes
   `Math.trunc(7 / 2)`.

One trap: negating a negation. Printed naively, `- -1` gives `--1`,
which is JavaScript's **decrement** operator and a syntax error.
Parentheses around every negation, `(-(-1))`, avoid it. A test
checks this.

| Mini          | JavaScript          |
|---------------|---------------------|
| `42`          | `42`                |
| `-1`          | `(-1)`              |
| `1 + 2`       | `(1 + 2)`           |
| `7 / 2`       | `Math.trunc(7 / 2)` |
| `- -1`        | `(-(-1))`           |
| `(1 + 2) * 3` | `((1 + 2) * 3)`     |

`generate` (provided) writes `console.log(` + the expression + `);`.

**Rust you'll use:** [borrowing](rust-concepts.md#10-borrowing--and-mut)
(`&Expr` to read, `&mut String` to write),
[pattern matching](rust-concepts.md#8-pattern-matching) (nested
patterns, arm order),
[strings](rust-concepts.md#5-strings-string-str-and-utf-8) (`push`,
`push_str`).

**Your task:** implement `emit_expr(expr, out)`. It *appends* the
JavaScript for `expr` to `out`, calling itself for the children.

<details>
<summary>Hint 1</summary>

`match &expr.kind`. For a child (a `&Box<Expr>`), call
`emit_expr(child, out)`: Rust passes the `Box` as an `&Expr`
automatically, and `out` is already a `&mut String`, so pass it as is.

</details>

<details>
<summary>Hint 2: division</summary>

Match division *before* the other binary operators, with a nested
pattern: `ExprKind::Binary { op: BinOp::Div, lhs, rhs } => ...`. Then
`ExprKind::Binary { op, lhs, rhs } => ...` handles the rest. Arms are
tried in order.

</details>

```bash
cargo test -p mini codegen
```

<details>
<summary>Solution</summary>

```rust
fn emit_expr(expr: &Expr, out: &mut String) {
    match &expr.kind {
        ExprKind::Int(value) => out.push_str(&value.to_string()),
        ExprKind::Neg(operand) => {
            out.push_str("(-");
            emit_expr(operand, out);
            out.push(')');
        }
        ExprKind::Binary {
            op: BinOp::Div,
            lhs,
            rhs,
        } => {
            out.push_str("Math.trunc(");
            emit_expr(lhs, out);
            out.push_str(" / ");
            emit_expr(rhs, out);
            out.push(')');
        }
        ExprKind::Binary { op, lhs, rhs } => {
            out.push('(');
            emit_expr(lhs, out);
            out.push(' ');
            out.push_str(op.symbol());
            out.push(' ');
            emit_expr(rhs, out);
            out.push(')');
        }
    }
}
```

Why append to one `&mut String` instead of returning a `String` from
each call? Returning would build and copy a new string at every node.
With a single buffer, each piece of text is written exactly once. For
small inputs both are fine, but the buffer style is what real code
generators use.

</details>

---

## Part 6 — Run it for real

Every phase works. Run the whole test suite:

```bash
cargo test -p mini
```

All 47 tests should pass, including `tests/end_to_end.rs`. One of those
tests is worth reading: `compiled_programs_compute_the_right_answer`
compiles each expression, **runs the JavaScript with Node**, and checks
that it prints what the same expression gives in Rust. That's how you
verify a compiler preserves meaning: run its output.

Now use the compiler like a user:

```bash
cargo run -q -p mini -- mini/programs/arithmetic.mini
```

```bash
cargo run -q -p mini -- mini/programs/arithmetic.mini | node
```

```bash
cargo run -q -p mini -- mini/programs/broken.mini
```

That last one prints:

```text
error: unexpected character `$`
 --> mini/programs/broken.mini:1:11
  |
1 | 10 / (5 - $)
  |           ^
```

Write your own `.mini` files and break them in different ways. Every
mistake should produce a message like this, never a crash.

### How it fits together (provided code, worth reading)

**`src/lib.rs` → `compile()`** chains the phases with `?`. The first
error stops compilation and goes back to the caller.

**`src/main.rs`** is the only code that touches the outside world: it
reads the arguments and the file, prints the JavaScript to stdout and
errors to stderr, and sets the exit code. See
[the command-line bits](rust-concepts.md#22-the-command-line-bits).

**`src/diagnostic.rs` → `render()`** turns a span (byte offsets) into
what a human needs:

1. **Line number**: count the `\n` before the span.
2. **Column**: count the *characters*, not bytes, from the start of
   that line. An `é` earlier on the line is 2 bytes but 1 column.
3. **The source line**, quoted, with `^` under the span. The underline
   is clipped to that line and is at least one `^` wide: an empty span,
   like end of input, still needs a mark.

The format copies rustc's on purpose: users already know how to read
it.

---

## Part 7 — Clean up

1. In `src/lib.rs`, **delete the scaffolding `allow`** (the three
   comment lines and `#![allow(dead_code, unused_imports, unused_variables)]`).
2. Lint and format:

   ```bash
   cargo clippy -p mini --all-targets
   ```

   ```bash
   cargo fmt
   ```

   Clippy should report nothing. If it warns, read the explanation:
   each Clippy warning comes with a link to why.
3. Final check: `cargo test -p mini`, all 47 green.

### What you can now do

- [ ] Explain the three phases of a compiler and the data between them
- [ ] Write a lexer: peek/bump, maximal munch, the `Eof` token, spans
- [ ] Turn a grammar into a recursive descent parser, with precedence and associativity
- [ ] Explain why code generation is about meaning, not syntax
- [ ] Report errors as values, with spans, never panics
- [ ] Use structs, enums, `match`, `Option`, `Result` and `?`, `Box`, borrowing, and the `Display` trait in Rust

---

## Stretch exercises

Optional, roughly from easiest to hardest. For each one, **write the
test first**, watch it fail, then make it pass.

1. **Remainder `%`.** Add it to the lexer, `BinOp`, `term` and the code
   generator. (JavaScript's `%` truncates like Rust's for integers. Can
   you prove it with an end-to-end test case?)
2. **Comments.** Skip everything from `//` to the end of the line. The
   lexer must now look *two* characters ahead, because `/` alone is
   still division. Add a `peek_second()` helper.
3. **Fewer parentheses.** Make the generator print only the parentheses
   JavaScript needs: `1 + 2 * 3` stays `1 + 2 * 3`, while `(1 + 2) * 3`
   keeps its parentheses. Give each operator a precedence number, and
   watch out for `1 - (2 - 3)` (associativity!) and for `- -1`.
4. **Better end-of-input errors.** `printf '(1 + 2' > t.mini` gives a
   good error, but `printf '(1 + 2\n' > t.mini` points at an *empty*
   line 2, because the `Eof` token sits after the newline. Make it point
   just after the last real token instead.
5. **Big numbers.** JavaScript numbers are floating point, exact only up
   to 2^53 − 1 = 9007199254740991, so `9007199254740993` silently
   becomes `...992` in Node. Either reject bigger literals with a
   diagnostic, or emit JavaScript `BigInt` (`123n`). What does each
   choice cost?
6. **Constant folding.** Compute the result at compile time and emit
   `console.log(7);`. What must happen for `1 / 0`, or for `i64`
   overflow? (Remember: the compiler must not crash. See `checked_div`
   and `checked_add`.)

---

**Done?** Head back to the [tutorial overview](../README.md) for what
comes next.
