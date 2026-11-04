# Part 1 — Maxim, and how languages are specified

[← Step 3 overview](guide.md)

No code in this part. First a tour of Maxim, the language you'll
specify and parse. Then how languages are specified, and the tool that
will check yours.

---

## A tour of Maxim

Maxim is a small functional language that compiles to JavaScript, in
the family of Elm, Gleam and OCaml. The reference compiler in
`example/` is a Rust translation of the original, written in Haskell.

This tour shows the syntax by example: enough to write the spec in
Parts 2 and 3. This step's Maxim is a dialect of the reference's, with
two changes: line breaks end statements (in the reference, statements
just follow each other), and block comments nest. For everything else,
the reference has the final word when in doubt: its grammar is
`example/src/maxim.y`, and its lexer is in `example/src/lexer/`. The
programs in `mini-parser/corpus/ok/` use every feature; read `todo.mx`
now, it's a small realistic program.

### A file is a module

```text
module Geometry

import List

type Shape =
  | Circle(Float)
  | Rect(Float, Float)

fn area(s: Shape) -> Float {
  match s {
    Circle(r) -> 3.14 * r * r
    Rect(w, h) -> w * h
  }
}
```

A file starts with `module` and the module's name, then declarations,
in any order:

| Declaration                              | Example                                              |
|------------------------------------------|------------------------------------------------------|
| an import                                | `import List`                                        |
| a type with variants                     | `type Option<a> = Some(a) \| None` (the first `\|` is optional) |
| a record type                            | `type Point = { x: Float, y: Float }`                |
| a function                               | `fn area(s: Shape) -> Float { … }` (the return type is optional) |
| a value                                  | `let pi: Float = 3.14` (the type is optional)        |
| a conversion between types               | `cast (d: Float) -> Celsius { … }`                   |
| a component (for HTML views)             | `component Card(title: String)`                      |

**Names** come in two kinds, told apart by their first letter:
lowercase for variables, functions, fields and type variables (`area`,
`a`), capitalized for types, constructors and modules (`Shape`,
`Circle`, `List`). `_` alone isn't a name: it's a wildcard.

**Types** are names, with arguments in angle brackets (`List<a>`,
`Dict<String, List<Int>>`), or function types: `(Int, String) -> Bool`,
whose parameters can have labels, `(acc: b, item: a) -> b`.

### Expressions

Everything in a function body is an expression, and a block's value is
its last statement's. There are no semicolons: a line break ends a
statement, so there's one statement per line.

```text
fn summary(tasks: List<Task>) -> String {
  let open = tasks |> List.filter(fn(t) { !t.done }) |> List.length
  let total = List.length(tasks)
  "${open} of ${total} tasks left (${total - open} done)"
}
```

| Expression                | Example                                                             |
|---------------------------|---------------------------------------------------------------------|
| numbers                   | `42`, `1_000_000`, `3.14`                                           |
| strings                   | `"Hello\n"`, with holes: `"${count} items"`                         |
| names                     | `x`, `Circle`, `List.map` (a field of a module)                     |
| lists                     | `[1, 2, 3]`, `[x, ..rest]` (`..` spreads a list at the end)         |
| calls                     | `f(x)`, with labels: `User(name: "Alice", age: 30)`, with a spread: `User(..u, age: 31)` |
| partial application       | `add(1, _, 3)`: `_` is the missing argument, the result a function  |
| fields                    | `user.name`                                                         |
| lambdas                   | `fn(x) { x + 1 }`, `fn(x: Int) -> Int { x + 1 }`                    |
| blocks                    | `{ a + 1 }`, or several statements, one per line                    |
| `if`                      | `if x > 0 { 1 } else if x == 0 { 0 } else { -1 }`: `else` is required |
| `match`                   | see below                                                           |
| `?`                       | `Int.parse(text)?`: unwrap a `Result`, or return its error          |
| casts                     | `21.5 :: Celsius`: convert with a `cast` declaration                |

Operators, from loosest to tightest. Every line binds tighter than the
ones above it:

