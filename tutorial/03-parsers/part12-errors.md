# Part 12 — Errors and differential testing

[← Step 3 overview](guide.md)

A parser for people makes good errors, and a parser for tools never
fails. This part finishes both, then tests your parser the hard way:
against the reference, on tens of thousands of programs.

**Rust you'll use:** [slice patterns](../02-compiler-layers/rust-concepts.md#4-slice-patterns-and-more-patterns),
[property testing](../02-compiler-layers/rust-concepts.md#16-property-based-testing-with-proptest),
[snapshot testing](rust-concepts.md#15-snapshot-testing-with-expect-test).

---

## The idea: resilient parsing

Step 1's parser stopped at the first error, and so does the reference
parser in `example/`. That's fine for a compiler run, and useless for
an editor: while you type `fn area(s`, the file is broken, and the
editor still has to highlight it, complete names, and show the outline
of the rest.

Your parser is **resilient**: any text gives a tree, every byte in it,
plus a list of errors. You built this into every layer already. Here's
the inventory:

| Where                        | What                                                                                   | Part |
|------------------------------|----------------------------------------------------------------------------------------|------|
| raw lexer                    | errors are flags and `Unknown` tokens: it never stops                                  | 4    |
| cooked lexer                 | an error is a message and a range; the token stays in the list                        | 5    |
| `expect`                     | reports a missing token without consuming anything                                     | 7    |
| `err_recover`                | skips a token in an `ERROR` node, unless it belongs to someone else                    | 7    |
| recovery sets                | `TYPE_RECOVERY`, `PAT_RECOVERY`, `EXPR_RECOVERY`, `LIST_RECOVERY`: what each context leaves to its callers | 8, 9 |
| loops                        | every iteration consumes a token or stops; the step limit enforces it                  | 7–10 |
| `block_expr`                 | stops at a declaration: a missing `}` costs one error, not the rest of the file        | 9    |
| `decl`                       | skips junk up to the next declaration keyword, in one `ERROR` node                     | 10   |
| `parse`                      | keeps one error per position: the first, usually the cause                            | 7    |

matklad's [Resilient LL Parsing Tutorial](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)
builds the same thing from scratch, with more examples.

What's left: errors that are better reported **after** parsing, and one
mistake that's better parsed than skipped.

---

## 12a. Validation

Read `src/validation.rs`: `validate` walks the finished tree, and calls
`validate_paren` on every `PAREN_EXPR`, through the typed AST. The
scaffold's `validate_paren` does nothing yet, so `()` and `(1, 2)`
are accepted.

This is the other half of Part 9c's cover grammar: the parser accepted
any number of expressions in parentheses; now, with the whole node in
hand, say what's wrong. rust-analyzer has the same split: its parser
accepts a little more than Rust, and `validation.rs` rejects the rest
with precise messages.

Write `validate_paren`. Its expressions are `paren.exprs()`; the error
covers the whole `PAREN_EXPR`:

| Inside        | Message                                                                     |
|---------------|-----------------------------------------------------------------------------|
| nothing       | ``Maxim has no `()` value: a function without parameters is `fn() { … }` `` |
| `_` alone     | `` `_` stands for a missing argument: it can't be in parentheses ``         |
| one expression| fine                                                                        |
| more          | `Maxim has no tuples: use a record type, or separate arguments`             |

```bash
cargo test -p mini-parser --lib validation
```

<details>
<summary>Hint: slice patterns</summary>

Collect the expressions in a `Vec`, and `match exprs.as_slice()`:
`[]`, `[ast::Expr::HoleExpr(_)]`, `[_]`, and `[..]` for the rest, in
that order (the first that matches wins).

</details>

<details>
<summary>Solution</summary>

```rust
fn validate_paren(paren: &ast::ParenExpr, errors: &mut Vec<SyntaxError>) {
    let exprs: Vec<ast::Expr> = paren.exprs().collect();
    let msg = match exprs.as_slice() {
        [] => "Maxim has no `()` value: a function without parameters is `fn() { … }`",
        [ast::Expr::HoleExpr(_)] => "`_` stands for a missing argument: it can't be in parentheses",
        [_] => return,
        [..] => "Maxim has no tuples: use a record type, or separate arguments",
    };
    errors.push(SyntaxError::new(msg, paren.syntax().text_range()));
}
```

`(_)` is your spec's disambiguation rule 3, the one the grammar couldn't
express. Validation is where such rules go.

</details>

---

## 12b. Parse the mistake

`(x) => x * 2` is how lambdas are written in JavaScript, and in older
Maxim. Today it's an error, and a common one. A generic message,
`expected a declaration` at the `=>`, wouldn't help much. Better:
recognize it, and say what to write instead.

Read `expr`: after an expression, if the next token is `=>`, it calls
`arrow_lambda`, with what was parsed before the `=>` (if anything).
Until you write it, it's a `todo!()`: a program with `=>` makes your
parser panic.
Write `arrow_lambda`: start a node that precedes `lhs` (or a new node
if there's none), report `lambdas are written `fn(x) { … }` in Maxim`
at the `=>`, consume it, parse the body as an expression, and complete
the node as an `ERROR`.

```bash
cargo test -p mini-parser --lib expressions
```

<details>
<summary>Solution</summary>

```rust
fn arrow_lambda(p: &mut Parser<'_>, lhs: Option<CompletedMarker>) -> CompletedMarker {
    let m = match lhs {
        Some(lhs) => lhs.precede(p),
        None => p.start(),
    };
    p.error("lambdas are written `fn(x) { … }` in Maxim");
    p.bump(T![=>]);
    expr_bp(p, 0);
    m.complete(p, ERROR)
}
```

Parsing the body matters as much as the message: the rest of the file
parses as if the lambda had been written right, so the one mistake
gives one error. The reference compiler's grammar does the same: it has
a rule for arrow lambdas whose only job is to report this error.

</details>

---

## The corpus, broken

```bash
cargo test -p mini-parser --test corpus
```

Next to each program of `corpus/err/`, a `.error` file holds its first
error. Only the first, because what comes after the first error depends
on recovery choices, which could differ between two correct parsers;
the first can't. (An LL or LR parser detects an error at the first
token that can't continue a valid program, its **viable prefix**: any
two correct parsers stop at the same place.)

---

## Differential testing

Open `tests/differential.rs`. Two parsers of the same language must
agree on one thing: **is this program valid?** The reference parser in
`example/` is generated from a grammar and has years of use; when yours
disagrees with it, one of them is wrong, and the test shows you the
program.

```bash
cargo test -p mini-parser --test differential
```

It checks three things:

1. Every program of `corpus/ok/` is valid for both, and every program
   of `corpus/err/` invalid for both.
2. **Edited programs**: proptest picks a valid program, and makes one
   to three small edits at random places, deleting a token, doubling
   one, inserting or replacing one from a vocabulary of every Maxim
   token. The result sits right at the border between valid and
   invalid, where parsers disagree when one of them is wrong.
3. **Token soup**: random sequences of tokens after a module header.
   Mostly invalid, and full of combinations nobody would write.

Programs with HTML are skipped: the reference accepts them, and yours
doesn't, by design.

### When it fails

proptest shrinks the failing program, then prints it:

```text
the reference parser says this program is invalid, yours says valid:
module M

let v = a < b > c
```

Then find out who's right. Your spec says what Maxim is; the
reference's grammar is `example/src/maxim.y`; and the two layout rules
explain most surprises. If yours is wrong, fix it, and add the program
to a unit test. If you're sure the reference is wrong, note it: you've
found a bug in a real compiler.

This isn't hypothetical. While this step was being written, the
differential tests caught a mistake in the corpus itself: a program
meant to be invalid, `a b` alone on a line of a block, which both
parsers accept. It's two statements, `a` then `b`, without a separator:
valid Maxim. They also caught a deliberately planted bug, a trailing
comma accepted in lists, in a few seconds.

### How many cases?

`ProptestConfig { cases: 5000, .. }`: 5000 random programs per
property, in a few seconds. Try 50 000 once, and leave it running
while you read the next part.

---

## Properties of any text

Open `tests/lossless.rs`. For **any** text at all, not just Maxim:

- lexing and parsing don't panic;
- the tokens' texts, end to end, are the text; and so is the tree's
  text;
- every error points inside the text.

```bash
cargo test -p mini-parser --test lossless
```

This is the contract an editor relies on. One input it doesn't try:
ten thousand `(` in a row. Each one is a recursive call, and the stack
overflows. rustc and rust-analyzer have the same weakness, and limit
nesting depth; `cargo parse` runs the parser on a thread with a large
stack (read `main.rs`).

---

## Check

```bash
cargo test -p mini-parser
```

Everything passes: unit tests, corpus snapshots, differential tests,
properties.

---

**Next:** [Part 13 — Explore, and clean up](part13-explore.md)
