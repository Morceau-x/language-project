# Part 3 — The syntactic grammar

[← Step 3 overview](guide.md)

You write section 3 of `mini-parser/spec/maxim.md`: how tokens make a
Maxim program. Its terminals are tokens: quoted texts for keywords and
punctuation (`"fn"`, `"->"`), and the token names of Part 1 (`LOWER`,
`STRING_START`…). Its rules are `PascalCase`.

Write the sections in the order of the tasks, which isn't the order of
the document: each task's check needs the rules of the ones before.

---

## 3a. Types

Define `Type` in section 3.1.

From the tour: a type is a capitalized name with optional arguments in
angle brackets (`Dict<String, List<a>>`), a lowercase name (a type
variable), or a function type `(Int, b) -> c`, whose parameters can
have labels (`(acc: b, item: a) -> b`) and can be zero (`() -> Int`).
No trailing commas, anywhere in Maxim.

```bash
cargo check-grammar types
```

<details>
<summary>Hint: lists without trailing commas</summary>

"One or more `X` separated by commas" is `X ( "," X )*`. "Zero or
more" is that, optional: `( X ( "," X )* )?`.

</details>

<details>
<summary>Solution</summary>

```ebnf
Type      ::= UPPER TypeArgs? | LOWER | FnType
TypeArgs  ::= "<" Type ( "," Type )* ">"
FnType    ::= "(" ( ParamType ( "," ParamType )* )? ")" "->" Type
ParamType ::= ( LOWER ":" )? Type
```

```markdown
A `LOWER` type is a type variable: `List<a>`. A function type's
parameters can have labels: `(acc: b, item: a) -> b`.

Lists never end with a comma: `(a, b,) -> c` is an error, and so is
`f(a, b,)`.
```

`(Int) -> (Int) -> Int` is `(Int) -> ((Int) -> Int)`: the `Type` after
`->` can be another function type, so `->` groups to the right.

</details>

---

## 3b. Patterns

Define `Pattern` in section 3.2, from the tour: `_`, a name, a number,
a string (without holes), a constructor (`None`, `Some(x)`,
`Geo.Circle(r)`), a list pattern (`[]`, `[x, y]`, `[x, ..rest]`,
`[x, .._]`). A rest `..` needs at least one pattern before it, and
comes last.

```bash
cargo check-grammar patterns
```

<details>
<summary>Solution</summary>

```ebnf
Pattern     ::= "_" | LOWER | INT | FLOAT | STRING | CtorPattern | ListPattern
CtorPattern ::= ( UPPER "." )? UPPER ( "(" Pattern ( "," Pattern )* ")" )?
ListPattern ::= "[" ( Pattern ( "," Pattern )* ( "," ".." ( LOWER | "_" ) )? )? "]"
```

```markdown
There are no negative number patterns: a match arm can't start with
`-`.
```

</details>

---

## 3c. Expressions

Section 3.3: the operators. Write two things: the table of operators,
loosest first, with their associativity (from the tour); and the
rules.

### The idea: one rule per precedence level

In step 1, your parser had one function per precedence level: `expr`
for `+` and `-`, `term` for `*` and `/`. A grammar does the same with
one rule per level, whose operands are the next level:

```ebnf
AddExpr ::= MulExpr ( ( "+" | "-" ) MulExpr )*
MulExpr ::= Atom ( ( "*" | "/" ) Atom )*
```

`a + b * c` can only be read one way: `b * c` is a `MulExpr`, an
operand of the `+`. This is called a **stratified** grammar.

The shape of each level says how its operators **associate**:

| Shape                        | Means                                                        | Example                     |
|------------------------------|--------------------------------------------------------------|-----------------------------|
| `A ( op A )*`                | left-associative: `a - b - c` is `(a - b) - c`               | most operators              |
| `A ( op Self )?`             | right-associative: the right side recurses into the level    | `**`, `++`                  |
| `A ( op A )?`                | non-associative: one operator at most                        | `==`, `<`                   |

The repetition doesn't say "left" by itself: `a - b - c` is a flat list
of three operands. Specs say in prose that a repetition associates to
the left, and so will yours. The checker reads repetitions that way.

