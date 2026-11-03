# Part 7 — The parser's engine

[← Step 3 overview](guide.md)

Before the grammar, the machinery every grammar function uses: reading
tokens, starting and finishing nodes, reporting errors. It's
rust-analyzer's design, in `src/parser/mod.rs`.

**Rust you'll use:** [`Cell`](rust-concepts.md#9-cell-mutation-behind-shared-references),
[`Drop`](rust-concepts.md#10-drop-code-that-runs-when-a-value-dies),
[`const fn` and bit sets](rust-concepts.md#11-const-fn-and-bit-sets),
[tests that must panic](rust-concepts.md#12-tests-that-must-panic-and-track_caller).

---

## The idea: a parser that doesn't build the tree

In step 1, each grammar function returned a node: `Expr::binary(op,
lhs, rhs)`. rust-analyzer's parser returns nothing. It **emits
events** (the ones the builder of Part 6 consumes), and grammar
functions mark where nodes start and end:

```rust
// FnType ::= ParamTypeList "->" Type
fn fn_type(p: &mut Parser<'_>) {
    let m = p.start();          // a node starts here…
    param_type_list(p);
    p.expect(T![->]);
    type_(p);
    m.complete(p, FN_TYPE);     // …and ends here: it's a FN_TYPE
}
```

What this buys:

- **The parser knows nothing about trees**: no allocation of nodes, no
  trivia (the builder puts it back), no positions. The same events
  could build another kind of tree.
- **Broken input is easy.** A grammar function that finds an
  unexpected token reports an error and goes on; the node is still
  completed, just with fewer children. No `Result` to propagate, no
  partial node to repair: the tree is whatever was emitted.
- **A node's kind can be decided at the end**: `p.start()` doesn't say
  what it starts. `m.complete(p, LET_DECL)` or `m.complete(p,
  LET_STMT)`: the same function can parse both (Part 9).

---

## Read the code

Open `src/parser/mod.rs`.

**`Input`** is what the parser reads: the tokens that aren't trivia,
and one bit per token, `newline_before`. That bit is your spec's
`BOL_LPAREN`: rather than a token kind of its own, a fact about where a
token is. The tree keeps the real kind, `L_PAREN`, and the places that
accept any `(` don't have to list two kinds. (rust-analyzer keeps a bit
per token too, for the same kind of reason: whether two tokens touch,
so that `>` `>` can be read as `>>`.)

**`Parser`** has a position in the input, the events emitted so far,
and `steps`. Read `nth`: every look at a token counts a step, and
consuming a token resets the count. A grammar function that loops
without consuming anything would look at the same token forever; with
the limit, it panics with `the parser seems stuck` instead of hanging.
Every loop of the grammar must consume a token or stop: the step limit
turns a violation into a test failure you can debug.

Then the API, which Parts 8 to 10 use everywhere:

| Method                     | Does                                                                     |
|----------------------------|--------------------------------------------------------------------------|
| `current()`, `nth(n)`      | the kind of the current token, of the token `n` ahead (`EOF` past the end) |
| `at(kind)`, `nth_at(n, kind)`, `at_ts(set)` | is the current (or `n`th) token of that kind, or in that set? |
| `at_line_start()`          | is there a line break before the current token?                          |
| `bump(kind)`               | consume the current token, which must be a `kind` (it panics otherwise)  |
| `bump_any()`               | consume the current token, whatever it is                                |
| `eat(kind)`                | consume the current token if it's a `kind`; say whether it did           |
| `expect(kind)`             | `eat`, or report "expected …"                                            |
| `start()`                  | start a node: returns a `Marker`                                         |
| `error(msg)`               | report an error at the current token                                     |
| `err_recover(msg, set)`    | report an error, and skip the current token unless it's in `set`        |

```bash
cargo test -p mini-parser --lib parser::
```

---

## 7a. The input

Write `Input::from_lexed`: go through the lexer's tokens **including
the final `EOF`** (`0..=lexed.len()`); skip trivia, but remember
whether any of it contains `\n`; push every other token's kind, with
that bit, and reset it.

```bash
cargo test -p mini-parser --lib parser::tests::the_input
```

<details>
<summary>Solution</summary>

```rust
pub(crate) fn from_lexed(lexed: &Lexed<'_>) -> Input {
    let mut input = Input {
        kind: Vec::new(),
        newline_before: Vec::new(),
    };
    let mut newline = false;
    for i in 0..=lexed.len() {
        let kind = lexed.kind(i);
        if kind.is_trivia() {
            newline |= lexed.text(i).contains('\n');
        } else {
            input.kind.push(kind);
            input.newline_before.push(newline);
            newline = false;
        }
    }
    input
}
```

A block comment that spans lines counts as a line break: its text
contains `\n`.

</details>

---

## 7b. Reading tokens

Write `eat`, `expect` and `at_line_start`.

- `eat(kind)`: if the current token is a `kind`, consume it with
  `do_bump` (which emits the `Token` event) and return `true`.
- `expect(kind)`: `eat`, or report the error `expected {…}`, using
  `kind.describe()`: "expected `)`", "expected a lowercase name".
- `at_line_start()`: the bit of the current token.

```bash
cargo test -p mini-parser --lib parser::tests::looking_ahead
cargo test -p mini-parser --lib parser::tests::at_line_start
```

<details>
<summary>Solution</summary>

```rust
pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
    if !self.at(kind) {
        return false;
    }
    self.do_bump(kind);
    true
}
```

```rust
pub(crate) fn expect(&mut self, kind: SyntaxKind) -> bool {
    if self.eat(kind) {
        return true;
    }
    self.error(format!("expected {}", kind.describe()));
    false
}
```

```rust
pub(crate) fn at_line_start(&self) -> bool {
    self.input.newline_before(self.pos)
}
```

`expect` reports and returns: it doesn't consume the unexpected token.
The caller decides what's next, and the next `expect` usually fails at
the same token: `parse` keeps one error per position (read it in
`lib.rs`), so you see the first.

</details>

---

## 7c. Markers

A **marker** is a node being built. `p.start()` pushes a placeholder
`Start` event, a tombstone, and returns a `Marker` that remembers its
index. Then:

- `m.complete(p, kind)` fills the placeholder in with the kind, pushes
  `Finish`, and returns a `CompletedMarker`;
- `m.abandon(p)` gives up: the placeholder stays a tombstone, and
  everything since the start goes to the parent. If nothing was emitted
  since the start, it can even pop the placeholder;
- `completed.precede(p)` starts a new node that will be the **parent**
  of the completed one: the forward parents of Part 6. It starts a new
  marker, then writes in the completed node's `Start` event the
  distance to the new one.

Write `start`, `complete`, `abandon` and `precede`.

### The drop bomb

A marker that's neither completed nor abandoned is a bug: its
placeholder stays a tombstone, its node silently disappears. Nothing
would notice, except a strange tree much later. So a `Marker` carries
a **drop bomb**: a value that panics when it's dropped, unless it was
**defused**. `complete` and `abandon` defuse it; forgetting to call
them panics right where the marker goes out of scope.

Write `DropBomb::drop`: panic with the bomb's message, unless it's
defused, or the thread is already panicking (`std::thread::panicking()`):
a panic while unwinding from another panic aborts the whole program,
and the first panic's message would be lost.

```bash
cargo test -p mini-parser --lib parser::tests::markers
cargo test -p mini-parser --lib parser::tests::precede
cargo test -p mini-parser --lib parser::tests::a_forgotten
cargo test -p mini-parser --lib parser::tests::eat_and_expect
```

<details>
<summary>Hint: `complete` takes `mut self`</summary>

`complete(mut self, …)` takes the marker by value: after `m.complete(p,
KIND)`, `m` is gone, so it can't be completed twice. `mut` lets the
method defuse its own bomb. When the method returns, the marker is
dropped, and its defused bomb with it.

</details>

<details>
<summary>Solution</summary>

```rust
pub(crate) fn start(&mut self) -> Marker {
    let pos = self.events.len() as u32;
    self.events.push(Event::tombstone());
    Marker {
        pos,
        bomb: DropBomb::new("a Marker must be completed or abandoned"),
    }
}
```

```rust
pub(crate) fn complete(mut self, p: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
    self.bomb.defuse();
    match &mut p.events[self.pos as usize] {
        Event::Start { kind: slot, .. } => *slot = kind,
        _ => unreachable!("a marker points at a Start event"),
    }
    p.events.push(Event::Finish);
    CompletedMarker { pos: self.pos }
}
```

```rust
pub(crate) fn abandon(mut self, p: &mut Parser<'_>) {
    self.bomb.defuse();
    if self.pos as usize == p.events.len() - 1 {
        // Nothing happened since `start`: drop its event.
        p.events.pop();
    }
    // Otherwise its `Start` event stays a tombstone.
}
```

```rust
pub(crate) fn precede(self, p: &mut Parser<'_>) -> Marker {
    let new = p.start();
    match &mut p.events[self.pos as usize] {
        Event::Start { forward_parent, .. } => *forward_parent = Some(new.pos - self.pos),
        _ => unreachable!("a marker points at a Start event"),
    }
    new
}
```

```rust
fn drop(&mut self) {
    // While unwinding from another panic, a second one would abort.
    if !self.defused && !std::thread::panicking() {
        panic!("{}", self.msg);
    }
}
```

rust-analyzer uses the `drop_bomb` crate for this; it's the same ten
lines.

</details>

---

## 7d. Recovery

When a grammar function finds a token it can't use, it reports an
error. Then what? Two bad options: stop parsing (the rest of the file
gets no tree), or skip the token (but it might be exactly what the
caller's caller expects: a `}` that ends the block, or the `fn` of the
next declaration).

`err_recover(msg, recovery)` decides with a **recovery set**: the
tokens that are better left for someone else. If the current token is
in the set (or is `EOF`), it only reports. Otherwise it reports, and
consumes the token in an `ERROR` node, so the tree still holds it.

Sets of tokens are `TokenSet`s: read `src/parser/token_set.rs`. A
`TokenSet` is a `u128` with one bit per kind; the token kinds come
first in `SyntaxKind`, so they all fit. `TokenSet::new` is a `const
fn`, so the grammar can declare sets as constants, evaluated at
compile time:

```rust
const DECL_FIRST: TokenSet = TokenSet::new(&[T![import], T![type], T![fn], …]);
```

Write `err_recover`.

```bash
cargo test -p mini-parser --lib parser::
```

All the engine's tests should pass.

<details>
<summary>Solution</summary>

```rust
pub(crate) fn err_recover(&mut self, msg: &str, recovery: TokenSet) {
    if self.at_ts(recovery) || self.at(EOF) {
        self.error(msg);
        return;
    }
    let m = self.start();
    self.error(msg);
    self.bump_any();
    m.complete(self, ERROR);
}
```

`err_and_bump(msg)` is `err_recover` with an empty set: always skip.

</details>

### Where to go on after an error

Every grammar function of Parts 8 to 10 makes this choice where it can
fail, with a recovery set made for its context: after a broken type,
what can come next is a `,`, a `)`, a `>`, a `=`, a `{`, or the start
of the next declaration. Good sets are what make one typo give one
error rather than fifty. Part 12 comes back to it.

---

**Next:** [Part 8 — Types and patterns](part8-types-patterns.md)
