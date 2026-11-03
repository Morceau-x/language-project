//! The parser: tokens in, events out.
//!
//! The design is rust-analyzer's. The parser doesn't build a tree: it
//! emits a flat list of *events* (start a node, a token, finish the
//! node, an error), and `tree::build_tree` turns them into a tree
//! afterwards. The parser only sees the tokens that aren't trivia, so
//! the grammar never has to think about whitespace or comments, yet the
//! tree keeps them all.

mod grammar;
mod token_set;

use std::cell::Cell;

use crate::SyntaxKind::{self, *};
use crate::lexer::Lexed;
pub(crate) use token_set::TokenSet;

/// What the parser reads: the tokens that aren't trivia, each with one
/// more bit of information: does a line break come before it?
pub(crate) struct Input {
    kind: Vec<SyntaxKind>,
    newline_before: Vec<bool>,
}

impl Input {
    pub(crate) fn from_lexed(lexed: &Lexed<'_>) -> Input {
        todo!("Part 7a: from_lexed")
    }

    /// The kind of token `i`; `EOF` past the end.
    fn kind(&self, i: usize) -> SyntaxKind {
        self.kind.get(i).copied().unwrap_or(EOF)
    }

    fn newline_before(&self, i: usize) -> bool {
        self.newline_before.get(i).copied().unwrap_or(false)
    }
}

/// What the parser emits.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// Start a node. `forward_parent` is set when another node, started
    /// later, must become this one's parent: see `CompletedMarker::precede`.
    /// It's the distance to that node's `Start` event.
    Start {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    /// Finish the innermost node.
    Finish,
    /// The next token that isn't trivia.
    Token { kind: SyntaxKind },
    /// An error, at the next token that isn't trivia.
    Error { msg: String },
}

impl Event {
    /// A `Start` that starts nothing: what an abandoned marker leaves.
    pub(crate) fn tombstone() -> Event {
        Event::Start {
            kind: TOMBSTONE,
            forward_parent: None,
        }
    }
}

/// Parse a whole file.
pub(crate) fn parse(input: &Input) -> Vec<Event> {
    let mut p = Parser::new(input);
    grammar::source_file(&mut p);
    p.events
}

/// How many times the parser may look at the same token. A grammar
/// function that loops without consuming anything would hit this limit:
/// a bug in the parser becomes a panic instead of a hang.
const STEP_LIMIT: u32 = 10_000;

pub(crate) struct Parser<'t> {
    input: &'t Input,
    /// The current token.
    pos: usize,
    events: Vec<Event>,
    /// How many times the current token was looked at. A `Cell`: looking
    /// is `&self`, but counts.
    steps: Cell<u32>,
}

impl<'t> Parser<'t> {
    fn new(input: &'t Input) -> Parser<'t> {
        Parser {
            input,
            pos: 0,
            events: Vec::new(),
            steps: Cell::new(0),
        }
    }

    /// The kind of the current token.
    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    /// The kind of the token `n` tokens ahead: `nth(0)` is the current one.
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        let steps = self.steps.get();
        assert!(steps <= STEP_LIMIT, "the parser seems stuck");
        self.steps.set(steps + 1);
        self.input.kind(self.pos + n)
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth_at(0, kind)
    }

    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        self.nth(n) == kind
    }

    pub(crate) fn at_ts(&self, kinds: TokenSet) -> bool {
        kinds.contains(self.current())
    }

    /// Is the current token the first on its line?
    pub(crate) fn at_line_start(&self) -> bool {
        todo!("Part 7b: at_line_start")
    }

    /// Consume the current token if it's a `kind`.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        todo!("Part 7b: eat")
    }

    /// Consume the current token, which must be a `kind`.
    pub(crate) fn bump(&mut self, kind: SyntaxKind) {
        assert!(self.eat(kind), "bump: expected {kind:?}");
    }

    /// Consume the current token, whatever it is, unless it's `EOF`.
    pub(crate) fn bump_any(&mut self) {
        let kind = self.current();
        if kind != EOF {
            self.do_bump(kind);
        }
    }

    /// Consume a `kind`, or report that it's missing.
    pub(crate) fn expect(&mut self, kind: SyntaxKind) -> bool {
        todo!("Part 7b: expect")
    }

    /// Start a node here.
    pub(crate) fn start(&mut self) -> Marker {
        todo!("Part 7c: start")
    }

    /// Report an error at the current token, without consuming it.
    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        self.events.push(Event::Error { msg: msg.into() });
    }

    /// Report an error, and skip the current token, in an `ERROR` node.
    pub(crate) fn err_and_bump(&mut self, msg: &str) {
        self.err_recover(msg, TokenSet::EMPTY);
    }

    /// Report an error. Then skip the current token in an `ERROR` node,
    /// unless it's in `recovery`: a token the caller's caller expects,
    /// which is better left for it.
    pub(crate) fn err_recover(&mut self, msg: &str, recovery: TokenSet) {
        todo!("Part 7d: err_recover")
    }

    fn do_bump(&mut self, kind: SyntaxKind) {
        self.pos += 1;
        self.steps.set(0);
        self.events.push(Event::Token { kind });
    }
}

