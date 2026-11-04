//! The raw lexer: cuts the text into raw tokens.
//!
//! Like rustc_lexer, it is as dumb as it can be: it never fails, never
//! allocates, and doesn't know keywords. Each raw token is a kind and a
//! length, nothing else. Mistakes become flags (`terminated: false`) or
//! `Unknown` tokens, for the next layer to turn into error messages.

use super::cursor::Cursor;

/// A raw token: what kind, and how many bytes long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawToken {
    pub kind: RawKind,
    pub len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawKind {
    /// Spaces, tabs and line breaks (`\n` or `\r\n`).
    Whitespace,
    /// `// …`, up to the end of the line.
    LineComment,
    /// `/* … */`, which can nest. Unterminated if the text ends first.
    BlockComment { terminated: bool },
    /// A name or a keyword: `area`, `Shape`, `fn`, `_`.
    Ident,
    /// `42`, `1_000`.
    Int,
    /// `3.14`.
    Float,
    /// A string from its opening quote: `"…"`, or `"…${` if it opens an
    /// interpolation hole. Unterminated if the line ends first.
    Str { terminated: bool, opens_hole: bool },
    /// The rest of a string after a hole closes: `}…"`, or `}…${` if it
    /// opens another hole.
    StrContinuation { terminated: bool, opens_hole: bool },
    /// Punctuation, one to two characters: `(`, `->`, `|>`…
    Punct,
    /// A character that can't start any token.
    Unknown,
    /// The end of the text.
    Eof,
}

