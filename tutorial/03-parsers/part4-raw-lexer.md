# Part 4 — A raw lexer, like rustc's

[← Step 3 overview](guide.md)

From specification to code. The lexer comes in two layers, as in Rust's
compilers; this part writes the first, the **raw lexer**, in
`src/lexer/raw.rs`.

**Rust you'll use:** [iterators over characters](rust-concepts.md#1-characters-strings-and-bytes),
[`matches!` and patterns on tuples](rust-concepts.md#2-matches-and-patterns-on-tuples).

---

## The idea: a lexer that can't fail

Rust's compiler and rust-analyzer share one lexer, a crate called
`rustc_lexer`. Its job is deliberately small:

```rust
pub struct Token {
    pub kind: TokenKind,   // Ident, Literal { kind, .. }, Whitespace, Semi…
    pub len: u32,          // in bytes
}

pub fn tokenize(input: &str) -> impl Iterator<Item = Token>
```

A token is **a kind and a length**. No position: it's the sum of the
lengths before it. No text: it's `input[start..start + len]`. No value:
the number `0x1F` isn't converted. Not even keywords: `fn` is an
`Ident`, like `area`.

And it **never fails**. An unterminated string is a string token with
a flag, `terminated: false`. An unknown character is an `Unknown`
token of one character. Every byte of the input ends up in exactly one
token, whatever the input.

Why so little? Because then it can be shared. rustc and rust-analyzer
want different things from the tokens: rustc interns names and builds
its own token type, rust-analyzer keeps every byte for its lossless
trees. Each adds a second layer on top, that **cooks** the raw tokens
for its needs: that's Part 5. And a lexer that does one thing is easy to
test and fast.

Your raw lexer works the same way:

```rust
pub struct RawToken {
    pub kind: RawKind,
    pub len: u32,
}

pub enum RawKind {
    Whitespace,
    LineComment,
    BlockComment { terminated: bool },
    Ident,
    Int,
    Float,
    Str { terminated: bool, opens_hole: bool },
    StrContinuation { terminated: bool, opens_hole: bool },
    Punct,
    Unknown,
    Eof,
}
```

One difference with rustc_lexer: it reads **multi-character
punctuation** as one token, `->`, `|>`, `**`. rustc_lexer reads `>>` as
two `>` tokens and lets the parser glue them, because in Rust,
`Vec<Vec<u8>>` ends with two closing brackets, not a shift. Maxim has
no `>>` operator, so the problem doesn't arise, and the longest match
of the spec can be applied right away.

---

## Read the code

Open `src/lexer/cursor.rs`. The `Cursor` is how the lexer reads, and
it's rustc_lexer's:

| Method                | Does                                                            |
|-----------------------|-----------------------------------------------------------------|
| `first()`, `second()` | the next character, the one after, without consuming them; `EOF_CHAR` (`'\0'`) past the end |
| `bump()`              | consume the next character                                      |
| `eat_while(pred)`     | consume characters while `pred` holds                           |
| `is_eof()`            | is everything read? (how to tell a real `'\0'` from the end)    |
| `pos_within_token()`  | how many bytes the current token has so far                     |

Peeking two characters ahead is all Maxim needs: is `1.` followed by a
digit?

Then open `src/lexer/raw.rs`. `advance_token` is the lexer: it bumps
the first character, and dispatches on it. Whitespace and identifiers
are done: read `whitespace()`, it's the model for the rest.

```rust
let kind = match first {
    ' ' | '\t' | '\n' => self.whitespace(),
    '\r' if self.first() == '\n' => self.whitespace(),
    '/' if self.first() == '/' => self.line_comment(),
    '/' if self.first() == '*' => self.block_comment(),
    'a'..='z' | 'A'..='Z' | '_' => {
        self.eat_while(is_ident_continue);
        RawKind::Ident
    }
    '0'..='9' => self.number(),
    '"' => { … }
    _ => self.punct(first),
};
```

When a helper runs, the first character is already consumed: it's in
`first`, the argument, and `self.first()` is the character *after* it.

```bash
cargo test -p mini-parser --lib lexer::raw
```

Two tests pass already: whitespace and identifiers.

---

## 4a. Comments

Write `line_comment` and `block_comment`. The first `/` is consumed;
`self.first()` is the second `/`, or the `*`.

- A line comment goes up to the line break, not included: stop at `\n`
  and at `\r`.
- A block comment goes up to the first `*/`, included. If the text ends
  first, it's unterminated: return `BlockComment { terminated: false }`,
  having consumed everything.

```bash
cargo test -p mini-parser --lib lexer::raw::tests::comments
```

<details>
<summary>Hint: the block comment's loop</summary>

`bump()` returns an `Option<char>`: `None` at the end. Bump one
character at a time; after a `*`, check whether `self.first()` is `/`.

</details>

<details>
<summary>Solution</summary>

```rust
fn line_comment(&mut self) -> RawKind {
    self.bump(); // the second `/`
    self.eat_while(|c| c != '\n' && c != '\r');
    RawKind::LineComment
}
```

```rust
fn block_comment(&mut self) -> RawKind {
    self.bump(); // the `*`
    loop {
        match self.bump() {
            None => return RawKind::BlockComment { terminated: false },
            Some('*') if self.first() == '/' => {
                self.bump();
                return RawKind::BlockComment { terminated: true };
            }
            Some(_) => {}
        }
    }
}
```

`/*/` isn't a complete comment: after `/*`, the `/` is a character
inside it.

</details>

---

## 4b. Punctuation

Write `punct`: every one- and two-character punctuation of the spec's
`PUNCTUATION` rule, longest first. `first` is the consumed character,
`self.first()` the next. Anything else is `Unknown`, one character
long: `@`, `;`, `$`, and `&` or `~` alone.

The `/` that starts a comment never gets here: `advance_token` checked
for it before.

```bash
cargo test -p mini-parser --lib lexer::raw::tests::punctuation
cargo test -p mini-parser --lib lexer::raw::tests::half_operators
```

<details>
<summary>Hint: longest match</summary>

First check whether `(first, self.first())` is a two-character token;
if so, bump the second character. Otherwise, `first` alone is a token,
or it's unknown. A `match` on the pair of characters, with `|` between
patterns, keeps it short: `('=', '=' | '>')` matches `==` and `=>`.

</details>

<details>
<summary>Solution</summary>

```rust
fn punct(&mut self, first: char) -> RawKind {
    let second = self.first();
    let two_chars = matches!(
        (first, second),
        (':', ':')
            | ('.', '.')
            | ('=', '=' | '>')
            | ('!', '=')
            | ('<', '=')
            | ('>', '=')
            | ('|', '|' | '>')
            | ('&', '&')
            | ('-', '>')
            | ('+', '+')
            | ('*', '*')
            | ('~', '/')
    );
    if two_chars {
        self.bump();
        return RawKind::Punct;
    }
    match first {
        '(' | ')' | '{' | '}' | '[' | ']' | '<' | '>' | ',' | ':' | '.' | '=' | '!' | '|'
        | '-' | '+' | '*' | '/' | '%' | '?' => RawKind::Punct,
        _ => RawKind::Unknown,
    }
}
```

The raw token doesn't say *which* punctuation it is: its text does, and
Part 5 maps the text to a kind.

</details>

---

## 4c. Numbers

Write `number`: the first digit is consumed. Read digits and `_`; then,
if a `.` follows **and a digit follows it**, it's a float: read the `.`
and the digits after it.

`1.x` is an integer, then `.`, then a name: a field of the number `1`.
`1..2` is `1`, `..`, `2`. That's why the lexer must look two characters
ahead, and the spec says `DIGITS "." DIGITS`.

```bash
cargo test -p mini-parser --lib lexer::raw::tests::numbers
```

<details>
<summary>Solution</summary>

```rust
fn number(&mut self) -> RawKind {
    self.eat_while(is_digit_continue);
    // `1.5` is a float, but `1.x` and `1..2` start with an integer.
    if self.first() == '.' && self.second().is_ascii_digit() {
        self.bump();
        self.eat_while(is_digit_continue);
        RawKind::Float
    } else {
        RawKind::Int
    }
}
```

</details>

---

## 4d. Strings

Write `string_chunk`: read a piece of string **up to and including its
end**, and say how it ended. It returns `(terminated, opens_hole)`:

| The piece ends with           | Returns          |
|-------------------------------|------------------|
| `"`                           | `(true, false)`  |
| `${`                          | `(true, true)`   |
| a line break (not consumed)   | `(false, false)` |
| the end of the text           | `(false, false)` |

A backslash escapes the next character. The raw lexer doesn't check
escapes (Part 5 does), but it must skip the character after a `\` when
it's one that would end the piece: `\"`, `\$` (so `\${` doesn't open a
hole), and `\\` (so `\\"` ends the string: the backslash is the one
escaped).

