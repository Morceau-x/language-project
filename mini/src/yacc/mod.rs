//! The generated front end. At build time, grmtools turns `mini.l` into
//! a lexer and `mini.y` into an LR(1) parser (see `build.rs`). This
//! file connects them to the rest of the compiler:
//!
//! ```text
//! source ──lexer (mini.l)──▶ lexemes ──parser (mini.y)──▶ CST ──prune──▶ AST
//! ```

use cfgrammar::TIdx;
use lrlex::{DefaultLexeme, DefaultLexerTypes};
use lrpar::{LexError, LexParseError, Lexeme, NonStreamingLexer, ParseRepair};

use crate::ast::Expr;
use crate::cst::Cst;
use crate::diagnostic::Diagnostic;
use crate::prune::prune;
use crate::span::Span;

// Bring in the generated code: the modules `mini_l` (the lexer) and
// `mini_y` (the parser). To read it, look for `mini.y.rs` under
// `target/debug/build/mini-*/out/`.
lrlex::lrlex_mod!("yacc/mini.l");
lrpar::lrpar_mod!("yacc/mini.y");

/// Parse a whole program with the generated parser, then prune its
/// parse tree into an AST. Same result as the hand-written
/// `parser::parse`, by a different road.
pub fn parse(source: &str) -> Result<Expr, Diagnostic> {
    prune(&parse_cst(source)?)
}

/// Lex and parse `source` into a concrete syntax tree.
pub fn parse_cst(source: &str) -> Result<Cst<'_>, Diagnostic> {
    let lexerdef = mini_l::lexerdef();
    let lexer = lexerdef.lexer(source);
    let (tree, errors) = mini_y::parse(&lexer);
    // grmtools recovers from syntax errors and keeps parsing, so it may
    // report several. Like the hand-written parser, we stop at the first.
    if let Some(error) = errors.first() {
        return Err(diagnostic(source, error));
    }
    Ok(tree.expect("no errors, so the parser built a tree"))
}

// ---------------------------------------------------------------------
// Helpers for the grammar actions in `mini.y`.

/// An `Expr` node with these children.
fn node(children: Vec<Cst<'_>>) -> Cst<'_> {
    Cst::Node {
        rule: "Expr",
        children,
    }
}

/// The leaf for a token the parser shifted.
///
/// The lexeme arrives as a `Result`: `Err` means error recovery
/// *invented* it to repair a syntax error. We build the leaf either way:
/// when there was an error, `parse_cst` throws the tree away.
fn token<'src>(
    lexer: &dyn NonStreamingLexer<'src, DefaultLexerTypes>,
    lexeme: Result<DefaultLexeme, DefaultLexeme>,
) -> Cst<'src> {
    let lexeme = lexeme.unwrap_or_else(|invented| invented);
    let span = lexeme.span();
    Cst::Token {
        kind: token_name(lexeme.tok_id()),
        text: lexer.span_str(span),
        span: Span::new(span.start(), span.end()),
    }
}

/// A token's name in the grammar (`INT`, `+`…), or `EOF` for the end of
/// input, which grmtools adds as an extra token.
fn token_name(id: u32) -> &'static str {
    mini_y::token_epp(TIdx(id)).unwrap_or("EOF")
}

// ---------------------------------------------------------------------
// Errors.