/// A node that was started and isn't finished yet.
pub(crate) struct Marker {
    /// The index of its `Start` event.
    pos: u32,
    bomb: DropBomb,
}

impl Marker {
    /// Finish the node: everything since `start` is in it.
    pub(crate) fn complete(mut self, p: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
        todo!("Part 7c: complete")
    }

    /// Forget the node: what's in it goes to its parent.
    pub(crate) fn abandon(mut self, p: &mut Parser<'_>) {
        todo!("Part 7c: abandon")
    }
}

/// A finished node.
#[derive(Clone, Copy)]
pub(crate) struct CompletedMarker {
    /// The index of its `Start` event.
    pos: u32,
}

impl CompletedMarker {
    /// Start a new node *before* this one, which becomes its first
    /// child: after parsing `a`, seeing `+` means `a` is the left side
    /// of a `BIN_EXPR` that should have started before it.
    pub(crate) fn precede(self, p: &mut Parser<'_>) -> Marker {
        todo!("Part 7c: precede")
    }
}

/// Panics if dropped before `defuse` is called: a marker forgotten by
/// the grammar is a bug, and this finds it right where it happens.
struct DropBomb {
    msg: &'static str,
    defused: bool,
}

impl DropBomb {
    fn new(msg: &'static str) -> DropBomb {
        DropBomb {
            msg,
            defused: false,
        }
    }

    fn defuse(&mut self) {
        self.defused = true;
    }
}

