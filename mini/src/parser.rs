//! The parser: turns the flat list of tokens into a tree (the AST).
//!
//! It is a *recursive descent* parser: one function per grammar rule,
//! calling each other the way the rules refer to each other. The
//! grammar, from lowest to highest precedence:
//!
//! ```text
//! program = expr EOF ;
//! expr    = term { ( "+" | "-" ) term } ;     sums of terms
//! term    = unary { ( "*" | "/" ) unary } ;   products of factors
//! unary   = "-" unary | primary ;
//! primary = INT | IDENT | "(" expr ")" ;
//! ```
//!
//! `{ ... }` means "zero or more times". Rules lower in the list bind
//! tighter, so `1 + 2 * 3` parses as `1 + (2 * 3)`.

use crate::ast::{BinOp, Expr, ExprKind};
use crate::diagnostic::Diagnostic;
use crate::span::Span;
use crate::token::{Token, TokenKind};
use std::ops::Deref;

/// Parse a whole program. `tokens` must end with [`TokenKind::Eof`], as
/// [`crate::lexer::lex`] guarantees.
pub fn parse(tokens: &[Token<'_>]) -> Result<Expr, Diagnostic> {
    let mut parser = Parser::new(tokens);
    let expr = parser.expr()?;
    parser.expect(TokenKind::Eof)?;
    Ok(expr)
}

/// Walks through the tokens one at a time.
struct Parser<'t> {
    tokens: &'t [Token<'t>],
    /// Index of the next token to read.
    pos: usize,
}