| Operators                    | Meaning                     | Grouping                                   |
|------------------------------|-----------------------------|--------------------------------------------|
| `\|>`                        | pipeline: `x \|> f` is `f(x)` | left to right                            |
| `\|\|`                       | or                          | left to right                              |
| `&&`                         | and                         | left to right                              |
| `==` `!=`                    | equality                    | can't be chained: `a == b == c` is an error |
| `<` `>` `<=` `>=`            | comparison                  | can't be chained                           |
| `++`                         | concatenation (deprecated)  | right to left                              |
| `+` `-`                      | addition                    | left to right                              |
| `*` `/` `~/` `%`             | multiplication; `~/` divides integers | left to right                    |
| `-x` `!x`                    | negation                    |                                            |
| `**`                         | power                       | right to left                              |
| `.f` `(…)` `?` `:: T`        | field, call, unwrap, cast   | left to right                              |

One surprise: `**` binds tighter than a `-` before it, as in Python
and math: `-x ** 2` is `-(x ** 2)`. But `x ** -2` is fine.

### Pattern matching

```text
fn describe(xs: List<Int>) -> String {
  match xs {
    [] -> "empty"
    [x] if x > 100 -> "one big"
    [x, ..rest] -> "starts with ${x}"
  }
}
```

A `match` has one or more arms: a pattern, an optional guard `if …`,
`->`, and an expression. Each arm is on its own line, except after an
arm whose result is a block: `match x { A -> { 1 } B -> { 2 } }`.
Patterns are `_`, a name, a number or a string, a constructor with
patterns inside (`Circle(r)`, `Geometry.Circle(r)`, `None`), or a list
pattern (`[]`, `[x, y]`, `[x, ..rest]`, `[x, .._]`). There are no
negative number patterns.

### Line breaks

A line break ends a statement, but not every line break does, or you
couldn't split a long expression. Maxim uses the rule of Scala (Go,
Swift and Kotlin have close ones): a line break ends a statement when
the line before could end one, and the next line could start one.

```text
let total = price +
  tax                   ← continues: a line can't end with `+`
let names = users
  |> List.map(f)        ← continues: a statement can't start with `|>`
let city = user.address
  .city                 ← continues: nor with `.`
let gap = price
- discount              ← two statements: `-discount` is one on its own
log(total)              ← a call
(more) |> log           ← a new statement, not log(total)(more)
let sum = add(a,
  b)                    ← inside parentheses, line breaks don't count
```

So a long expression breaks *after* an operator, or *before* `|>` and
`.`. `-` and `(` can start a statement, so on a new line they start
one. And `{` too: as in Go, a block's `{` goes at the end of the line,
not on the next.

```text
fn area(s: Shape) -> Float
{                       ← an error: the declaration ended at `Float`
```

`else` can't start a statement: `}` then `else` on the next line is
fine.

### Comments

`// to the end of the line` and `/* anywhere, on several lines */`.
Block comments nest, as in Rust: `/* a /* b */ c */` is one comment, so
you can comment out code that has comments in it.

### What this step leaves out

**HTML elements**, written in the code like JSX: `<p>Hello, ${name}</p>`.
They need their own lexer mode and their own grammar; Part 13 sketches
how to add them. Your lexer reports an error where one would start.

---

## How languages are specified

### BNF and EBNF

In 1959, John Backus described the syntax of ALGOL with rules like
this one, and Peter Naur used his notation in the ALGOL 60 report. It
became **Backus–Naur form**, BNF:

```text
<expr> ::= <term> | <expr> + <term>
```

A **grammar** is a list of rules. Each rule defines a **nonterminal**
(here `<expr>`) as one or more sequences of nonterminals and
**terminals**, the symbols that appear in the text itself (`+`). A text
belongs to the language if the rules can produce it, starting from one
nonterminal, the **start symbol**.

BNF has only sequences and alternatives: repetition needs recursion,
optional parts need an empty alternative. **Extended** BNF adds
operators for both, as in regular expressions. Wirth's EBNF (1977),
used by the Go spec, writes `{ x }` for repetition and `[ x ]` for an
option; ISO standardized a verbose variant (`x, [ y ], { z } ;`) in
1996. The variant you'll use comes from the **W3C**, which specified
XML with it in 1998: it borrows regular expression syntax, `x? y* z+`,
so it reads naturally. The Rust Reference uses a close relative.

The notation table is at the top of `mini-parser/spec/maxim.md`. Read
it now. Two operators deserve a second look:

- `[a-z_]` and `[^"\]` are **character classes**, as in regular
  expressions: one character among the listed ones, or not among them.
