# Part 5 — Cooking tokens

[← Step 3 overview](guide.md)

The second layer of the lexer, in `src/lexer/mod.rs`: from raw tokens
to the tokens the parser reads. rustc calls this step **cooking** the
tokens.

**Rust you'll use:** [`Peekable` and `next_if`](rust-concepts.md#3-peekable-and-next_if),
[`Option` combinators](rust-concepts.md#4-option-and-bool-combinators),
[declarative macros](rust-concepts.md#5-declarative-macros-macro_rules) (to read `T![…]`).

---

## The idea: one layer per job

The raw lexer cut the text. Cooking does the rest:

- give each token its **`SyntaxKind`**: `fn` is `FN_KW`, `area` is
  `LOWER_IDENT`, `->` is `THIN_ARROW`;
- turn the raw lexer's flags into **error messages**, with the range
  they point at;
- apply the **rules that need context**, from section 2.6 of your spec:
  which `}` closes a hole, and which `<` would start HTML.

In rustc, this layer is `rustc_parse::lexer`. In rust-analyzer, it's
`LexedStr`, and yours follows it:

```rust
pub struct Lexed<'a> {
    text: &'a str,
    kind: Vec<SyntaxKind>,  // one per token, then `EOF`
    start: Vec<u32>,        // where each token starts, then the end of the text
    errors: Vec<LexError>,  // a message and a range each
}
```

Whitespace and comments are tokens like the others: `WHITESPACE` and
`COMMENT`. The parser will skip them (Part 7), but the syntax tree
keeps them (Part 6): the tokens are **lossless**, their texts put end
to end are the source.

### `SyntaxKind`

Open `src/syntax_kind.rs`. One enum lists every kind of **token** and
of **node** (nodes come from the parser, in Parts 7 to 10): rust-analyzer
does the same, so that a tree can store both with one type. Read:

- `is_trivia`, `from_keyword`, `from_punct`, `describe` (for error
  messages: "expected `)`");
- the `T![…]` macro at the bottom: `T![fn]` is `SyntaxKind::FN_KW`,
  `T![->]` is `THIN_ARROW`. The grammar of Parts 8 to 10 reads like the
  spec thanks to it: `p.expect(T![->])`.

### The cooker

Now read `lex` and the `Cooker`. `lex` runs the raw lexer to the end,
and gives each raw token to `cook`, which:

1. computes the token's range;
2. picks its kind with a `match` on the raw kind;
3. remembers two things for the context rules: whether a line break was
   seen since the last token that isn't trivia (`newline_since_token`),
   and that last token's kind (`last_token`);
4. pushes the token.

`cook` calls the four functions you write.

```bash
cargo test -p mini-parser --lib lexer::tests
```

---

## 5a. Keywords and names

Write `ident_kind`: the kind of an identifier's text. `_` alone is
`UNDERSCORE`; a keyword is its keyword kind (`SyntaxKind::from_keyword`
knows them); a name starting with an uppercase letter is
`UPPER_IDENT`; anything else, `LOWER_IDENT`.

This is how lexers recognize keywords: read the longest identifier,
then look it up. `iffy` is never mistaken for `if` then `fy`.

```bash
cargo test -p mini-parser --lib lexer::tests::keywords
cargo test -p mini-parser --lib lexer::tests::unknown_characters
```

<details>
<summary>Solution</summary>

```rust
fn ident_kind(text: &str) -> SyntaxKind {
    if text == "_" {
        UNDERSCORE
    } else if let Some(keyword) = SyntaxKind::from_keyword(text) {
        keyword
    } else if text.starts_with(|c: char| c.is_ascii_uppercase()) {
        UPPER_IDENT
    } else {
        LOWER_IDENT
    }
}
```

`str::starts_with` takes a character, a string, or a function from
`char` to `bool`.

</details>

---

## 5b. Strings and escapes

Write the method `string`, which `cook` calls for every piece of
string: report `"this string never closes"` on the piece's range if it
isn't terminated, then one error per bad escape.

And write `bad_escapes`: given a piece of string, return the escapes
Maxim doesn't know, as messages with ranges **relative to the piece**.
Known: `\n`, `\t`, `\"`, `\\`, `\$`, and `\u{…}` with 1 to 6
hexadecimal digits that make a valid `char`. Messages:

| Escape                                          | Message                     | Range             |
|-------------------------------------------------|-----------------------------|-------------------|
| `\` then an unknown character, like `\q`        | ``unknown escape `\q` ``    | the `\` and the character |
| `\u` not followed by `{`, 1 to 6 hex digits, `}`, or not a valid `char` | `invalid unicode escape` | from the `\` to where reading stopped |
| `\` at the very end                             | ``unknown escape `\` ``     | the `\`           |

```bash
cargo test -p mini-parser --lib lexer::tests::unterminated
cargo test -p mini-parser --lib lexer::tests::bad_escapes
cargo test -p mini-parser --lib lexer::tests::numbers_strings
```

Read the expected errors in `bad_escapes_are_errors` first: they pin
down the ranges.

<details>
<summary>Hint 1: reading the escapes</summary>

`text.char_indices()` gives each character with its byte offset. Make
it `.peekable()`: then `chars.next_if(|&(_, c)| c == '{')` consumes the
next character only if it matches, and `chars.peek()` gives the offset
where reading stopped (or the end of the text, if there's nothing
left).

</details>

<details>
<summary>Hint 2: a valid `char`</summary>

`u32::from_str_radix(&digits, 16)` parses the hexadecimal digits, and
`char::from_u32` refuses surrogates and values above `0x10FFFF`. Both
return an `Option`, or a `Result` that `.ok()` turns into one: chain
them with `and_then`.

</details>

<details>
<summary>Solution</summary>

```rust
fn string(&mut self, text: &str, range: TextRange, terminated: bool) {
    if !terminated {
        self.error("this string never closes", range);
    }
    for (msg, escape) in bad_escapes(text) {
        let start = range.start() + escape.start();
        self.error(msg, TextRange::new(start, start + escape.len()));
    }
}
```

```rust
fn bad_escapes(text: &str) -> Vec<(String, TextRange)> {
    let mut errors = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        if c != '\\' {
            continue;
        }
        let end = |chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>| {
            chars.peek().map_or(text.len(), |&(i, _)| i)
        };
        match chars.next() {
            Some((_, 'n' | 't' | '"' | '\\' | '$')) => {}
            Some((_, 'u')) => {
                let mut digits = String::new();
                let mut valid = chars.next_if(|&(_, c)| c == '{').is_some();
                if valid {
                    while let Some((_, c)) = chars.next_if(|&(_, c)| c.is_ascii_hexdigit()) {
                        digits.push(c);
                    }
                    valid = chars.next_if(|&(_, c)| c == '}').is_some()
                        && (1..=6).contains(&digits.len())
                        && u32::from_str_radix(&digits, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .is_some();
                }
                if !valid {
                    let range = TextRange::new(start as u32, end(&mut chars) as u32);
                    errors.push(("invalid unicode escape".to_string(), range));
                }
            }
            Some((_, other)) => {
                let range = TextRange::new(start as u32, end(&mut chars) as u32);
                let msg = format!("unknown escape `\\{}`", other.escape_debug());
                errors.push((msg, range));
            }
            None => {
                let range = TextRange::new(start as u32, text.len() as u32);
                errors.push(("unknown escape `\\`".to_string(), range));
            }
        }
    }
    errors
}
```

`end` is a closure, not a function, because it needs `text`. It takes
`chars` as a parameter rather than capturing it: a closure that
captured `chars` would borrow it for as long as the closure lives, and
the loop couldn't call `chars.next()` anymore.

The token stays a `STRING` with errors attached: the parser can still
use it, and only the error reporting knows it's broken.

</details>

---

## 5c. Holes

Interpolated strings: in Part 4, you saw that only the cooker knows
which `}` closes a hole. It keeps a **stack** of the holes open right
now, `holes: Vec<Hole>`, innermost last:

- `cook` pushes a `Hole` when a piece of string ends with `${`
  (`STRING_START`, `STRING_MIDDLE`), and pops one when a piece starts
  with the `}` of a hole (`STRING_MIDDLE`, `STRING_END`). That's done.
- Inside a hole, braces nest: in `"${ { a } }"`, the first `}` closes
  the block. Each `Hole` counts the `{` still open in it: its `depth`.

Write the two missing pieces:

- `track_brace(kind)`: inside a hole, count its braces: `L_CURLY` adds
  one to the innermost hole's depth, `R_CURLY` takes one away.
- `at_hole_end(next)`: inside a hole, is the next character the `}`
  that closes the innermost one? It is if it's a `}` and the depth is 0.

Both are only called inside a hole (read the callers: `lex` and
`punct`): `self.holes.last()` is never `None` there, so
`.expect("inside a hole")` is right.

Why a stack and not one counter? Strings nest inside holes:
`"a${ "b${c}d" }e"`. When the inner string's hole closes, the outer
hole must still be open, with its own count.

```bash
cargo test -p mini-parser --lib lexer::tests::interpolated
cargo test -p mini-parser --lib lexer::tests::holes_nest
cargo test -p mini-parser --lib lexer::tests::a_hole
cargo test -p mini-parser --lib lexer::tests::a_closing_brace
```

Read also, in `cook`, the check that whitespace and comments in a hole
contain no line break: section 2.6's "a hole must fit on one line". And
`finish`, which reports a hole still open at the end of the text.

<details>
<summary>Solution</summary>

```rust
fn at_hole_end(&self, next: char) -> bool {
    let hole = self.holes.last().expect("inside a hole");
    next == '}' && hole.depth == 0
}
```

```rust
fn track_brace(&mut self, kind: SyntaxKind) {
    let hole = self.holes.last_mut().expect("inside a hole");
    match kind {
        L_CURLY => hole.depth += 1,
        R_CURLY => hole.depth -= 1,
        _ => {}
    }
}
```

This stack is a **lexer mode**: what the lexer does at a `}` depends
on a state that tokens before it set. JavaScript lexers have the same
for template literals, and Maxim's full lexer (in `example/`) has more
modes, for HTML.

</details>

---

## 5d. HTML

Section 2.6, rule 3: a `<` followed by a letter, where an expression
can start, would start HTML, which this parser leaves out. `punct`
calls `starts_html` for every `<`; when it says yes, the token becomes
an `ERROR` with the message `HTML elements are not supported by this
parser`.

Write `can_end_expr(kind)`: can an expression end with a token of this
kind? The list is in your spec. Then `starts_html(range)`: is the
character right after the `<` (`range` is the `<`'s range) an ASCII
letter, and can an expression start here? It can after a line break
(`self.newline_since_token`), at the very start (`self.last_token` is
`None`), and after a token that can't end an expression.

```bash
cargo test -p mini-parser --lib lexer::tests
```

All the lexer's tests should pass.

<details>
<summary>Hint: `None` counts as "can start"</summary>

`Option::is_none_or(f)` is `true` for `None`, and `f(value)` for
`Some(value)`: exactly "no token before, or one that can't end an
expression".

</details>

<details>
<summary>Solution</summary>

```rust
fn starts_html(&self, range: TextRange) -> bool {
    let rest = &self.lexed.text[range.end() as usize..];
    let before_letter = rest.starts_with(|c: char| c.is_ascii_alphabetic());
    let expression_can_start =
        self.newline_since_token || self.last_token.is_none_or(|kind| !can_end_expr(kind));
    before_letter && expression_can_start
}
```

```rust
pub(crate) fn can_end_expr(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        LOWER_IDENT
            | UPPER_IDENT
            | UNDERSCORE
            | INT_NUMBER
            | FLOAT_NUMBER
            | STRING
            | STRING_END
            | R_PAREN
            | R_CURLY
            | R_BRACK
            | QUESTION
    )
}
```

One token of context is the cheapest context there is: no parser, no
mode stack. The tradeoff is that the rule must hold for every
grammatical situation, and it's easy to get wrong. Rust's lexer has no
such rule at all: a language designed with a separate lexer in mind
avoids them.

`can_end_expr` is `pub(crate)`: the parser uses it too, in Part 7, for
the other context rule, the one that makes a line break a `NEWLINE`.

</details>

---

## Check

```bash
cargo test -p mini-parser --lib lexer
```

Your lexer works on real files now:

```bash
cargo parse --emit=tokens mini-parser/corpus/ok/strings.mx
```

```text
MODULE_KW@0..6 "module"
WHITESPACE@6..7 " "
UPPER_IDENT@7..14 "Strings"
WHITESPACE@14..16 "\n\n"
LET_KW@16..19 "let"
…
```

Try the broken ones too, `corpus/err/unterminated-string.mx` and
`corpus/err/html.mx`: the errors come after the tokens.

---

**Next:** [Part 6 — Lossless syntax trees](part6-trees.md)
