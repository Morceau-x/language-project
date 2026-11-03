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

Define `Type` in section 3.1, and `LParen`: a `(`, whether or not it
starts a line. Most `(` in Maxim can start a line; only a call's
can't, and calls come in 3c.

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
FnType    ::= LParen ( ParamType ( "," ParamType )* )? ")" "->" Type
ParamType ::= ( LOWER ":" )? Type
LParen    ::= "(" | BOL_LPAREN
```

```markdown
A `LOWER` type is a type variable: `List<a>`. A function type's
parameters can have labels: `(acc: b, item: a) -> b`. `LParen` is any
`(`: everywhere except in calls, a `(` can start a line (see 2.6).

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
CtorPattern ::= ( UPPER "." )? UPPER ( LParen Pattern ( "," Pattern )* ")" )?
ListPattern ::= "[" ( Pattern ( "," Pattern )* ( "," ".." ( LOWER | "_" ) )? )? "]"
```

```markdown
There are no negative number patterns: a match arm can't start with
`-` (see 3.6).
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
follow each other: `user.name.length`, `f(x)?.y`. A call's `(` is
`"("`, never `BOL_LPAREN`: that's the layout rule. Call arguments are
expressions, labeled expressions (`age: 30`) or spreads (`..u`).

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

The table of Hint 1, then:

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
expressions. Statements have no separator, nor do match arms.

```bash
cargo check-grammar expressions
```

The checker tests how expressions group: `a + b * c` must group as
`⟨a + ⟨b * c⟩⟩`. If yours doesn't, it shows how it grouped it.

<details>
<summary>Solution</summary>

```ebnf
Atom          ::= LOWER | UPPER | INT | FLOAT | STRING | "_"
                | Interpolation | Group | List | Block | Lambda | If | Match
Interpolation ::= STRING_START Expr ( STRING_MIDDLE Expr )* STRING_END
Group         ::= LParen Expr ")"
List          ::= "[" ( Expr ( "," Expr )* ( "," ".." Expr )? )? "]"
Block         ::= "{" Stmt* "}"
Stmt          ::= "let" LOWER ( ":" Type )? "=" Expr | Expr
Lambda        ::= "fn" Params ReturnType? Block
Params        ::= LParen ( Param ( "," Param )* )? ")"
Param         ::= ( LOWER | "_" ) ( ":" Type )?
ReturnType    ::= "->" Type
If            ::= "if" Expr Block "else" ( Block | If )
Match         ::= "match" Expr "{" Arm+ "}"
Arm           ::= Pattern ( "if" Expr )? "->" Expr
```

```markdown
`_` is an expression only as a placeholder for a missing argument:
`add(1, _)` is a function that takes the missing one. A block's value is
its last statement's.

Statements and match arms have no separator: no `;`, and a line break
is just whitespace. One ends where the next one starts.
```

</details>

---

## 3e. Modules and declarations

Section 3.5: `Module`, a header and declarations, and the six
declarations of the tour: `import`, `type` (with variants or a record,
and optional type parameters `<a, b>`), `fn`, `cast`, `component` and
`let`. Variants' arguments are types in parentheses: `Rect(Float,
Float)`.

```bash
cargo check-grammar programs
```

This runs your whole grammar on the programs of `corpus/`: the valid
ones must be accepted, the invalid ones rejected (unless the lexer
rejects them first).

<details>
<summary>Solution</summary>

```ebnf
Module     ::= "module" UPPER Decl*
Decl       ::= Import | TypeDecl | Fn | Cast | Component | LetDecl
Import     ::= "import" UPPER
TypeDecl   ::= "type" UPPER TypeParams? "=" TypeBody
TypeParams ::= "<" LOWER ( "," LOWER )* ">"
TypeBody   ::= "|"? Variant ( "|" Variant )*
             | "{" Field ( "," Field )* "}"
