# Part 1 — Inputs

[← Step 2 overview](guide.md)

A warm-up on code you know: teach the hand-written front end about
names. One new token kind, one new tree node, three small changes.

**Rust you'll use:** [lifetimes in types](rust-concepts.md#1-lifetimes-in-types),
[owned vs borrowed strings](rust-concepts.md#2-owned-vs-borrowed-string-vs-str).

---

## The idea: tokens borrow, trees own

Open `src/token.rs`. The new token kind holds the name:

```rust
pub enum TokenKind<'src> {
    Int(i64),
    Ident(&'src str),
    // ...
}
```

`&'src str` is a *borrowed* string: a pointer into the source text and
a length, no copy. The lexer cuts the source into thousands of tokens;
copying each name into its own `String` would cost an allocation per
name, for nothing, since the source text is still there.

The price is the **lifetime** `'src`: a token can't outlive the text it
points into. Rust tracks this through the types, so every type that
holds a token now carries the lifetime: `Token<'src>`,
`TokenKind<'src>`. The signatures in your lexer and parser needed it
too, and they're already updated:

```rust
impl<'src> Lexer<'src> {
    fn next_token(&mut self) -> Result<Token<'src>, Diagnostic> { ... }
}
```

Why spell out `'src`? With `-> Result<Token, Diagnostic>`, Rust's
elision rules would tie the token to the borrow of `self`, the lexer,
instead of to the source. Then `lex` couldn't return its tokens: they
would seem to point into the lexer, which `lex` drops when it returns.
`Token<'src>` says what's true: the token points into the source, which
outlives the lexer.

The AST makes the other choice. Open `src/ast.rs`:

```rust
pub enum ExprKind {
    Int(i64),
    /// An input: `x`. The tree owns its copy of the name, so it can
    /// outlive the source text and the tokens.
    Var(String),
    // ...
}
```

`String` is *owned*: the tree has its own copy. Trees live longer than
tokens, get passed around and transformed, and later layers (the IR,
the backends) keep the names after the source text is gone. One small
copy per name, and no lifetime on `Expr`. Borrow in short-lived data
that's read once, own in long-lived data that moves around: that's the
usual trade-off.

---

## 1a. `Lexer::ident`

When `next_token` sees a letter or `_`, a name starts. `ident` consumes
the rest of it (letters, digits, underscores: maximal munch again) and
returns `TokenKind::Ident` with the name, borrowed from the source.

Two changes in `src/lexer.rs`:

1. Implement `ident` (at the bottom of the `impl`).
2. In `next_token`, add an arm that calls it, **before** your catch-all
   arm (`v => ...`), which would otherwise take every letter.

<details>
<summary>Hint 1: the arm</summary>

`'a'..='z' | 'A'..='Z' | '_' => self.ident(start),`. `ident` doesn't
fail (any name is fine), so it returns a `TokenKind`, not a `Result`.

</details>

<details>
<summary>Hint 2: <code>ident</code></summary>

The same loop as in `int`, with
`c.is_ascii_alphanumeric() || c == '_'`. The name is
`&self.source[start..self.pos]`: a slice of the source, so its lifetime
is `'src`, exactly what `TokenKind<'src>` needs.

</details>

```bash
cargo test -p mini --lib lexer
```

<details>
<summary>Solution</summary>

```rust
'a'..='z' | 'A'..='Z' | '_' => self.ident(start),
```

```rust
fn ident(&mut self, start: usize) -> TokenKind<'src> {
    while self
        .peek()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        self.bump();
    }
    TokenKind::Ident(&self.source[start..self.pos])
}
```

Try returning `TokenKind::Ident(&self.source[start..self.pos].to_string())`
instead: the compiler refuses, because the `String` would be dropped at
the end of the function while the token still points into it. That's
the borrow checker preventing a dangling pointer.

</details>

---

## 1b. Print an input

In `src/ast.rs`, the S-expression printer needs an arm for `Var`. A
name prints as itself: `x * 2` is `(* x 2)`.

```bash
cargo test -p mini --lib ast
```

<details>
<summary>Solution</summary>

```rust
ExprKind::Var(name) => write!(f, "{name}"),
```

</details>

---

## 1c. Parse an input

`primary = INT | IDENT | "(" expr ")" ;`. In `Parser::primary`, add an
arm for `TokenKind::Ident`. `Expr::var(name, span)` (provided) builds
the node, copying the borrowed name into an owned `String`.

```bash
cargo test -p mini --lib parser
```

<details>
<summary>Solution</summary>

```rust
TokenKind::Ident(name) => Ok(Expr::var(name, token.span)),
```

</details>

---

## Check

The whole hand-written front end knows about names now:

```bash
cargo test -p mini --lib -- lexer parser ast
```

(Filters after `--` go to the test runner, which accepts several.)

```bash
cargo mini --emit=tokens mini/programs/inputs.mini
```

```bash
cargo mini --emit=ast mini/programs/inputs.mini
```

```text
(+ (* x (+ 2 3)) (* y 0))
```

And an error message, using your token's `Display`:

```bash
echo 'x y' > /tmp/two-names.mini && cargo mini /tmp/two-names.mini
```

```text
error: expected end of input, found name `y`
 --> /tmp/two-names.mini:1:3
  |
1 | x y
  |   ^
```

---

**Next:** [Part 2 — LR parsing with Bison](part2-bison.md)
