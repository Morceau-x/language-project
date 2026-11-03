# Part 3 — A generated parser in Rust

[← Step 2 overview](guide.md)

Now the same idea inside Mini. **grmtools** is a family of Rust crates
that does what Bison and flex do: `lrlex` generates a lexer from
regular expressions, `lrpar` generates an LR parser from a grammar in
yacc syntax. The reference compiler in `example/` uses it too
(`example/src/maxim.y`). (Unlike Bison's default LALR(1), grmtools
builds its tables with Pager's algorithm, which has the full power of
canonical LR(1) with tables as small as LALR's: a few grammars that
Bison rejects with conflicts, grmtools accepts.)

You'll write Mini's lexer and grammar for it. Mini then has **two front
ends**: your hand-written one, and the generated one. In Part 4 they
must build exactly the same trees.

**Rust you'll use:** [dependencies and build scripts](rust-concepts.md#3-dependencies-and-build-scripts).
You write no Rust in this part, only the `.l` and `.y` files.

Go back to the workspace root (`cd ../..` from `mini/bison`): from here
on, every command runs from there.

---

## The idea: code that writes code

A parser generator runs **before** your code compiles, and its output
is Rust code that gets compiled with the rest. Cargo has a place for
that: the **build script**. Open `mini/build.rs`:

```rust
fn main() {
    let built = CTLexerBuilder::new()
        .lrpar_config(|parser| {
            parser
                .grammar_in_src_dir("yacc/mini.y")
                .expect("cannot read src/yacc/mini.y")
        })
        .lexer_in_src_dir("yacc/mini.l")
        .expect("cannot read src/yacc/mini.l")
        .allow_missing_terms_in_lexer(true)
        .build();
    ...
}
```

Cargo compiles and runs `build.rs` before building the crate, each time
`mini.l` or `mini.y` changes. grmtools reads both files, builds the
automaton, and writes two Rust files into a folder Cargo provides for
generated files, `OUT_DIR`. Then `src/yacc/mod.rs` pulls them in:

```rust
lrlex::lrlex_mod!("yacc/mini.l");   // the module `mini_l`: the lexer
lrpar::lrpar_mod!("yacc/mini.y");   // the module `mini_y`: the parser
```

So **a mistake in the grammar is a build error**, reported before any
of your Rust is compiled. Including conflicts: unlike Bison, grmtools
refuses to build a grammar with any.

Read `src/yacc/mod.rs` now. `parse_cst` is the whole pipeline:

```rust
pub fn parse_cst(source: &str) -> Result<Cst<'_>, Diagnostic> {
    let lexerdef = mini_l::lexerdef();
    let lexer = lexerdef.lexer(source);
    let (tree, errors) = mini_y::parse(&lexer);
    if let Some(error) = errors.first() {
        return Err(diagnostic(source, error));
    }
    Ok(tree.expect("no errors, so the parser built a tree"))
}
```

The generated parser returns a tree *and* a list of errors: grmtools
recovers from syntax errors and keeps going (more on that below).

The tree it builds is a `Cst`, a **concrete syntax tree**, or parse
tree. Open `src/cst.rs`: a `Cst` is either a token (its kind, text and
span) or a node (a rule name and its children). It's the tree from
Part 2b, the one Bison's reductions built, with every token kept. Part
4 turns it into an AST.

---

## 3a. The lexer: `mini.l`

Open `src/yacc/mini.l`. After `%%`, each line is a rule: a regular
expression, then the name of the token it produces, in quotes.

```text
[0-9]+ "INT"
\s+ ;
```

`;` instead of a name means "skip it": that's how whitespace
disappears. When several rules match at the current position, the
**longest match** wins (maximal munch, as in your lexer); on a tie, the
rule written first wins.

Add a rule for each missing token: `IDENT`, and the six punctuation
tokens. Name the punctuation tokens by their own text: `"+"`, `"("`…

<details>
<summary>Hint 1: names</summary>

`[a-zA-Z_][a-zA-Z0-9_]*`: a letter or `_`, then any number of letters,
digits or `_`. The same rule as your `ident`.

</details>

<details>
<summary>Hint 2: escaping</summary>

`+`, `*`, `(` and `)` mean something in a regular expression, so they
need a backslash: `\+ "+"`. `-` and `/` don't.

</details>

Check that it still builds (the tests need the grammar too, which is
the next task):

```bash
cargo build -p mini
```

<details>
<summary>Solution</summary>

```text
[0-9]+ "INT"
[a-zA-Z_][a-zA-Z0-9_]* "IDENT"
\+ "+"
- "-"
\* "*"
/ "/"
\( "("
\) ")"
\s+ ;
```

</details>

---

## 3b. The grammar: `mini.y`

Open `src/yacc/mini.y`. The format is Bison's, with Rust instead of C:

```text
%start Expr
%%
Expr -> Cst<'input>:
      'INT' { node(vec![token($lexer, $1)]) }
    ;
```

- `Expr -> Cst<'input>` declares the rule `Expr` and the Rust type of
  its value: a parse tree, borrowing from the input text.
- Tokens are quoted: `'INT'`, `'+'`. Unquoted names are rules.
- Actions are Rust expressions. `$1`, `$2`… are the parts' values:
  a `Cst` for a rule, and for a token, its *lexeme* (its kind and
  span). `$lexer` reads a lexeme's text.
- `node(children)` and `token($lexer, lexeme)` are helpers from
  `mod.rs`: they build an `Expr` node, and a token leaf.

