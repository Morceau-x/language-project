# Part 4 — Parse trees and pruning

[← Step 2 overview](guide.md)

The generated parser builds a parse tree. The rest of the compiler
wants an AST. This part writes the layer in between: **pruning**.

**Rust you'll use:** [slice patterns](rust-concepts.md#4-slice-patterns-and-more-patterns),
[lifetimes in types](rust-concepts.md#1-lifetimes-in-types) (`Cst<'src>`).

---

## The idea: concrete vs abstract syntax

Put the two trees for `-(1 + x) * y` side by side:

```bash
mini --emit=cst mini/programs/negated-sum.mini
```

```text
Expr                                   (* (neg (+ 1 x)) y)
├── Expr
│   ├── "-"
│   └── Expr
│       ├── "("
│       ├── Expr
│       │   ├── Expr
│       │   │   └── INT "1"
│       │   ├── "+"
│       │   └── Expr
│       │       └── IDENT "x"
│       └── ")"
├── "*"
└── Expr
    └── IDENT "y"
```

The **concrete syntax tree** (CST) on the left records *how the
program was written*: every token, including the parentheses, with the
text of each literal not interpreted yet, and one node per rule the
parser reduced. With the stratified grammar of Part 2a, it would be
even bigger: four nodes (`expr`, `term`, `unary`, `primary`) above
every number.

The **abstract syntax tree** (AST) on the right keeps *what it means*.
Pruning goes from one to the other:

- drop the parentheses: the shape of the tree already groups things
  (but keep their span, so errors underline them);
- turn the operator tokens into `BinOp`s, and `-` followed by a tree
  into `Neg`;
- turn the text of a literal into its value: `"1"` into `1i64`, which
  can fail, so pruning can report errors too;
- collapse the nodes that wrap a single token.

### Why build a CST at all?

The classic yacc style builds the AST directly in the actions:
`Expr '+' Expr { binary($1, $3) }`. That's what you did in Part 2, with
numbers instead of trees. Building a full parse tree first costs
memory, but has fans:

- **Tools that edit code need every token.** A formatter (rustfmt,
  Prettier), a refactoring in an IDE, or a linter that suggests fixes
  must print code back exactly as it was, comments and all. rust-analyzer,
  Roslyn (C#), Swift's syntax library and tree-sitter all build
  lossless CSTs first, and derive the AST from them.
- **Separation.** The grammar actions stay trivial and uniform; all the
  interpretation lives in one plain Rust function you can test on its
  own.
- The reference compiler in `example/` works this way too:
  `example/src/cst.rs` and `example/src/cst/lower.rs`.

Mini's CST isn't fully lossless (it drops whitespace), but the idea is
the same.

---

## 4. `prune`

Open `src/prune.rs`. Every node in Mini's CST is an `Expr` node, and
which alternative of the grammar it came from shows in its children:

| Children                      | Alternative     | AST                                      |
|-------------------------------|-----------------|------------------------------------------|
| `INT`                         | `'INT'`         | `Expr::int`, or "integer literal is too large" |
| `IDENT`                       | `'IDENT'`       | `Expr::var`                              |
| `"-"`, tree                   | negation        | `Expr::neg`                              |
| `"("`, tree, `")"`            | parentheses     | the inner expression, with the span of the parentheses |
| tree, operator token, tree    | binary operator | `Expr::binary`                           |

Matching on the *shape* of a list is what **slice patterns** do:

```rust
match children.as_slice() {
    [Cst::Token { kind: "IDENT", text, span }] => { /* exactly one child: an IDENT token */ }
    [lhs, Cst::Token { kind, .. }, rhs] => { /* three children, the middle one a token */ }
    // ...
}
```

Implement `prune`. Your step-1 parser does the same jobs: compare with
`Parser::primary` (parentheses and their span) and `Lexer::int` (the
literal too large for an `i64`).

<details>
<summary>Hint 1: the start</summary>

`prune` receives a `&Cst`, which could in principle be a token. The
grammar guarantees it's a node, so:

```rust
let Cst::Node { children, .. } = tree else {
    unreachable!("a tree from the grammar is always a node")
};
match children.as_slice() {
    // one arm per row of the table
}
```

`unreachable!` is a panic that documents why it can't happen. Use it
for *your* invariants, never for user mistakes.

</details>

<details>
<summary>Hint 2: arm order</summary>

Both the negation `["-", tree]` and the parentheses `["(", tree, ")"]`
start with a token, while a binary operator starts with a tree, so the
patterns don't overlap as long as the parentheses arm comes before the
binary one (both have three children). A final `other => unreachable!(...)`
covers what the grammar can't produce.

</details>

<details>
<summary>Hint 3: parentheses</summary>

The node's span covers the parentheses: `tree.span()`. Replace the
inner expression's span with it, like in step 1:
`Ok(Expr { span: tree.span(), ..prune(inner)? })`.

</details>

```bash
cargo test -p mini --lib prune
```

<details>
<summary>Solution</summary>

```rust
pub fn prune(tree: &Cst<'_>) -> Result<Expr, Diagnostic> {
    let Cst::Node { children, .. } = tree else {
        unreachable!("a tree from the grammar is always a node")
    };
    match children.as_slice() {
        [Cst::Token { kind: "INT", text, span }] => match text.parse::<i64>() {
            Ok(value) => Ok(Expr::int(value, *span)),
            Err(_) => Err(Diagnostic::new("integer literal is too large", *span)),
        },
        [Cst::Token { kind: "IDENT", text, span }] => Ok(Expr::var(text, *span)),
        [Cst::Token { kind: "-", span, .. }, operand] => Ok(Expr::neg(*span, prune(operand)?)),
        [Cst::Token { kind: "(", .. }, inner, Cst::Token { kind: ")", .. }] => Ok(Expr {
            span: tree.span(),
            ..prune(inner)?
        }),
        [lhs, Cst::Token { kind, .. }, rhs] => {
            let op = match *kind {
                "+" => BinOp::Add,
                "-" => BinOp::Sub,
                "*" => BinOp::Mul,
                "/" => BinOp::Div,
                other => unreachable!("not a binary operator: {other}"),
            };
            Ok(Expr::binary(op, prune(lhs)?, prune(rhs)?))
        }
        other => unreachable!("no grammar rule has these children: {other:?}"),
    }
}
```

`cargo fmt` spreads the struct patterns over several lines each: it
keeps struct patterns on one line only when they're very short.

`*span` and `*kind`: in a pattern on a reference (`children` holds
`&Cst`), the bindings are references too, `&Span` and `&&str`. `Span`
is `Copy`, so `*span` copies it out.

</details>

---

## Two front ends, one tree

`yacc::parse` (provided) is `parse_cst` followed by `prune`. Now Mini
has two complete front ends:

```bash
mini --emit=ast mini/programs/negated-sum.mini
mini --emit=ast --parser=yacc mini/programs/negated-sum.mini
```

They must agree, spans included, on every program. Two tests check it.
The first runs a list of programs through both
(`both_front_ends_build_the_same_tree` in `tests/end_to_end.rs`):

```bash
cargo test -p mini --test end_to_end both_front_ends
```

The second doesn't use a list: it generates hundreds of random
programs, and random sequences of tokens that are mostly invalid. The
two parsers must agree on every one: same tree, or both reject it.

```bash
cargo test -p mini --test properties both_front_ends
```

This is **differential testing**: when two implementations of the same
thing exist, test them against each other. It finds bugs that no one
thought of writing a test for. Part 6 explains how the random programs
are made. (If one of these fails, read the failure: proptest prints the
smallest input it found that still fails.)

---

**Next:** [Part 5 — An intermediate representation](part5-ir.md)
