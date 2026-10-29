//! Tokens and trivia: the lossless unit of Maxim syntax.
//!
//! Every token carries its leading trivia (whitespace and comments before
//! it, possibly spanning newlines) and trailing trivia (same-line only, by
//! construction — the PureScript CST rule). Concatenating
//! `leading + text + trailing` over a token stream reproduces the source
//! byte for byte; a property test enforces this forever.

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewlineKind {
    Lf,
    CrLf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trivia {
    Spaces(u32),
    Tabs(u32),
    /// Only ever appears in leading trivia (trailing trivia stops at the
    /// first newline).
    Newline(NewlineKind),
    /// `// ...` without the newline.
    LineComment(String),
    /// `/* ... */` verbatim, may contain newlines. Non-nesting (as in
    /// JS/TS).
    BlockComment(String),
}

impl Trivia {
    pub fn write_text(&self, out: &mut String) {
        match self {
            Trivia::Spaces(n) => out.extend(std::iter::repeat_n(' ', *n as usize)),
            Trivia::Tabs(n) => out.extend(std::iter::repeat_n('\t', *n as usize)),
            Trivia::Newline(NewlineKind::Lf) => out.push('\n'),
            Trivia::Newline(NewlineKind::CrLf) => out.push_str("\r\n"),
            Trivia::LineComment(t) => out.push_str(t),
            Trivia::BlockComment(t) => out.push_str(t),
        }
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        self.write_text(&mut s);
        s
    }

    /// Does rendering this trivium move to another line?
    pub fn crosses_line(&self) -> bool {
        match self {
            Trivia::Newline(_) => true,
            Trivia::BlockComment(t) => t.contains('\n'),
            Trivia::Spaces(_) | Trivia::Tabs(_) | Trivia::LineComment(_) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
    Pow,
    Concat,
    Eq,
    Neq,
    Le,
    Ge,
    And,
    Or,
}

impl Operator {
    pub fn text(self) -> &'static str {
        match self {
            Operator::Add => "+",
            Operator::Sub => "-",
            Operator::Mul => "*",
            Operator::Div => "/",
            Operator::IntDiv => "~/",
            Operator::Mod => "%",
            Operator::Pow => "**",
            Operator::Concat => "++",
            Operator::Eq => "==",
            Operator::Neq => "!=",
            Operator::Le => "<=",
            Operator::Ge => ">=",
            Operator::And => "&&",
            Operator::Or => "||",
        }
    }
}

/// A non-negative integer literal of arbitrary precision (the Haskell
/// compiler uses `Integer`), kept as its canonical decimal rendering: no
/// underscores, no leading zeros. Equality/ordering are numeric.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntLit {
    norm: String,
}

impl IntLit {
    /// From a raw lexeme: digits and `_` separators.
    pub fn from_raw(raw: &str) -> IntLit {
        let digits: String = raw.chars().filter(|c| *c != '_').collect();
        let trimmed = digits.trim_start_matches('0');
        IntLit {
            norm: if trimmed.is_empty() {
                "0".to_string()
            } else {
                trimmed.to_string()
            },
        }
    }

    /// The canonical decimal rendering (what Haskell's `show` produces).
    pub fn as_str(&self) -> &str {
        &self.norm
    }

    /// The value as an f64, for `Int -> Float` style needs. Large values
    /// lose precision exactly as JS number literals would.
    pub fn to_f64(&self) -> f64 {
        self.norm.parse().unwrap_or(f64::INFINITY)
    }
}

impl PartialOrd for IntLit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IntLit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // No leading zeros, so longer means larger; same length compares
        // lexicographically.
        (self.norm.len(), &self.norm).cmp(&(other.norm.len(), &other.norm))
    }
}

impl std::fmt::Display for IntLit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.norm)
    }
}

/// An `f64` with total equality/ordering/hashing via its bit pattern.
/// The lexer only produces finite values, where bitwise equality matches
/// Haskell's `Double` equality.
#[derive(Clone, Copy, Debug)]
pub struct F64(pub f64);

impl PartialEq for F64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for F64 {}

