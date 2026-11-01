# Part 2 — LR parsing with Bison

[← Step 2 overview](guide.md)

Your step-1 parser is hand-written. The other way to get a parser is to
**generate** it: write the grammar, and a tool writes the parser. The
classic tool is **yacc** ("yet another compiler compiler", 1975), and
its GNU version **Bison**. They still run inside PostgreSQL, PHP, Bash
and many others.

This part has no Rust in it. You write Mini's grammar for Bison, which
generates a calculator in C. Then you watch the generated parser work,
read the automaton Bison built, and break the grammar on purpose to
meet **conflicts**, the central idea of LR parsing. Part 3 then does
the same thing in Rust.

Work in `mini/bison/`, in a terminal:

```bash
cd mini/bison
```

Open `calc.y`. It already has everything except the grammar: a small
hand-written lexer in C, `main`, and a one-rule grammar that accepts a
single integer. Build it and try it:

```bash
bison calc.y && clang calc.tab.c -o calc && echo 42 | ./calc
```

```text
42
```

`bison calc.y` writes `calc.tab.c`, about 1,500 lines of C. `clang`
compiles it. (Both outputs are in `.gitignore`.)

---

## The idea: LR parsing

Your recursive descent parser works **top-down**: `expr()` decides
first that it's parsing an expression, then calls `term()` to find its
parts. It reads the grammar from the top, predicting what comes next.

An **LR parser** works **bottom-up**. It keeps a stack, and at each
step does one of two things:

- **shift**: push the next token onto the stack;
- **reduce**: the top of the stack matches the right-hand side of a
  rule, like `expr '+' term`: pop those symbols, and push the rule's
  left-hand side, `expr`, instead.

It succeeds when the whole input is reduced to the start symbol. The
hard part is deciding, at each step, whether to shift or reduce, and by
which rule. That's what Bison computes from the grammar: a table, the
**LALR(1) automaton**, that tells the parser what to do given the state
it's in and **1** token of lookahead.

---

## 2a. The grammar, as Bison sees it

Replace the `program` rule in `calc.y` with Mini's grammar from step 1,
in Bison's notation. Leave out names: the calculator only does integers.

```text
rule:
    alternative           { action }
  | alternative           { action }
  ;
```

Three differences from the EBNF in step 1:

1. **No `{ ... }` repetition.** BNF only has alternatives and recursion.
   `expr = term { "+" term }` becomes "an `expr` is an `expr` plus a
   `term`, or a `term`":

   ```text
   expr:
       expr '+' term
     | term
     ;
   ```

   That rule is **left-recursive**: `expr` starts with `expr`. In your
   recursive descent parser, `fn expr()` would call `expr()` first,
   forever: that's why step 1 used a loop. LR parsers have no such
   problem. They *prefer* left recursion, which gives left
   associativity for free.

2. **Tokens.** Single characters are written in quotes: `'+'`, `'('`.
   `INT` is declared with `%token INT`, and its value comes from the
   lexer.

3. **Actions.** Each alternative can end with C code in braces. `$1`,
   `$2`, `$3` are the values of the alternative's parts, and you set
   `$$`, the value of the rule. `expr '+' term { $$ = $1 + $3; }`
   computes the sum *while parsing*. That's syntax-directed
   translation. An alternative without an action gets `$$ = $1`.

Write five rules: `program` (print the value of an `expr`, as the
current rule does for `INT`), `expr`, `term`, `unary` and `primary`.
For `/`, report a division by zero like this:

```c
if ($3 == 0) {
    yyerror("division by zero");
    YYABORT;
}
```

```bash
bison -Wall calc.y && clang calc.tab.c -o calc
```

```bash
for e in '1 + 2 * 3' '(1 + 2) * 3 - 10 / 4' '-(2 * 3) + - -1' '1 - 2 - 3' '1 / 0' '(1 + 2'; do echo "$e" | ./calc; done
```

```text
7
7
-5
-4
error: division by zero
error: syntax error, unexpected end of file
```

<details>
<summary>Solution</summary>

```text
program:
    expr                    { printf("%lld\n", $1); }
  ;

expr:
    expr '+' term           { $$ = $1 + $3; }
  | expr '-' term           { $$ = $1 - $3; }
  | term
  ;

term:
    term '*' unary          { $$ = $1 * $3; }
  | term '/' unary          {
                              if ($3 == 0) {
                                  yyerror("division by zero");
                                  YYABORT;
                              }
                              $$ = $1 / $3;
                            }
  | unary
  ;

unary:
    '-' unary               { $$ = -$2; }
  | primary
  ;

primary:
    INT
  | '(' expr ')'            { $$ = $2; }
  ;
```