- `A - B` is a **difference**: what `A` matches, except what `B`
  matches. `LOWER ::= ... - KEYWORD` says "a lowercase name, but not a
  keyword". It's the W3C notation's most useful addition, and it has no
  equivalent in pure BNF.

### Two grammars: lexical and syntactic

Specs split the grammar in two, and so will yours:

- The **lexical grammar** describes **tokens**: what a name, a number,
  a string or a comment is, character by character. Tokens are
  simple: regular expressions can describe them (no nesting).
- The **syntactic grammar** describes how tokens make a program. Its
  terminals are whole tokens, not characters, and whitespace is gone.
  This is where nesting lives, and the reason it needs a grammar.

Why split? Each half stays simple. Without it, the syntactic grammar
would have to allow whitespace and comments between every two
symbols. And it matches the implementation: a lexer, then a parser.

The **token interface** is the vocabulary the two halves share. In
Maxim's spec, a quoted text in the syntactic grammar (`"fn"`, `"->"`)
is the keyword or punctuation spelled that way, and these names are
the other kinds of tokens:

| Token           | Is                                                    | Example        |
|-----------------|-------------------------------------------------------|----------------|
| `LOWER`         | a lowercase name                                      | `area`, `_tmp` |
| `UPPER`         | a capitalized name                                    | `Shape`        |
| `INT`           | an integer                                            | `1_000`        |
| `FLOAT`         | a decimal number                                      | `3.14`         |
| `STRING`        | a string without holes                                | `"hi"`         |
| `STRING_START`  | a string up to its first hole                         | `"Hi ${`       |
| `STRING_MIDDLE` | from a hole's end to the next hole                    | `} and ${`     |
| `STRING_END`    | from the last hole's end to the closing quote         | `}!"`          |
| `NEWLINE`       | a line break that ends a statement                    | see "Line breaks" |

`NEWLINE` is a token the lexical grammar can't define: whether a line
break is one depends on the tokens around it. Part 2 describes it in
prose.

### What a spec says besides its grammar

A grammar alone doesn't make a spec. Look at a real one, the
[Go spec](https://go.dev/ref/spec), and you'll find, next to the EBNF:

- **A precedence table** for operators, and how they associate.
- **Disambiguation rules**: where the grammar allows two readings,
  which one is meant. C's "an `else` belongs to the nearest `if`" is
  the classic example.
- **Context rules** for the lexer: Go's rule for inserting semicolons
  at the ends of lines (Maxim's `NEWLINE` is the same idea),
  JavaScript's rule for when `/` starts a regular expression.
- **What's left to later phases**: a program can follow the grammar
  and still be rejected by the type checker.

Your spec will have all four.

### Checking a spec: `cargo check-grammar`

A spec written in prose can be wrong, and nobody notices until the
implementations disagree. Yours is checked by a tool, in
`mini-parser/check-grammar/`. It reads the ` ```ebnf ` blocks of
`spec/maxim.md`, then:

1. **Rules:** checks that the rules fit together: every rule used is
   defined, lexical rules don't use syntactic ones, the syntactic
   grammar only uses tokens…
2. **lexical:** runs each token rule on samples it must match (`1_000`
   is an `INT`) and must not match (`_1` isn't).
3. **types, patterns, expressions:** cuts sample texts into tokens and
   checks that your grammar accepts the valid ones and rejects the
   invalid ones. For operators, it also checks **how** your grammar
   groups them, written with angle brackets: `a + b * c` must group as
   `⟨a + ⟨b * c⟩⟩`, and if your grammar can group it two ways, that's
   an error too.
4. **programs:** runs your grammar on every program of `corpus/`.
5. **ll1:** shows where a recursive descent parser will need to look
   more than one token ahead (Part 3 explains).

```bash
cargo check-grammar lexical types
```

runs only those sections.

How can it run a grammar it has never seen? It uses an **Earley
parser**, an algorithm (Jay Earley, 1968) that accepts *any*
context-free grammar, ambiguous or left-recursive, at the cost of
speed. It's the right tool for checking a grammar, and the wrong one
for a compiler. The samples are cut into tokens by a small lexer of
the checker's own, which follows the finished spec, so you can check
your syntactic grammar before you write your own lexer.

Run it now, on the skeleton:

```bash
cargo check-grammar
```

15 problems: everything is still to write.

---

**Next:** [Part 2 — The lexical grammar](part2-lexical-spec.md)