Your task: the grammar from Part 2d. Add the six missing alternatives
(four binary operators, negation, parentheses, and `'IDENT'`), each
building a node from **all** its parts, tokens included, in order. Then
add the precedence declarations before the first `%%`. Two differences
from Bison:

- grmtools has no `%precedence`: write `%nonassoc NEG`. (Non-associative
  means "two in a row is an error", which can't happen with a prefix
  operator, so it means the same thing here.)
- grmtools rejects tokens that are declared but never used, and `NEG`
  is only a precedence level: add `%expect-unused 'NEG'`.

Also add `%avoid_insert "INT" "IDENT"`: it tells error recovery that
inventing a number or a name is a bad way to repair a syntax error.

<details>
<summary>Hint 1: one alternative</summary>

```text
    | Expr '+' Expr { node(vec![$1, token($lexer, $2), $3]) }
```

`$1` and `$3` are already trees; `$2` is a lexeme, which `token` turns
into a leaf.

</details>

<details>
<summary>Hint 2: negation</summary>

`%prec NEG` goes after the alternative's symbols, before the action:

```text
    | '-' Expr %prec NEG { node(vec![token($lexer, $1), $2]) }
```

</details>

```bash
cargo test -p mini --lib yacc
```

The tests print the tree's shape with brackets: `1 + 2 * 3` must give
`[1 + [2 * 3]]`.

<details>
<summary>Solution</summary>

```text
%start Expr
// Error recovery may invent tokens to repair a syntax error. Inventing a
// number or a name out of thin air is a poor repair: avoid it if possible.
%avoid_insert "INT" "IDENT"

// Operator precedence, from lowest to highest: one level per line.
%left '+' '-'
%left '*' '/'
%nonassoc NEG
// NEG is not a real token: it only names a precedence level for `%prec`.
%expect-unused 'NEG'

%%
Expr -> Cst<'input>:
      Expr '+' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | Expr '-' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | Expr '*' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | Expr '/' Expr { node(vec![$1, token($lexer, $2), $3]) }
    | '-' Expr %prec NEG { node(vec![token($lexer, $1), $2]) }
    | '(' Expr ')' { node(vec![token($lexer, $1), $2, token($lexer, $3)]) }
    | 'INT' { node(vec![token($lexer, $1)]) }
    | 'IDENT' { node(vec![token($lexer, $1)]) }
    ;
```

</details>

Now look at a parse tree:

```bash
cargo mini --emit=cst mini/programs/inputs.mini
```

```text
Expr
├── Expr
│   ├── Expr
│   │   └── IDENT "x"
│   ├── "*"
│   └── Expr
│       ├── "("
│       ├── Expr
│       │   ├── Expr
│       │   │   └── INT "2"
...
```

---

## Break it on purpose

**A conflict.** Comment out the four lines from `%left '+' '-'` to
`%expect-unused 'NEG'` (`//` works in `.y` files), and remove the
`%prec NEG`. Then build:

```bash
cargo build -p mini
```

```text
  Shift/Reduce conflict, can shift '+' or reduce 'Expr' at .../mini/src/yacc/mini.y:18:1
  19|       Expr '+' Expr { node(vec![$1, token($lexer, $2), $3]) }
                  ^ Shift
  18| Expr -> Cst<'input>:
      ^^^^ Reduced rule
  23|     | '-' Expr { node(vec![token($lexer, $1), $2]) }
             -  ---- Reduced productions
```

The same conflicts as in Bison, but they stop the build: the panic in
`build.rs` turns grmtools' report into a Cargo error. Put the lines
back.

**A syntax error.** Compare the two front ends' messages:

```bash
echo '1 +' > /tmp/e.mini
cargo mini --emit=ast /tmp/e.mini
cargo mini --emit=ast --parser=yacc /tmp/e.mini
```

(`--parser=yacc` needs Part 4 to finish the job, but syntax errors are
found before that, so this already works.)

| Input   | Hand-written                                   | Generated                                            |
|---------|------------------------------------------------|------------------------------------------------------|
| `1 +`   | expected an expression, found end of input     | expected a name or an integer, found end of input    |
| `1 2`   | expected end of input, found integer `2`       | expected `*` or `+` or `-` or `/`, found integer `2` |
| `(1 + 2`| expected `)`, found end of input               | expected `)`, found end of input                     |
| `(1))`  | expected end of input, found `)`               | unexpected `)`                                       |

The generated parser knows exactly which **tokens** are valid in each
state of its automaton, and its error recovery (an algorithm called
CPCT+) searches for small repairs that would make the input valid. The
messages above come from those repairs: see `diagnostic` in
`src/yacc/mod.rs`. The hand-written parser knows **concepts** instead:
"an expression". That's one reason rustc, clang, Go, Swift and
TypeScript all hand-write their parsers: error messages, and full
control over recovery. Generated parsers win on other things: the
grammar is short, checked for ambiguities, and always matches the
parser.

---

## Look at the generated code

Find it (Cargo keeps one copy per build configuration; this lists the
newest first):

```bash
ls -t target/debug/build/mini-*/out/yacc/mini.y.rs
```

Open the first one. The grammar and the automaton are serialized into
byte strings (`__GRM_DATA`, `__STABLE_DATA`), and each of your actions
became a function (`__gt_action_0`, …) that takes the values of the
parts off the parser's stack. Never edit this file: the next build overwrites it. The
grammar is the source; this is a build product, like a `.o` file.

---

**Next:** [Part 4 — Parse trees and pruning](part4-parse-trees.md)