```bash
cargo test -p mini-parser --lib lexer::raw
```

All the raw lexer's tests should pass.

### How do holes end?

Read `continue_string` and the test `continue_string_reads_after_a_hole`.
`"a${x}b"` lexes as `"a${`, `x`, `}b"`. But the raw lexer can't know
that this `}` ends a hole: it doesn't count braces. Without help, it
reads `}` as punctuation and `b"` as an identifier, then an
unterminated string: look at the test `a_string_with_a_hole_stops_at_the_hole`.

So `continue_string` exists for the cooking layer, which counts
braces, to call instead of `advance_token` when the next `}` closes a
hole. The raw lexer stays simple, and the context goes where context
is known. That's Part 5c.

<details>
<summary>Hint: the loop</summary>

Loop on `self.first()`, without consuming: decide, then bump. Check
`is_eof()` in a guard, `_ if self.is_eof() => …`, before the catch-all:
past the end, `first()` returns `'\0'`, which matches nothing else.

</details>

<details>
<summary>Solution</summary>

```rust
fn string_chunk(&mut self) -> (bool, bool) {
    loop {
        match self.first() {
            '"' => {
                self.bump();
                return (true, false);
            }
            '$' if self.second() == '{' => {
                self.bump();
                self.bump();
                return (true, true);
            }
            '\\' => {
                self.bump();
                // Skip what could end the string or open a hole; the
                // other escapes are checked by the next layer.
                if matches!(self.first(), '\\' | '"' | '$') {
                    self.bump();
                }
            }
            '\n' | '\r' => return (false, false),
            _ if self.is_eof() => return (false, false),
            _ => {
                self.bump();
            }
        }
    }
}
```

</details>

---

## Check

```bash
cargo test -p mini-parser --lib lexer::raw
```

The raw lexer passes the property every lexer of this step must have:
**the lengths of the tokens add up to the length of the text**. Every
branch of `advance_token` consumes at least one character, so the
loop ends, and nothing is skipped.

---

**Next:** [Part 5 — Cooking tokens](part5-cooking.md)
