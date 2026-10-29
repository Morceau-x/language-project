//! Tokens: the "words" of a Mini program, produced by the lexer.

use std::fmt;

use crate::span::Span;

/// One token: what kind it is, and where it is in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Every kind of token Mini knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// An integer literal such as `42`, with its value already parsed.
    Int(i64),
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// The end of the input. The lexer always emits exactly one, last, so
    /// the parser never runs off the end of the token list.
    Eof,
}

/// How a token kind is named in error messages: "expected `)`, found
/// end of input".
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(value) => write!(f, "integer `{value}`"),
            TokenKind::Plus => f.write_str("`+`"),
            TokenKind::Minus => f.write_str("`-`"),
            TokenKind::Star => f.write_str("`*`"),
            TokenKind::Slash => f.write_str("`/`"),
            TokenKind::LParen => f.write_str("`(`"),
            TokenKind::RParen => f.write_str("`)`"),
            TokenKind::Eof => f.write_str("end of input"),
        }
    }
}
