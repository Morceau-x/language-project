# Maxim: the syntax

This document specifies the syntax of Maxim: which texts are Maxim
programs, and how they are structured. It doesn't say what programs
mean, and it leaves out HTML elements (`<div>…</div>`), which have their
own syntax.

`cargo check-grammar` checks the grammar in this document: it reads the
` ```ebnf ` blocks, and runs them on sample programs.

<!--
You write this document in Parts 2 and 3 of step 3
(tutorial/03-parsers/). Section 1 is done. In the others, each TODO
says what to write. The TODOs are HTML comments like this one: delete
each when its section is written.
-->

## 1. Notation

The grammar is written in the EBNF notation of the W3C (the one the XML
specification uses). A grammar is a list of rules, each defining a
symbol:

```text
symbol ::= expression
```

| Expression     | Matches                                                        |
|----------------|----------------------------------------------------------------|
| `"text"`, `'text'` | exactly that text; `'"'` is a double quote                 |
| `#xA`          | the character with that hexadecimal code point (here, `\n`)    |
| `[a-z_]`       | one character from the class: ranges, characters, `#xN` codes  |
| `[^"#xA]`      | one character *not* in the class                               |
| `A B`          | `A`, then `B`                                                  |
| `A \| B`       | `A` or `B`                                                     |
| `A - B`        | what matches `A` but doesn't match `B`                         |
| `A?`           | `A` or nothing                                                 |
| `A*`           | `A` zero or more times                                         |
| `A+`           | `A` one or more times                                          |
| `( … )`        | grouping                                                       |
| `/* … */`      | a comment                                                      |

From tightest to loosest: `?`, `*` and `+`; then `-`; then
juxtaposition; then `|`. So `A B - C` means `A (B - C)`: write
`(A B) - C` to subtract from a sequence.

Two grammars share this notation:

- The **lexical grammar** (section 2) cuts the text into tokens. Its
  symbols are in `UPPER_CASE`. Its terminals are characters.
- The **syntactic grammar** (section 3) arranges the tokens into a
  program. Its symbols are in `PascalCase`. Its terminals are tokens:
  a quoted text like `"fn"` or `"->"` is the keyword or the punctuation
  spelled that way, and an `UPPER_CASE` name like `LOWER` is any token
  of that kind.

## 2. Lexical structure

<!-- TODO (Part 2): how the lexer reads the text: longest match, and
which rule wins a tie. -->

### 2.1 Whitespace and comments

```ebnf
/* TODO (Part 2a): WHITESPACE and COMMENT. */
```

### 2.2 Names and keywords

```ebnf
/* TODO (Part 2b): KEYWORD, LOWER and UPPER. */
```

### 2.3 Numbers

```ebnf
/* TODO (Part 2c): INT and FLOAT. */
```

### 2.4 Strings

```ebnf
/* TODO (Part 2d): STRING, STRING_START, STRING_MIDDLE and STRING_END. */
```

### 2.5 Punctuation

```ebnf
/* TODO (Part 2e): PUNCTUATION. */
```

### 2.6 Rules that need context

<!-- TODO (Part 2f): the three rules the lexical grammar can't express:
the `}` that closes a hole, the `(` that starts a line (BOL_LPAREN), and
the `<` that starts HTML. -->

## 3. Syntax

### 3.1 Types

```ebnf
/* TODO (Part 3a): Type, and LParen: any `(`. */
```

### 3.2 Patterns

```ebnf
/* TODO (Part 3b): Pattern. */
```

### 3.3 Expressions

<!-- TODO (Part 3c): the table of operators, loosest first. -->

```ebnf
/* TODO (Part 3c): Expr, its operators and postfix operators. */
```

### 3.4 Atoms, blocks and statements

```ebnf
/* TODO (Part 3d): Atom and the rest, with the parameters of lambdas. */
```

### 3.5 Modules and declarations

```ebnf
/* TODO (Part 3e): Module and the declarations. */
```

### 3.6 Disambiguation

<!-- TODO (Part 3f): the rules that decide where the grammar is
ambiguous. -->
