//! The lexer: source text in, tokens out.
//!
//! Two layers, as in Rust's compilers:
//!
//! - `raw` cuts the text into raw tokens. Like rustc_lexer, it knows no
//!   keywords and reports no errors.
//! - This module "cooks" them: it gives each token its `SyntaxKind`,
//!   turns the raw lexer's flags into error messages, and handles the
//!   rules that need context: where an interpolated string resumes, and
//!   where a `<` would start an HTML element.
//!
//! Like rust-analyzer's `LexedStr`, the result keeps every token,
//! whitespace and comments included: the parser skips them, the syntax
//! tree keeps them.

mod cursor;
mod raw;

use crate::SyntaxKind::{self, *};
use crate::text_range::TextRange;
use cursor::Cursor;
use raw::{RawKind, RawToken};

/// The tokens of a text: kinds and positions, and the lexical errors.
pub struct Lexed<'a> {
    text: &'a str,
    kind: Vec<SyntaxKind>,
    /// Where each token starts, plus one last entry: the end of the text.
    start: Vec<u32>,
    errors: Vec<LexError>,
}

/// A lexical error: an unknown character, a string that never closes…
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub msg: String,
    pub range: TextRange,
}

impl<'a> Lexed<'a> {
    /// The number of tokens, not counting the final `EOF`.
    pub fn len(&self) -> usize {
        self.kind.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The kind of token `i`. Token `len()` is `EOF`.
    pub fn kind(&self, i: usize) -> SyntaxKind {
        self.kind[i]
    }

    pub fn text_range(&self, i: usize) -> TextRange {
        TextRange::new(self.start[i], self.start[i + 1])
    }

    pub fn text(&self, i: usize) -> &'a str {
        self.text_range(i).slice(self.text)
    }

    pub fn errors(&self) -> &[LexError] {
        &self.errors
    }
}

/// Cut `text` into tokens.
pub fn lex(text: &str) -> Lexed<'_> {
    let mut cooker = Cooker {
        lexed: Lexed {
            text,
            kind: Vec::new(),
            start: Vec::new(),
            errors: Vec::new(),
        },
        offset: 0,
        holes: Vec::new(),
        newline_since_token: false,
        last_token: None,
    };
    let mut cursor = Cursor::new(text);
    loop {
        // At a `}` that closes a hole, the string goes on.
        let in_hole = !cooker.holes.is_empty();
        let raw = if in_hole && cooker.at_hole_end(cursor.first()) {
            cursor.continue_string()
        } else {
            cursor.advance_token()
        };
        if raw.kind == RawKind::Eof {
            break;
        }
        cooker.cook(raw);
    }
    cooker.finish()
}

/// An interpolation hole that is still open: `"…${` was read, its `}`
/// wasn't yet.
struct Hole {
    /// Where its string starts, to point at it if it never closes.
    string_start: u32,
    /// How many `{` inside the hole are still open: only a `}` at depth
    /// 0 closes the hole.
    depth: u32,
}

/// Turns raw tokens into `Lexed`.
struct Cooker<'a> {
    lexed: Lexed<'a>,
    /// Where the next token starts.
    offset: u32,
    /// The holes open right now, innermost last. They nest:
    /// `"a${ "b${c}" }"`.
    holes: Vec<Hole>,
    /// Whether a line break was seen since the last token that isn't
    /// trivia.
    newline_since_token: bool,
    /// The last token that isn't trivia.
    last_token: Option<SyntaxKind>,
}

