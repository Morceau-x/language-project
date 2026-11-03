# Step 3 — Parsers, in depth

Steps 1 and 2 parsed a language with four operators. This step parses a
real one: **Maxim**, the language of the reference compiler in
`example/`, all of it except its HTML elements. Modules, types,
functions, lambdas, pattern matching, string interpolation, a dozen
precedence levels, no semicolons.

You do it twice, the way language designers do:

1. **On paper.** You write Maxim's grammar as a specification, in
   standard EBNF, with the prose rules a grammar can't express. A tool
   checks it against hundreds of samples.
2. **In Rust.** You write a lexer and a parser built like Rust's own:
   a lexer in two layers like `rustc_lexer`, and a parser like
   rust-analyzer's, which builds a **lossless syntax tree** (every
   space and comment is in it), never stops at the first error, and
   gets a typed AST layer on top.

```text
let area = fn(s: Shape) { match s { Circle(r) -> 3.14 * r ** 2 } }
```

```text
LET_DECL@0..66                                 your syntax tree, with every byte
  LET_KW@0..3 "let"
  WHITESPACE@3..4 " "
  NAME@4..8
    LOWER_IDENT@4..8 "area"
  …
```

```text
$ cargo parse mini-parser/corpus/err/tuple.mx
error: Maxim has no tuples: use a record type, or separate arguments
 --> mini-parser/corpus/err/tuple.mx:3:12
  |
3 | let pair = (1, 2)
  |            ^^^^^^
```

**Before you start:** finish [Step 2](../02-compiler-layers/guide.md),
and skim [the Rust you need](rust-concepts.md) for this step. This is
the longest step so far: expect 15 to 25 hours. Each part below is a
session or two.

**Contents**

