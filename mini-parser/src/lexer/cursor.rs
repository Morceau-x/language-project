//! A cursor over the source text: the lexer's only way to read it.
//!
//! The same design as rustc_lexer's `Cursor`: peek at the next one or
//! two characters, consume one, and remember where the current token
//! started.

use std::str::Chars;

/// What `first` and `second` return past the end of the text. A real
/// `'\0'` in the text is told apart with `is_eof`.
pub const EOF_CHAR: char = '\0';

/// Reads the text one character at a time, and measures the token being
/// read.
pub struct Cursor<'a> {
    /// The characters not read yet.
    chars: Chars<'a>,
    /// How many bytes were left when the current token started.
    len_remaining: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(text: &'a str) -> Cursor<'a> {
        Cursor {
            chars: text.chars(),
            len_remaining: text.len(),
        }
    }

    /// The next character, without consuming it.
    pub fn first(&self) -> char {
        // `Chars` is a cheap iterator to clone: two pointers.
        self.chars.clone().next().unwrap_or(EOF_CHAR)
    }

    /// The character after the next one, without consuming anything.
    pub fn second(&self) -> char {
        let mut chars = self.chars.clone();
        chars.next();
        chars.next().unwrap_or(EOF_CHAR)
    }

    pub fn is_eof(&self) -> bool {
        self.chars.as_str().is_empty()
    }

    /// Consume the next character.
    pub fn bump(&mut self) -> Option<char> {
        self.chars.next()
    }

    /// Consume characters while `predicate` holds.
    pub fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while !self.is_eof() && predicate(self.first()) {
            self.bump();
        }
    }

    /// How many bytes the current token has so far.
    pub fn pos_within_token(&self) -> u32 {
        (self.len_remaining - self.chars.as_str().len()) as u32
    }

    /// Start a new token here.
    pub fn reset_pos_within_token(&mut self) {
        self.len_remaining = self.chars.as_str().len();
    }
}
