//! Raw token recognition: the hand-written replacement for the alex DFA.
//! The scan loop, trivia attachment, string/block-comment scanning, and
//! error reporting live in `lexer::mod`; this module only knows how to
//! match the next lexeme.
//!
//! The cursor walks UTF-8 bytes while tracking positions in Unicode code
//! points (continuation bytes do not advance the column).

use crate::lexer::token::{Action, Operator, TokenKind};

/// Position-tracking input cursor. Lines/columns are 1-based code-point
/// counts; `cp` is the code-point offset, `byte` the byte offset into the
/// full buffer (used to slice exact token text).
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    pub line: u32,
    pub col: u32,
    pub cp: u32,
    pub byte: usize,
}

impl Cursor {
    pub fn start() -> Cursor {
        Cursor {
            line: 1,
            col: 1,
            cp: 0,
            byte: 0,
        }
    }
}

/// The analogue of `alexGetByte`: step one byte, advancing the code-point
/// position only on non-continuation bytes.
pub fn get_byte(buf: &[u8], c: Cursor) -> Option<(u8, Cursor)> {
    let b = *buf.get(c.byte)?;
    let is_continuation = (0x80..0xC0).contains(&b);
    let (line, col, cp) = if is_continuation {
        (c.line, c.col, c.cp)
    } else if b == 0x0A {
        (c.line + 1, 1, c.cp + 1)
    } else {
        (c.line, c.col + 1, c.cp + 1)
    };
    Some((
        b,
        Cursor {
            line,
            col,
            cp,
            byte: c.byte + 1,
        },
    ))
}

pub fn peek_byte(buf: &[u8], c: Cursor) -> Option<u8> {
    get_byte(buf, c).map(|(b, _)| b)
}

/// Which rule set applies: normal code or the inside of a JSX open tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Tag,
}

/// The result of one matcher step (the analogue of `AlexReturn`; there
/// are no skip rules, so no `AlexSkip`).
pub enum Scan {
    Eof,
    /// No rule matched. The cursor is where the DFA died: after any
    /// partially-matched prefix (`~` without `/`, `&` without `&`, a
    /// lone `\r`, tag-mode `/` without `>`), matching alex's error
    /// position exactly.
    Error(Cursor),
    /// A lexeme was matched: the cursor past it and its action.
    Token(Cursor, Action),
}

pub fn is_tag_letter(b: u8) -> bool {
    is_lower_byte(b) || is_upper_byte(b)
}

pub fn is_lower_byte(b: u8) -> bool {
    b.is_ascii_lowercase()
}

pub fn is_upper_byte(b: u8) -> bool {
    b.is_ascii_uppercase()
}

pub fn is_digit_byte(b: u8) -> bool {
    b.is_ascii_digit()
}

pub fn is_attr_tail_byte(b: u8) -> bool {
    is_tag_letter(b) || is_digit_byte(b) || b == b'-'
}

pub fn is_ident_tail_byte(b: u8) -> bool {
    is_tag_letter(b) || is_digit_byte(b) || b == b'_'
}

/// Step the cursor over `n` bytes that are known to be ASCII.
fn ascii_advance(buf: &[u8], mut c: Cursor, n: usize) -> Cursor {
    for _ in 0..n {
        c = get_byte(buf, c).expect("ascii_advance past end").1;
    }
    c
}

/// Consume a maximal run of bytes satisfying `keep`, starting at `c`.
fn walk(buf: &[u8], mut c: Cursor, keep: impl Fn(u8) -> bool) -> Cursor {
    while let Some((b, c2)) = get_byte(buf, c) {
        if keep(b) {
            c = c2;
        } else {
            break;
        }
    }
    c
}