impl Cooker<'_> {
    fn cook(&mut self, raw: RawToken) {
        let range = TextRange::new(self.offset, self.offset + raw.len);
        let text = range.slice(self.lexed.text);
        let kind = match raw.kind {
            RawKind::Whitespace => WHITESPACE,
            RawKind::LineComment => COMMENT,
            RawKind::BlockComment { terminated } => {
                if !terminated {
                    self.error("this comment never finds its `*/`", range);
                }
                COMMENT
            }
            RawKind::Ident => ident_kind(text),
            RawKind::Int => INT_NUMBER,
            RawKind::Float => FLOAT_NUMBER,
            RawKind::Str {
                terminated,
                opens_hole,
            } => {
                self.string(text, range, terminated);
                if opens_hole {
                    self.holes.push(Hole {
                        string_start: range.start(),
                        depth: 0,
                    });
                    STRING_START
                } else {
                    STRING
                }
            }
            RawKind::StrContinuation {
                terminated,
                opens_hole,
            } => {
                self.string(text, range, terminated);
                let hole = self
                    .holes
                    .pop()
                    .expect("only a hole's `}` continues a string");
                if opens_hole {
                    self.holes.push(Hole { depth: 0, ..hole });
                    STRING_MIDDLE
                } else {
                    STRING_END
                }
            }
            RawKind::Punct => self.punct(text, range),
            RawKind::Unknown => {
                self.error(
                    format!("unrecognized character `{}`", text.escape_debug()),
                    range,
                );
                ERROR
            }
            RawKind::Eof => unreachable!("lex stops at the end of the text"),
        };
        if kind.is_trivia() {
            if text.contains('\n') {
                self.newline_since_token = true;
                if !self.holes.is_empty() {
                    self.error("a string's `${…}` must fit on one line", range);
                }
            }
        } else {
            self.newline_since_token = false;
            self.last_token = Some(kind);
        }
        self.push(kind, raw.len);
    }

    fn push(&mut self, kind: SyntaxKind, len: u32) {
        self.lexed.kind.push(kind);
        self.lexed.start.push(self.offset);
        self.offset += len;
    }

    fn error(&mut self, msg: impl Into<String>, range: TextRange) {
        self.lexed.errors.push(LexError {
            msg: msg.into(),
            range,
        });
    }

    /// Check a piece of string: its escapes, and that it ends.
    fn string(&mut self, text: &str, range: TextRange, terminated: bool) {
        todo!("Part 5b: string")
    }

    fn punct(&mut self, text: &str, range: TextRange) -> SyntaxKind {
        let kind =
            SyntaxKind::from_punct(text).expect("the raw lexer only reads known punctuation");
        if !self.holes.is_empty() {
            self.track_brace(kind);
        }
        if kind == L_ANGLE && self.starts_html(range) {
            self.error("HTML elements are not supported by this parser", range);
            return ERROR;
        }
        kind
    }

    /// Inside a hole: is the next character, `next`, the `}` that closes
    /// the innermost hole? It is if no `{` opened inside it is still open.
    fn at_hole_end(&self, next: char) -> bool {
        todo!("Part 5c: at_hole_end")
    }

    /// Inside a hole: count its braces, so that `at_hole_end` knows which
    /// `}` closes it.
    fn track_brace(&mut self, kind: SyntaxKind) {
        todo!("Part 5c: track_brace")
    }

    /// Does the `<` at `range` start an HTML element, as in
    /// `let x = <div>`? In Maxim it does when it's followed by a letter
    /// and an expression can start here: after a token that can't end
    /// one, or at the start of a line.
    fn starts_html(&self, range: TextRange) -> bool {
        todo!("Part 5d: starts_html")
    }
}

impl<'a> Cooker<'a> {
    fn finish(mut self) -> Lexed<'a> {
        if let Some(hole) = self.holes.first() {
            let range = TextRange::new(hole.string_start, self.offset);
            self.error("this string never closes", range);
        }
        self.lexed.kind.push(EOF);
        self.lexed.start.push(self.offset);
        // The extra entry: where the `EOF` token "ends".
        self.lexed.start.push(self.offset);
        self.lexed
    }
}

/// The kind of an identifier: a keyword, `_`, or a name. Uppercase names
/// are types, constructors and modules; lowercase names are the rest.
fn ident_kind(text: &str) -> SyntaxKind {
    todo!("Part 5a: ident_kind")
}

/// Can an expression end with a token of this kind? After one that
/// can't, an expression can start. The parser uses it too, for line
/// breaks.
pub(crate) fn can_end_expr(kind: SyntaxKind) -> bool {
    todo!("Part 5d: can_end_expr")
}

