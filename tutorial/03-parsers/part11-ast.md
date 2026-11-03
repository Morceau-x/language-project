# Part 11 — A typed AST

[← Step 3 overview](guide.md)

The syntax tree is complete, lossless, and untyped: a function and a
call are both `SyntaxNode`s. Code that *uses* the tree wants types. This
part adds them as a view, the way rust-analyzer does, in `src/ast/`,
then writes a first client, in `src/outline.rs`.

**Rust you'll use:** [traits with constructors](rust-concepts.md#14-traits-with-constructors-astnode),
[declarative macros](rust-concepts.md#5-declarative-macros-macro_rules).

---

## The idea: types over an untyped tree

Step 1's AST was typed: an `Expr` enum, a `BinOp`, a `Box` per child.
The parser built it directly, and it could only hold valid programs:
there was no way to represent a binary expression without its right
operand.

rust-analyzer keeps both: the untyped tree is the data, and the typed
AST is a **view** over it. Each AST type wraps a `SyntaxNode` of the
right kind, and its methods find the parts among the node's children:

```rust
pub struct Fn {
    syntax: SyntaxNode,          // a node of kind FN
}

impl Fn {
    pub fn name(&self) -> Option<Name> { child(&self.syntax) }       // the first NAME child
    pub fn param_list(&self) -> Option<ParamList> { child(&self.syntax) }
    pub fn body(&self) -> Option<BlockExpr> { child(&self.syntax) }
}
```

Nothing is copied: a typed node is a pointer to the same tree. And
**every accessor returns an `Option`** (or an iterator): the tree may
come from a broken program, and any part may be missing. Code using
the AST must handle that, and the types make it impossible to forget.

Alternatives with several node kinds, like an expression, are enums
with one variant per kind:

```rust
pub enum Expr {
    Literal(Literal),
    PathExpr(PathExpr),
    BinExpr(BinExpr),
    // …
}
```

### Read the code

Open `src/ast/mod.rs`:

- **`AstNode`** is the trait every typed node implements: `can_cast(kind)`
  says whether a node kind fits, `cast(node)` makes the typed view if it
  does, `syntax()` gives the node back.
- **`support`** has the three functions accessors are made of: the
  first child of a type, all the children of a type, the first child
  token of a kind.
- **`ast_node!`** and **`ast_enum!`** are macros that write the
  boilerplate: a struct and its `AstNode` impl per node kind, an enum
  and its impl per group of kinds.

Then `src/ast/nodes.rs`: one `ast_node!` per node kind, the enums
(`Decl`, `Type`, `Pat`, `Expr`, `Stmt`, `ElseBranch`, `Arg`), and the
accessors.

rust-analyzer doesn't write this file by hand: it **generates** it from
a grammar of the tree's shape, `rust.ungram`, written in a notation
close to your spec's. Part 13 suggests doing the same.

```bash
cargo test -p mini-parser --lib ast
```

---

## 11a. Accessors that need thought

Most accessors are one line: `child(&self.syntax)` finds the first
child of the return type. Seven are less obvious. Write them:

- **`BinExpr::lhs` and `rhs`.** Both operands are `Expr`s. The left is
  the first `Expr` child, the right the second. (Use `children`, an
  iterator, and `nth`.)
- **`BinExpr::op_token` and `PrefixExpr::op_token`.** The operator is
  the node's first child *token* that isn't trivia: operands are nodes.
  `first_token`, at the bottom of the file, does exactly that.
- **`IfExpr::then_branch`.** The block after the condition. The first
  `BlockExpr` child? Not if the condition is itself a block: `if { a }
  { b } else { c }` is valid Maxim (a strange one). The then-branch is
  the second `Expr` child, if it's a block.
- **`IfExpr::else_branch`.** What follows the `else` token: a block or
  another `if`. Again, not simply the last child: in a broken `if`,
  there may be no `else`. Find the `else` token (`support::token`), then
  the first child node after it that casts to `ElseBranch`.
- **`CtorPat::qualifier`.** In `Geo.Circle(r)`, there are two
  `NAME_REF`s, and the first is the qualifier. In `Circle(r)`, there's
  one, and no qualifier.

```bash
cargo test -p mini-parser --lib ast
```

<details>
<summary>Hint: `qualifier`</summary>

Take an iterator over the `NameRef` children. The first one is the
qualifier only if there's a second one: `names.next().and(first)` is
`first` if the second exists, and `None` otherwise.

</details>

<details>
<summary>Solution</summary>

```rust
pub fn lhs(&self) -> Option<Expr> {
    child(&self.syntax)
}

pub fn rhs(&self) -> Option<Expr> {
    children(&self.syntax).nth(1)
}

pub fn op_token(&self) -> Option<SyntaxToken> {
    first_token(&self.syntax)
}
```

`PrefixExpr::op_token` is the same as `BinExpr`'s.

```rust
pub fn then_branch(&self) -> Option<BlockExpr> {
    // The condition can be a block too: `if { a } { b } else { c }`.
    children::<Expr>(&self.syntax)
        .nth(1)
        .and_then(|expr| match expr {
            Expr::BlockExpr(block) => Some(block),
            _ => None,
        })
}

pub fn else_branch(&self) -> Option<ElseBranch> {
    let else_kw = token(&self.syntax, T![else])?;
    self.syntax
        .children()
        .filter(|node| node.text_range().start() >= else_kw.text_range().end())
        .find_map(ElseBranch::cast)
}
```

```rust
pub fn qualifier(&self) -> Option<NameRef> {
    let mut names = children::<NameRef>(&self.syntax);
    let first = names.next();
    names.next().and(first)
}
```

`children::<NameRef>(…)`: the "turbofish" `::<…>` gives the type
parameter explicitly, because nothing else in the expression says
which `AstNode` to look for.

</details>

---

## 11b. An outline

A first client of the typed AST: an **outline** of a file, one line per
declaration with its signature and without its body, like the outline
panel of an editor.

```text
module Todo
import List
type Priority = Low | Medium | High
type Task = { title, done, priority, tags }
let initial: List<Task>
fn priorityLabel(p: Priority) -> String
fn update(msg: Msg, tasks: List<Task>) -> List<Task>
```

Write `outline`, which formats each kind of declaration, and
`type_decl`, which formats a type: its name, its type parameters, then
its variants' names or its fields' names. `params`, `ret_type` and
`text` are done: `text` prints a node's source (trimmed), or `?` when
the node is missing. Read the tests for the exact format: types show
names only, functions show their source.

```bash
cargo test -p mini-parser --lib outline
```

<details>
<summary>Hint: matching on declarations</summary>

`file.decls()` gives `ast::Decl`s: `match decl { ast::Decl::Import(import)
=> …, ast::Decl::Fn(f) => …, … }`. Each variant holds the typed node,
with its accessors.

</details>

<details>
<summary>Solution</summary>

```rust
pub fn outline(file: &ast::SourceFile) -> String {
    let mut out = String::new();
    if let Some(header) = file.module_header() {
        out += &format!("module {}\n", text(header.name()));
    }
    for decl in file.decls() {
        let line = match decl {
            ast::Decl::Import(import) => format!("import {}", text(import.name_ref())),
            ast::Decl::TypeDecl(decl) => type_decl(&decl),
            ast::Decl::Fn(f) => format!(
                "fn {}{}{}",
                text(f.name()),
                params(f.param_list()),
                ret_type(f.ret_type())
            ),
            ast::Decl::Cast(cast) => format!(
                "cast {}{}",
                params(cast.param_list()),
                ret_type(cast.ret_type())
            ),
            ast::Decl::Component(component) => format!(
                "component {}{}",
                text(component.name()),
                params(component.param_list())
            ),
            ast::Decl::LetDecl(decl) => match decl.ty() {
                Some(ty) => format!("let {}: {}", text(decl.name()), text(Some(ty))),
                None => format!("let {}", text(decl.name())),
            },
        };
        out += &line;
        out.push('\n');
    }
    out
}
```

```rust
fn type_decl(decl: &ast::TypeDecl) -> String {
    let mut line = format!("type {}", text(decl.name()));
    if let Some(params) = decl.type_param_list() {
        let names: Vec<String> = params.names().map(|name| name.text()).collect();
        line += &format!("<{}>", names.join(", "));
    }
    if let Some(variants) = decl.variant_list() {
        let names: Vec<String> = variants.variants().map(|v| text(v.name())).collect();
        line += &format!(" = {}", names.join(" | "));
    } else if let Some(fields) = decl.record_field_list() {
        let names: Vec<String> = fields.fields().map(|f| text(f.name())).collect();
        line += &format!(" = {{ {} }}", names.join(", "));
    }
    line
}
```

The `match` on `Decl` is exhaustive: add a declaration kind to Maxim,
and this function stops compiling until it handles it. That's what the
typed layer buys.

</details>

Try it:

```bash
cargo parse --emit=outline mini-parser/corpus/ok/todo.mx
cargo parse --emit=outline mini-parser/corpus/err/missing-paren.mx
```

The second file is broken, and still has an outline: the declaration
with the error shows what could be parsed of it.

---

**Next:** [Part 12 — Errors and differential testing](part12-errors.md)