/// Match the next lexeme at `c` (maximal munch, first-rule-wins on ties),
/// mirroring the rule set in the Haskell `Alex.x`.
pub fn next_lexeme(buf: &[u8], c: Cursor, mode: Mode) -> Scan {
    use Action::{
        BeginBlockComment, BeginString, Float, Hole, Int, JsxAttrName, LineComment, Lower,
        NewlineCrLf, NewlineLf, Plain, Spaces, Tabs, Upper,
    };
    use TokenKind::{
        Arrow, Bang, Bar, Colon, ColonColon, Comma, Dot, DotDot, FatArrow, Gt, JsxSelfClose,
        JsxTagEnd, LBrace, LBracket, LParen, Lt, Op, Pipeline, Question, RBrace, RBracket, RParen,
    };

    let Some((b, c1)) = get_byte(buf, c) else {
        return Scan::Eof;
    };
    let peek1 = peek_byte(buf, c1);

    // Two-byte token helper: consume the second byte too.
    let two = |action: Action| Scan::Token(ascii_advance(buf, c1, 1), action);
    let one = |action: Action| Scan::Token(c1, action);

    // Trivia and string openings are shared between normal code and JSX
    // tag mode (attributes may span lines and take string values).
    match b {
        b'\r' => {
            return if peek1 == Some(b'\n') {
                two(NewlineCrLf)
            } else {
                // Partial match: the DFA consumed the `\r` before dying.
                Scan::Error(c1)
            };
        }
        b'\n' => return one(NewlineLf),
        b' ' => return Scan::Token(walk(buf, c1, |b| b == b' '), Spaces),
        b'\t' => return Scan::Token(walk(buf, c1, |b| b == b'\t'), Tabs),
        b'/' if peek1 == Some(b'/') => {
            let end = walk(buf, ascii_advance(buf, c1, 1), |b| b != b'\n' && b != b'\r');
            return Scan::Token(end, LineComment);
        }
        b'/' if peek1 == Some(b'*') => return two(BeginBlockComment),
        b'"' => return one(BeginString),
        _ => {}
    }

    match mode {
        Mode::Normal => match b {
            b'0'..=b'9' => {
                let int_end = walk(buf, c1, |b| is_digit_byte(b) || b == b'_');
                // Float: `digits [digits _]* . digits [digits _]*` — the
                // dot must be followed by a digit, else backtrack to int.
                if peek_byte(buf, int_end) == Some(b'.') {
                    let after_dot = ascii_advance(buf, int_end, 1);
                    if peek_byte(buf, after_dot).is_some_and(is_digit_byte) {
                        let frac_end = walk(buf, after_dot, |b| is_digit_byte(b) || b == b'_');
                        return Scan::Token(frac_end, Float);
                    }
                }
                Scan::Token(int_end, Int)
            }
            b'_' => {
                let end = walk(buf, c1, is_ident_tail_byte);
                if end.byte > c1.byte {
                    Scan::Token(end, Lower)
                } else {
                    one(Hole)
                }
            }
            b'a'..=b'z' => Scan::Token(walk(buf, c1, is_ident_tail_byte), Lower),
            b'A'..=b'Z' => Scan::Token(walk(buf, c1, is_ident_tail_byte), Upper),
            b'|' => match peek1 {
                Some(b'>') => two(Plain(Pipeline)),
                Some(b'|') => two(Plain(Op(Operator::Or))),
                _ => one(Plain(Bar)),
            },
            b'-' => match peek1 {
                Some(b'>') => two(Plain(Arrow)),
                _ => one(Plain(Op(Operator::Sub))),
            },
            b'=' => match peek1 {
                Some(b'>') => two(Plain(FatArrow)),
                Some(b'=') => two(Plain(Op(Operator::Eq))),
                _ => one(Plain(TokenKind::Eq)),
            },
            b'!' => match peek1 {
                Some(b'=') => two(Plain(Op(Operator::Neq))),
                _ => one(Plain(Bang)),
            },
            b'<' => match peek1 {
                Some(b'=') => two(Plain(Op(Operator::Le))),
                _ => one(Plain(Lt)),
            },
            b'>' => match peek1 {
                Some(b'=') => two(Plain(Op(Operator::Ge))),
                _ => one(Plain(Gt)),
            },
            b'&' => match peek1 {
                Some(b'&') => two(Plain(Op(Operator::And))),
                // Partial match: the DFA consumed the `&` before dying.
                _ => Scan::Error(c1),
            },
            b'+' => match peek1 {
                Some(b'+') => two(Plain(Op(Operator::Concat))),
                _ => one(Plain(Op(Operator::Add))),
            },
            b'*' => match peek1 {
                Some(b'*') => two(Plain(Op(Operator::Pow))),
                _ => one(Plain(Op(Operator::Mul))),
            },
            b'~' => match peek1 {
                Some(b'/') => two(Plain(Op(Operator::IntDiv))),
                // Partial match: the DFA consumed the `~` before dying.
                _ => Scan::Error(c1),
            },
            b'/' => one(Plain(Op(Operator::Div))),
            b'%' => one(Plain(Op(Operator::Mod))),
            b'?' => one(Plain(Question)),
            b':' => match peek1 {
                Some(b':') => two(Plain(ColonColon)),
                _ => one(Plain(Colon)),
            },
            b'.' => match peek1 {
                Some(b'.') => two(Plain(DotDot)),
                _ => one(Plain(Dot)),
            },
            b',' => one(Plain(Comma)),
            b'(' => one(Plain(LParen)),
            b')' => one(Plain(RParen)),
            b'{' => one(Plain(LBrace)),
            b'}' => one(Plain(RBrace)),
            b'[' => one(Plain(LBracket)),
            b']' => one(Plain(RBracket)),
            _ => Scan::Error(c),
        },
        Mode::Tag => match b {
            b'a'..=b'z' => {
                let seg_end = attr_segment(buf, c1);
                // Optional `:` joining a second segment (`on:click`).
                let end = match peek_byte(buf, seg_end) {
                    Some(b':') => {
                        let after_colon = ascii_advance(buf, seg_end, 1);
                        match get_byte(buf, after_colon) {
                            Some((b2, c2)) if is_lower_byte(b2) => attr_segment(buf, c2),
                            _ => seg_end,
                        }
                    }
                    _ => seg_end,
                };
                Scan::Token(end, JsxAttrName)
            }
            b'=' => one(Plain(TokenKind::Eq)),
            b'{' => one(Plain(LBrace)),
            b'>' => one(Plain(JsxTagEnd)),
            b'/' if peek1 == Some(b'>') => two(Plain(JsxSelfClose)),
            // Partial match: the DFA consumed the `/` before dying.
            b'/' => Scan::Error(c1),
            _ => Scan::Error(c),
        },
    }
}

/// The tail of an attribute-name segment after its leading lowercase
/// letter: `$attrTail*` backing off so the match never ends with a dash.
fn attr_segment(buf: &[u8], after_first: Cursor) -> Cursor {
    let mut end = after_first;
    let mut c = after_first;
    while let Some((b, c2)) = get_byte(buf, c) {
        if !is_attr_tail_byte(b) {
            break;
        }
        c = c2;
        if b != b'-' {
            end = c2;
        }
    }
    end
}