impl Drop for DropBomb {
    fn drop(&mut self) {
        todo!("Part 7c: drop")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::tree::build_tree;
    use crate::{SyntaxNode, T};
    use expect_test::{Expect, expect};

    /// Run `grammar` on `text` in a root node, and dump the tree.
    fn check(text: &str, grammar: impl FnOnce(&mut Parser<'_>), expect: Expect) {
        let lexed = lex(text);
        let input = Input::from_lexed(&lexed);
        let mut p = Parser::new(&input);
        let root = p.start();
        grammar(&mut p);
        while !p.at(EOF) {
            p.bump_any();
        }
        root.complete(&mut p, SOURCE_FILE);
        let (green, errors) = build_tree(&lexed, p.events);
        let mut out = SyntaxNode::new_root(green).debug_dump();
        for error in errors {
            out += &format!("{error}\n");
        }
        expect.assert_eq(&out);
    }

    #[test]
    fn the_input_skips_trivia_and_remembers_line_breaks() {
        let lexed = lex("a /* x */ b\n c /*\n*/ d e");
        let input = Input::from_lexed(&lexed);
        assert_eq!(
            input.kind,
            [
                LOWER_IDENT,
                LOWER_IDENT,
                LOWER_IDENT,
                LOWER_IDENT,
                LOWER_IDENT,
                EOF
            ]
        );
        assert_eq!(
            input.newline_before,
            [false, false, true, true, false, false]
        );
    }

    #[test]
    fn looking_ahead() {
        let lexed = lex("fn f(\nx");
        let input = Input::from_lexed(&lexed);
        let p = Parser::new(&input);
        assert_eq!(p.current(), T![fn]);
        assert!(p.at(T![fn]));
        assert!(p.nth_at(1, LOWER_IDENT));
        assert_eq!(p.nth(2), T!['(']);
        assert_eq!(p.nth(4), EOF);
        assert_eq!(p.nth(100), EOF);
        assert!(p.at_ts(TokenSet::new(&[T![let], T![fn]])));
        assert!(!p.at_line_start());
    }

    #[test]
    fn at_line_start() {
        let lexed = lex("a\n(b");
        let input = Input::from_lexed(&lexed);
        let mut p = Parser::new(&input);
        assert!(!p.at_line_start());
        p.bump(LOWER_IDENT);
        assert!(p.at_line_start());
    }

    #[test]
    fn eat_and_expect() {
        check(
            "( x",
            |p| {
                assert!(p.eat(T!['(']));
                assert!(!p.eat(T![')']));
                assert!(!p.expect(T![')']));
                p.bump(LOWER_IDENT);
                assert!(!p.expect(T![')']));
                p.bump_any(); // at EOF: does nothing
            },
            expect![[r#"
                SOURCE_FILE@0..3
                  L_PAREN@0..1 "("
                  WHITESPACE@1..2 " "
                  LOWER_IDENT@2..3 "x"
                error 2..3: expected `)`
                error 3..3: expected `)`
            "#]],
        );
    }

    #[test]
    fn markers_make_nodes() {
        check(
            "f x",
            |p| {
                let m = p.start();
                p.bump(LOWER_IDENT);
                m.complete(p, NAME);
                let m = p.start();
                m.abandon(p);
                let m = p.start();
                let inner = p.start();
                p.bump(LOWER_IDENT);
                inner.complete(p, NAME_REF);
                m.abandon(p);
            },
            expect![[r#"
                SOURCE_FILE@0..3
                  NAME@0..1
                    LOWER_IDENT@0..1 "f"
                  WHITESPACE@1..2 " "
                  NAME_REF@2..3
                    LOWER_IDENT@2..3 "x"
            "#]],
        );
    }

    #[test]
    fn precede_wraps_a_finished_node() {
        // `a + b + c`, parsed left to right: each `+` turns what came
        // before into the left side of a new BIN_EXPR.
        check(
            "a + b + c",
            |p| {
                let name = |p: &mut Parser<'_>| {
                    let m = p.start();
                    p.bump(LOWER_IDENT);
                    m.complete(p, PATH_EXPR)
                };
                let mut lhs = name(p);
                while p.at(T![+]) {
                    let m = lhs.precede(p);
                    p.bump(T![+]);
                    name(p);
                    lhs = m.complete(p, BIN_EXPR);
                }
            },
            expect![[r#"
                SOURCE_FILE@0..9
                  BIN_EXPR@0..9
                    BIN_EXPR@0..5
                      PATH_EXPR@0..1
                        LOWER_IDENT@0..1 "a"
                      WHITESPACE@1..2 " "
                      PLUS@2..3 "+"
                      WHITESPACE@3..4 " "
                      PATH_EXPR@4..5
                        LOWER_IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    PLUS@6..7 "+"
                    WHITESPACE@7..8 " "
                    PATH_EXPR@8..9
                      LOWER_IDENT@8..9 "c"
            "#]],
        );
    }

    #[test]
    fn err_recover_skips_what_the_caller_cannot_use() {
        check(
            "@ ) x",
            |p| {
                let recovery = TokenSet::new(&[T![')']]);
                p.err_recover("expected a name", recovery); // skips `@`
                p.err_recover("expected a name", recovery); // leaves `)`
                p.bump(T![')']);
                p.err_and_bump("expected `(`"); // skips `x`
                p.err_and_bump("expected `(`"); // at the end: nothing to skip
            },
            expect![[r#"
                SOURCE_FILE@0..5
                  ERROR@0..1
                    ERROR@0..1 "@"
                  WHITESPACE@1..2 " "
                  R_PAREN@2..3 ")"
                  WHITESPACE@3..4 " "
                  ERROR@4..5
                    LOWER_IDENT@4..5 "x"
                error 0..1: expected a name
                error 2..3: expected a name
                error 4..5: expected `(`
                error 5..5: expected `(`
            "#]],
        );
    }

    #[test]
    #[should_panic(expected = "a Marker must be completed or abandoned")]
    fn a_forgotten_marker_panics() {
        let lexed = lex("x");
        let input = Input::from_lexed(&lexed);
        let mut p = Parser::new(&input);
        let _forgotten = p.start();
    }

    #[test]
    #[should_panic(expected = "the parser seems stuck")]
    fn a_parser_that_never_moves_panics() {
        let lexed = lex("x");
        let input = Input::from_lexed(&lexed);
        let p = Parser::new(&input);
        while !p.at(EOF) {
            // A bug: never consumes `x`.
        }
    }
}