The other common style is a single ambiguous rule,
`Expr ::= Expr Op Expr | Atom`, plus the precedence table in prose. Go's
spec and the Rust Reference do that. It's shorter, but the grammar
alone no longer says what `a + b * c` means, and the checker reports
it as ambiguous. Use the stratified style.

### Two subtleties

**Prefix operators and `**`.** `-x ** 2` is `-(x ** 2)`, but `x ** -2`
is allowed: the operand of `-` is a power, and the right side of `**`
can start with a `-`. Write the two levels so that both work.

**Postfix operators** bind tightest: `.field`, a call `(…)`, `?`, and
a cast `:: Type` (only a capitalized name: `x :: Celsius`). They can
follow each other: `user.name.length`, `f(x)?.y`. Call arguments are
expressions, labeled expressions (`age: 30`) or spreads (`..u`).

Where do line breaks go? Nowhere: no rule of this section mentions
`NEWLINE`, so an expression can't contain one. In `f` newline `(x)`,
the line break is a `NEWLINE` (2.6), so it's not a call. In `a +`
newline `b`, it's whitespace, so the grammar never sees it.

You can't check this task alone: expressions contain blocks, lambdas,
matches…, the rules of 3d. Write both, then check.

<details>
<summary>Hint 1: the table</summary>

| Operators              | Associativity     |
|------------------------|-------------------|
| `\|>`                  | left              |
| `\|\|`                 | left              |
| `&&`                   | left              |
| `==` `!=`              | none: no chaining |
| `<` `>` `<=` `>=`      | none: no chaining |
| `++`                   | right             |
| `+` `-`                | left              |
| `*` `/` `~/` `%`       | left              |
| prefix `-` `!`         |                   |
| `**`                   | right             |
| postfix `.x` `(…)` `?` `::T` | left        |

(In Markdown, a `|` inside a table cell is written `\|`.)

</details>

<details>
<summary>Hint 2: prefix and power</summary>

```ebnf
Unary ::= ( "-" | "!" ) Unary | Power
Power ::= Postfix ( "**" Unary )?
```

`-x ** 2`: `Unary` takes the `-`, then its operand `Unary` is a
`Power`, `x ** 2`. `x ** -y`: the right side of `**` is a `Unary`.
And `a ** b ** c` groups to the right, because that `Unary` can be a
`Power` again.

</details>

<details>
<summary>Solution</summary>

The line "Operators, from loosest to tightest:", the table of Hint 1,
then:

```markdown
`**` binds tighter than a prefix `-` on its left: `-x ** 2` is
`-(x ** 2)`. Its right operand can still start with one: `x ** -2`.

The grammar has one rule per line of the table. A repetition `( op B )*`
is left-associative, a recursion `( op Self )?` right-associative, and
an option `( op B )?` allows one operator only.
```

```ebnf
Expr       ::= PipeExpr
PipeExpr   ::= OrExpr ( "|>" OrExpr )*
OrExpr     ::= AndExpr ( "||" AndExpr )*
AndExpr    ::= EqExpr ( "&&" EqExpr )*
EqExpr     ::= CmpExpr ( ( "==" | "!=" ) CmpExpr )?
CmpExpr    ::= ConcatExpr ( ( "<" | ">" | "<=" | ">=" ) ConcatExpr )?
ConcatExpr ::= AddExpr ( "++" ConcatExpr )?
AddExpr    ::= MulExpr ( ( "+" | "-" ) MulExpr )*
MulExpr    ::= Unary ( ( "*" | "/" | "~/" | "%" ) Unary )*
Unary      ::= ( "-" | "!" ) Unary | Power
Power      ::= Postfix ( "**" Unary )?
Postfix    ::= Atom PostfixOp*
PostfixOp  ::= "." ( LOWER | UPPER )
             | "(" ( Arg ( "," Arg )* )? ")"
             | "?"
             | "::" UPPER
Arg        ::= Expr | LOWER ":" Expr | ".." Expr
```

```markdown
A call takes positional arguments `f(x)`, labeled ones `f(x: 1)`, and
spreads `User(..u, age: 2)`. Which combinations make sense is for the
compiler to check, not the syntax.
```