/// Turn a grmtools error into a Mini diagnostic, worded like the
/// hand-written front end's errors.
fn diagnostic(source: &str, error: &LexParseError<u32, DefaultLexerTypes>) -> Diagnostic {
    match error {
        LexParseError::LexError(error) => {
            let start = error.span().start();
            let c = source[start..].chars().next().unwrap_or(' ');
            Diagnostic::new(
                format!("unexpected character `{c}`"),
                Span::new(start, start + c.len_utf8()),
            )
        }
        LexParseError::ParseError(error) => {
            let lexeme = error.lexeme();
            let span = Span::new(lexeme.span().start(), lexeme.span().end());
            let found = match token_name(lexeme.tok_id()) {
                "EOF" => "end of input".to_string(),
                "INT" => format!("integer `{}`", &source[span.start..span.end]),
                "IDENT" => format!("name `{}`", &source[span.start..span.end]),
                other => format!("`{other}`"),
            };
            // Error recovery found ways to repair the input. The tokens it
            // would insert first are exactly what the parser expected.
            let mut expected: Vec<String> = Vec::new();
            for repairs in error.repairs() {
                if let Some(ParseRepair::Insert(id)) = repairs.first() {
                    let name = match token_name(id.0) {
                        "INT" => "an integer".to_string(),
                        "IDENT" => "a name".to_string(),
                        other => format!("`{other}`"),
                    };
                    if !expected.contains(&name) {
                        expected.push(name);
                    }
                }
            }
            // The repairs come in no particular order: sort them, so the
            // message is the same every time.
            expected.sort();
            let message = if expected.is_empty() {
                format!("unexpected {found}")
            } else {
                format!("expected {}, found {found}", expected.join(" or "))
            };
            Diagnostic::new(message, span)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of the parse tree, with every node in brackets except
    /// those around a single token: `1 + 2 * 3` gives `[1 + [2 * 3]]`.
    fn shape(source: &str) -> String {
        fn walk(tree: &Cst<'_>) -> String {
            match tree {
                Cst::Token { text, .. } => text.to_string(),
                Cst::Node { children, .. } if children.len() == 1 => walk(&children[0]),
                Cst::Node { children, .. } => {
                    let parts: Vec<String> = children.iter().map(walk).collect();
                    format!("[{}]", parts.join(" "))
                }
            }
        }
        match parse_cst(source) {
            Ok(tree) => walk(&tree),
            Err(diagnostic) => format!("error: {}", diagnostic.message),
        }
    }

    #[test]
    fn grammar_parses_literals_and_names() {
        assert_eq!(shape("42"), "42");
        assert_eq!(shape("width"), "width");
    }

    #[test]
    fn grammar_parses_each_operator() {
        assert_eq!(shape("1 + x"), "[1 + x]");
        assert_eq!(shape("1 - x"), "[1 - x]");
        assert_eq!(shape("1 * x"), "[1 * x]");
        assert_eq!(shape("1 / x"), "[1 / x]");
        assert_eq!(shape("-x"), "[- x]");
    }

    #[test]
    fn grammar_keeps_parentheses() {
        assert_eq!(shape("(1)"), "[( 1 )]");
        assert_eq!(shape("(1 + 2) * 3"), "[[( [1 + 2] )] * 3]");
    }

    #[test]
    fn grammar_gives_products_higher_precedence() {
        assert_eq!(shape("1 + 2 * 3"), "[1 + [2 * 3]]");
        assert_eq!(shape("1 * 2 - 3"), "[[1 * 2] - 3]");
        assert_eq!(shape("1 - 2 / 3"), "[1 - [2 / 3]]");
    }

    #[test]
    fn grammar_operators_are_left_associative() {
        assert_eq!(shape("1 - 2 - 3"), "[[1 - 2] - 3]");
        assert_eq!(shape("8 / 4 / 2"), "[[8 / 4] / 2]");
        assert_eq!(shape("1 + 2 - 3 + 4"), "[[[1 + 2] - 3] + 4]");
    }

    #[test]
    fn grammar_gives_negation_the_highest_precedence() {
        assert_eq!(shape("-2 * 3"), "[[- 2] * 3]");
        assert_eq!(shape("2 * -3"), "[2 * [- 3]]");
        assert_eq!(shape("-x - -y"), "[[- x] - [- y]]");
        assert_eq!(shape("- -1"), "[- [- 1]]");
    }

    #[test]
    fn tokens_keep_their_text_and_span() {
        let tree = parse_cst("( x1 )").unwrap();
        let Cst::Node { children, .. } = &tree else {
            panic!("expected a node, got {tree:?}");
        };
        assert_eq!(
            children[0],
            Cst::Token {
                kind: "(",
                text: "(",
                span: Span::new(0, 1)
            }
        );
        assert_eq!(children[1].span(), Span::new(2, 4));
        assert_eq!(tree.span(), Span::new(0, 6));
    }

    #[test]
    fn display_draws_the_tree() {
        assert_eq!(
            parse_cst("-(1 + x)").unwrap().to_string(),
            "\
Expr
├── \"-\"
└── Expr
    ├── \"(\"
    ├── Expr
    │   ├── Expr
    │   │   └── INT \"1\"
    │   ├── \"+\"
    │   └── Expr
    │       └── IDENT \"x\"
    └── \")\"
"
        );
    }

    #[test]
    fn syntax_errors_say_what_was_expected() {
        assert_eq!(
            parse_cst("(1 + 2"),
            Err(Diagnostic::new(
                "expected `)`, found end of input",
                Span::new(6, 6)
            ))
        );
        assert_eq!(
            parse_cst("1 + * 2"),
            Err(Diagnostic::new(
                "expected a name or an integer, found `*`",
                Span::new(4, 5)
            ))
        );
    }

    #[test]
    fn lexical_errors_point_at_the_character() {
        assert_eq!(
            parse_cst("1 + $"),
            Err(Diagnostic::new("unexpected character `$`", Span::new(4, 5)))
        );
    }
}