(C's `long long` arithmetic has undefined behaviour on overflow, unlike
Mini. Fine for a toy; Part 7 shows why it matters in a real backend.)

</details>

---

## 2b. Watch it parse

`calc.y` turns on Bison's tracing code (`%define parse.trace`), and
`main` enables it when the `YYDEBUG` variable is set:

```bash
echo 42 | YYDEBUG=1 ./calc
```

```text
Next token is token INT (42)
Shifting token INT (42)
Entering state 1
Reducing stack by rule 10 (line 71):
   $1 = token INT (42)
-> $$ = nterm primary (42)
Entering state 8
Reducing stack by rule 9 (line 67):
   $1 = nterm primary (42)
-> $$ = nterm unary (42)
Entering state 7
Reducing stack by rule 7 (line 62):
   $1 = nterm unary (42)
-> $$ = nterm term (42)
...
-> $$ = nterm expr (42)
```

(Your state and rule numbers may differ, depending on the order of your
rules.) One shift, then four reductions: `INT` becomes a `primary`,
which becomes a `unary`, a `term`, an `expr`. Those reductions *are*
the parse tree, built from the leaves up:

```text
expr
└── term
    └── unary
        └── primary
            └── INT 42
```

Four nodes to say "the number 42": remember that for Part 4, where
trees like this get *pruned*.

Now a real expression:

```bash
echo '1 + 2 * 3' | YYDEBUG=1 ./calc 2>&1 | grep -E 'Shifting|Reducing|->|Next token'
```

Follow the stack. The interesting moment is after `1 + 2`: the stack
holds `expr '+' term`, which matches the rule `expr: expr '+' term`.
The parser *could* reduce it to `expr`. Instead, it reads the next
token, sees `*`, and **shifts** it. Reducing would compute `1 + 2`
first, which is wrong.

**Question:** how does the parser know to shift here, but reduce in
`1 + 2 - 3` after `1 + 2`?

<details>
<summary>Answer</summary>

The lookahead token. In the state the parser is in after
`expr '+' term`, the automaton says: on `*` or `/`, shift; on anything
else, reduce. The grammar implies it: `term: term '*' unary` can only
continue with `*` if the `term` stays a `term` for now. The next
section shows that table.

</details>

---

## 2c. The automaton

```bash
bison --report=all calc.y
```

This writes `calc.output`, the automaton in readable form. Open it.
First the grammar, with numbered rules. Then each **state**: a set of
**items**, rules with a dot `•` marking how much of them has been seen,
and the action for each lookahead token.

Find the state containing `expr: term •`. It looks like this:

```text
State 6

    4 expr: term •  [$end, '+', '-', ')']
    5 term: term • '*' unary
    6     | term • '/' unary

    '*'  shift, and go to state 14
    '/'  shift, and go to state 15

    $default  reduce using rule 4 (expr)
```

The parser has just seen a `term`. Two possibilities:

- it's a whole `expr` (rule 4, dot at the end): reduce, but only if the
  next token can follow an `expr`, listed in brackets: end of input, `+`,
  `-` or `)`;
- it's the start of a longer `term` (rules 5, 6, dot before `*` or `/`):
  shift the operator.

One token of lookahead is enough to choose. That's what the **1** in
LALR(1) means, and why `*` binds tighter than `+`: it's not written
anywhere as such, it falls out of the grammar's layers.

**Exercise:** how many states does your automaton have? (Search for the
last `State`.) Remember the number for 2d.

<details>
<summary>Answer</summary>

21 states, with the rules in the order of the solution above.

</details>

---

## 2d. Conflicts, and precedence declarations

The stratified grammar works, but it's long, and every new precedence
level adds a rule. Yacc offers another way. Replace `expr`, `term`,
`unary` and `primary` with one rule that says what an expression can
be, and nothing about precedence:

```text
expr:
    expr '+' expr           { $$ = $1 + $3; }
  | expr '-' expr           { $$ = $1 - $3; }
  | expr '*' expr           { $$ = $1 * $3; }
  | expr '/' expr           { ...division... }
  | '-' expr                { $$ = -$2; }
  | '(' expr ')'            { $$ = $2; }
  | INT
  ;
```

```bash
bison -Wall calc.y
```

```text
calc.y: warning: 20 shift/reduce conflicts [-Wconflicts-sr]
```

The grammar is **ambiguous**: `1 + 2 * 3` has two parse trees, one per
grouping. Bison can show you, with counterexamples:

```bash
bison -Wcounterexamples calc.y 2>&1 | less
```

```text
calc.y: warning: shift/reduce conflict on token '+' [-Wcounterexamples]
  Example: expr '+' expr • '+' expr
  Shift derivation
    expr
    ↳ 2: expr '+' expr
                  ↳ 2: expr • '+' expr
  Reduce derivation
    expr
    ↳ 2: expr                 '+' expr
         ↳ 2: expr '+' expr •
```

