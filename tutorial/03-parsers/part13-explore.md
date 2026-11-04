# Part 13 — Explore, and clean up

[← Step 3 overview](guide.md)

The parser is done. First, the cleanup; then, ideas to go further, from
an evening to a weekend each.

---

## Clean up

`src/lib.rs` starts with an `#![allow(…)]` attribute: while the step
was in progress, unfinished functions left code unused, and the
compiler would have warned about it. Delete the attribute and its
comment, then:

```bash
cargo clippy -p mini-parser --all-targets
cargo test -p mini-parser
```

No warnings, every test green. If clippy finds something in your code,
fix it: its suggestions are usually good Rust.

---

## Look inside

1. **The tree of your own code.** Write a program with a mistake in
   each declaration, and read `cargo parse --emit=tree`. Where do the
   `ERROR` nodes go? Is the rest of each declaration where you'd
   expect?
2. **Two parsers, two dialects.** Read the reference compiler's parser
   in `example/`: `src/cst.rs`, its tree, is typed from the start and
   has no place for an error; compare with your two layers. Then its
   lexer's `LParenBOL`, and the top of `src/maxim.y`: how it handles
   the `-` and `(` that your `NEWLINE` settled. Which design would you
   rather explain to a newcomer?
3. **The checker.** `mini-parser/check-grammar/src/earley.rs` is an
   Earley parser in about 550 lines, and `ll1.rs` computes FIRST and
   FOLLOW sets in about 170. Read them with Part 3 in mind.

---

## Stretch exercises

1. **HTML.** The part of Maxim this step left out. In `example/`, read
   the lexer's modes (`Frame` in `src/lexer/mod.rs`: tags, attributes,
   text, `{…}` splices) and the `Jsx` rules of `src/maxim.y`. Add them
   to your spec (the lexical part is the interesting one: element text
   is a different language), then to your lexer and parser. Then remove
   the HTML skip from `tests/differential.rs`.
2. **Comments that stick.** rust-analyzer attaches the comments right
   before a declaration to the declaration (`n_attached_trivias` in its
   builder), so that moving a function moves its comment. Do the same
   in `tree/builder.rs`: when a declaration node starts, the comments
   just before it (with no blank line between) go inside it. Update the
   corpus snapshots, after reviewing the diff.
3. **A formatter.** With a lossless tree, a formatter is a walk that
   prints tokens and decides the whitespace between them: one space
   around binary operators, a line break between statements, two spaces
   of indentation per block. Keep every comment. Two properties to test
   with proptest: formatting a valid program gives a valid program
   with the same tree (ignoring trivia), and formatting twice gives the
   same text as formatting once.
4. **Incremental reparsing.** When one character changes in a block,
   only that block needs reparsing. rust-analyzer does it
   (`crates/syntax/src/parsing/reparsing.rs`): find the smallest
   `BLOCK_EXPR` around the edit, reparse its text alone, and replace its
   green node, sharing every other node of the old tree. The property
   to test: an incremental reparse gives the same tree as a full one.
5. **"Expected one of…"** Make `expect` and the `err_recover` calls
   remember every kind the parser checked with `at` at the error's
   position, and list them: `expected `)`, `,` or an expression`. rustc
   does this: its parser records every token it checked for. Which
   messages get better? Which get worse?
6. **Generate the AST.** Write `maxim.ungram`, a grammar of the tree's
   shape in rust-analyzer's [ungrammar](https://github.com/rust-analyzer/ungrammar)
   notation (`Fn = 'fn' Name ParamList RetType? body:BlockExpr`), and a
   test that generates `src/ast/nodes.rs` from it and fails if the
   file is out of date. That's how rust-analyzer keeps its thousands
   of lines of AST in sync.
7. **Generate programs from your spec.** A random walk through the
   grammar of `spec/maxim.md` (pick an alternative, a number of
   repetitions…) produces random valid token sequences. Turn them back
   into text: a `NEWLINE` is a line break, and the other tokens need a
   space between them only where they'd stick together. Feed them to
   your parser: every one must be valid, except those with `(_)`. The
   checker's EBNF reader in `check-grammar/src/ebnf.rs` can be reused.
8. **Fuzzing.** `cargo install cargo-fuzz`, and fuzz `parse` with
   arbitrary bytes, and the differential test with arbitrary strings.
   A fuzzer finds what proptest's vocabulary can't express.
9. **How fast?** Generate a 100 000-line Maxim file and time your
   parser on it. Where does it spend its time (`cargo flamegraph`)? Try
   interning the green tokens, as rowan does, so that every `WHITESPACE
   " "` is one shared allocation.
10. **Tree-sitter.** Editors highlight code with
    [tree-sitter](https://tree-sitter.github.io/), a generalized LR
    parser generator that also never fails. Write a tree-sitter grammar
    for Maxim from your spec. Its lexer can't decide `NEWLINE` alone:
    grammars like Python's and Ruby's add an "external scanner", a bit
    of C that decides, like your `from_lexed`.

---

**Done?** Head back to the [tutorial overview](../README.md) for what
comes next.