Variant    ::= UPPER ( LParen Type ( "," Type )* ")" )?
Field      ::= LOWER ":" Type
Fn         ::= "fn" LOWER Params ReturnType? Block
Cast       ::= "cast" Params ReturnType? Block
Component  ::= "component" UPPER Params
LetDecl    ::= "let" LOWER ( ":" Type )? "=" Expr
```

With a line before the code block: "A file is a module: a header, then
declarations."

</details>

---

## 3f. Ambiguity, and what the grammar can't say

Your grammar is complete, and it's **ambiguous**. Take this block:

```text
{
  a
  - 1
}
```

`Stmt*` allows two readings: one statement `a - 1`, or two statements
`a` and `-1`. Same with `{ f (x) }`: a call, or `f` then a
parenthesized `x`? (Not with a `BOL_LPAREN`: that's what it's for.)
These are the layout rules of the tour. The checker's samples avoid
them, but a parser must decide.

Grammars of real languages are ambiguous in places; C's dangling
`else` is the classic. Specs settle them with **disambiguation rules**,
in prose. Write section 3.6: the two layout rules, and one more thing
the grammar allows but Maxim doesn't, `(_)`, which the checker can't
see either.

<details>
<summary>Solution</summary>

```markdown
The grammar above is ambiguous in a few places, where these rules
decide.

1. **An expression goes as far as it can.** A statement, a match arm's
   result or a `let`'s value ends only where no operator can continue
   it. In `{ a` newline `- 1 }`, the `- 1` continues `a`: one statement,
   `a - 1`, not two. That's why there are no negative number patterns:
   in `match n { 0 -> a -1 -> b }`, `-1` would continue `a`.
2. **A `(` on the same line is a call.** After an expression, a `"("`
   always starts a call, never a new statement. A `BOL_LPAREN` never
   starts a call (`PostfixOp` only accepts `"("`), so a line that starts
   with `(` always starts something new.
3. **`(_)` is not an expression.** A `Group` can't contain `_` alone.
```

Then a last section, to say what the grammar doesn't decide:

```markdown
## 4. Left out

Some programs that follow this grammar are still rejected, by the
compiler's later phases: a block must end with an expression, labeled
and positional arguments can't be mixed, the `++` operator was removed
in favor of interpolation, and so on.
```

Rule 1 is how an LR parser like the reference's behaves by default:
on a shift/reduce conflict (shift `-` into the expression, or reduce
the statement), it shifts. Your Pratt parser will too, without trying.

</details>

---

## Lookahead: what the spec tells the parser

```bash
cargo check-grammar ll1
```

```text
Lookahead: where one token isn't enough to choose (LL(1))
  ! `AddExpr`: at `( ( "+" | "-" ) MulExpr )*`, `"-"` can start the repeated or optional part, and also what comes after it
  ! `Postfix`: at `PostfixOp*`, `"("` can start the repeated or optional part, and also what comes after it
  ! `Arg`: `LOWER` can start more than one of its alternatives
  ! `ListPattern`: at `( "," Pattern )*`, `","` can start the repeated or optional part, and also what comes after it
  ! `CtorPattern`: at `( UPPER "." )?`, `UPPER` can start the repeated or optional part, and also what comes after it
  ! `ParamType`: at `( LOWER ":" )?`, `LOWER` can start the repeated or optional part, and also what comes after it
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
3. **Follow a disambiguation rule.** The `"-"` in `AddExpr`, and the
   `"("` in `Postfix`: after an expression, they could continue it or
   start the next statement. These are your two layout rules: continue
   the expression, and treat a `"("` on the same line as a call. A
   `BOL_LPAREN` is a different token, which is why it causes no `!`.

The reference compiler's grammar has a comment saying exactly that
(`example/src/maxim.y`, near the top): its conflicts are on `-` and
`(`, and it resolves them with precedence declarations.

### Try it

1. Write `CtorPattern` left-factored, as in point 2. Its `!`
   disappears.
2. Change `PostfixOp`'s `"("` to `LParen`. The checker now accepts
   `f` newline `(x)`, and finds an ambiguity. Put it back.
3. Make `AddExpr` left-recursive, `AddExpr ::= AddExpr ( "+" | "-" )
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