After `expr '+' expr`, with `+` coming next, the parser can't decide:
reduce now, giving `(1 + 2) + 3`, or shift, giving `1 + (2 + 3)`. That
is a **shift/reduce conflict**. (There are also reduce/reduce
conflicts, where two rules could be reduced: those are usually real
mistakes in a grammar.)

Bison only *warns*, and resolves every shift/reduce conflict by
shifting. See what that does:

```bash
clang calc.tab.c -o calc
for e in '1 - 2 - 3' '2 * 3 + 4' '-1 + 2'; do echo "$e" | ./calc; done
```

```text
2
14
-3
```

All three are wrong: always shifting makes every operator
right-associative with the same precedence, so `2 * 3 + 4` is
`2 * (3 + 4)`. A conflict is not a warning to ignore: **it's a decision
the grammar didn't make**.

Make it with **precedence declarations**, in the declarations section
(before the first `%%`), from the lowest precedence to the highest:

```text
%left '+' '-'
%left '*' '/'
%precedence NEG
```

and mark the negation rule:

```text
  | '-' expr %prec NEG      { $$ = -$2; }
```

Each line is one level; `%left` makes the operators of that level
left-associative. Bison resolves each conflict by comparing the
precedence of the **rule** that could be reduced (that of its last
token, or the one given by `%prec`) with the precedence of the
**lookahead token**:

- the token's is higher: shift (in `1 + 2 • * 3`, `*` beats `+`);
- the rule's is higher: reduce (in `-1 • + 2`, `NEG` beats `+`);
- the same level: associativity decides; `%left` means reduce (in
  `1 - 2 • - 3`).

`NEG` is not a token the lexer produces. It only names a precedence
level, because `-` already has one, as a binary operator, and negation
needs a higher one. `%precedence` declares a level without
associativity: negation is a prefix operator, so associativity never
comes up.

```bash
bison -Wall calc.y && clang calc.tab.c -o calc
for e in '1 - 2 - 3' '2 * 3 + 4' '-1 + 2' '1 + 2 * 3' '- -1'; do echo "$e" | ./calc; done
```

```text
-4
10
1
7
1
```

No conflicts, correct answers. Look at the report again
(`bison --report=all calc.y`): the state after `expr '+' expr` now says
how each conflict was resolved:

```text
    2 expr: expr • '+' expr
    2     | expr '+' expr •  [$end, '+', '-', ')']
    3     | expr • '-' expr
    4     | expr • '*' expr
    5     | expr • '/' expr

    '*'  shift, and go to state 11
    '/'  shift, and go to state 12

    $default  reduce using rule 2 (expr)

    Conflict between rule 2 and token '+' resolved as reduce (%left '+').
    Conflict between rule 2 and token '-' resolved as reduce (%left '-').
    Conflict between rule 2 and token '*' resolved as shift ('+' < '*').
    Conflict between rule 2 and token '/' resolved as shift ('+' < '/').
```

Same decisions as the stratified grammar's state 6, written down
explicitly. And the automaton is smaller: count the states.

<details>
<summary>Solution: the whole rules section, and the declarations</summary>

```text
%left '+' '-'
%left '*' '/'
%precedence NEG
```

```text
program:
    expr                    { printf("%lld\n", $1); }
  ;

expr:
    expr '+' expr           { $$ = $1 + $3; }
  | expr '-' expr           { $$ = $1 - $3; }
  | expr '*' expr           { $$ = $1 * $3; }
  | expr '/' expr           {
                              if ($3 == 0) {
                                  yyerror("division by zero");
                                  YYABORT;
                              }
                              $$ = $1 / $3;
                            }
  | '-' expr %prec NEG      { $$ = -$2; }
  | '(' expr ')'            { $$ = $2; }
  | INT
  ;
```

18 states, against 21.

</details>

Keep this version: Part 3 uses the same grammar.

---

## Takeaways

- An LR parser shifts tokens onto a stack and reduces rules, bottom-up.
  A table built from the grammar, the automaton, picks the action from
  the current state and one lookahead token.
- LR parsers handle left recursion naturally, and left recursion gives
  left associativity.
- A **conflict** means the grammar allows two parses at some point.
  Yacc-style tools let you write a short ambiguous grammar and settle
  every conflict with precedence declarations. Precedence climbing and
  Pratt parsing do the same in hand-written parsers.
- Never ship a grammar with unexplained conflicts: Bison's default
  (shift) silently picked wrong answers above. Many projects require
  zero conflicts; Bison's `%expect 0` turns any conflict into an error.

---

**Next:** [Part 3 — A generated parser in Rust](part3-generated-parser.md)