`EqExpr` and `CmpExpr` are separate levels: `a == b < c` is fine, it's
`a == (b < c)`. Only two operators of the *same* level can't chain.

</details>

---

## 3d. Atoms, blocks and statements

Section 3.4: `Atom`, the operands of the postfix operators, and what's
inside them. From the tour:

- names (both kinds), numbers, strings, `_`;
- an interpolated string: a `STRING_START`, an expression, then
  `STRING_MIDDLE` and an expression any number of times, then a
  `STRING_END`;
- a parenthesized expression, a list (with an optional spread at the
  end), a block;
- a lambda, `fn`, parameters, an optional return type, a block.
  Parameters are names or `_`, with an optional type; write them as
  their own rules, `Params` and `ReturnType`: functions will use them
  too;
- `if … else …`, where `else` is required, and can be followed by
  another `if`;
- `match`, with at least one arm. An arm is a pattern, an optional
  guard (`if` and an expression), `->`, an expression.

A block holds statements: `let` bindings (with an optional type) and
expressions, separated by `NEWLINE`s. It can be empty, `{}`.

Match arms are separated by `NEWLINE`s too, with one exception: after
an arm whose result is a block, the next arm can follow on the same
line, `match x { A -> { 1 } B -> { 2 } }`. Rust has the same rule for
its commas: `A => { 1 } B => { 2 }` needs none.

```bash
cargo check-grammar expressions
```

The checker tests how expressions group: `a + b * c` must group as
`⟨a + ⟨b * c⟩⟩`. If yours doesn't, it shows how it grouped it.

<details>
<summary>Hint 1: separators</summary>

"Zero or more `X` separated by `S`" is `( X ( S X )* )?`, the comma
lists' shape.

</details>

<details>
<summary>Hint 2: the arms</summary>

Whether the next arm needs a `NEWLINE` depends on the arm before it, so
there are two kinds of arm: `Arm`, any arm, and `BlockArm`, an arm
whose result is a `Block`. Then write the list of arms recursively:
`Arms` is an arm, then either nothing or a `NEWLINE` and more arms; or
a `BlockArm`, then more arms directly.

</details>

<details>
<summary>Solution</summary>

```ebnf
Atom          ::= LOWER | UPPER | INT | FLOAT | STRING | "_"
                | Interpolation | Group | List | Block | Lambda | If | Match
Interpolation ::= STRING_START Expr ( STRING_MIDDLE Expr )* STRING_END
Group         ::= "(" Expr ")"
List          ::= "[" ( Expr ( "," Expr )* ( "," ".." Expr )? )? "]"
Block         ::= "{" ( Stmt ( NEWLINE Stmt )* )? "}"
Stmt          ::= "let" LOWER ( ":" Type )? "=" Expr | Expr
Lambda        ::= "fn" Params ReturnType? Block
Params        ::= "(" ( Param ( "," Param )* )? ")"
Param         ::= ( LOWER | "_" ) ( ":" Type )?
ReturnType    ::= "->" Type
If            ::= "if" Expr Block "else" ( Block | If )
Match         ::= "match" Expr "{" Arms "}"
Arms          ::= Arm ( NEWLINE Arms )? | BlockArm Arms
Arm           ::= Pattern ( "if" Expr )? "->" Expr
BlockArm      ::= Pattern ( "if" Expr )? "->" Block
```

```markdown
`_` is an expression only as a placeholder for a missing argument:
`add(1, _)` is a function that takes the missing one. A block's value is
its last statement's.

Statements are separated by line breaks, one per line: there's no `;`.
Match arms too, except that an arm whose result is a block can be
followed by another on the same line: `match x { A -> { 1 } B -> { 2 } }`.
```

`A -> { 1 }` newline `B -> 2` is an `Arm` then a `NEWLINE`: the
`BlockArm` path is only needed without the line break. And
`A -> { 1 } + 2 B -> 3` is an error: that result isn't a block, only
starts with one.

</details>

---

## 3e. Modules and declarations

