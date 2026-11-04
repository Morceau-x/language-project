# Part 9 — Expressions

[← Step 3 overview](guide.md)

The biggest part of the grammar, in `src/parser/grammar/expressions.rs`
and `atoms.rs`: operators with **Pratt parsing**, then everything an
expression can be.

**Rust you'll use:** [`let … else`](rust-concepts.md#13-let-else-and-bindings-in-patterns),
[`Option` combinators](rust-concepts.md#4-option-and-bool-combinators).

---

## The idea: Pratt parsing

Your spec has ten levels of operators, one rule each. A recursive
descent parser could have ten functions too, like step 1's `expr` and
`term`, each calling the next. It works, but every expression, even
`x`, goes down ten calls, and adding an operator means adding a
function.

**Pratt parsing** (Vaughan Pratt, 1973; the same idea is also called
"precedence climbing") does all the levels in one loop, driven by a
table. Each binary operator has two **binding powers**: how tightly it
holds the operand on its left, and the one on its right. Higher binds
tighter:

```text
expression:    a   +   b   *   c
binding power:   13  14  15  16
```

Between `b`'s two neighbours, `*` pulls harder (15) than `+` (14):
`b` goes to `*`. The whole algorithm:

```rust
/// An expression whose operators all bind at least as tightly as `min_bp`.
fn expr_bp(p, min_bp) {
    let mut lhs = <an operand>;
    loop {
        let op = <the next token>;
        let Some((l_bp, r_bp)) = binding_power(op) else { break };  // not an operator
        if l_bp < min_bp { break; }   // this operator binds too loosely: it's not ours
        <consume op>;
        let rhs = expr_bp(p, r_bp);   // its right operand: what binds tighter than it
        lhs = <op applied to lhs and rhs>;
    }
}
```

Trace `a + b * c`, starting with `expr_bp(0)`:

1. `lhs = a`. Next is `+`, `(13, 14)`, and 13 ≥ 0: consume it, and parse
   its right operand with `expr_bp(14)`:
   1. `lhs = b`. Next is `*`, `(15, 16)`, 15 ≥ 14: consume it, parse
      `expr_bp(16)`, which returns `c` (nothing after it). `lhs = b * c`.
   2. Nothing after: return `b * c`.
2. `lhs = a + (b * c)`. Nothing after: done.

And `a - b - c`? Inside `-`'s right operand, `expr_bp(14)` meets the
second `-`, whose left power 13 is less than 14: it stops, and returns
just `b`. So `(a - b) - c`: a right power higher than the left makes an
operator **left-associative**. Swap them, `(12, 11)`, and the second
operator gets taken inside: **right-associative**.

That's the whole trick. matklad's [Simple but Powerful Pratt Parsing](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html)
tells it at length, and rust-analyzer's expression parser is built on
it.

### With markers

In your parser, "`lhs` = op applied to lhs and rhs" is a `precede`:
`lhs` is a completed node, which becomes the first child of a new
`BIN_EXPR`:

```rust
let m = lhs.precede(p);
p.bump(op);
expr_bp(p, r_bp);
lhs = m.complete(p, BIN_EXPR);
```

That's exactly the case forward parents (Part 6) were made for.

### The shapes

```text
LITERAL         INT_NUMBER | FLOAT_NUMBER | STRING
PATH_EXPR       NAME_REF                                   x, Shape
HOLE_EXPR       "_"
PREFIX_EXPR     ("-" | "!")  Expr                           -x
BIN_EXPR        Expr  <operator>  Expr                      a + b
FIELD_EXPR      Expr  "."  NAME_REF                         user.name
CALL_EXPR       Expr  ARG_LIST                              f(x)
ARG_LIST        "("  (Expr | LABELED_ARG | SPREAD),*  ")"
LABELED_ARG     NAME_REF  ":"  Expr                         age: 30
SPREAD          ".."  Expr                                  ..rest
TRY_EXPR        Expr  "?"
CAST_EXPR       Expr  "::"  NAME_REF                        x :: Celsius
```

Read the code first: in `expressions.rs`, `EXPR_FIRST`, `expr` (which
calls `expr_bp(p, 0)`; ignore `arrow_lambda` until Part 12),
`PREFIX_BINDING_POWER`, `is_comparison`, and `postfix`, a stub that
parses no postfix operator yet. In `atoms.rs`, `atom` dispatches on the
first token: literals, names and `_` are done.

---

## 9a. Binary and prefix operators

Write `infix_binding_power`, `expr_bp` and `lhs`.

**The table.** Turn your spec's precedence table into pairs: loosest
first, `|>` at `(1, 2)`, two numbers per level. Left-associative levels
are `(n, n + 1)`, right-associative ones `(n + 1, n)`. For the
non-associative comparisons, either works: chaining is an error,
handled below.

**Prefix operators.** `lhs` parses an operand: a `-` or `!` with its
own operand, in a `PREFIX_EXPR`; or an atom, then its postfix
operators. The operand of a prefix operator is `expr_bp(p,
PREFIX_BINDING_POWER)`: an expression whose operators bind at least
that tightly. With 17, between `*` (15) and `**` (19 and 20), `-a * b`
is `(-a) * b`, and `-x ** 2` is `-(x ** 2)`. And `x ** -y` works with
no special case: the right operand of `**` starts with `-`, which `lhs`
handles.

**Line breaks.** The loop stops at a `NEWLINE`: `if p.at_newline()`,
break, before looking at the operator. Only one binary operator can
also start a statement, `-`, so it's the only one this changes: in a
block, `a` newline `- 1` is two statements. `a +` newline `b` never
sees a `NEWLINE` (the line break after `+` is whitespace), nor does
`a` newline `|> f`.

**No chaining.** `a < b < c` is an error. In the loop, remember the
left binding power of the last comparison parsed *in this loop*: if the
next operator is a comparison at the same level, report `comparisons
can't be chained: write `a < b && b < c`` at it, and parse on (the tree
doesn't care). Any other operator resets it. `a == b < c` is fine: two
levels.

`expr_bp` and `lhs` return `Option<CompletedMarker>`: `None` when
there's no expression at all (`atom` reported `expected an expression`).
`?` passes the `None` on.

```bash
cargo test -p mini-parser --lib expressions::tests
```

The tests of this task are `multiplication_binds_tighter_than_addition`,
`most_operators_are_left_associative`, `power_and_concatenation_are_right_associative`,
`the_precedence_ladder`, `prefix_operators`, `power_binds_tighter_than_prefix_minus`,
`comparisons_dont_chain` and `a_line_break_before_a_minus_ends_the_expression`. They print expressions with every
operator in parentheses: `(a + (b * c))`.

<details>
<summary>Hint: the table</summary>

| Operators              | Binding powers |
|------------------------|----------------|
| `\|>`                  | `(1, 2)`       |
| `\|\|`                 | `(3, 4)`       |
| `&&`                   | `(5, 6)`       |
| `==` `!=`              | `(7, 8)`       |
| `<` `>` `<=` `>=`      | `(9, 10)`      |
| `++`                   | `(12, 11)`     |
| `+` `-`                | `(13, 14)`     |
| `*` `/` `~/` `%`       | `(15, 16)`     |
| prefix `-` `!`         | `17`           |
| `**`                   | `(20, 19)`     |

</details>

<details>
<summary>Solution</summary>

```rust
fn infix_binding_power(kind: SyntaxKind) -> Option<(u8, u8)> {
    let bp = match kind {
        T![|>] => (1, 2),
        T![||] => (3, 4),
        T![&&] => (5, 6),
        T![==] | T![!=] => (7, 8),
        T![<] | T![>] | T![<=] | T![>=] => (9, 10),
        T![++] => (12, 11),
        T![+] | T![-] => (13, 14),
        T![*] | T![/] | T![~/] | T![%] => (15, 16),
        T![**] => (20, 19),
        _ => return None,
    };
    Some(bp)
}
```

```rust
fn expr_bp(p: &mut Parser<'_>, min_bp: u8) -> Option<CompletedMarker> {
    let mut lhs = lhs(p)?;
    // The left binding power of the comparison just parsed in this loop:
    // a second one at the same level would chain.
    let mut last_comparison = None;
    loop {
        // A line break that ends a statement ends the expression too.
        if p.at_newline() {
            break;
        }
        let op = p.current();
        let Some((l_bp, r_bp)) = infix_binding_power(op) else {
            break;
        };
        if l_bp < min_bp {
            break;
        }
        if is_comparison(op) {
            if last_comparison == Some(l_bp) {
                p.error("comparisons can't be chained: write `a < b && b < c`");
            }
            last_comparison = Some(l_bp);
        } else {
            last_comparison = None;
        }
        let m = lhs.precede(p);
        p.bump(op);
        expr_bp(p, r_bp);
        lhs = m.complete(p, BIN_EXPR);
    }
    Some(lhs)
}
```

```rust
fn lhs(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    if p.at(T![-]) || p.at(T![!]) {
        let m = p.start();
        p.bump_any();
        expr_bp(p, PREFIX_BINDING_POWER);
        return Some(m.complete(p, PREFIX_EXPR));
    }
    let atom = atoms::atom(p)?;
    Some(postfix(p, atom))
}
```

A missing right operand, as in `1 +`, gives a `BIN_EXPR` with one
child and the error `expected an expression`: the test
`missing_operands` (9b) shows it.

</details>

### The `-` of Part 3

Without the `NEWLINE` check, the loop is **greedy**: after an operand,
if the next token is an operator, it takes it. That's the reference
compiler's rule, "an expression goes as far as it can", and its LR
parser gets it the same way. The check makes your parser follow your
spec instead: one `if`, because the lexer's rule already did the
thinking.

---

## 9b. Postfix operators and arguments

Replace the stub `postfix` with the real loop: after an atom, while the
next token is a postfix operator, wrap what you have:

| Next token                   | Node                                                     |
|------------------------------|----------------------------------------------------------|
| `.`                          | `FIELD_EXPR`: a `NAME_REF`, lowercase or capitalized (`Geo.Circle`); else `expected a name after `.`` |
| `(`                          | `CALL_EXPR`, with an `ARG_LIST`                          |
| `?`                          | `TRY_EXPR`                                               |
| `::`                         | `CAST_EXPR`: a capitalized `NAME_REF`                    |

As in `expr_bp`, a `NEWLINE` ends the loop: `f` newline `(x)` is two
statements. Here too, only one postfix operator can start a statement:
`(`. `user` newline `.name` goes on.

Then `arg_list`, `arg` and `spread`. An argument is a spread (`..x`),
a labeled argument (`label: x`, two tokens of lookahead: Part 3's
`Arg` warning), or an expression. `spread` is used by lists too.

```bash
cargo test -p mini-parser --lib expressions::tests
```

The tests of this task: `postfix_operators_bind_tightest`,
`call_arguments`, `a_paren_on_the_next_line_is_not_a_call`,
`line_breaks_continue_after_operators_and_before_pipelines`,
`missing_operands`. (`lambdas_are_atoms` waits for 9e, and
`arrow_lambdas_get_their_own_error` for Part 12.)

<details>
<summary>Hint: the loop</summary>

```rust
loop {
    lhs = match p.current() {
        T![.] => { let m = lhs.precede(p); …; m.complete(p, FIELD_EXPR) }
        …
        _ => return lhs,
    }
}
```

Each arm wraps `lhs` and gives back the new, bigger node. The
`NEWLINE` check goes at the top of the loop.

</details>

<details>
<summary>Solution</summary>

```rust
fn postfix(p: &mut Parser<'_>, mut lhs: CompletedMarker) -> CompletedMarker {
    loop {
        // `f` then `(x)` on the next line: two statements, not a call.
        if p.at_newline() {
            return lhs;
        }
        lhs = match p.current() {
            T![.] => {
                let m = lhs.precede(p);
                p.bump(T![.]);
                match p.current() {
                    kind @ (LOWER_IDENT | UPPER_IDENT) => name_ref(p, kind),
                    _ => p.error("expected a name after `.`"),
                }
                m.complete(p, FIELD_EXPR)
            }
            T!['('] => {
                let m = lhs.precede(p);
                arg_list(p);
                m.complete(p, CALL_EXPR)
            }
            T![?] => {
                let m = lhs.precede(p);
                p.bump(T![?]);
                m.complete(p, TRY_EXPR)
            }
            T![::] => {
                let m = lhs.precede(p);
                p.bump(T![::]);
                name_ref(p, UPPER_IDENT);
                m.complete(p, CAST_EXPR)
            }
            _ => return lhs,
        }
    }
}
```

```rust
fn arg_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T!['(']);
    if !p.at(T![')']) {
        loop {
            arg(p);
            if !p.eat(T![,]) {
                break;
            }
        }
    }
    p.expect(T![')']);
    m.complete(p, ARG_LIST);
}
```

```rust
fn arg(p: &mut Parser<'_>) {
    if p.at(T![..]) {
        atoms::spread(p);
    } else if p.at(LOWER_IDENT) && p.nth_at(1, T![:]) {
        let m = p.start();
        name_ref(p, LOWER_IDENT);
        p.bump(T![:]);
        expr(p);
        m.complete(p, LABELED_ARG);
    } else {
        expr(p);
    }
}
```

```rust
pub(super) fn spread(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![..]);
    expr(p);
    m.complete(p, SPREAD);
}
```

Inside the parentheses of a call, the lexer's rule turned every line
break into whitespace, so `f(a,` newline `b)` works without a thought
here. The test `line_breaks_continue_after_operators_and_before_pipelines`
shows it, with a `- b` on its own line inside a call: still one
expression.

</details>

---

## 9c. Parentheses and lists

```text
PAREN_EXPR      "("  Expr,*  ")"                           (a + b)
LIST_EXPR       "["  Expr,*  (","  SPREAD)?  "]"            [x, ..rest]
```

Write `paren_expr` and `list_expr`.

A parenthesized expression holds one expression. But parse **more**: a
`(` then expressions separated by commas, maybe none, then `)`. The
grammar says `(1, 2)` is an error; parsing it anyway lets Part 12 say
*which* error: `Maxim has no tuples`, rather than `expected )`. This is
a **cover grammar**: a looser rule that covers several things, checked
afterwards. JavaScript's spec uses one for `(a, b) => …`, where the
parser can't tell parameters from a parenthesized expression until it
sees the `=>`.

A list is like a list pattern: after a `,`, a `..` starts the spread,
which ends the list.

```bash
cargo test -p mini-parser --lib atoms::tests::literals
cargo test -p mini-parser --lib atoms::tests::parens
cargo test -p mini-parser --lib atoms::tests::lists
cargo test -p mini-parser --lib atoms::tests::broken_lists
```

<details>
<summary>Solution</summary>

```rust
fn paren_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['(']);
    if !p.at(T![')']) {
        expr(p);
        while p.eat(T![,]) {
            expr(p);
        }
    }
    p.expect(T![')']);
    m.complete(p, PAREN_EXPR)
}
```

```rust
fn list_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['[']);
    if !p.at(T![']']) {
        expr(p);
        while p.eat(T![,]) {
            if p.at(T![..]) {
                spread(p);
                break;
            }
            expr(p);
        }
    }
    p.expect(T![']']);
    m.complete(p, LIST_EXPR)
}
```

</details>

---

## 9d. Blocks and statements

```text
BLOCK_EXPR      "{"  (LET_STMT | EXPR_STMT)*  "}"       one per line
LET_STMT        "let"  NAME  (":"  Type)?  "="  Expr
EXPR_STMT       Expr
```

Write `block_expr`, `stmt`, and `let_binding` in `items.rs`, which
parses both a `let` statement and a top-level `let` declaration: the
caller says which kind of node to complete, `LET_STMT` or `LET_DECL`.
That's the event design paying off: a marker's kind is chosen at the
end.

**Statements are separated by `NEWLINE`s**, and the separator isn't a
token: it's the bit on the next statement's first token. `block_expr`
loops until `}` (or the end of the file). Before every statement but
the first, if there's no `NEWLINE`, report `expected a line break: each
statement starts on its own line`, then parse the statement anyway:
`{ a b }` gives one error and two statements. Report it only if the
current token can start a statement (`let`, or in `EXPR_FIRST`):
anything else gets `stmt`'s own error, and one per place is enough.

`stmt` is a `let`, an expression (the current token is in
`EXPR_FIRST`), or neither: then `err_and_bump("expected a
statement")`, which always consumes the token. Without that, a stray
`)` in a block would loop forever.

Where does each statement end? Where `expr` stops: at a token that
can't continue it, or at a `NEWLINE` (9a and 9b). Nothing more to
write for that.

**A block can't hold a declaration.** If the `}` of a function is
missing, the parser would otherwise swallow every declaration after it
as statements. So the loop also stops at a token that can only start a
declaration: one in `DECL_ONLY`, or `fn` followed by a lowercase name
(a lambda is `fn(`). Then `expect(T!['}'])` reports the missing brace,
and the next declarations parse normally. It's a **recovery
heuristic**: a guess about what the programmer meant, that keeps one
mistake from making a hundred errors.

```bash
cargo test -p mini-parser --lib atoms::tests::blocks
cargo test -p mini-parser --lib atoms::tests::line_breaks_end
cargo test -p mini-parser --lib items::tests::lets
```

<details>
<summary>Solution</summary>

```rust
pub(super) fn block_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(T!['{']);
    let mut first = true;
    while !p.at(T!['}']) && !p.at(EOF) {
        // `fn name` or `type` can't be in a block: its `}` is missing.
        // Leave them to the declarations, rather than swallow the file.
        if p.at_ts(DECL_ONLY) || (p.at(T![fn]) && p.nth_at(1, LOWER_IDENT)) {
            break;
        }
        if !first && !p.at_newline() && (p.at(T![let]) || p.at_ts(EXPR_FIRST)) {
            p.error("expected a line break: each statement starts on its own line");
        }
        first = false;
        stmt(p);
    }
    p.expect(T!['}']);
    m.complete(p, BLOCK_EXPR)
}
```

```rust
fn stmt(p: &mut Parser<'_>) {
    if p.at(T![let]) {
        items::let_binding(p, LET_STMT);
    } else if p.at_ts(EXPR_FIRST) {
        let m = p.start();
        expr(p);
        m.complete(p, EXPR_STMT);
    } else {
        p.err_and_bump("expected a statement");
    }
}
```

```rust
pub(super) fn let_binding(p: &mut Parser<'_>, kind: SyntaxKind) {
    let m = p.start();
    p.bump(T![let]);
    name(p, LOWER_IDENT);
    if p.eat(T![:]) {
        types::type_(p);
    }
    p.expect(T![=]);
    expressions::expr(p);
    m.complete(p, kind);
}
```

Read the test `line_breaks_end_statements`: `a`, `- 1` and `(b)` on
three lines are three statements; `c` then `+ 1` and `|> f` on the
next lines is one. Every decision was made by the lexer's rule, in
`from_lexed`: the grammar only asks `at_newline()`.

Why report a missing line break, rather than stop the block? The
programmer's intent is clear, and the rest of the block parses fine:
an error that doesn't derail the parse is the best kind.

</details>

---

## 9e. Parameters and lambdas

```text
LAMBDA_EXPR     "fn"  PARAM_LIST  RET_TYPE?  BLOCK_EXPR
PARAM_LIST      "("  PARAM,*  ")"
PARAM           (NAME | "_")  (":"  Type)?
RET_TYPE        "->"  Type
```

Write `param_list`, `param` and `opt_ret_type` in `grammar/mod.rs`
(functions will use them in Part 10), and `lambda_expr` in `atoms.rs`.

- `param_list` is called where parameters must be: if there's no `(`,
  report `expected `(` and the parameters`. If the `(` is on the next
  line, as in `fn f` newline `(x)`, call `not_on_new_line(p)` (in
  `mod.rs`): it reports `a line break here ends the statement: put
  this `(` at the end of the line before`, and the parse goes on as if
  the `(` were on the right line.
- `param`: a name or `_`, then maybe `:` and a type. Anything else:
  `expected a parameter`, with `err_recover` and `LIST_RECOVERY`; the
  marker you started must then be abandoned, not completed: there's no
  parameter.
- `opt_ret_type`: a `RET_TYPE` if there's a `->`, nothing otherwise.
- The body is a block: call `block` (in `mod.rs`), which reports
  `expected a block `{ … }`` if there's no `{`, and calls
  `not_on_new_line` if the `{` is on the next line.

```bash
cargo test -p mini-parser --lib atoms::tests::lambdas
cargo test -p mini-parser --lib grammar::tests
cargo test -p mini-parser --lib expressions::tests::lambdas_are_atoms
```

<details>
<summary>Solution</summary>

```rust
fn param_list(p: &mut Parser<'_>) {
    if !p.at(T!['(']) {
        p.error("expected `(` and the parameters");
        return;
    }
    not_on_new_line(p);
    let m = p.start();
    p.bump(T!['(']);
    if !p.at(T![')']) {
        loop {
            param(p);
            if !p.eat(T![,]) {
                break;
            }
        }
    }
    p.expect(T![')']);
    m.complete(p, PARAM_LIST);
}
```

```rust
fn param(p: &mut Parser<'_>) {
    let m = p.start();
    match p.current() {
        LOWER_IDENT => name(p, LOWER_IDENT),
        T![_] => p.bump(T![_]),
        _ => {
            m.abandon(p);
            p.err_recover("expected a parameter", LIST_RECOVERY);
            return;
        }
    }
    if p.eat(T![:]) {
        types::type_(p);
    }
    m.complete(p, PARAM);
}
```

```rust
fn opt_ret_type(p: &mut Parser<'_>) {
    if p.at(T![->]) {
        let m = p.start();
        p.bump(T![->]);
        types::type_(p);
        m.complete(p, RET_TYPE);
    }
}
```

```rust
fn lambda_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(T![fn]);
    param_list(p);
    opt_ret_type(p);
    block(p);
    m.complete(p, LAMBDA_EXPR)
}
```

A lambda is an atom: postfix operators apply to it, `fn(x) { x }(1)`
calls it, and `fn(x) { x } |> g` pipes it.

Your spec says a `{` or `(` on the next line ends the declaration, and
the grammar has no `NEWLINE` there: so the program is invalid. Why
does the parser report it and go on, rather than stop the function
there? Because stopping would leave a function without a body, then a
block where a declaration should be: two confusing errors for one
mistake. Go's compiler does the same: its error for a `{` on its own
line names the cause, the line break, rather than the missing body.

</details>

---

## 9f. `if` and `match`

```text
IF_EXPR         "if"  Expr  BLOCK_EXPR  ("else"  (BLOCK_EXPR | IF_EXPR))?
MATCH_EXPR      "match"  Expr  MATCH_ARM_LIST
MATCH_ARM_LIST  "{"  MATCH_ARM*  "}"                       one per line, or after a block
MATCH_ARM       Pattern  MATCH_GUARD?  "->"  Expr
MATCH_GUARD     "if"  Expr
```

Write `if_expr`, `match_expr`, `match_arm_list` and `match_arm`.

- An `if` without `else` is an error: `expected `else`: an `if` is an
  expression, so it needs both branches`. Complete the `IF_EXPR`
  anyway.
- The condition and the scrutinee are expressions, followed by `{`. No
  special case: an expression can't continue with a `{` (Maxim has no
  struct literals, which give Rust exactly this problem: Rust forbids
  them in `if` conditions).
- `match_arm_list`: if there's no `{`, report `expected `{` and the
  match arms`; if it's on the next line, `not_on_new_line(p)`. Right
  after the `{`, a `}` means no arms: report `expected a match arm: a
  `match` needs at least one`. Then loop until `}`: if the current
  token can start a pattern (`patterns::PAT_FIRST`), parse an arm;
  else `err_and_bump` (the guard against looping, as in blocks), with
  a special message for `-`: `expected a match arm: negative numbers
  can't be patterns`.
- **Separators.** Your spec's `Arms`: an arm needs a `NEWLINE` before
  it, unless the arm before it had a block as its result. That's
  Part 3's "parse first, decide after": `match_arm` returns whether its
  result was a `BLOCK_EXPR` (`CompletedMarker::kind()`, from Part 7),
  and the loop remembers whether the next arm needs a `NEWLINE`. When
  it's missing, report `expected a line break between these match
  arms, or a block `{ … }` as the result of the first`, only at a token
  that can start a pattern, as in blocks.
- A guard is on the pattern's line: in `A` newline `if x -> …`, the
  `if` starts a new arm (an error), not a guard. Take it only if
  `!p.at_newline()`.

```bash
cargo test -p mini-parser --lib atoms::tests::ifs
cargo test -p mini-parser --lib atoms::tests::matches
cargo test -p mini-parser --lib atoms::tests::arms_with
cargo test -p mini-parser --lib atoms::tests::broken_matches
```

<details>
<summary>Hint: what `expr` returns</summary>

`expr(p)` returns an `Option<CompletedMarker>`: `None` if there was
no expression. "Is it `Some`, and a block?" is
`result.is_some_and(|result| result.kind() == BLOCK_EXPR)`. A result
like `{ 1 } + 2` is a `BIN_EXPR`, so the next arm needs its line.

</details>

<details>
<summary>Solution</summary>

```rust
fn if_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(T![if]);
    expr(p);
    block(p);
    if p.at(T![else]) {
        p.bump(T![else]);
        if p.at(T![if]) {
            if_expr(p);
        } else {
            block(p);
        }
    } else {
        p.error("expected `else`: an `if` is an expression, so it needs both branches");
    }
    m.complete(p, IF_EXPR)
}
```

```rust
fn match_expr(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(T![match]);
    expr(p);
    match_arm_list(p);
    m.complete(p, MATCH_EXPR)
}
```

```rust
fn match_arm_list(p: &mut Parser<'_>) {
    if !p.at(T!['{']) {
        p.error("expected `{` and the match arms");
        return;
    }
    not_on_new_line(p);
    let m = p.start();
    p.bump(T!['{']);
    if p.at(T!['}']) {
        p.error("expected a match arm: a `match` needs at least one");
    }
    let mut needs_newline = false;
    while !p.at(T!['}']) && !p.at(EOF) {
        if needs_newline && !p.at_newline() && p.at_ts(patterns::PAT_FIRST) {
            p.error("expected a line break between these match arms, or a block `{ … }` as the result of the first");
        }
        needs_newline = if p.at_ts(patterns::PAT_FIRST) {
            !match_arm(p)
        } else if p.at(T![-]) {
            p.err_and_bump("expected a match arm: negative numbers can't be patterns");
            false
        } else {
            p.err_and_bump("expected a match arm");
            false
        };
    }
    p.expect(T!['}']);
    m.complete(p, MATCH_ARM_LIST);
}
```

```rust
fn match_arm(p: &mut Parser<'_>) -> bool {
    let m = p.start();
    patterns::pattern(p);
    if p.at(T![if]) && !p.at_newline() {
        let guard = p.start();
        p.bump(T![if]);
        expr(p);
        guard.complete(p, MATCH_GUARD);
    }
    p.expect(T![->]);
    let result = expr(p);
    m.complete(p, MATCH_ARM);
    result.is_some_and(|result| result.kind() == BLOCK_EXPR)
}
```

`match x { -1 -> a }`: the special message, then `1 -> a` parses as an
arm. The error points at the cause, and the rest of the match still
has a tree. After an error, `needs_newline` is `false`: the junk was
reported already, no need to add a missing line break.

Why the exception for blocks? A block's `}` ends the arm as clearly as
a line break: `A -> { 1 } B -> { 2 }` can't be misread. Rust has the
same exception for its commas between arms. Without a block, where an
arm ends would depend on what can continue an expression: the
reference grammar's problem with `-` and `(` again.

</details>

---

## 9g. Interpolated strings

```text
INTERPOLATED_STRING   STRING_START  Expr  (STRING_MIDDLE  Expr)*  STRING_END
```

Write `interpolated_string`: the lexer of Part 5 did the hard part.

```bash
cargo test -p mini-parser --lib atoms
```

All of `atoms.rs`'s tests should pass.

<details>
<summary>Solution</summary>

```rust
fn interpolated_string(p: &mut Parser<'_>) -> CompletedMarker {
    let m = p.start();
    p.bump(STRING_START);
    loop {
        expr(p);
        if !p.eat(STRING_MIDDLE) {
            break;
        }
    }
    p.expect(STRING_END);
    m.complete(p, INTERPOLATED_STRING)
}
```

`"${}"` reports `expected an expression` at the `}"`: a `STRING_END`,
which is in `EXPR_RECOVERY`, so it isn't skipped, and `expect` finds
it.

</details>

---

**Next:** [Part 10 — Declarations and whole files](part10-declarations.md)
