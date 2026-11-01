//! The lexer (also called scanner or tokenizer): turns source text into
//! tokens, the "words" the parser works with.

use crate::diagnostic::Diagnostic;
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// Split `source` into tokens. On success the last token is always
/// [`TokenKind::Eof`].
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token()?;
        tokens.push(token);
        if token.kind == TokenKind::Eof {
            return Ok(tokens);
        }
    }
}

/// Walks through the source text one character at a time.
struct Lexer<'src> {
    source: &'src str,
    /// Byte offset of the next character to read.
    pos: usize,
}

impl<'src> Lexer<'src> {
    fn new(source: &'src str) -> Lexer<'src> {
        Lexer { source, pos: 0 }
    }

    /// The next character, without consuming it (`None` at end of input).
    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    /// Consume the next character and return it (`None` at end of input).
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    /// Skip spaces, tabs and newlines: they separate tokens but mean nothing.
    fn skip_whitespace(&mut self) {
        loop {
            let c = self.peek();
            let is_white = c.is_some_and(|c| c.is_whitespace());
            if is_white {
                self.bump();
            } else {
                break;
            }
        }
    }

    /// Read the next token, or report a character that starts no token.
    fn next_token(&mut self) -> Result<Token, Diagnostic> {
        self.skip_whitespace();
        let start = self.pos;

        let Some(c) = self.bump() else {
            return Ok(Token {
                kind: TokenKind::Eof,
                span: Span { start, end: start },
            });
        };
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            v => {
                return if c.is_numeric() {
                    let t = self.int(start)?;
                    let end = self.pos;
                    Ok(Token {
                        kind: t,
                        span: Span::new(start, end),
                    })
                } else {
                    let end = self.pos;
                    Err(Diagnostic::new(
                        format!("unexpected character `{v}`"),
                        Span { start, end },
                    ))
                };
            }
        };

        let end = self.pos;
        Ok(Token {
            kind,
            span: Span::new(start, end),
        })
    }

    /// Finish an integer literal that starts at `start` (its first digit
    /// is already consumed) and parse its value.
    fn int(&mut self, start: usize) -> Result<TokenKind, Diagnostic> {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        let text = &self.source[start..self.pos];
        text.parse::<i64>().map_or_else(
            |_| {
                Err(Diagnostic {
                    message: "integer literal is too large".to_string(),
                    span: Span {
                        start,
                        end: self.pos,
                    },
                })
            },
            |v| Ok(TokenKind::Int(v)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    /// Just the token kinds, for tests that don't care about spans.
    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source)
            .unwrap()
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn bump_consumes_one_character_at_a_time() {
        let mut lexer = Lexer::new("ab");
        assert_eq!(lexer.bump(), Some('a'));
        assert_eq!(lexer.bump(), Some('b'));
        assert_eq!(lexer.bump(), None);
    }

    #[test]
    fn bump_counts_bytes_not_characters() {
        let mut lexer = Lexer::new("é1");
        assert_eq!(lexer.bump(), Some('é'));
        assert_eq!(lexer.pos, 2, "`é` is two bytes in UTF-8");
    }

    #[test]
    fn skip_whitespace_stops_at_the_next_token() {
        let mut lexer = Lexer::new(" \t\n 1");
        lexer.skip_whitespace();
        assert_eq!(lexer.peek(), Some('1'));
        assert_eq!(lexer.pos, 4);
    }

    #[test]
    fn skip_whitespace_does_nothing_without_whitespace() {
        let mut lexer = Lexer::new("1 ");
        lexer.skip_whitespace();
        assert_eq!(lexer.pos, 0);
    }

    #[test]
    fn next_token_empty_source_is_just_eof() {
        assert_eq!(kinds(""), vec![Eof]);
        assert_eq!(kinds("  \n "), vec![Eof]);
    }

    #[test]
    fn next_token_operators_and_parentheses() {
        assert_eq!(
            kinds("+-*/()"),
            vec![Plus, Minus, Star, Slash, LParen, RParen, Eof]
        );
    }

    #[test]
    fn next_token_skips_whitespace_between_tokens() {
        assert_eq!(kinds(" ( \n\t) "), vec![LParen, RParen, Eof]);
    }

    #[test]
    fn next_token_records_spans() {
        let spans: Vec<Span> = lex("+ (\n)").unwrap().iter().map(|t| t.span).collect();
        assert_eq!(
            spans,
            vec![
                Span::new(0, 1),
                Span::new(2, 3),
                Span::new(4, 5),
                Span::new(5, 5)
            ]
        );
    }

    #[test]
    fn next_token_rejects_unknown_characters() {
        assert_eq!(
            lex("+ $"),
            Err(Diagnostic::new("unexpected character `$`", Span::new(2, 3)))
        );
    }

    #[test]
    fn next_token_rejects_unknown_non_ascii_characters() {
        assert_eq!(
            lex("é"),
            Err(Diagnostic::new("unexpected character `é`", Span::new(0, 2)))
        );
    }

    #[test]
    fn int_reads_all_the_digits() {
        assert_eq!(kinds("0 42 007"), vec![Int(0), Int(42), Int(7), Eof]);
        assert_eq!(kinds("12+3"), vec![Int(12), Plus, Int(3), Eof]);
    }

    #[test]
    fn int_spans_cover_every_digit() {
        assert_eq!(lex("  123").unwrap()[0].span, Span::new(2, 5));
    }

    #[test]
    fn int_rejects_literals_too_large_for_i64() {
        assert_eq!(kinds("9223372036854775807"), vec![Int(i64::MAX), Eof]);
        assert_eq!(
            lex("1 + 9223372036854775808"),
            Err(Diagnostic::new(
                "integer literal is too large",
                Span::new(4, 23)
            ))
        );
    }
}
