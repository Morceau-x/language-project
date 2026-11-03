# Part 6 — Lossless syntax trees

[← Step 3 overview](guide.md)

The parser's output is a tree. Before the parser, the tree: what it
looks like, how it's stored, and how it's built. All in `src/tree/`.

**Rust you'll use:** [`Rc`: shared ownership](rust-concepts.md#6-rc-shared-ownership),
[iterators you build](rust-concepts.md#7-iterators-you-build-from_fn-and-use),
[`mem::replace`](rust-concepts.md#8-memreplace-and-memtake),
[`writeln!` into a `String`](../02-compiler-layers/rust-concepts.md#11-writing-text-fmtwrite-and-writeln).

---

## The idea: a tree with every byte

Step 2's parse tree dropped whitespace; its AST dropped parentheses
too. This tree drops nothing:

```text
f( x)

CALL_EXPR@0..5
  PATH_EXPR@0..1
    NAME_REF@0..1
      LOWER_IDENT@0..1 "f"
  ARG_LIST@1..5
    L_PAREN@1..2 "("
    WHITESPACE@2..3 " "
    PATH_EXPR@3..4
      NAME_REF@3..4
        LOWER_IDENT@3..4 "x"
    R_PAREN@4..5 ")"
```

That's rust-analyzer's format, and yours: one line per node or token,
its kind, its range in bytes, and for tokens, the text. Tokens are the
leaves, and **their texts, in order, are the source**. Whitespace and
comments (the **trivia**) are tokens like the others.

Why keep everything? Because tools that change code need it. A
formatter, a refactoring that renames a function, an editor that
indents: they all rewrite the tree and print it back, and the comments
must survive. With a lossless tree, printing is just writing the
tokens.

Nodes are **untyped**: a `CALL_EXPR` and a `FN` are the same Rust
type, with a different `SyntaxKind`. The tree doesn't enforce that a
`CALL_EXPR` has an `ARG_LIST`: a broken program gives a tree without
one. Types come later, as a view (Part 11).

### Green and red

The tree is stored in two layers, a design rust-analyzer's `rowan`
crate took from Roslyn, the C# compiler:

- The **green** tree holds the data. A `GreenNode` has a kind, a
  length and children; a `GreenToken` has a kind and a text. Neither
  knows where it is, nor its parent. They're immutable, and shared
  with `Rc`.
- The **red** tree is a view over the green one, made on the fly as
  you walk it. A `SyntaxNode` is a green node plus its parent and its
  offset in the text.

Why two? Because the green tree, knowing no positions, can be shared.
When you type a character in a function, the editor can rebuild the
green nodes on the path from that function to the root, and reuse
every other node of the old tree, unchanged: their positions moved,
but positions aren't in them. And identical subtrees can be one: rowan
stores each distinct token, like `WHITESPACE " "`, once. The red nodes
cost an allocation each time you visit one, and are thrown away when
you're done: that's the price.

Read `src/tree/mod.rs` now: the green types, then `SyntaxNode`,
`SyntaxToken` and `SyntaxElement`.

---

## 6a. Building a green tree

`GreenNodeBuilder` builds a tree from the top down, with three calls:
`start_node(kind)`, `token(kind, text)`, `finish_node()`. The tests'
`call()` builds `f( x)` with it. Then `finish()` returns the root.

Write the four methods. The builder keeps a stack, `parents`: the
nodes started and not finished yet, each with the children it has so
far.

- `start_node` pushes a new node with no children.
- `token` adds a token to the children of the node on top of the stack.
- `finish_node` pops the top node, makes it a `GreenNode`, and adds it
  to the children of the new top. If the stack is now empty, that node
  is the root: keep it in `self.root`.
- `finish` returns the root. It panics if a node is still open
  (`"finish with unfinished nodes"`), or if there's no root.

```bash
cargo test -p mini-parser --lib tree::tests::green_nodes
cargo test -p mini-parser --lib tree::tests::finish_with
```

<details>
<summary>Solution</summary>

```rust
pub fn start_node(&mut self, kind: SyntaxKind) {
    self.parents.push((kind, Vec::new()));
}
```

```rust
pub fn token(&mut self, kind: SyntaxKind, text: &str) {
    let token = GreenElement::Token(Rc::new(GreenToken::new(kind, text)));
    let (_, siblings) = self.parents.last_mut().expect("a token outside any node");
    siblings.push(token);
}
```

```rust
pub fn finish_node(&mut self) {
    let (kind, children) = self.parents.pop().expect("finish_node without start_node");
    let node = Rc::new(GreenNode::new(kind, children));
    match self.parents.last_mut() {
        Some((_, siblings)) => siblings.push(GreenElement::Node(node)),
        None => self.root = Some(node),
    }
}
```

```rust
pub fn finish(self) -> Rc<GreenNode> {
    assert!(self.parents.is_empty(), "finish with unfinished nodes");
    self.root.expect("finish before the root node is finished")
}
```

`GreenNode::new` adds up its children's lengths once, so that a node's
length never needs a walk.

</details>

---

## 6b. Walking the red tree

A `SyntaxNode` is an `Rc<NodeData>`: its green node, its parent (a
`SyntaxNode` too), and its offset. Cloning one copies a pointer. Write:

- `children_with_tokens`: the children, in order, as `SyntaxElement`s.
  Each child's offset is the parent's offset plus the lengths of the
  children before it. Each child's parent is `self`.
- `descendants`: this node and every node below it, **parents before
  children**, in the order of the text (a preorder walk).
- `write_text`: append the text of every token under this node to
  `out`. `text()` calls it.

```bash
cargo test -p mini-parser --lib tree::tests::children
cargo test -p mini-parser --lib tree::tests::descendants
cargo test -p mini-parser --lib tree::tests::the_text
```

### The return types

`children_with_tokens` returns `impl Iterator<Item = SyntaxElement> +
use<>`. The `use<>` says the iterator borrows nothing from `self`: it
owns a clone of the node. Without it, the iterator would borrow `self`,
and `node.children()` couldn't outlive `node`, which gets in the way
everywhere (Part 11's accessors return iterators over children). The
scaffold returns `std::iter::from_fn(|| todo!(…))`: an iterator that
panics when used, because `todo!()` alone isn't an iterator.

<details>
<summary>Hint 1: an iterator with state</summary>

`std::iter::from_fn(move || …)` makes an iterator from a closure that
returns `Option<Item>`: `Some` for each item, `None` at the end. The
closure keeps its state in captured variables: a clone of `self`, the
index of the next child, and the running offset.

</details>

<details>
<summary>Hint 2: preorder with a stack</summary>

Keep a stack of nodes to visit, starting with `self`. Pop one, push its
children in **reverse** order (so the first child is popped next), and
return it.

</details>

<details>
<summary>Solution</summary>

```rust
pub fn children_with_tokens(&self) -> impl Iterator<Item = SyntaxElement> + use<> {
    let parent = self.clone();
    let mut offset = self.0.offset;
    let mut index = 0;
    std::iter::from_fn(move || {
        let child = parent.0.green.children().get(index)?;
        index += 1;
        let child_offset = offset;
        offset += child.text_len();
        Some(match child {
            GreenElement::Node(green) => SyntaxElement::Node(SyntaxNode(Rc::new(NodeData {
                green: Rc::clone(green),
                parent: Some(parent.clone()),
                offset: child_offset,
            }))),
            GreenElement::Token(green) => SyntaxElement::Token(SyntaxToken {
                green: Rc::clone(green),
                parent: parent.clone(),
                offset: child_offset,
            }),
        })
    })
}
```

```rust
pub fn descendants(&self) -> impl Iterator<Item = SyntaxNode> + use<> {
    let mut stack = vec![self.clone()];
    std::iter::from_fn(move || {
        let node = stack.pop()?;
        let children: Vec<SyntaxNode> = node.children().collect();
        stack.extend(children.into_iter().rev());
        Some(node)
    })
}
```

```rust
fn write_text(&self, out: &mut String) {
    for child in self.children_with_tokens() {
        match child {
            SyntaxElement::Node(node) => node.write_text(out),
            SyntaxElement::Token(token) => out.push_str(token.text()),
        }
    }
}
```

`parent.0.green.children().get(index)?` ends the iterator: `?` on an
`Option` returns `None` from the closure.

</details>

---

## 6c. The dump

Write `dump(depth, out)`, which `debug_dump` calls with depth 0: one
line for the node, `KIND@start..end`, indented by two spaces per level
of depth; then each child, nodes one level deeper, tokens on their own
line at that deeper level. `SyntaxToken`'s `Debug` already prints
`KIND@start..end "text"`, and `SyntaxNode`'s prints `KIND@start..end`.

```bash
cargo test -p mini-parser --lib tree::tests
```

<details>
<summary>Solution</summary>

```rust
fn dump(&self, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    writeln!(out, "{indent}{:?}@{}", self.kind(), self.text_range()).unwrap();
    for child in self.children_with_tokens() {
        match child {
            SyntaxElement::Node(node) => node.dump(depth + 1, out),
            SyntaxElement::Token(token) => {
                writeln!(out, "{indent}  {token:?}").unwrap();
            }
        }
    }
}
```

`writeln!` into a `String` can't fail, but returns a `Result` anyway
(the same macro writes to files): `unwrap()` is fine.

</details>

---

## 6d. From events to a tree

The parser (Part 7) won't call `GreenNodeBuilder` itself. It emits a
list of **events**:

```rust
pub(crate) enum Event {
    Start { kind: SyntaxKind, forward_parent: Option<u32> },
    Finish,
    Token { kind: SyntaxKind },
    Error { msg: String },
}
```

`src/tree/builder.rs` turns them into a tree, together with the
lexer's tokens. The parser only sees tokens that aren't trivia, so a
`Token` event is "the next token that isn't trivia". The builder walks
the lexer's tokens in step with the events, and puts the trivia back.

### Where does trivia go?

```text
fn f() { // hi
  x
}
```

Is the comment inside the block, or before the statement `x`? Each
answer is a choice; here is yours, a simple version of
rust-analyzer's:

- trivia **between two children goes to the parent**: a node starts at
  its first real token and ends at its last;
- at the edges of the file, trivia goes to the root, `SOURCE_FILE`.

So in the builder:

- `start_node`: unless it's the root, first add the trivia waiting
  before this point (to the current node, which becomes the parent).
- `token`: add the waiting trivia, then the token. That's done.
- `finish_node`: when the root finishes, add the trivia left at the
  end of the file.

### Forward parents

The parser sometimes learns *after* a node that it needs a parent.
Parsing `a + b`, it reads `a` and completes a `PATH_EXPR`; then it sees
`+`, and `a` must become the first child of a `BIN_EXPR`, whose `Start`
should have come **before** `a`'s. Inserting an event in the middle of
the list would shift every index after it. So instead, the parser
appends a new `Start BIN_EXPR` at the end, and notes in `a`'s `Start`
how far ahead its parent's `Start` is:

```text
0  Start PATH_EXPR  forward_parent: Some(3)  ─┐  "my parent starts 3 events later"
1  Token LOWER_IDENT                          │
2  Finish                                     │
3  Start BIN_EXPR                            ◀┘
4  Token PLUS
5  Start PATH_EXPR
6  Token LOWER_IDENT
7  Finish
8  Finish
```

When the builder meets event 0, it follows the chain: event 3 is a
parent, and it might have a `forward_parent` of its own (`a + b + c`
gives a chain of two). It starts the nodes from the outermost
(the end of the chain) to `PATH_EXPR`, and replaces the `Start` events
it used with **tombstones**, `Start { kind: TOMBSTONE, .. }`, so that
they don't start anything when the loop reaches them.

Write `build_tree`, and the builder's `start_node`, `finish_node` and
`eat_trivia` (add the trivia tokens that come next, if any; `token`,
`error` and `finish` are done).

```bash
cargo test -p mini-parser --lib tree::
```

The tests of `builder.rs` feed it hand-written events: read them, they
are the clearest description of what it does.

<details>
<summary>Hint 1: the loop</summary>

`for i in 0..events.len()`, and take each event out with
`mem::replace(&mut events[i], Event::tombstone())`: you own it, and the
list keeps a harmless tombstone. Then `match` on it: a `TOMBSTONE`
start does nothing, `Finish`, `Token` and `Error` call the builder.

</details>

<details>
<summary>Hint 2: following the chain</summary>

Collect the kinds in a `Vec`: the event's kind, then each forward
parent's (`idx += distance`, and `mem::replace` that event too, to get
its kind and its own `forward_parent`). Then start them in **reverse**:
the last parent found is the outermost.

</details>

<details>
<summary>Solution</summary>

```rust
pub(crate) fn build_tree(
    lexed: &Lexed<'_>,
    mut events: Vec<Event>,
) -> (Rc<GreenNode>, Vec<SyntaxError>) {
    let mut builder = Builder {
        lexed,
        pos: 0,
        depth: 0,
        inner: GreenNodeBuilder::default(),
        errors: Vec::new(),
    };
    // The kinds of the nodes to start at one event: a node and the nodes
    // that `precede` put before it, outermost last.
    let mut kinds = Vec::new();
    for i in 0..events.len() {
        match mem::replace(&mut events[i], Event::tombstone()) {
            Event::Start {
                kind: TOMBSTONE, ..
            } => {}
            Event::Start {
                kind,
                forward_parent,
            } => {
                // Follow the chain of forward parents. Each is a later
                // Start event, made a tombstone here so that it isn't
                // started twice.
                kinds.push(kind);
                let mut idx = i;
                let mut forward_parent = forward_parent;
                while let Some(distance) = forward_parent {
                    idx += distance as usize;
                    forward_parent = match mem::replace(&mut events[idx], Event::tombstone()) {
                        Event::Start {
                            kind,
                            forward_parent,
                        } => {
                            kinds.push(kind);
                            forward_parent
                        }
                        _ => unreachable!("a forward parent is a Start event"),
                    };
                }
                for kind in kinds.drain(..).rev() {
                    builder.start_node(kind);
                }
            }
            Event::Finish => builder.finish_node(),
            Event::Token { kind } => builder.token(kind),
            Event::Error { msg } => builder.error(msg),
        }
    }
    builder.finish()
}
```

```rust
fn start_node(&mut self, kind: SyntaxKind) {
    // Trivia before a node goes to its parent: the node starts at its
    // first real token. The root has no parent: it gets them all.
    if self.depth > 0 {
        self.eat_trivia();
    }
    self.depth += 1;
    self.inner.start_node(kind);
}
```

```rust
fn finish_node(&mut self) {
    self.depth -= 1;
    // Trivia after a node's last token goes to its parent, except at
    // the end of the file: the root takes it.
    if self.depth == 0 {
        self.eat_trivia();
    }
    self.inner.finish_node();
}
```

```rust
fn eat_trivia(&mut self) {
    while self.pos < self.lexed.len() && self.lexed.kind(self.pos).is_trivia() {
        self.inner
            .token(self.lexed.kind(self.pos), self.lexed.text(self.pos));
        self.pos += 1;
    }
}
```

`kinds.drain(..)` empties the `Vec` while iterating it, so the same
`Vec` serves for the next event, without a new allocation.

</details>

### What rust-analyzer does differently

It attaches comments right before an item to the item: in

```text
// Computes the area.
fn area(s: Shape) -> Float { … }
```

the comment belongs to the `FN` node, so that moving the function
moves its comment. That's a heuristic on top of the same builder
(`n_attached_trivias`); Part 13 suggests adding it.

---

**Next:** [Part 7 — The parser's engine](part7-engine.md)