impl<'t> Parser<'t> {
    fn new(tokens: &'t [Token<'t>]) -> Parser<'t> {
        Parser { tokens, pos: 0 }
    }

    /// The next token, without consuming it.
    fn peek(&self) -> Token<'t> {
        self.tokens[self.pos]
    }

    /// Consume the next token and return it. Never moves past `Eof`, so
    /// `peek` always has a token to return.
    fn advance(&mut self) -> Token<'t> {
        let token = self.peek();
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    /// Consume the next token if it is a `kind`, or report what was found
    /// instead.
    fn expect(&mut self, kind: TokenKind<'_>) -> Result<Token<'t>, Diagnostic> {
        let next = self.peek();
        if next.kind != kind {
            Err(Diagnostic {
                message: format!("expected {kind}, found {}", next.kind),
                span: next.span,
            })
        } else {
            Ok(self.advance())
        }
    }

    /// `expr = term { ( "+" | "-" ) term } ;`
    fn expr(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.term()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
    }

    /// `term = unary { ( "*" | "/" ) unary } ;`
    fn term(&mut self) -> Result<Expr, Diagnostic> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                _ => return Ok(lhs),
            };
            self.advance();
            let rhs = self.unary()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
    }

    /// `unary = "-" unary | primary ;`
    fn unary(&mut self) -> Result<Expr, Diagnostic> {
        if self.peek().kind == TokenKind::Minus {
            let minus = self.advance();
            let operand = self.unary()?;
            return Ok(Expr::neg(minus.span, operand));
        }
        self.primary()
    }

    /// `primary = INT | IDENT | "(" expr ")" ;`
    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek();
        match token.kind {
            TokenKind::Int(value) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Int(value),
                    span: token.span,
                })
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.expr()?;
                let close = self.expect(TokenKind::RParen)?;
                Ok(Expr {
                    span: token.span.merge(close.span),
                    ..inner
                })
            }
            _ => self.identifier(),
        }
    }

    fn identifier(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.advance();
        match token.kind {
            TokenKind::Ident(value) => Ok(Expr {
                kind: ExprKind::Var(value.to_string()),
                span: token.span,
            }),
            _ => Err(Diagnostic {
                message: format!("expected an expression, found {}", token.kind),
                span: token.span,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::span::Span;

    /// Run one parser method on `source` and print the tree it built.
    fn parse_with(
        source: &str,
        rule: impl FnOnce(&mut Parser<'_>) -> Result<Expr, Diagnostic>,
    ) -> String {
        let tokens = lex(source).unwrap();
        let mut parser = Parser::new(&tokens);
        match rule(&mut parser) {
            Ok(expr) => expr.to_string(),
            Err(diagnostic) => format!("error: {}", diagnostic.message),
        }
    }

    fn parse_str(source: &str) -> Result<Expr, Diagnostic> {
        parse(&lex(source).unwrap())
    }

    #[test]
    fn expect_consumes_a_matching_token() {
        let tokens = lex("()").unwrap();
        let mut parser = Parser::new(&tokens);
        let lparen = parser.expect(TokenKind::LParen).unwrap();
        assert_eq!(lparen.span, Span::new(0, 1));
        assert_eq!(parser.peek().kind, TokenKind::RParen);
    }

    #[test]
    fn expect_reports_what_it_found_instead() {
        let tokens = lex("1").unwrap();
        let mut parser = Parser::new(&tokens);
        assert_eq!(
            parser.expect(TokenKind::RParen),
            Err(Diagnostic::new(
                "expected `)`, found integer `1`",
                Span::new(0, 1)
            ))
        );
        assert_eq!(parser.pos, 0, "a failed expect consumes nothing");
    }

    #[test]
    fn primary_parses_an_integer() {
        assert_eq!(parse_with("42", |p| p.primary()), "42");
    }

    #[test]
    fn primary_parses_an_input_name() {
        assert_eq!(parse_with("width", |p| p.primary()), "width");
        let expr = parse_str("  x").unwrap();
        assert_eq!(expr, Expr::var("x", Span::new(2, 3)));
    }

    #[test]
    fn primary_rejects_what_cannot_start_an_expression() {
        assert_eq!(
            parse_with("*", |p| p.primary()),
            "error: expected an expression, found `*`"
        );
        assert_eq!(
            parse_with("", |p| p.primary()),
            "error: expected an expression, found end of input"
        );
    }

    #[test]
    fn unary_parses_negation() {
        assert_eq!(parse_with("-1", |p| p.unary()), "(neg 1)");
        assert_eq!(parse_with("- -1", |p| p.unary()), "(neg (neg 1))");
        assert_eq!(parse_with("7", |p| p.unary()), "7");
    }

    #[test]
    fn unary_needs_an_operand() {
        assert_eq!(
            parse_with("-", |p| p.unary()),
            "error: expected an expression, found end of input"
        );
    }

    #[test]
    fn term_parses_products_left_to_right() {
        assert_eq!(parse_with("2 * 3", |p| p.term()), "(* 2 3)");
        assert_eq!(parse_with("8 / 4 / 2", |p| p.term()), "(/ (/ 8 4) 2)");
        assert_eq!(parse_with("2 * -3", |p| p.term()), "(* 2 (neg 3))");
        assert_eq!(parse_with("-2 * 3", |p| p.term()), "(* (neg 2) 3)");
    }

    #[test]
    fn expr_parses_sums_left_to_right() {
        assert_eq!(parse_with("1 + 2", |p| p.expr()), "(+ 1 2)");
        assert_eq!(parse_with("1 - 2 - 3", |p| p.expr()), "(- (- 1 2) 3)");
    }

    #[test]
    fn expr_gives_products_higher_precedence() {
        assert_eq!(parse_with("1 + 2 * 3", |p| p.expr()), "(+ 1 (* 2 3))");
        assert_eq!(parse_with("1 * 2 - 3", |p| p.expr()), "(- (* 1 2) 3)");
    }

    #[test]
    fn expr_parentheses_group() {
        assert_eq!(parse_with("(1 + 2) * 3", |p| p.expr()), "(* (+ 1 2) 3)");
        assert_eq!(parse_with("-(1 - 2)", |p| p.expr()), "(neg (- 1 2))");
        assert_eq!(parse_with("((7))", |p| p.expr()), "7");
    }

    #[test]
    fn expr_spans_cover_the_whole_expression() {
        let expr = parse_str("(1 + 2) * 3").unwrap();
        assert_eq!(expr.span, Span::new(0, 11));
        let crate::ast::ExprKind::Binary { lhs, .. } = expr.kind else {
            panic!("expected a binary expression");
        };
        assert_eq!(
            lhs.span,
            Span::new(0, 7),
            "the span includes the parentheses"
        );
    }

    #[test]
    fn expr_with_inputs() {
        assert_eq!(
            parse_with("x * (2 + 3) + y * 0", |p| p.expr()),
            "(+ (* x (+ 2 3)) (* y 0))"
        );
    }

    #[test]
    fn parse_rejects_a_missing_closing_parenthesis() {
        assert_eq!(
            parse_str("(1 + 2"),
            Err(Diagnostic::new(
                "expected `)`, found end of input",
                Span::new(6, 6)
            ))
        );
    }

    #[test]
    fn parse_rejects_leftover_tokens() {
        assert_eq!(
            parse_str("1 2"),
            Err(Diagnostic::new(
                "expected end of input, found integer `2`",
                Span::new(2, 3)
            ))
        );
    }

    #[test]
    fn parse_rejects_two_names_in_a_row() {
        assert_eq!(
            parse_str("x y"),
            Err(Diagnostic::new(
                "expected end of input, found name `y`",
                Span::new(2, 3)
            ))
        );
    }

    #[test]
    fn parse_rejects_a_missing_operand() {
        assert_eq!(
            parse_str("1 +"),
            Err(Diagnostic::new(
                "expected an expression, found end of input",
                Span::new(3, 3)
            ))
        );
    }
}