- [The big picture](#the-big-picture)
- [How to work through this step](#how-to-work-through-this-step)
- [Part 0 — Look around](#part-0--look-around)
- [Part 1 — Maxim, and how languages are specified](part1-specifying.md): a tour of the language, EBNF, the checker
- [Part 2 — The lexical grammar](part2-lexical-spec.md): tokens, on paper
- [Part 3 — The syntactic grammar](part3-syntax-spec.md): precedence, ambiguity, lookahead, on paper
- [Part 4 — A raw lexer, like rustc's](part4-raw-lexer.md): a cursor, and tokens that are just a kind and a length
- [Part 5 — Cooking tokens](part5-cooking.md): keywords, errors, and the rules that need context
- [Part 6 — Lossless syntax trees](part6-trees.md): green and red trees, from events
- [Part 7 — The parser's engine](part7-engine.md): lookahead, markers, events, recovery
- [Part 8 — Types and patterns](part8-types-patterns.md): recursive descent, from the spec
- [Part 9 — Expressions](part9-expressions.md): Pratt parsing, postfix operators, blocks
- [Part 10 — Declarations and whole files](part10-declarations.md): the parser is complete
- [Part 11 — A typed AST](part11-ast.md): rust-analyzer's `ast` layer, and a first client
- [Part 12 — Errors and differential testing](part12-errors.md): recovery, validation, testing against the reference
- [Part 13 — Explore, and clean up](part13-explore.md): HTML, incremental parsing, formatters…

---

## The big picture

### Why a second parser for Maxim?

`example/` already has one: a grammar for grmtools, the LR parser
generator of step 2 (`example/src/maxim.y`). It stops at the first
error, and gives a tree that only the compiler can use. Real languages
need more:

- **An editor parses code that is being typed**, so nearly always
  broken. The parser must give a tree anyway, with errors in it, and
  keep every comment and space, so that refactorings and formatters can
  rewrite code without destroying it.
- **Error messages are the parser's user interface.** "expected `)`"
  is better than "expected one of 23 tokens".

That's why rustc, rust-analyzer, Go, Swift, TypeScript and C#'s Roslyn
all hand-write their parsers. Yours follows rust-analyzer, the most
documented of them: its design notes are worth reading alongside this
step ([syntax.md](https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/syntax.md),
and matklad's [Simple but Powerful Pratt Parsing](https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html)
and [Resilient LL Parsing](https://matklad.github.io/2023/05/21/resilient-ll-parsing-tutorial.html)).

### Why a specification first?

A hand-written parser has no grammar file: the language is whatever
the code does. Nothing warns you when it accepts something it
shouldn't, or when two parts disagree. A specification fixes that:

- it says exactly what the language is, for people;
- a tool can check it, and find ambiguities;
- it tells you where a recursive descent parser needs more than one
  token of lookahead, before you write one line of the parser;
- and differential testing (Part 12) checks that your parser, the
  reference parser and the spec all agree.

### The layers

```text
                     Parts 2, 3: the spec (spec/maxim.md), checked by `cargo check-grammar`
                                                  ┆ guides
                                                  ▼
 text ──raw lexer──▶ raw tokens ──cook──▶ tokens ──parser──▶ events ──builder──▶ syntax tree ──▶ typed AST
       (Part 4)     kind + length  (5)   with trivia  (7–10)  start, token,  (6)  lossless       (11)
                                                                finish, error      green + red
                                                                                       │
                                                         validation (12) ──▶ errors ◀──┘
```

| Part | You build                                   | Compiler ideas                                                                                   | Files                                |
|------|---------------------------------------------|--------------------------------------------------------------------------------------------------|--------------------------------------|
| 1    | nothing yet: you read                       | specifications, BNF and EBNF, lexical vs syntactic grammars, the token interface                  |                                      |
| 2    | the lexical grammar                         | regular languages, longest match, the `-` operator, context-sensitive lexing                     | `spec/maxim.md`                      |
| 3    | the syntactic grammar                       | precedence levels in a grammar, associativity, ambiguity, FIRST and FOLLOW, LL(1)                | `spec/maxim.md`                      |
| 4    | a raw lexer                                 | rustc_lexer's design: a cursor, maximal munch, errors as flags, never failing                     | `src/lexer/raw.rs`                   |
| 5    | the cooked lexer                            | keywords, error messages, lexer modes, one token of context                                      | `src/lexer/mod.rs`                   |
| 6    | syntax trees                                | green and red trees (rowan), trivia, building a tree from events                                 | `src/tree/`                          |
| 7    | the parser's engine                         | events, markers, forward parents, token sets, error recovery, guaranteed progress                | `src/parser/mod.rs`                  |
| 8    | types and patterns                          | recursive descent from a grammar, left factoring, two tokens of lookahead                        | `src/parser/grammar/`                |
| 9    | expressions                                 | Pratt parsing, binding powers, prefix and postfix operators, statements without separators        | `src/parser/grammar/`                |
| 10   | declarations                                | the whole language; snapshot tests                                                               | `src/parser/grammar/items.rs`        |
| 11   | a typed AST, and an outline                 | typed views over an untyped tree, macros that write code                                         | `src/ast/`, `src/outline.rs`         |
| 12   | validation and better errors                | resilient parsing, cover grammars, differential and property testing                             | `src/validation.rs`, `tests/`        |
| 13   | whatever you choose                         | HTML and lexer modes, incremental reparsing, formatters…                                         |                                      |

### Best practices this step adds

| Practice                                      | Why                                                                                         | Where                     |
|-----------------------------------------------|---------------------------------------------------------------------------------------------|---------------------------|
| **Specify, then implement**                   | the spec is checked; the parser is checked against it and against the reference              | `spec/maxim.md`           |
| **Lossless trees**                            | tools that rewrite code need every byte; the parser's tests can check that nothing is lost   | `src/tree/`               |
| **The parser never fails**                    | any text gives a tree and a list of errors: what an editor needs                            | `src/parser/`             |
| **Parse loosely, validate afterwards**        | with the whole node in hand, errors can say *what* is wrong, not just *where*                | `src/validation.rs`       |
| **Snapshot tests**                            | a tree dump shows everything at once; a failure shows the exact difference                  | `corpus/`, `expect![…]`   |
| **Fail loudly on parser bugs**                | a forgotten marker panics where it's forgotten; a parser that stops moving panics too        | `src/parser/mod.rs`       |
| **Test against another implementation**       | the reference parser decides what's valid; thousands of random edits find the disagreements | `tests/differential.rs`   |

---

## How to work through this step

Parts 2 and 3 are writing, not code: you fill in `mini-parser/spec/maxim.md`,
and `cargo check-grammar` checks it. From Part 4 on, the rhythm is
the usual one: **read the test, write the code, run the test.** Each
task is a `todo!()` with its tests, folded hints and a folded solution.

Do the parts in order: each one uses the ones before. The tests of a
task only need the tasks before it.

The parser's tests are **snapshot tests**, the way rust-analyzer writes
them: the expected output is written in the test, inside `expect![[…]]`
(a crate by rust-analyzer's authors, `expect-test`). When the output
differs, the test fails and shows the difference, line by line.

**Never run the tests with `UPDATE_EXPECT=1`.** That environment
variable makes expect-test overwrite each expected output with your
actual output: the tests would pass, and stop testing anything. It's
for writing new tests.

Run one module's tests with a filter:

```bash
cargo test -p mini-parser --lib lexer::raw
```

`--lib` runs the unit tests in `src/`. The tests in `mini-parser/tests/`
need the whole parser; the guide says when to run them. To see
everything at once:

```bash
cargo test -p mini-parser --no-fail-fast
```

---

## Part 0 — Look around

### The first build

```bash
cargo test -p mini-parser --no-fail-fast
```

The first build takes a while: it builds the reference compiler in
`example/`, which the tests compare your parser with. You should see
**5 passed** and 114 failed. The failures say
`not yet implemented: Part 4a: line_comment` and the like.

```bash
cargo check-grammar
```

The checker reads `mini-parser/spec/maxim.md`, which is a skeleton for
now: it reports **15 problems**, one per thing to write.

### The commands

Two shortcuts, defined in `.cargo/config.toml` at the workspace root:

| Command                                  | Runs                                                              | Works after |
|------------------------------------------|-------------------------------------------------------------------|-------------|
| `cargo check-grammar [SECTION…]`         | the spec checker, on `mini-parser/spec/maxim.md`                  | now         |
| `cargo parse --emit=tokens FILE.mx`      | your lexer: every token, whitespace and comments included         | Part 5      |
| `cargo parse --emit=tree FILE.mx`        | your parser: the syntax tree, then the errors                     | Part 10     |
| `cargo parse FILE.mx`                    | your parser: just the errors, with the lines they're on           | Part 10     |
| `cargo parse --emit=outline FILE.mx`     | one line per declaration, from the typed AST                      | Part 11     |

Run them from the workspace root.

### The files

Everything is in `mini-parser/`, a new crate of the workspace.

| File                                   | What                                                                    | Yours?                        |
|----------------------------------------|-------------------------------------------------------------------------|-------------------------------|
| `spec/maxim.md`                        | the specification                                                       | **Parts 2 and 3**             |
| `check-grammar/`                       | the spec checker: an EBNF reader, an Earley parser, an LL(1) analysis   | provided; read it if curious  |
| `corpus/ok/*.mx`, `corpus/err/*.mx`    | valid and invalid Maxim programs, with their expected trees and errors  | provided                      |
| `src/syntax_kind.rs`                   | `SyntaxKind`: every kind of token and node; the `T![…]` macro           | provided, read it             |
| `src/text_range.rs`                    | `TextRange`: a range of bytes in the text                               | provided                      |
| `src/lexer/cursor.rs`                  | the cursor the lexer reads the text with                                | provided, read it             |
| `src/lexer/raw.rs`                     | the raw lexer                                                           | **Part 4**                    |
| `src/lexer/mod.rs`                     | the cooked lexer: `lex(text) -> Lexed`                                  | **Part 5**                    |
| `src/tree/`                            | green and red trees, and the builder                                    | **Part 6**                    |
| `src/parser/mod.rs`, `token_set.rs`    | the parser's engine                                                     | **Part 7**                    |
| `src/parser/grammar/`                  | the grammar, one function per rule                                      | **Parts 8 to 10**             |
| `src/ast/`                             | the typed AST                                                           | **Part 11**                   |
| `src/outline.rs`                       | a client of the typed AST                                               | **Part 11**                   |
| `src/validation.rs`                    | checks on the finished tree                                             | **Part 12**                   |
| `src/lib.rs`                           | `parse(text) -> Parse`: the layers, chained                             | provided, read it             |
| `src/main.rs`                          | the `cargo parse` command                                               | provided                      |
| `tests/corpus.rs`                      | snapshot tests on the corpus                                            | provided                      |
| `tests/differential.rs`                | your parser against the reference parser                                | provided                      |
| `tests/lossless.rs`                    | properties that hold for any text                                       | provided                      |

**Next:** [Part 1 — Maxim, and how languages are specified](part1-specifying.md)