impl Cursor<'_> {
    /// Read the next token.
    pub fn advance_token(&mut self) -> RawToken {
        let Some(first) = self.bump() else {
            return RawToken {
                kind: RawKind::Eof,
                len: 0,
            };
        };
        let kind = match first {
            ' ' | '\t' | '\n' => self.whitespace(),
            '\r' if self.first() == '\n' => self.whitespace(),
            '/' if self.first() == '/' => self.line_comment(),
            '/' if self.first() == '*' => self.block_comment(),
            'a'..='z' | 'A'..='Z' | '_' => {
                self.eat_while(is_ident_continue);
                RawKind::Ident
            }
            '0'..='9' => self.number(),
            '"' => {
                let (terminated, opens_hole) = self.string_chunk();
                RawKind::Str {
                    terminated,
                    opens_hole,
                }
            }
            _ => self.punct(first),
        };
        self.finish_token(kind)
    }

    /// Read the rest of a string after one of its holes, from the `}`
    /// that closes the hole. Only the caller knows that this `}` closes a
    /// hole: the raw lexer doesn't track nesting.
    pub fn continue_string(&mut self) -> RawToken {
        debug_assert_eq!(self.first(), '}');
        self.bump();
        let (terminated, opens_hole) = self.string_chunk();
        self.finish_token(RawKind::StrContinuation {
            terminated,
            opens_hole,
        })
    }

    fn finish_token(&mut self, kind: RawKind) -> RawToken {
        let token = RawToken {
            kind,
            len: self.pos_within_token(),
        };
        self.reset_pos_within_token();
        token
    }

    fn whitespace(&mut self) -> RawKind {
        loop {
            match (self.first(), self.second()) {
                (' ' | '\t' | '\n', _) => {
                    self.bump();
                }
                ('\r', '\n') => {
                    self.bump();
                    self.bump();
                }
                _ => return RawKind::Whitespace,
            }
        }
    }

    fn line_comment(&mut self) -> RawKind {
        todo!("Part 4a: line_comment")
    }

    fn block_comment(&mut self) -> RawKind {
        todo!("Part 4a: block_comment")
    }

    fn number(&mut self) -> RawKind {
        todo!("Part 4c: number")
    }

    /// Read a piece of string up to and including its end: the closing
    /// quote, or the `${` of a hole. Returns `(terminated, opens_hole)`.
    /// Strings are on one line: a line break ends an unterminated one.
    fn string_chunk(&mut self) -> (bool, bool) {
        todo!("Part 4d: string_chunk")
    }

    /// Punctuation, longest match first: `->` before `-`.
    fn punct(&mut self, first: char) -> RawKind {
        todo!("Part 4b: punct")
    }
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_digit_continue(c: char) -> bool {
    c.is_ascii_digit() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::{Expect, expect};

    /// One raw token per line: its kind and its text.
    fn check(text: &str, expect: Expect) {
        let mut cursor = Cursor::new(text);
        let mut out = String::new();
        let mut offset = 0;
        loop {
            let token = cursor.advance_token();
            if token.kind == RawKind::Eof {
                break;
            }
            let end = offset + token.len as usize;
            out += &format!("{:?} {:?}\n", token.kind, &text[offset..end]);
            offset = end;
        }
        expect.assert_eq(&out);
    }

    #[test]
    fn whitespace_runs_are_one_token() {
        check(
            " \t\n\r\n x",
            expect![[r#"
                Whitespace " \t\n\r\n "
                Ident "x"
            "#]],
        );
    }

    #[test]
    fn a_lone_carriage_return_is_unknown() {
        check(
            "a\rb",
            expect![[r#"
                Ident "a"
                Unknown "\r"
                Ident "b"
            "#]],
        );
    }

    #[test]
    fn comments() {
        check(
            "// one\n/* two\n */ /* three",
            expect![[r#"
                LineComment "// one"
                Whitespace "\n"
                BlockComment { terminated: true } "/* two\n */"
                Whitespace " "
                BlockComment { terminated: false } "/* three"
            "#]],
        );
    }

    #[test]
    fn comments_nest() {
        check(
            "/* a /* b */ c */ x /* d /* e */",
            expect![[r#"
                BlockComment { terminated: true } "/* a /* b */ c */"
                Whitespace " "
                Ident "x"
                Whitespace " "
                BlockComment { terminated: false } "/* d /* e */"
            "#]],
        );
        check(
            "/**/ /*/ */ /***/",
            expect![[r#"
                BlockComment { terminated: true } "/**/"
                Whitespace " "
                BlockComment { terminated: true } "/*/ */"
                Whitespace " "
                BlockComment { terminated: true } "/***/"
            "#]],
        );
    }

    #[test]
    fn identifiers_include_keywords_and_underscores() {
        check(
            "fn area _ _tmp Shape2",
            expect![[r#"
                Ident "fn"
                Whitespace " "
                Ident "area"
                Whitespace " "
                Ident "_"
                Whitespace " "
                Ident "_tmp"
                Whitespace " "
                Ident "Shape2"
            "#]],
        );
    }

    #[test]
    fn numbers() {
        check(
            "1_000 3.14 1.x 1..2",
            expect![[r#"
                Int "1_000"
                Whitespace " "
                Float "3.14"
                Whitespace " "
                Int "1"
                Punct "."
                Ident "x"
                Whitespace " "
                Int "1"
                Punct ".."
                Int "2"
            "#]],
        );
    }

    #[test]
    fn punctuation_takes_the_longest_match() {
        check(
            "->-|>||=>==::.",
            expect![[r#"
                Punct "->"
                Punct "-"
                Punct "|>"
                Punct "||"
                Punct "=>"
                Punct "=="
                Punct "::"
                Punct "."
            "#]],
        );
    }

    #[test]
    fn half_operators_are_unknown() {
        check(
            "& ~ @",
            expect![[r#"
                Unknown "&"
                Whitespace " "
                Unknown "~"
                Whitespace " "
                Unknown "@"
            "#]],
        );
    }

    #[test]
    fn strings_and_escapes() {
        check(
            r#""a\"b" "c\${d}" "e"#,
            expect![[r#"
                Str { terminated: true, opens_hole: false } "\"a\\\"b\""
                Whitespace " "
                Str { terminated: true, opens_hole: false } "\"c\\${d}\""
                Whitespace " "
                Str { terminated: false, opens_hole: false } "\"e"
            "#]],
        );
    }

    #[test]
    fn a_string_stops_at_the_end_of_the_line() {
        check(
            "\"a\nb\"",
            expect![[r#"
                Str { terminated: false, opens_hole: false } "\"a"
                Whitespace "\n"
                Ident "b"
                Str { terminated: false, opens_hole: false } "\""
            "#]],
        );
    }

    #[test]
    fn a_string_with_a_hole_stops_at_the_hole() {
        check(
            r#""a${x}b""#,
            expect![[r#"
                Str { terminated: true, opens_hole: true } "\"a${"
                Ident "x"
                Punct "}"
                Ident "b"
                Str { terminated: false, opens_hole: false } "\""
            "#]],
        );
    }

    #[test]
    fn continue_string_reads_after_a_hole() {
        let mut cursor = Cursor::new("}b${c}d\"");
        assert_eq!(
            cursor.continue_string(),
            RawToken {
                kind: RawKind::StrContinuation {
                    terminated: true,
                    opens_hole: true
                },
                len: 4
            }
        );
        cursor.advance_token(); // c
        assert_eq!(
            cursor.continue_string(),
            RawToken {
                kind: RawKind::StrContinuation {
                    terminated: true,
                    opens_hole: false
                },
                len: 3
            }
        );
    }
}