impl std::hash::Hash for F64 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl F64 {
    /// Rendering in the spirit of Haskell's `show Double`: always
    /// float-shaped (`1.0`, never a bare `1`), so an emitted JS literal
    /// stays a float literal. GHC switches to scientific notation at
    /// different thresholds; that drift is within the behavioral bar.
    pub fn show(&self) -> String {
        let s = format!("{}", self.0);
        if s.contains(['.', 'e', 'E']) || self.0.is_nan() || self.0.is_infinite() {
            s
        } else {
            format!("{s}.0")
        }
    }
}

impl PartialOrd for F64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for F64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Lower(String),
    Upper(String),
    /// A lone `_`.
    Hole,
    Int(IntLit),
    Float(F64),
    /// Decoded payload; the raw source slice (with quotes and escapes)
    /// lives in `Token::text`.
    Str(String),
    /// `"chunk${` — an interpolated string opens: the decoded chunk
    /// before the first hole. The hole's expression follows as ordinary
    /// tokens.
    StringBegin(String),
    /// `}chunk${` — a hole closes and another opens.
    StringMid(String),
    /// `}chunk"` — the last hole closes and the string ends.
    StringEnd(String),
    /// `<div`, `<my-tag`, `<Card` — a JSX element opens. The payload is
    /// the tag name; the raw slice (with the `<`) lives in `Token::text`.
    /// A lowercase tag is an intrinsic element, an uppercase one names a
    /// component module.
    JsxTagStart(String),
    /// `</div>`, bundled as one token; the payload is the tag name.
    JsxClose(String),
    /// The `>` that ends an open tag: children follow.
    JsxTagEnd,
    /// `/>` — the element is complete, no children.
    JsxSelfClose,
    /// An attribute name in tag position: `class`, `data-id`,
    /// `on:click` — keywords like `type` are plain names here.
    JsxAttrName(String),
    /// A run of element text. Decoded payload (may span lines); the raw
    /// slice (escapes intact) lives in `Token::text`.
    JsxText(String),
    /// `${` inside element text — a String hole opens, closed by a
    /// plain `}`.
    DollarBrace,
    Module,
    Import,
    Type,
    Fn,
    Cast,
    Component,
    Let,
    Match,
    If,
    Else,
    LParen,
    /// A `(` whose leading trivia crosses a newline. The grammar's call
    /// rule accepts only `LParen` (a call's paren must sit on the same
    /// line as the callee — the JS ASI hazard, solved Kotlin-style);
    /// grouping and lambda rules accept both.
    LParenBOL,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    /// `::` — the cast operator.
    ColonColon,
    Dot,
    DotDot,
    Eq,
    Bar,
    Bang,
    Arrow,
    FatArrow,
    Pipeline,
    /// Postfix `?`: unwrap a Result/Option or early-return its failure.
    Question,
    /// `<` and `>` are their own kinds: comparison operators in
    /// expressions, generic brackets in types. The contexts are disjoint
    /// in v0 (no type ascription expressions).
    Lt,
    Gt,
    Op(Operator),
    /// Carries the file's final trivia as leading.
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub leading: Vec<Trivia>,
    pub kind: TokenKind,
    /// Exact source slice (keeps the exact printer trivial and literals
    /// exact).
    pub text: String,
    /// Span of the token proper, excluding trivia.
    pub span: Span,
    /// Never contains `Trivia::Newline`.
    pub trailing: Vec<Trivia>,
}

impl Token {
    /// Leading + text + trailing: the lossless rendering of one token.
    pub fn write_full_text(&self, out: &mut String) {
        for tr in &self.leading {
            tr.write_text(out);
        }
        out.push_str(&self.text);
        for tr in &self.trailing {
            tr.write_text(out);
        }
    }

    pub fn full_text(&self) -> String {
        let mut s = String::new();
        self.write_full_text(&mut s);
        s
    }
}

/// What a matcher rule tells the driver in `lexer::mod`. Keeping actions
/// as plain data keeps the matcher dumb and the real logic in testable
/// code (the same split the Haskell keeps between Alex.x and Lexer.hs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Plain(TokenKind),
    /// An attribute name matched in tag mode (payload from the lexeme).
    JsxAttrName,
    Lower,
    Upper,
    Hole,
    Int,
    Float,
    Spaces,
    Tabs,
    NewlineLf,
    NewlineCrLf,
    LineComment,
    BeginString,
    BeginBlockComment,
}