Section 3.5: `Module`, a header and declarations, and the six
declarations of the tour: `import`, `type` (with variants or a record,
and optional type parameters `<a, b>`), `fn`, `cast`, `component` and
`let`. Variants' arguments are types in parentheses: `Rect(Float,
Float)`. Declarations are separated by `NEWLINE`s, one per line.

```bash
cargo check-grammar programs
```

This runs your whole grammar on the programs of `corpus/`: the valid
ones must be accepted, the invalid ones rejected (unless the lexer
rejects them first).

<details>
<summary>Solution</summary>

```ebnf
Module     ::= "module" UPPER ( NEWLINE Decl )*
Decl       ::= Import | TypeDecl | Fn | Cast | Component | LetDecl
Import     ::= "import" UPPER
TypeDecl   ::= "type" UPPER TypeParams? "=" TypeBody
TypeParams ::= "<" LOWER ( "," LOWER )* ">"
TypeBody   ::= "|"? Variant ( "|" Variant )*
             | "{" Field ( "," Field )* "}"
Variant    ::= UPPER ( "(" Type ( "," Type )* ")" )?
Field      ::= LOWER ":" Type
Fn         ::= "fn" LOWER Params ReturnType? Block
Cast       ::= "cast" Params ReturnType? Block
Component  ::= "component" UPPER Params
LetDecl    ::= "let" LOWER ( ":" Type )? "=" Expr
```

With a line before the code block:

```markdown
A file is a module: a header, then declarations, one per line.
```

and one after it:

```markdown
A `NEWLINE` can't come in the middle of a declaration: in `fn f() ->
Int` newline `{`, the line break before `{` is a `NEWLINE`, so the
function has no body. As in Go, a `{` goes at the end of the line.
```

A line break inside a declaration is fine wherever 2.6 makes it
whitespace: `type Shape =` newline `| Circle(Float)` (after `=`, and
before `|`), or a record's fields on several lines (after `{` and `,`,
before `}`).

</details>

---

## 3f. Ambiguity, and what the grammar can't say

Without `NEWLINE`, this grammar would be **ambiguous**. Take this
block:

```text
{
  a
  - 1
}
```

With `Block ::= "{" Stmt* "}"`, there are two readings: one statement
`a - 1`, or two statements, `a` and `-1`. Same with `{ f (x) }`: a
call, or `f` then a parenthesized `x`? The reference compiler's grammar
is like that, and its spec settles it with **disambiguation rules** in
prose: "an expression goes as far as it can", "a `(` on a new line
starts a new statement". Grammars of real languages are ambiguous in
places, and C's dangling `else` is the classic.

Yours isn't, there: the line break before `- 1` is a `NEWLINE`, and no
rule allows a `NEWLINE` inside an expression, so it's two statements;
`{ a - 1 }` on one line has no `NEWLINE`, so it's one. Rule 2 of 2.6
did the disambiguating, in the lexer, where one token of context on
each side is enough.

One thing remains that the grammar allows but Maxim doesn't: `(_)`.
`_` is an `Atom` (for `add(1, _)`), so `"(" Expr ")"` accepts `(_)`.
The checker can't see that either. Write section 3.6: what `NEWLINE`
settles, and that rule.

<details>
<summary>Solution</summary>

```markdown
Thanks to `NEWLINE`, the grammar says where every statement ends: `a`
newline `- 1` is two statements, `a - 1` on one line is one, and `f`
newline `(x)` is two. One rule remains that the grammar can't express:

1. **`(_)` is not an expression.** A `Group` can't contain `_` alone.
```

Then a last section, to say what the grammar doesn't decide:

```markdown
## 4. Left out

Some programs that follow this grammar are still rejected, by the
compiler's later phases: a block must end with an expression, labeled
and positional arguments can't be mixed, the `++` operator was removed
in favor of interpolation, and so on.
```

The reference's LR parser settles `a` newline `- 1` the other way: on
a shift/reduce conflict (shift `-` into the expression, or reduce the
statement), it shifts, so the expression goes on. Your Pratt parser
will stop at the `NEWLINE` instead (Part 9).

</details>

---

## Lookahead: what the spec tells the parser

```bash
cargo check-grammar ll1
```

```text
Lookahead: where one token isn't enough to choose (LL(1))
  ! `Arg`: `LOWER` can start more than one of its alternatives
  ! `Arms`: `UPPER`, `LOWER`, `INT`, `FLOAT`, `STRING`, `"_"`, `"["` can start more than one of its alternatives
  ! `ParamType`: at `( LOWER ":" )?`, `LOWER` can start the repeated or optional part, and also what comes after it
  ! `ListPattern`: at `( "," Pattern )*`, `","` can start the repeated or optional part, and also what comes after it
  ! `CtorPattern`: at `( UPPER "." )?`, `UPPER` can start the repeated or optional part, and also what comes after it
  ! `List`: at `( "," Expr )*`, `","` can start the repeated or optional part, and also what comes after it
```

(The order may differ, and with your grammar, the list too.)

### The idea: FIRST and FOLLOW

A recursive descent parser makes a choice at every `|`, `?`, `*`: which
alternative, again or stop? It wants to decide from the **current
token** alone. For that, the options must start with different tokens.

- **FIRST(X)** is the set of tokens `X` can start with.
  FIRST(`Pattern`) is `"_"`, `LOWER`, `INT`, `FLOAT`, `STRING`, `UPPER`
  and `"["`.
- **FOLLOW(X)** is the set of tokens that can come right after an `X`.
  It matters for options that can be empty: to decide to *skip*
  `( "," Pattern )*`, the parser must see a token that can follow it.

A grammar where every choice can be made by looking at one token, using
these sets, is **LL(1)**: Left to right, Leftmost derivation, 1 token
of lookahead (step 2's guide has the full story). Each `!` above is a
choice where one token isn't enough. Your parser will handle each one,
in one of three ways:

1. **Look one more token ahead.** In `Arg`, a `LOWER` can start
   `LOWER ":" Expr` or an `Expr` like `x + 1`: the second token decides.
   Same in `ParamType`: `a` alone is a type variable, `a:` a label. The
   parser's `nth(1)` (Part 7) does this.
2. **Rewrite the rule.** `( UPPER "." )? UPPER` can't decide at the
   first `UPPER`, but `UPPER ( "." UPPER )?`, the same language, can:
   that's **left factoring**. The parser writes `CtorPattern` that way.
   The lists too: after a `","`, decide between `".."` and a pattern.
3. **Parse first, decide after.** In `Arms`, an `Arm` and a
   `BlockArm` start the same way, and differ only at the end: whether
   the result is a block. No amount of lookahead helps, a block can be
   any length. The parser parses an arm, then looks at what its result
   was: if a block, the next arm can follow without a `NEWLINE`. It's
   left factoring again, with the parser remembering what it parsed.

What's missing is as telling. Without `NEWLINE`, a `"-"` in `AddExpr`
and a `"("` in `Postfix` would get a `!` each: after an expression,
they could continue it or start the next statement. The reference
compiler's grammar has exactly these two conflicts, and a comment near
the top of `example/src/maxim.y` says how it resolves them: with a
precedence declaration for `-`, and with a lexer that tells a `(` at
the start of a line apart from the others. Your `NEWLINE` settles both
before the parser sees them.

### Try it

1. Write `CtorPattern` left-factored, as in point 2. Its `!`
   disappears.
2. Make the statement separators optional, `Block ::= "{" ( Stmt
   NEWLINE? )* "}"`. The checker now accepts `{ a b }`, and the two
   `!` of the reference grammar are back, on `"-"` and `"("`. Put it
   back.
3. Remove the `BlockArm` alternative from `Arms`. The checker rejects
   `match x { A -> { 1 } B -> { 2 } }`, and two programs of the corpus.
   Put it back.
4. Make `AddExpr` left-recursive, `AddExpr ::= AddExpr ( "+" | "-" )
   MulExpr | MulExpr`: the checker still accepts it (Earley parsing
   doesn't mind), and the LL(1) section warns that recursive descent
   would loop. Put it back.

---

## Check

```bash
cargo check-grammar
```

```text
All checks pass.
```

Your spec is done. From here on, the parser follows it rule by rule:
each grammar function of Parts 8 to 10 quotes the rule it implements.

---

**Next:** [Part 4 — A raw lexer, like rustc's](part4-raw-lexer.md)