/// The escapes in a piece of string that Maxim doesn't know, as error
/// messages and ranges relative to the piece. Known escapes: `\n`,
/// `\t`, `\"`, `\\`, `\$` and `\u{…}` with 1 to 6 hex digits.
fn bad_escapes(text: &str) -> Vec<(String, TextRange)> {
    todo!("Part 5b: bad_escapes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::{Expect, expect};

    /// One token per line, then the errors.
    fn check(text: &str, expect: Expect) {
        let lexed = lex(text);
        let mut out = String::new();
        for i in 0..lexed.len() {
            out += &format!("{:?} {:?}\n", lexed.kind(i), lexed.text(i));
        }
        for error in lexed.errors() {
            out += &format!("error {}: {}\n", error.range, error.msg);
        }
        expect.assert_eq(&out);
    }

    #[test]
    fn keywords_names_and_underscores() {
        check(
            "module fn let match x Shape _ _x fnord",
            expect![[r#"
            MODULE_KW "module"
            WHITESPACE " "
            FN_KW "fn"
            WHITESPACE " "
            LET_KW "let"
            WHITESPACE " "
            MATCH_KW "match"
            WHITESPACE " "
            LOWER_IDENT "x"
            WHITESPACE " "
            UPPER_IDENT "Shape"
            WHITESPACE " "
            UNDERSCORE "_"
            WHITESPACE " "
            LOWER_IDENT "_x"
            WHITESPACE " "
            LOWER_IDENT "fnord"
        "#]],
        );
    }

    #[test]
    fn every_punctuation() {
        check(
            "( ) { } [ ] < > , : :: . .. = == != <= >= | || |> && ! -> => ? + ++ - * ** / ~/ %",
            expect![[r#"
                L_PAREN "("
                WHITESPACE " "
                R_PAREN ")"
                WHITESPACE " "
                L_CURLY "{"
                WHITESPACE " "
                R_CURLY "}"
                WHITESPACE " "
                L_BRACK "["
                WHITESPACE " "
                R_BRACK "]"
                WHITESPACE " "
                L_ANGLE "<"
                WHITESPACE " "
                R_ANGLE ">"
                WHITESPACE " "
                COMMA ","
                WHITESPACE " "
                COLON ":"
                WHITESPACE " "
                COLON2 "::"
                WHITESPACE " "
                DOT "."
                WHITESPACE " "
                DOT2 ".."
                WHITESPACE " "
                EQ "="
                WHITESPACE " "
                EQ2 "=="
                WHITESPACE " "
                NEQ "!="
                WHITESPACE " "
                LTEQ "<="
                WHITESPACE " "
                GTEQ ">="
                WHITESPACE " "
                PIPE "|"
                WHITESPACE " "
                PIPE2 "||"
                WHITESPACE " "
                PIPE_GT "|>"
                WHITESPACE " "
                AMP2 "&&"
                WHITESPACE " "
                BANG "!"
                WHITESPACE " "
                THIN_ARROW "->"
                WHITESPACE " "
                FAT_ARROW "=>"
                WHITESPACE " "
                QUESTION "?"
                WHITESPACE " "
                PLUS "+"
                WHITESPACE " "
                PLUS2 "++"
                WHITESPACE " "
                MINUS "-"
                WHITESPACE " "
                STAR "*"
                WHITESPACE " "
                STAR2 "**"
                WHITESPACE " "
                SLASH "/"
                WHITESPACE " "
                TILDE_SLASH "~/"
                WHITESPACE " "
                PERCENT "%"
            "#]],
        );
    }

    #[test]
    fn numbers_strings_and_comments() {
        check(
            "42 3.14 \"hi\" // note\n/* block */",
            expect![[r#"
            INT_NUMBER "42"
            WHITESPACE " "
            FLOAT_NUMBER "3.14"
            WHITESPACE " "
            STRING "\"hi\""
            WHITESPACE " "
            COMMENT "// note"
            WHITESPACE "\n"
            COMMENT "/* block */"
        "#]],
        );
    }

    #[test]
    fn unknown_characters_are_errors() {
        check(
            "a @ b ; c\rd",
            expect![[r#"
            LOWER_IDENT "a"
            WHITESPACE " "
            ERROR "@"
            WHITESPACE " "
            LOWER_IDENT "b"
            WHITESPACE " "
            ERROR ";"
            WHITESPACE " "
            LOWER_IDENT "c"
            ERROR "\r"
            LOWER_IDENT "d"
            error 2..3: unrecognized character `@`
            error 6..7: unrecognized character `;`
            error 9..10: unrecognized character `\r`
        "#]],
        );
    }

    #[test]
    fn unterminated_comments_and_strings_are_errors() {
        check(
            "\"open\nx /* never",
            expect![[r#"
            STRING "\"open"
            WHITESPACE "\n"
            LOWER_IDENT "x"
            WHITESPACE " "
            COMMENT "/* never"
            error 0..5: this string never closes
            error 8..16: this comment never finds its `*/`
        "#]],
        );
    }

    #[test]
    fn bad_escapes_are_errors() {
        check(
            r#""ok\n\t\"\\\$ \q \u{1F389} \u{} \u{110000} \u{1234567} \u12""#,
            expect![[r#"
            STRING "\"ok\\n\\t\\\"\\\\\\$ \\q \\u{1F389} \\u{} \\u{110000} \\u{1234567} \\u12\""
            error 14..16: unknown escape `\q`
            error 27..31: invalid unicode escape
            error 32..42: invalid unicode escape
            error 43..54: invalid unicode escape
            error 55..57: invalid unicode escape
        "#]],
        );
    }

    #[test]
    fn interpolated_strings() {
        check(
            r#""a${x}b${y + 1}c""#,
            expect![[r#"
            STRING_START "\"a${"
            LOWER_IDENT "x"
            STRING_MIDDLE "}b${"
            LOWER_IDENT "y"
            WHITESPACE " "
            PLUS "+"
            WHITESPACE " "
            INT_NUMBER "1"
            STRING_END "}c\""
        "#]],
        );
    }

    #[test]
    fn holes_nest_and_count_braces() {
        check(
            r#""a${ { b } }c${ "d${e}f" }g""#,
            expect![[r#"
            STRING_START "\"a${"
            WHITESPACE " "
            L_CURLY "{"
            WHITESPACE " "
            LOWER_IDENT "b"
            WHITESPACE " "
            R_CURLY "}"
            WHITESPACE " "
            STRING_MIDDLE "}c${"
            WHITESPACE " "
            STRING_START "\"d${"
            LOWER_IDENT "e"
            STRING_END "}f\""
            WHITESPACE " "
            STRING_END "}g\""
        "#]],
        );
    }

    #[test]
    fn a_hole_that_never_closes() {
        check(
            r#""a${x"#,
            expect![[r#"
            STRING_START "\"a${"
            LOWER_IDENT "x"
            error 0..5: this string never closes
        "#]],
        );
    }

    #[test]
    fn a_hole_must_fit_on_one_line() {
        check(
            "\"a${x\n}\"",
            expect![[r#"
            STRING_START "\"a${"
            LOWER_IDENT "x"
            WHITESPACE "\n"
            STRING_END "}\""
            error 5..6: a string's `${…}` must fit on one line
        "#]],
        );
    }

    #[test]
    fn a_closing_brace_outside_holes_is_just_a_brace() {
        check(
            "} \"a\" }",
            expect![[r#"
            R_CURLY "}"
            WHITESPACE " "
            STRING "\"a\""
            WHITESPACE " "
            R_CURLY "}"
        "#]],
        );
    }

    #[test]
    fn angle_brackets_where_an_expression_can_start_are_html() {
        check(
            "x = <div",
            expect![[r#"
            LOWER_IDENT "x"
            WHITESPACE " "
            EQ "="
            WHITESPACE " "
            ERROR "<"
            LOWER_IDENT "div"
            error 4..5: HTML elements are not supported by this parser
        "#]],
        );
        check(
            "a <b",
            expect![[r#"
            LOWER_IDENT "a"
            WHITESPACE " "
            L_ANGLE "<"
            LOWER_IDENT "b"
        "#]],
        );
        check(
            "a\n<b",
            expect![[r#"
            LOWER_IDENT "a"
            WHITESPACE "\n"
            ERROR "<"
            LOWER_IDENT "b"
            error 2..3: HTML elements are not supported by this parser
        "#]],
        );
        check(
            "List<a>",
            expect![[r#"
            UPPER_IDENT "List"
            L_ANGLE "<"
            LOWER_IDENT "a"
            R_ANGLE ">"
        "#]],
        );
        check(
            "f(<x",
            expect![[r#"
            LOWER_IDENT "f"
            L_PAREN "("
            ERROR "<"
            LOWER_IDENT "x"
            error 2..3: HTML elements are not supported by this parser
        "#]],
        );
        check(
            "a < 1",
            expect![[r#"
            LOWER_IDENT "a"
            WHITESPACE " "
            L_ANGLE "<"
            WHITESPACE " "
            INT_NUMBER "1"
        "#]],
        );
    }

    #[test]
    fn lexing_loses_nothing() {
        let text = "module M\r\n\n/* c */ let x = \"a${ b }c\" @ é";
        let lexed = lex(text);
        let joined: String = (0..lexed.len()).map(|i| lexed.text(i)).collect();
        assert_eq!(joined, text);
        assert_eq!(lexed.kind(lexed.len()), EOF);
        assert_eq!(
            lexed.text_range(lexed.len()),
            TextRange::empty(text.len() as u32)
        );
    }
}
