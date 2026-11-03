# Part 8 — Types and patterns

[← Step 3 overview](guide.md)

The grammar, at last: one function per rule of your spec, in
`src/parser/grammar/`. This part does types and patterns, which need
nothing else.

---

## The idea: the spec, rule by rule

Step 1's parser was recursive descent; so is this one, with two
differences: the functions emit events instead of returning nodes, and
they never stop at an error. Each function:

- **starts at the first token of its rule**, and stops after the last;
- **decides with the current token**: that's what FIRST sets are for.
  Where one token isn't enough, Part 3's LL(1) report told you, and
  `nth_at(1, …)` looks one further;
- **always completes its node**, even when parts are missing: an
  error, and a node with fewer children.

The spec's operators translate directly:

| Spec                 | Code                                                     |
|----------------------|----------------------------------------------------------|
| `A B`                | `a(p); b(p);`, or `p.expect(T![…])` for a token          |
| `A \| B`             | `match p.current() { … }` on their first tokens          |
| `A?`                 | `if p.at(<first token of A>) { a(p); }`                  |
| `A ( "," A )*`       | `a(p); while p.eat(T![,]) { a(p); }`                     |
| `( A ( "," A )* )?` before `")"` | `if !p.at(T![')']) { loop { a(p); if !p.eat(T![,]) { break; } } }` |

### Read the code

Open `src/parser/grammar/mod.rs`. `source_file` and `module_header` are
done; so are `name` and `name_ref`, which wrap a name token in a `NAME`
node (a name being defined: a function's, a parameter's) or a
`NAME_REF` node (a name being used). rust-analyzer makes the same
distinction: a rename only changes `NAME_REF`s that point at the
renamed `NAME`.

At the bottom, two test helpers: `check(entry, text, expect)` runs one
grammar function on a text, in a root node, and compares the tree's
dump; `check_file` parses a whole file. The tests of each grammar file
use them.

### The shape of the tree

Which nodes, and what's in them, is your choice in a new parser, but
here the tests fix it. Each task gives the shapes. The kinds are in
`src/syntax_kind.rs`; `?` is "if present", `,*` "separated by commas".

---

## 8a. Types

```text
NAMED_TYPE       NAME_REF  TYPE_ARG_LIST?                 List<a>, a
TYPE_ARG_LIST    "<"  Type,*  ">"                          <String, a>
FN_TYPE          PARAM_TYPE_LIST  "->"  Type              (a) -> b
PARAM_TYPE_LIST  "("  PARAM_TYPE,*  ")"                    (acc: b, a)
PARAM_TYPE       (NAME  ":")?  Type                       acc: b
```

Write the five functions of `types.rs`. `type_` is called where a type
must be: if there's none, report `expected a type` with
`err_recover` and `TYPE_RECOVERY`. The others start at their first
token, which their caller checked: `p.bump(T![<])`. The `(` of a
function type is any `(`: no `at_line_start` check.

`param_type` needs two tokens of lookahead: `a` alone is a type
variable, `a:` is a label. It's Part 3's `ParamType` warning.

```bash
cargo test -p mini-parser --lib grammar::types
```

<details>
<summary>Hint: `type_`</summary>

A `match p.current()` with three arms, one per alternative of `Type`:
`UPPER_IDENT`, `LOWER_IDENT` and `T!['(']`, plus the error. For a named
type, start a marker, call `name_ref`, then `type_arg_list` if a `<`
follows, and complete `NAMED_TYPE`.

</details>

<details>
<summary>Solution</summary>

```rust
pub(super) fn type_(p: &mut Parser<'_>) {
    match p.current() {
        UPPER_IDENT => {
            let m = p.start();
            name_ref(p, UPPER_IDENT);
            if p.at(T![<]) {
                type_arg_list(p);
            }
            m.complete(p, NAMED_TYPE);
        }
        LOWER_IDENT => {
            let m = p.start();
            name_ref(p, LOWER_IDENT);
            m.complete(p, NAMED_TYPE);
        }
        T!['('] => fn_type(p),
        _ => p.err_recover("expected a type", TYPE_RECOVERY),
    }
}
```

```rust
fn type_arg_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![<]);
    type_(p);
    while p.eat(T![,]) {
        type_(p);
    }
    p.expect(T![>]);
    m.complete(p, TYPE_ARG_LIST);
}
```

```rust
fn fn_type(p: &mut Parser<'_>) {
    let m = p.start();
    param_type_list(p);
    p.expect(T![->]);
    type_(p);
    m.complete(p, FN_TYPE);
}
```

