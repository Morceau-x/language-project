# Step 3 — The Rust you need

The Rust concepts step 3 adds to [step 2's](../02-compiler-layers/rust-concepts.md),
in the order you'll meet them. Same format: what it is, a small
example, and **In Mini**, where it shows up (here, in `mini-parser/`).

Skim the whole page once before you start. The [guide](guide.md) links
back to the sections you need for each part.

**Contents**

1. [Characters, strings and bytes](#1-characters-strings-and-bytes)
2. [`matches!`, and patterns on tuples](#2-matches-and-patterns-on-tuples)
3. [`Peekable` and `next_if`](#3-peekable-and-next_if)
4. [`Option` and `bool` combinators](#4-option-and-bool-combinators)
5. [Declarative macros: `macro_rules!`](#5-declarative-macros-macro_rules)
6. [`Rc`: shared ownership](#6-rc-shared-ownership)
7. [Iterators you build: `from_fn` and `use<>`](#7-iterators-you-build-from_fn-and-use)
8. [`mem::replace` and `mem::take`](#8-memreplace-and-memtake)
9. [`Cell`: mutation behind shared references](#9-cell-mutation-behind-shared-references)
10. [`Drop`: code that runs when a value dies](#10-drop-code-that-runs-when-a-value-dies)
11. [`const fn` and bit sets](#11-const-fn-and-bit-sets)
12. [Tests that must panic, and `#[track_caller]`](#12-tests-that-must-panic-and-track_caller)
13. [`let else`, and bindings in patterns](#13-let-else-and-bindings-in-patterns)
14. [Traits with constructors: `AstNode`](#14-traits-with-constructors-astnode)
15. [Snapshot testing with expect-test](#15-snapshot-testing-with-expect-test)
16. [Check yourself](#16-check-yourself)

---

## 1. Characters, strings and bytes

[Book ch. 8.2](https://doc.rust-lang.org/book/ch08-02-strings.html)

A `str` is UTF-8: a character takes 1 to 4 bytes. Indexes into a
string are **byte** offsets, and `len()` counts bytes:

```rust
let s = "é日";
assert_eq!(s.len(), 5);                 // 2 bytes for é, 3 for 日
assert_eq!(s.chars().count(), 2);
assert_eq!(&s[0..2], "é");              // slicing by bytes
// &s[0..1] would panic: it cuts é in half
```

Three ways to walk a string:

```rust
for c in s.chars() { … }                // characters
for (i, c) in s.char_indices() { … }    // characters, with their byte offset
let mut chars = s.chars();
chars.next();
assert_eq!(chars.as_str(), "日");        // what's left, as a &str
```

`Chars` is cheap to clone (two pointers): cloning it to look ahead
without consuming is a common trick.

`char` has the classifications you need: `is_ascii_digit`,
`is_ascii_alphabetic`, `is_ascii_alphanumeric`, `is_ascii_uppercase`,
`is_ascii_hexdigit`. And `escape_debug` prints a character the way
`{:?}` would, so a `\r` shows as `\r`, not as a carriage return.

**In Mini:** the `Cursor` (`lexer/cursor.rs`) is a `Chars` plus a
length: `first()` clones it to peek, `pos_within_token()` compares
`as_str().len()` before and after. `TextRange` holds byte offsets, so
`range.slice(text)` is free. Errors print characters with
`escape_debug`.

---

## 2. `matches!`, and patterns on tuples

`matches!(value, pattern)` is `true` if the value matches the pattern,
with `|` and guards allowed:

```rust
let is_bracket = matches!(c, '(' | ')' | '[' | ']');
let small_even = matches!(n, 0..=10 if n % 2 == 0);
```

To match two values at once, match a tuple. Patterns nest, so an
alternative can sit inside a tuple:

```rust
match (first, second) {
    ('=', '=' | '>') => …,      // == or =>
    ('-', '>') => …,            // ->
    ('a'..='z', _) => …,        // a range of characters
    _ => …,
}
```

**In Mini:** the raw lexer's `punct` (Part 4b) decides two-character
punctuation with one `matches!` on a tuple; `is_trivia`, `can_end_expr`
and `is_comparison` are `matches!` on kinds.

---

## 3. `Peekable` and `next_if`

[`Peekable`](https://doc.rust-lang.org/std/iter/struct.Peekable.html)

`.peekable()` wraps any iterator so you can look at the next item
without consuming it:

```rust
let mut it = "12ab".chars().peekable();
assert_eq!(it.peek(), Some(&'1'));            // still there
let digit = it.next_if(|c| c.is_ascii_digit()); // consume only if it matches
assert_eq!(digit, Some('1'));
while let Some(c) = it.next_if(|c| c.is_ascii_digit()) { … }
```

`next_if` replaces the "peek, test, then next" dance.

**In Mini:** `bad_escapes` (Part 5b) reads `\u{1F389}` with `next_if`:
the `{`, the hex digits, the `}`.

---

## 4. `Option` and `bool` combinators

[`Option`](https://doc.rust-lang.org/std/option/enum.Option.html)

Small methods that replace a `match`:

| Method                      | Gives                                                  |
|-----------------------------|--------------------------------------------------------|
| `opt.map_or(default, f)`    | `f(x)` for `Some(x)`, `default` for `None`              |
| `opt.and_then(f)`           | `f(x)` (itself an `Option`) for `Some(x)`, else `None`  |
| `opt.is_some_and(f)`        | `true` if `Some(x)` and `f(x)`                          |
| `opt.is_none_or(f)`         | `true` if `None`, or `Some(x)` and `f(x)`               |
| `opt.filter(f)`             | `Some(x)` if `f(x)`, else `None`                        |
| `opt.and(other)`            | `other` if `opt` is `Some`, else `None`                 |
| `result.ok()`               | a `Result` as an `Option`: `Err` becomes `None`        |
| `cond.then_some(x)`         | `Some(x)` if the `bool` is `true`, else `None`          |

```rust
let valid = u32::from_str_radix(&digits, 16)
    .ok()                       // Result → Option
    .and_then(char::from_u32)   // a function name works as a closure
    .is_some();
```

And `?` works on `Option` too, in a function (or closure) that returns
an `Option`: `None` returns early.

**In Mini:** `starts_html` uses `is_none_or` (Part 5d), `bad_escapes`
chains `ok` and `and_then`, `CtorPat::qualifier` uses `and` (Part 11a),
and the AST macros use `then_some`.

---

## 5. Declarative macros: `macro_rules!`

[Book ch. 20.5](https://doc.rust-lang.org/book/ch20-05-macros.html)

A macro is a function on code: it matches tokens and expands to code.
Arms are tried in order:

```rust
macro_rules! T {
    [fn] => { SyntaxKind::FN_KW };
    [->] => { SyntaxKind::THIN_ARROW };
    ['('] => { SyntaxKind::L_PAREN };    // a char literal: `(` alone would open a group
}
let kind = T![->];
```

Macros can capture pieces of code, with **fragment specifiers**, and
repeat:

```rust
macro_rules! ast_node {
    ($name:ident, $kind:ident) => {         // two identifiers
        pub struct $name { syntax: SyntaxNode }
        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool { kind == SyntaxKind::$kind }
            // …
        }
    };
}
ast_node!(Fn, FN);

macro_rules! ast_enum {
    ($name:ident { $($variant:ident),* $(,)? }) => {   // a list, maybe a trailing comma
        pub enum $name { $($variant($variant),)* }      // one variant per item
        // …  $($variant::can_cast(kind))||*  joins them with `||`
    };
}
```

`$(…),*` matches a comma-separated repetition, and the same `$(…)`
in the expansion repeats once per match. `$(#[$meta:meta])*` captures
attributes, like doc comments, to put them back on the struct.

`#[macro_export]` makes a macro usable from anywhere in the crate (and
outside), as `crate::T`; `$crate` in its body names the crate it was
defined in, wherever it's used.

**In Mini:** `T!` (`syntax_kind.rs`), `ast_node!` and `ast_enum!`
(`ast/mod.rs`). rust-analyzer generates its AST code instead of using
macros: past a few hundred nodes, generated code compiles faster and
reads better in an IDE.

---

## 6. `Rc`: shared ownership

[Book ch. 15.4](https://doc.rust-lang.org/book/ch15-04-rc.html)

A value normally has one owner. `Rc<T>` (reference-counted) lets
several share it: `Rc::clone` copies a pointer and increments a count;
the value is freed when the last `Rc` is dropped.

```rust
use std::rc::Rc;
let a = Rc::new(String::from("shared"));
let b = Rc::clone(&a);              // same String, count 2
assert!(Rc::ptr_eq(&a, &b));        // same allocation?
```

The shared value is immutable: you can't get a `&mut` from an `Rc`
that others hold. That's a feature for trees that are shared.

`Rc` isn't thread-safe; `Arc` is, at the cost of atomic counts. rowan
uses `Arc` for green nodes, so trees can cross threads.

An `Rc` cycle never gets freed: a parent pointing to its child and back
would leak. Red nodes point to their parent, but parents don't point to
their red children (those are made on demand), so there's no cycle.

**In Mini:** green nodes and tokens are `Rc`s (`tree/mod.rs`), shared
between the tree and every red node that views them. A `SyntaxNode` is
an `Rc<NodeData>`, which is why cloning one is cheap.

---

## 7. Iterators you build: `from_fn` and `use<>`

[`from_fn`](https://doc.rust-lang.org/std/iter/fn.from_fn.html)

`std::iter::from_fn(closure)` makes an iterator from a closure that
returns `Option<Item>`. The closure's captured variables are the
iterator's state:

```rust
fn countdown(from: u32) -> impl Iterator<Item = u32> {
    let mut n = from;
    std::iter::from_fn(move || {
        if n == 0 { return None; }
        n -= 1;
        Some(n + 1)
    })
}
```

`impl Iterator<Item = …>` hides the iterator's real type, which can't
be written (closures have no names). A hidden return type **captures**
the lifetimes in scope: in a method `fn children(&self) -> impl
Iterator`, it's assumed to borrow `self`. When it doesn't (it owns a
clone), say so with `use<>`, "captures nothing":

```rust
pub fn children(&self) -> impl Iterator<Item = SyntaxNode> + use<> { … }
```

Then `node.children()` can outlive `node`.

**In Mini:** `children_with_tokens` and `descendants` (Part 6b), and
the scaffold's placeholders: `std::iter::from_fn(|| todo!(…))` is an
iterator that panics when used.

---

## 8. `mem::replace` and `mem::take`

You can't move a value out of a `&mut` reference: the place would be
left empty. `std::mem::replace` moves it out and puts another value in
its place; `mem::take` puts the type's `Default`:

```rust
use std::mem;
let mut v = vec![1, 2];
let old = mem::take(&mut v);                   // old = [1, 2], v = []
let mut events = vec![Event::Finish];
let first = mem::replace(&mut events[0], Event::tombstone());
```

`Vec::drain(..)` is the same idea for a whole vector: it yields every
element by value and leaves the vector empty, keeping its allocation.

**In Mini:** `build_tree` (Part 6d) takes each event out of the list
with `mem::replace`, leaving a tombstone, and empties its `kinds`
vector with `drain(..)` after each node.

---

## 9. `Cell`: mutation behind shared references

[Book ch. 15.5](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html)

A `&self` method can't change `self`. Sometimes it must: a cache, a
counter. `Cell<T>` allows it for `Copy` values, by never handing out a
reference to its content, only copies:

```rust
use std::cell::Cell;
struct Counter { calls: Cell<u32> }
impl Counter {
    fn look(&self) -> u32 {
        self.calls.set(self.calls.get() + 1);   // through &self
        self.calls.get()
    }
}
```

This is **interior mutability**. `RefCell` does the same for non-`Copy`
values, checking the borrow rules at run time instead.

**In Mini:** `Parser::nth` takes `&self` (looking doesn't move the
parser), but counts every look in `steps: Cell<u32>`, to catch a parser
that stops moving (Part 7).

---

## 10. `Drop`: code that runs when a value dies

[Book ch. 15.3](https://doc.rust-lang.org/book/ch15-03-drop.html)

`impl Drop` runs code when a value goes out of scope, or is otherwise
dropped:

```rust
struct Guard(&'static str);
impl Drop for Guard {
    fn drop(&mut self) { println!("leaving {}", self.0); }
}
{
    let _g = Guard("block");
}   // prints "leaving block"
```

That's **RAII**: a resource is released by the value that owns it, at
a moment the compiler knows. Files close, locks unlock, memory frees.

A method that takes `self` by value (`fn complete(mut self, …)`)
consumes the value: after the call, it can't be used, and it's dropped
when the method returns. `mut self` lets the method modify it first.

`std::thread::panicking()` says whether the thread is unwinding from a
panic. Panicking again in `drop` during unwinding aborts the program.

**In Mini:** a `Marker` holds a `DropBomb` that panics if dropped
without being defused (Part 7c): forgetting to complete a node fails
loudly, where it happens.

---

## 11. `const fn` and bit sets

[Reference: const functions](https://doc.rust-lang.org/reference/const_eval.html#const-functions)

A `const fn` can run at compile time, to compute a `const`:

```rust
const fn bit(n: u8) -> u128 { 1u128 << n }
const MASK: u128 = bit(3) | bit(5);   // computed by the compiler
```

Not everything is allowed in a `const fn` yet: no `for` loops (they use
iterators), but `while` loops are fine. A panic in a `const` (like a
failed `assert!`) is a compile error.

An enum with `#[repr(u8)]` stores its variants as numbers 0, 1, 2…, and
`kind as u8` gets the number. With one bit per number, a set of kinds
is one integer: union is `|`, membership is `&`, and both are a single
instruction.

**In Mini:** `TokenSet` (`parser/token_set.rs`) is a `u128`, built by a
`const fn`, so the grammar's sets (`DECL_FIRST`, `EXPR_RECOVERY`…) are
constants computed at compile time.

---

## 12. Tests that must panic, and `#[track_caller]`

A test marked `#[should_panic]` passes if it panics, and fails if it
doesn't. With `expected`, the panic message must contain that text:

```rust
#[test]
#[should_panic(expected = "the parser seems stuck")]
fn a_parser_that_never_moves_panics() { … }
```

A helper that asserts reports, by default, its own line when it fails,
which is useless. `#[track_caller]` makes a panic inside it report the
caller's line instead:

```rust
#[track_caller]
fn check_parens(text: &str, expected: &str) {
    assert_eq!(parenthesize(text), expected);   // a failure points at the test that called it
}
```

**In Mini:** the drop bomb and the step limit are tested with
`should_panic` (Part 7); `check_parens` in the expression tests uses
`#[track_caller]`.

---

## 13. `let else`, and bindings in patterns

`let PATTERN = value else { … };` binds when the pattern matches, and
runs the `else` block otherwise, which must leave (`return`, `break`,
`continue`, or panic):

```rust
let Some((l_bp, r_bp)) = infix_binding_power(op) else {
    break;                   // not an operator: stop the loop
};
// l_bp and r_bp are usable here
```

It saves a level of nesting compared with `if let`.

A pattern can also **bind** what it matched with `@`:

```rust
match p.current() {
    kind @ (LOWER_IDENT | UPPER_IDENT) => name_ref(p, kind),
    _ => …,
}
```

**In Mini:** `expr_bp` (Part 9a), and `postfix` and `atom` for `@`.

---

## 14. Traits with constructors: `AstNode`

A trait's functions don't all take `self`. One that doesn't is called
on the type: `Fn::cast(node)`. When it returns `Self`, it's a
constructor, and the trait needs `where Self: Sized`, because a value
of an unsized type (like `dyn AstNode`) can't be returned:

```rust
pub trait AstNode {
    fn can_cast(kind: SyntaxKind) -> bool where Self: Sized;
    fn cast(syntax: SyntaxNode) -> Option<Self> where Self: Sized;
    fn syntax(&self) -> &SyntaxNode;
}
```

Generic code calls them through the type parameter, and can pass them
as functions:

```rust
fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
    parent.children().find_map(N::cast)    // `N::cast` is a fn(SyntaxNode) -> Option<N>
}

let first: Option<NameRef> = child(&node);       // N inferred from the type
let names = children::<NameRef>(&node);          // or given: the "turbofish"
```

**In Mini:** `ast/mod.rs` (Part 11): every typed node implements
`AstNode`, and every accessor is `child` or `children`.

---

## 15. Snapshot testing with expect-test

[expect-test](https://docs.rs/expect-test)

A snapshot test compares an output with the expected output written in
the test:

```rust
use expect_test::expect;

#[test]
fn a_tree() {
    let actual = parse("f(x)").debug_dump();
    expect![[r#"
        CALL_EXPR@0..4
          …
    "#]].assert_eq(&actual);
}
```

When they differ, the failure shows a diff. The indentation of the
expected text doesn't count. `expect_file![path]` does the same with an
expected output in a file.

Running the tests with `UPDATE_EXPECT=1` rewrites every expected
output with the actual one. It's how such tests are written: write
`expect![[""]]`, run with `UPDATE_EXPECT=1`, then **read** what it
wrote, and keep it if it's right. Never use it on tests you didn't
write: it makes them pass by making them test nothing.

**In Mini:** every grammar test, and `tests/corpus.rs` with the `.tree`
and `.error` files. rust-analyzer's crate; rust-analyzer's tests are
written this way too.

---

## 16. Check yourself

1. `s.len()` is 7 for a string of 4 characters. Why can that happen,
   and why is `&s[1..2]` risky?
2. Why does `first()` clone the `Chars` iterator instead of calling
   `next()`?
3. What does `T![->]` expand to, and why is `'('` written with quotes in
   the macro?
4. Two `SyntaxNode`s point to the same `GreenNode`. How much memory did
   the second one cost, roughly?
5. Why does `children()` need `+ use<>`? What would stop compiling
   without it?
6. `nth` takes `&self`. How does it count steps, and why not take
   `&mut self`?
7. A `Marker` is dropped without `complete` while a test is already
   failing with another panic. What does `DropBomb::drop` do, and why?
8. `const DECL_FIRST: TokenSet = TokenSet::new(&[T![fn], FN])`: `FN` is
   a node kind, not a token kind. What happens, and when?
9. Rewrite `let Some(x) = opt else { return None; };` with `?`.
10. Why does `AstNode::cast` need `where Self: Sized`?

<details>
<summary>Answers</summary>

1. Non-ASCII characters take several bytes. Slicing must cut at
   character boundaries, or it panics.
2. `next()` consumes; peeking must not. A clone is two pointers.
3. `SyntaxKind::THIN_ARROW` (as `$crate::SyntaxKind::THIN_ARROW`). A
   bare `(` in a macro pattern would open a group of tokens; `'('` is a
   char literal token.
4. One small allocation for its `NodeData` (a pointer, a parent, an
   offset): the green node itself is shared.
5. Without it, the returned iterator is assumed to borrow `self`, so
   `node.children()` couldn't outlive `node`: for example
   `parse(text).syntax_node().children()` would borrow a temporary.
6. Through `Cell<u32>`, interior mutability. Looking at tokens
   shouldn't need exclusive access: `at` and `nth` are called
   everywhere, often while other references exist.
7. Nothing: `std::thread::panicking()` is true, and a second panic
   would abort the program, hiding the first one.
8. `mask` asserts that the kind is a token kind (at most `UNDERSCORE`,
   the last one); `FN` isn't, so the assertion fails while the compiler
   evaluates the constant: a compile error, not a runtime one.
9. `let x = opt?;` (in a function returning `Option`).
10. It returns `Self` by value, which is impossible for an unsized type
    like `dyn AstNode`; the bound keeps `AstNode` usable as a trait
    object for `syntax()`.

</details>
