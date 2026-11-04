# Part 10 — Declarations and whole files

[← Step 3 overview](guide.md)

The last rules: the declarations, in `src/parser/grammar/items.rs`.
At the end of this part, your parser reads any Maxim file.

---

## Read the code

`decl` is done: it dispatches on the declaration's keyword. Read its
last arm. A token that can't start a declaration starts an `ERROR`
node, and everything up to the next declaration keyword goes in it:
one error for the whole run, not one per token. That's the coarsest
recovery, at the top level, and it's what makes the rest of a file
parse after a mistake anywhere.

`decl` returns whether it parsed a declaration, for `source_file`, in
`grammar/mod.rs`: your spec's `Module ::= "module" UPPER ( NEWLINE
Decl )*`. Before each declaration, it checks for the `NEWLINE`, and
reports `expected a line break: each declaration starts on its own
line` if it's missing, the way your blocks do for statements. After a
run of junk, it doesn't: the junk's error says enough.

`import` is done too.

---

## 10a. Type declarations

```text
TYPE_DECL          "type"  NAME  TYPE_PARAM_LIST?  "="  (VARIANT_LIST | RECORD_FIELD_LIST)
TYPE_PARAM_LIST    "<"  NAME,*  ">"                         <a, b>
VARIANT_LIST       "|"?  VARIANT  ("|"  VARIANT)*          | Circle(Float) | Empty
VARIANT            NAME  TUPLE_FIELD_LIST?                  Rect(Float, Float)
TUPLE_FIELD_LIST   "("  Type,*  ")"
RECORD_FIELD_LIST  "{"  RECORD_FIELD,*  "}"                { x: Float, y: Float }
RECORD_FIELD       NAME  ":"  Type
```

Write `type_decl` and the five functions it uses. After `=`, a `{`
means a record; anything else, variants (and `variant` reports a
missing name). A type with variants needs at least one, a record at
least one field, and a variant's parentheses at least one type.

A variant's `(` stays on its line, like a constructor pattern's. A
line break inside a type declaration is otherwise fine where the
lexer's rule makes it whitespace: after `=` or `,`, before `|` or `}`.

```bash
cargo test -p mini-parser --lib items::tests
```

The tests of this task: `imports`, `variant_types`, `record_types`,
`broken_types`.

<details>
<summary>Solution</summary>

```rust
fn type_decl(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![type]);
    name(p, UPPER_IDENT);
    if p.at(T![<]) {
        type_param_list(p);
    }
    p.expect(T![=]);
    if p.at(T!['{']) {
        record_field_list(p);
    } else {
        variant_list(p);
    }
    m.complete(p, TYPE_DECL);
}
```

```rust
fn type_param_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![<]);
    name(p, LOWER_IDENT);
    while p.eat(T![,]) {
        name(p, LOWER_IDENT);
    }
    p.expect(T![>]);
    m.complete(p, TYPE_PARAM_LIST);
}
```

```rust
fn variant_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.eat(T![|]);
    variant(p);
    while p.eat(T![|]) {
        variant(p);
    }
    m.complete(p, VARIANT_LIST);
}
```

```rust
fn variant(p: &mut Parser<'_>) {
    let m = p.start();
    name(p, UPPER_IDENT);
    if p.at(T!['(']) && !p.at_newline() {
        let fields = p.start();
        p.bump(T!['(']);
        types::type_(p);
        while p.eat(T![,]) {
            types::type_(p);
        }
        p.expect(T![')']);
        fields.complete(p, TUPLE_FIELD_LIST);
    }
    m.complete(p, VARIANT);
}
```

```rust
fn record_field_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T!['{']);
    record_field(p);
    while p.eat(T![,]) {
        record_field(p);
    }
    p.expect(T!['}']);
    m.complete(p, RECORD_FIELD_LIST);
}
```

```rust
fn record_field(p: &mut Parser<'_>) {
    let m = p.start();
    name(p, LOWER_IDENT);
    p.expect(T![:]);
    types::type_(p);
    m.complete(p, RECORD_FIELD);
}
```

`p.eat(T![|]);` ignores its result: the leading `|` is optional, and
there's nothing to do either way.

</details>

---

## 10b. Functions, casts and components

```text
FN          "fn"  NAME  PARAM_LIST  RET_TYPE?  BLOCK_EXPR
CAST        "cast"  PARAM_LIST  RET_TYPE?  BLOCK_EXPR
COMPONENT   "component"  NAME  PARAM_LIST
```

Write `fn_decl`, `cast_decl` and `component`, with what Part 9 gave
you: `param_list`, `opt_ret_type` and `block`.

One special message. At the top level, `fn(x) { x }` is a lambda where
a declaration should be. Rather than `expected a lowercase name`,
report `expected the function's name: a lambda can't stand alone at
the top level` when a `(` follows `fn`, and go on parsing it as a
function without a name.

A function's `{` on the next line, `fn f() -> Int` newline `{`, is
already handled: `block` reports it with `not_on_new_line` (Part 9).
Read the tests `one_declaration_per_line` and
`a_brace_on_the_next_line_is_reported` in `items.rs`.

```bash
cargo test -p mini-parser --lib grammar
```

All the grammar's tests should pass, except
`arrow_lambdas_get_their_own_error`, which waits for Part 12.

<details>
<summary>Solution</summary>

```rust
fn fn_decl(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![fn]);
    if p.at(T!['(']) {
        p.error("expected the function's name: a lambda can't stand alone at the top level");
    } else {
        name(p, LOWER_IDENT);
    }
    param_list(p);
    opt_ret_type(p);
    block(p);
    m.complete(p, FN);
}
```

```rust
fn cast_decl(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![cast]);
    param_list(p);
    opt_ret_type(p);
    block(p);
    m.complete(p, CAST);
}
```

```rust
fn component(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![component]);
    name(p, UPPER_IDENT);
    param_list(p);
    m.complete(p, COMPONENT);
}
```

</details>

---

## The whole language

Your parser is complete. Every program of the corpus must give the
tree in its `.tree` file:

```bash
cargo test -p mini-parser --test corpus valid
```

`corpus/ok/todo.mx` and `corpus/ok/todo.tree` are worth a look side by
side: 75 lines of Maxim, 1192 lines of tree. That's the kind of
output snapshot tests are made for: nobody would write these
assertions by hand, but anyone can review a diff of them.

Try the command:

```bash
cargo parse --emit=tree mini-parser/corpus/ok/hello.mx
```

```text
SOURCE_FILE@0..56
  MODULE_HEADER@0..11
    MODULE_KW@0..6 "module"
    WHITESPACE@6..7 " "
    NAME@7..11
      UPPER_IDENT@7..11 "Main"
  WHITESPACE@11..13 "\n\n"
  FN@13..55
  …
```

And on broken programs, without `--emit`, it prints the errors with
the lines they're on:

```bash
cargo parse mini-parser/corpus/err/missing-paren.mx
```

```text
error: expected `)`
 --> mini-parser/corpus/err/missing-paren.mx:3:13
  |
3 | fn f(x: Int -> Int {
  |             ^^
```

Write a broken program of your own and look at its tree with
`--emit=tree`: the `ERROR` nodes, and how much of the program still
has its normal shape.

The remaining failing tests are Part 11's and Part 12's:

```bash
cargo test -p mini-parser --no-fail-fast
```

---

**Next:** [Part 11 — A typed AST](part11-ast.md)
