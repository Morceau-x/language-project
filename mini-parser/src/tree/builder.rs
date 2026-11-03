//! From the parser's events to a green tree, putting the trivia back.

use std::mem;
use std::rc::Rc;

use super::{GreenNode, GreenNodeBuilder};
use crate::SyntaxError;
use crate::SyntaxKind::{self, *};
use crate::lexer::Lexed;
use crate::parser::Event;

/// Build the tree from all of `lexed`'s tokens, shaped by `events`.
/// Returns the tree and the parser's errors.
pub(crate) fn build_tree(
    lexed: &Lexed<'_>,
    mut events: Vec<Event>,
) -> (Rc<GreenNode>, Vec<SyntaxError>) {
    todo!("Part 6d: build_tree")
}

struct Builder<'a> {
    lexed: &'a Lexed<'a>,
    /// The next token of `lexed` to put in the tree.
    pos: usize,
    /// How many nodes are open.
    depth: usize,
    inner: GreenNodeBuilder,
    errors: Vec<SyntaxError>,
}

impl Builder<'_> {
    fn start_node(&mut self, kind: SyntaxKind) {
        todo!("Part 6d: start_node")
    }

    fn finish_node(&mut self) {
        todo!("Part 6d: finish_node")
    }

    fn token(&mut self, kind: SyntaxKind) {
        self.eat_trivia();
        self.inner.token(kind, self.lexed.text(self.pos));
        self.pos += 1;
    }

    /// Add the trivia tokens that come next, if any.
    fn eat_trivia(&mut self) {
        todo!("Part 6d: eat_trivia")
    }

    /// Record an error at the next token that isn't trivia.
    fn error(&mut self, msg: String) {
        let mut i = self.pos;
        while i < self.lexed.len() && self.lexed.kind(i).is_trivia() {
            i += 1;
        }
        self.errors
            .push(SyntaxError::new(msg, self.lexed.text_range(i)));
    }

    fn finish(self) -> (Rc<GreenNode>, Vec<SyntaxError>) {
        assert_eq!(
            self.pos,
            self.lexed.len(),
            "the parser must put every token in the tree"
        );
        (self.inner.finish(), self.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyntaxNode;
    use crate::lexer::lex;
    use expect_test::{Expect, expect};

    fn start(kind: SyntaxKind) -> Event {
        Event::Start {
            kind,
            forward_parent: None,
        }
    }

    fn token(kind: SyntaxKind) -> Event {
        Event::Token { kind }
    }

    /// Build the tree of `text` from hand-written events.
    fn check(text: &str, events: Vec<Event>, expect: Expect) {
        let lexed = lex(text);
        let (green, errors) = build_tree(&lexed, events);
        let mut out = SyntaxNode::new_root(green).debug_dump();
        for error in errors {
            out += &format!("{error}\n");
        }
        expect.assert_eq(&out);
    }

    #[test]
    fn trivia_between_nodes_goes_to_the_parent() {
        // `a b`: two expression statements in a block-like root.
        check(
            " a /* c */ b ",
            vec![
                start(SOURCE_FILE),
                start(PATH_EXPR),
                token(LOWER_IDENT),
                Event::Finish,
                start(PATH_EXPR),
                token(LOWER_IDENT),
                Event::Finish,
                Event::Finish,
            ],
            expect![[r#"
                SOURCE_FILE@0..13
                  WHITESPACE@0..1 " "
                  PATH_EXPR@1..2
                    LOWER_IDENT@1..2 "a"
                  WHITESPACE@2..3 " "
                  COMMENT@3..10 "/* c */"
                  WHITESPACE@10..11 " "
                  PATH_EXPR@11..12
                    LOWER_IDENT@11..12 "b"
                  WHITESPACE@12..13 " "
            "#]],
        );
    }

    #[test]
    fn trivia_inside_a_node_stays_inside() {
        check(
            "a + b",
            vec![
                start(SOURCE_FILE),
                start(BIN_EXPR),
                token(LOWER_IDENT),
                token(PLUS),
                token(LOWER_IDENT),
                Event::Finish,
                Event::Finish,
            ],
            expect![[r#"
                SOURCE_FILE@0..5
                  BIN_EXPR@0..5
                    LOWER_IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    PLUS@2..3 "+"
                    WHITESPACE@3..4 " "
                    LOWER_IDENT@4..5 "b"
            "#]],
        );
    }

    #[test]
    fn forward_parents_start_before_their_children() {
        // `a + b` as the parser emits it: PATH_EXPR is started first, and
        // `precede` adds a BIN_EXPR around it afterwards.
        check(
            "a + b",
            vec![
                start(SOURCE_FILE),
                Event::Start {
                    kind: PATH_EXPR,
                    forward_parent: Some(3),
                },
                token(LOWER_IDENT),
                Event::Finish,
                start(BIN_EXPR),
                token(PLUS),
                start(PATH_EXPR),
                token(LOWER_IDENT),
                Event::Finish,
                Event::Finish,
                Event::Finish,
            ],
            expect![[r#"
                SOURCE_FILE@0..5
                  BIN_EXPR@0..5
                    PATH_EXPR@0..1
                      LOWER_IDENT@0..1 "a"
                    WHITESPACE@1..2 " "
                    PLUS@2..3 "+"
                    WHITESPACE@3..4 " "
                    PATH_EXPR@4..5
                      LOWER_IDENT@4..5 "b"
            "#]],
        );
    }

    #[test]
    fn tombstones_start_nothing() {
        check(
            "a",
            vec![
                start(SOURCE_FILE),
                Event::tombstone(),
                token(LOWER_IDENT),
                Event::Finish,
            ],
            expect![[r#"
                SOURCE_FILE@0..1
                  LOWER_IDENT@0..1 "a"
            "#]],
        );
    }

    #[test]
    fn errors_point_at_the_next_token() {
        check(
            "a  )",
            vec![
                start(SOURCE_FILE),
                token(LOWER_IDENT),
                Event::Error {
                    msg: "expected `+`".to_string(),
                },
                token(R_PAREN),
                Event::Error {
                    msg: "expected more".to_string(),
                },
                Event::Finish,
            ],
            expect![[r#"
                SOURCE_FILE@0..4
                  LOWER_IDENT@0..1 "a"
                  WHITESPACE@1..3 "  "
                  R_PAREN@3..4 ")"
                error 3..4: expected `+`
                error 4..4: expected more
            "#]],
        );
    }
}