```rust
fn param_type_list(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T!['(']);
    if !p.at(T![')']) {
        loop {
            param_type(p);
            if !p.eat(T![,]) {
                break;
            }
        }
    }
    p.expect(T![')']);
    m.complete(p, PARAM_TYPE_LIST);
}
```

```rust
fn param_type(p: &mut Parser<'_>) {
    let m = p.start();
    if p.at(LOWER_IDENT) && p.nth_at(1, T![:]) {
        name(p, LOWER_IDENT);
        p.bump(T![:]);
    }
    type_(p);
    m.complete(p, PARAM_TYPE);
}
```

Read the test `broken_types`: `(Int)` isn't a type, so `fn_type`
reports `expected ->` and goes on; `List<>` gets `expected a type` at
the `>`, which stays for `type_arg_list` to consume (it's in the
recovery set); `5` is skipped in an `ERROR` node.

</details>

---

## 8b. Patterns

```text
WILDCARD_PAT   "_"
IDENT_PAT      NAME                                       x
LITERAL_PAT    INT_NUMBER | FLOAT_NUMBER | STRING         42
CTOR_PAT       NAME_REF  ("."  NAME_REF)?  ("("  Pattern,*  ")")?     Geo.Circle(r)
LIST_PAT       "["  Pattern,*  (","  REST_PAT)?  "]"       [x, ..rest]
REST_PAT       ".."  (NAME | "_")                         ..rest
```

Write the four functions of `patterns.rs`. `pattern` reports `expected
a pattern` with `PAT_RECOVERY`.

Two of Part 3's warnings are here:

- `CtorPattern ::= ( UPPER "." )? UPPER …` can't decide at the first
  `UPPER`. Left-factor it: always read an `UPPER`, then if a `.`
  follows, a second one. In the tree, the qualifier and the name are
  both `NAME_REF`s; Part 11 tells them apart.
- In a list pattern, after a `,`, look at the next token: `..` starts
  the rest, which ends the list; anything else is a pattern.

A rest's name is a `NAME` (it binds the rest), or `_`; anything else is
`expected a name or `_` after `..``.

```bash
cargo test -p mini-parser --lib grammar::patterns
```

<details>
<summary>Solution</summary>

```rust
pub(super) fn pattern(p: &mut Parser<'_>) {
    match p.current() {
        T![_] => {
            let m = p.start();
            p.bump(T![_]);
            m.complete(p, WILDCARD_PAT);
        }
        LOWER_IDENT => {
            let m = p.start();
            name(p, LOWER_IDENT);
            m.complete(p, IDENT_PAT);
        }
        INT_NUMBER | FLOAT_NUMBER | STRING => {
            let m = p.start();
            p.bump_any();
            m.complete(p, LITERAL_PAT);
        }
        UPPER_IDENT => ctor_pat(p),
        T!['['] => list_pat(p),
        _ => p.err_recover("expected a pattern", PAT_RECOVERY),
    }
}
```

```rust
fn ctor_pat(p: &mut Parser<'_>) {
    let m = p.start();
    name_ref(p, UPPER_IDENT);
    if p.eat(T![.]) {
        name_ref(p, UPPER_IDENT);
    }
    if p.eat(T!['(']) {
        pattern(p);
        while p.eat(T![,]) {
            pattern(p);
        }
        p.expect(T![')']);
    }
    m.complete(p, CTOR_PAT);
}
```

```rust
fn list_pat(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T!['[']);
    if !p.at(T![']']) {
        pattern(p);
        while p.eat(T![,]) {
            if p.at(T![..]) {
                rest_pat(p);
                break;
            }
            pattern(p);
        }
    }
    p.expect(T![']']);
    m.complete(p, LIST_PAT);
}
```

```rust
fn rest_pat(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![..]);
    match p.current() {
        LOWER_IDENT => name(p, LOWER_IDENT),
        T![_] => p.bump(T![_]),
        _ => p.error("expected a name or `_` after `..`"),
    }
    m.complete(p, REST_PAT);
}
```

`[..rest]` reports `expected a pattern` at the `..`, as the spec says:
the first pattern is required, and `..` can't start one.

</details>

### Every loop ends

Look at the loops you wrote: each iteration consumes at least one
token (the `,` it ate), or stops. That's the invariant the step limit
of Part 7 checks. The danger is a loop like "while not at `]`, parse a
pattern" on an input like `[)`: `pattern` reports an error and leaves
the `)` in place (it's in the recovery set), and the loop spins. Part
9's loops need a guard for that.

---

**Next:** [Part 9 — Expressions](part9-expressions.md)
