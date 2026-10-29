//! The lexer driver: runs the hand-written matcher, hand-scans strings,
//! block comments, and JSX element text (for precise diagnostics), then
//! folds trivia onto tokens (leading/trailing, PureScript-style) and
//! splits line-initial `(` into `LParenBOL`.
//!
//! JSX is lexed with a mode stack (`Frame`). A `<` in normal mode starts
//! an element exactly where an expression can begin: when the previous
//! meaningful token cannot end an expression, or when the `<` is
//! line-initial (layout rule 3 — the `LParenBOL` move). Tag mode is a
//! matcher mode; element text is hand-scanned. String-interpolation
//! holes and `${…}` holes in element text are single-line; plain `{…}`
//! splices may span lines.

pub mod cursor;
pub mod token;

use crate::diag::Diagnostic;
use crate::span::{FileId, Pos, Span};
use cursor::{Cursor, Mode, Scan, get_byte, is_tag_letter, next_lexeme, peek_byte};
use token::{Action, F64, IntLit, NewlineKind, Token, TokenKind, Trivia};

/// The lexer's mode stack. The top decides how the next lexeme is read:
/// an open hole scans normally (with brace tracking), a tag scans in tag
/// mode, children are hand-scanned text.
#[derive(Clone, Debug)]
enum Frame {
    /// A string's `${…}` hole: the opening quote (blame for an
    /// unterminated literal) and the brace nesting inside the hole.
    Interp(Cursor, u32),
    /// A `{…}` or `${…}` hole inside a JSX tag or its children: the
    /// opening brace, the nesting depth, and whether it is a `${` String
    /// hole (single-line, like interpolation).
    Brace(Cursor, u32, bool),
    /// Between `<name` and the `>` / `/>` that ends the open tag: the
    /// `<` position and the tag name.
    Tag(Cursor, String),
    /// Between `>` and `</name>`: the `<` position and the tag name.
    Children(Cursor, String),
}

/// Frames that forbid line breaks anywhere inside them.
fn bans_newline(f: &Frame) -> bool {
    match f {
        Frame::Interp(_, _) => true,
        Frame::Brace(_, _, dollar) => *dollar,
        Frame::Tag(_, _) | Frame::Children(_, _) => false,
    }
}

/// How a string chunk ended: at its closing quote, or at a `${` that
/// opens an interpolation hole.
enum ChunkEnd {
    Quote,
    Interp,
}

/// What stopped a run of JSX element text.
enum JsxStop {
    Tag,
    Brace,
    DollarBrace,
}

/// An intermediate scan item, before trivia attachment.
enum Item {
    Trivia(Trivia),
    Token(Span, TokenKind, String),
}

/// Lex a whole file. Fail-fast on the first lexical error (the parser is
/// fail-fast per file too; the driver still surfaces errors from other
/// modules in the same run).
pub fn lex_tokens(fid: FileId, src: &str) -> Result<Vec<Token>, Diagnostic> {
    Lx {
        fid,
        buf: src.as_bytes(),
    }
    .run()
}

struct Lx<'a> {
    fid: FileId,
    buf: &'a [u8],
}

impl<'a> Lx<'a> {
    fn pos_of(&self, c: Cursor) -> Pos {
        Pos::new(c.line, c.col, c.cp)
    }

    fn span_between(&self, a: Cursor, b: Cursor) -> Span {
        Span::new(self.fid, self.pos_of(a), self.pos_of(b))
    }

    fn slice_text(&self, a: Cursor, b: Cursor) -> String {
        String::from_utf8_lossy(&self.buf[a.byte..b.byte]).into_owned()
    }

    fn get(&self, c: Cursor) -> Option<(u8, Cursor)> {
        get_byte(self.buf, c)
    }

    fn peek(&self, c: Cursor) -> Option<u8> {
        peek_byte(self.buf, c)
    }

    /// Step one byte forward; total (the stopper's presence is already
    /// established by the scanner that returned it).
    fn advance1(&self, c: Cursor) -> Cursor {
        self.get(c).map_or(c, |(_, c2)| c2)
    }

    fn run(&self) -> Result<Vec<Token>, Diagnostic> {
        let mut inp = Cursor::start();
        let mut stack: Vec<Frame> = Vec::new();
        let mut acc: Vec<Item> = Vec::new();

        loop {
            if let Some(Frame::Children(open, name)) = stack.last() {
                let (open, name) = (*open, name.clone());
                inp = self.children(inp, &mut stack, &mut acc, open, &name)?;
                continue;
            }
            let mode = match stack.last() {
                Some(Frame::Tag(_, _)) => Mode::Tag,
                _ => Mode::Normal,
            };
            match next_lexeme(self.buf, inp, mode) {
                Scan::Eof => {
                    return match stack.last() {
                        // Children cannot reach here: the guard above
                        // diverts it to the hand scanner, which reports
                        // its own EOF.
                        Some(Frame::Interp(open, _)) => Err(self.unterm_string(*open, inp)),
                        Some(Frame::Brace(open, _, dollar)) => {
                            Err(self.unterm_hole(*open, *dollar, inp))
                        }
                        Some(Frame::Tag(open, name)) => Err(self.unterm_tag(*open, name, inp)),
                        _ => {
                            let eof_span = self.span_between(inp, inp);
                            Ok(fold_trivia(eof_span, acc))
                        }
                    };
                }
                Scan::Error(err_c) => return Err(self.unexpected_char(err_c)),
                Scan::Token(inp2, action) => {
                    let sp = self.span_between(inp, inp2);
                    let txt = self.slice_text(inp, inp2);
                    match action {
                        // Inside a hole, braces nest; the `}` that brings
                        // the depth back to 0 closes the hole. An
                        // interpolation hole's closer resumes the string
                        // literal; a JSX hole's closer is an ordinary `}`
                        // token. A `{` under a tag frame arrived via tag
                        // mode: an attribute hole.
                        Action::Plain(TokenKind::LBrace) => {
                            match stack.last_mut() {
                                Some(Frame::Interp(_, d)) => *d += 1,
                                Some(Frame::Brace(_, d, _)) => *d += 1,
                                Some(Frame::Tag(_, _)) => stack.push(Frame::Brace(inp, 0, false)),
                                _ => {}
                            }
                            acc.push(Item::Token(sp, TokenKind::LBrace, txt));
                            inp = inp2;
                        }
                        Action::Plain(TokenKind::RBrace) => match stack.last_mut() {
                            Some(Frame::Interp(open, d)) => {
                                if *d > 0 {
                                    *d -= 1;
                                    acc.push(Item::Token(sp, TokenKind::RBrace, txt));
                                    inp = inp2;
                                } else {
                                    let open = *open;
                                    stack.pop();
                                    let (inp3, payload, end) = self.scan_chunk(open, inp2)?;
                                    let sp2 = self.span_between(inp, inp3);
                                    let raw = self.slice_text(inp, inp3);
                                    match end {
                                        ChunkEnd::Quote => {
                                            acc.push(Item::Token(
                                                sp2,
                                                TokenKind::StringEnd(payload),
                                                raw,
                                            ));
                                        }
                                        ChunkEnd::Interp => {
                                            stack.push(Frame::Interp(open, 0));
                                            acc.push(Item::Token(
                                                sp2,
                                                TokenKind::StringMid(payload),
                                                raw,
                                            ));
                                        }
                                    }
                                    inp = inp3;
                                }
                            }
                            Some(Frame::Brace(_, d, _)) => {
                                if *d > 0 {
                                    *d -= 1;
                                } else {
                                    stack.pop();
                                }
                                acc.push(Item::Token(sp, TokenKind::RBrace, txt));
                                inp = inp2;
                            }
                            _ => {
                                acc.push(Item::Token(sp, TokenKind::RBrace, txt));
                                inp = inp2;
                            }
                        },
                        // Tag mode ends: `>` opens the children (same
                        // element, new frame), `/>` completes the element.
                        Action::Plain(TokenKind::JsxTagEnd)
                            if matches!(stack.last(), Some(Frame::Tag(_, _))) =>
                        {
                            let Some(Frame::Tag(open, name)) = stack.pop() else {
                                unreachable!()
                            };
                            stack.push(Frame::Children(open, name));
                            acc.push(Item::Token(sp, TokenKind::JsxTagEnd, txt));
                            inp = inp2;
                        }
                        Action::Plain(TokenKind::JsxSelfClose)
                            if matches!(stack.last(), Some(Frame::Tag(_, _))) =>
                        {
                            stack.pop();
                            acc.push(Item::Token(sp, TokenKind::JsxSelfClose, txt));
                            inp = inp2;
                        }
                        // `<` starts an element exactly where an
                        // expression can begin; everywhere else it is the
                        // comparison operator.
                        Action::Plain(TokenKind::Lt)
                            if self.peek(inp2).is_some_and(is_tag_letter) && jsx_starts(&acc) =>
                        {
                            inp = self.start_tag(inp, inp2, &mut stack, &mut acc)?;
                        }
                        Action::Plain(k) => {
                            acc.push(Item::Token(sp, k, txt));
                            inp = inp2;
                        }
                        Action::JsxAttrName => {
                            acc.push(Item::Token(sp, TokenKind::JsxAttrName(txt.clone()), txt));
                            inp = inp2;
                        }
                        Action::Lower => {
                            let k = keyword(&txt).unwrap_or_else(|| TokenKind::Lower(txt.clone()));
                            acc.push(Item::Token(sp, k, txt));
                            inp = inp2;
                        }
                        Action::Upper => {
                            acc.push(Item::Token(sp, TokenKind::Upper(txt.clone()), txt));
                            inp = inp2;
                        }
                        Action::Hole => {
                            acc.push(Item::Token(sp, TokenKind::Hole, txt));
                            inp = inp2;
                        }
                        Action::Int => {
                            let n = IntLit::from_raw(&txt);
                            acc.push(Item::Token(sp, TokenKind::Int(n), txt));
                            inp = inp2;
                        }
                        Action::Float => {
                            let digits: String = txt.chars().filter(|c| *c != '_').collect();
                            let d: f64 = digits.parse().expect("float lexeme parses");
                            acc.push(Item::Token(sp, TokenKind::Float(F64(d)), txt));
                            inp = inp2;
                        }
                        Action::Spaces => {
                            acc.push(Item::Trivia(Trivia::Spaces(txt.len() as u32)));
                            inp = inp2;
                        }
                        Action::Tabs => {
                            acc.push(Item::Trivia(Trivia::Tabs(txt.len() as u32)));
                            inp = inp2;
                        }
                        Action::NewlineLf => {
                            if stack.iter().any(bans_newline) {
                                return Err(self.interp_newline(inp));
                            }
                            acc.push(Item::Trivia(Trivia::Newline(NewlineKind::Lf)));
                            inp = inp2;
                        }
                        Action::NewlineCrLf => {
                            if stack.iter().any(bans_newline) {
                                return Err(self.interp_newline(inp));
                            }
                            acc.push(Item::Trivia(Trivia::Newline(NewlineKind::CrLf)));
                            inp = inp2;
                        }
                        Action::LineComment => {
                            acc.push(Item::Trivia(Trivia::LineComment(txt)));
                            inp = inp2;
                        }
                        Action::BeginString => {
                            let (inp3, payload, end) = self.scan_chunk(inp, inp2)?;
                            let sp2 = self.span_between(inp, inp3);
                            let raw = self.slice_text(inp, inp3);
                            match end {
                                ChunkEnd::Quote => {
                                    acc.push(Item::Token(sp2, TokenKind::Str(payload), raw));
                                }
                                ChunkEnd::Interp => {
                                    stack.push(Frame::Interp(inp, 0));
                                    acc.push(Item::Token(sp2, TokenKind::StringBegin(payload), raw));
                                }
                            }
                            inp = inp3;
                        }
                        Action::BeginBlockComment => {
                            let inp3 = self.scan_block_comment(inp, inp2)?;
                            let ctext = self.slice_text(inp, inp3);
                            if stack.iter().any(bans_newline) && ctext.contains('\n') {
                                return Err(self.interp_newline(inp));
                            }
                            acc.push(Item::Trivia(Trivia::BlockComment(ctext)));
                            inp = inp3;
                        }
                    }
                }
            }
        }
    }

    /// The `<` was confirmed as a JSX start: scan the tag name and enter
    /// tag mode. `inp` sits at the `<`, `after_lt` just past it. Returns
    /// the cursor past the tag name.
    fn start_tag(
        &self,
        inp: Cursor,
        after_lt: Cursor,
        stack: &mut Vec<Frame>,
        acc: &mut Vec<Item>,
    ) -> Result<Cursor, Diagnostic> {
        let (inp2, name) = self.scan_tag_name(after_lt)?;
        let sp = self.span_between(inp, inp2);
        let raw = self.slice_text(inp, inp2);
        stack.push(Frame::Tag(inp, name.clone()));
        acc.push(Item::Token(sp, TokenKind::JsxTagStart(name), raw));
        Ok(inp2)
    }

    /// Tag names: a lowercase start is an intrinsic element (dashes
    /// allowed, SVG camelCase tolerated, no trailing dash); an uppercase
    /// start names a component module (identifier charset, no dots —
    /// modules don't nest).
    fn scan_tag_name(&self, i0: Cursor) -> Result<(Cursor, String), Diagnostic> {
        let walk = |mut i: Cursor, keep: fn(u8) -> bool| {
            while let Some((b, i2)) = self.get(i) {
                if keep(b) {
                    i = i2;
                } else {
                    break;
                }
            }
            i
        };
        match self.get(i0) {
            Some((b, i1)) if cursor::is_lower_byte(b) => {
                let i_end = walk(i1, cursor::is_attr_tail_byte);
                let name = self.slice_text(i0, i_end);
                if name.ends_with('-') {
                    Err(self.bad_tag(i0, i_end, "an element name cannot end with `-`", vec![]))
                } else {
                    Ok((i_end, name))
                }
            }
            Some((b, i1)) if cursor::is_upper_byte(b) => {
                let i_end = walk(i1, cursor::is_ident_tail_byte);
                if self.peek(i_end) == Some(b'.') {
                    Err(self.bad_tag(
                        i0,
                        i_end,
                        "a component tag is a single module name",
                        vec![
                            "Modules don't nest; a component is `<Card>`, and the tag calls `Card.view`."
                                .to_string(),
                        ],
                    ))
                } else {
                    Ok((i_end, self.slice_text(i0, i_end)))
                }
            }
            _ => Err(self.bad_tag(
                i0,
                i0,
                "an element tag starts with a letter",
                vec!["Write `\\<` for a literal `<` in element text.".to_string()],
            )),
        }
    }

    /// Element text: hand-scanned so escapes decode and errors blame
    /// precisely. A run ends at `<` (tag or closing tag), `{` (splice),
    /// or `${` (String hole); a bare `}` is an error, as is EOF. Returns
    /// the cursor to continue from.
    fn children(
        &self,
        inp: Cursor,
        stack: &mut Vec<Frame>,
        acc: &mut Vec<Item>,
        open: Cursor,
        name: &str,
    ) -> Result<Cursor, Diagnostic> {
        let banned = stack.iter().any(bans_newline);
        let (i_stop, payload, stop) = self.scan_jsx_text(banned, open, name, inp)?;
        if i_stop.byte != inp.byte {
            acc.push(Item::Token(
                self.span_between(inp, i_stop),
                TokenKind::JsxText(payload),
                self.slice_text(inp, i_stop),
            ));
        }
        match stop {
            JsxStop::Brace => {
                let i2 = self.advance1(i_stop);
                stack.push(Frame::Brace(i_stop, 0, false));
                acc.push(Item::Token(
                    self.span_between(i_stop, i2),
                    TokenKind::LBrace,
                    self.slice_text(i_stop, i2),
                ));
                Ok(i2)
            }
            JsxStop::DollarBrace => {
                let i2 = self.advance1(self.advance1(i_stop));
                stack.push(Frame::Brace(i_stop, 0, true));
                acc.push(Item::Token(
                    self.span_between(i_stop, i2),
                    TokenKind::DollarBrace,
                    self.slice_text(i_stop, i2),
                ));
                Ok(i2)
            }
            JsxStop::Tag => {
                let i1 = self.advance1(i_stop);
                match self.peek(i1) {
                    Some(b'/') => {
                        let (i3, close_name) = self.scan_close_tag(i_stop, self.advance1(i1))?;
                        if stack.pop().is_none() {
                            return Err(self.bad_tag(i_stop, i3, "no element is open here", vec![]));
                        }
                        acc.push(Item::Token(
                            self.span_between(i_stop, i3),
                            TokenKind::JsxClose(close_name),
                            self.slice_text(i_stop, i3),
                        ));
                        Ok(i3)
                    }
                    Some(b) if is_tag_letter(b) => self.start_tag(i_stop, i1, stack, acc),
                    _ => Err(self.bad_tag(
                        i_stop,
                        i1,
                        "an element tag starts with a letter",
                        vec!["Write `\\<` for a literal `<` in element text.".to_string()],
                    )),
                }
            }
        }
    }

    /// `</` was consumed; scan the tag name and the mandatory `>`.
    fn scan_close_tag(&self, from: Cursor, i0: Cursor) -> Result<(Cursor, String), Diagnostic> {
        let (i1, name) = self.scan_tag_name(i0)?;
        match self.get(i1) {
            Some((b'>', i2)) => Ok((i2, name)),
            _ => Err(self.bad_tag(
                from,
                i1,
                "a closing tag is `</name>`, nothing else inside",
                vec![],
            )),
        }
    }

    fn scan_jsx_text(
        &self,
        banned: bool,
        open: Cursor,
        name: &str,
        i0: Cursor,
    ) -> Result<(Cursor, String, JsxStop), Diagnostic> {
        let mut racc: Vec<u8> = Vec::new();
        let mut i = i0;
        loop {
            let Some((b, i2)) = self.get(i) else {
                return Err(self.unterm_element(open, name, i));
            };
            match b {
                b'<' => return Ok((i, decode(racc), JsxStop::Tag)),
                b'{' => return Ok((i, decode(racc), JsxStop::Brace)),
                b'}' => return Err(self.stray_brace(i)),
                b'$' => {
                    if self.peek(i2) == Some(b'{') {
                        return Ok((i, decode(racc), JsxStop::DollarBrace));
                    }
                    racc.push(b'$');
                    i = i2;
                }
                b'\n' | b'\r' if banned => return Err(self.interp_newline(i)),
                b'\\' => match self.get(i2) {
                    None => return Err(self.unterm_element(open, name, i2)),
                    Some((e, i3)) => match e {
                        b'n' => {
                            racc.push(b'\n');
                            i = i3;
                        }
                        b't' => {
                            racc.push(b'\t');
                            i = i3;
                        }
                        b'"' | b'\\' | b'$' | b'<' | b'{' | b'}' => {
                            racc.push(e);
                            i = i3;
                        }
                        b'u' => {
                            let (i4, cp) = self.scan_unicode(i, i3)?;
                            push_cp(&mut racc, cp);
                            i = i4;
                        }
                        _ => return Err(self.bad_escape(i, i3)),
                    },
                },
                _ => {
                    racc.push(b);
                    i = i2;
                }
            }
        }
    }

    /// Strings: single-line, escapes `\n \t \" \\ \$ \u{...}`. A chunk
    /// ends at the closing quote or at `${`; a lone `$` stays literal.
    fn scan_chunk(
        &self,
        open_inp: Cursor,
        i0: Cursor,
    ) -> Result<(Cursor, String, ChunkEnd), Diagnostic> {
        let mut racc: Vec<u8> = Vec::new();
        let mut i = i0;
        loop {
            let Some((b, i2)) = self.get(i) else {
                return Err(self.unterm_string(open_inp, i));
            };
            match b {
                b'"' => return Ok((i2, decode(racc), ChunkEnd::Quote)),
                b'\n' | b'\r' => return Err(self.unterm_string(open_inp, i)),
                b'$' => match self.get(i2) {
                    Some((b'{', i3)) => return Ok((i3, decode(racc), ChunkEnd::Interp)),
                    _ => {
                        racc.push(b'$');
                        i = i2;
                    }
                },
                b'\\' => match self.get(i2) {
                    None => return Err(self.unterm_string(open_inp, i2)),
                    Some((e, i3)) => match e {
                        b'n' => {
                            racc.push(b'\n');
                            i = i3;
                        }
                        b't' => {
                            racc.push(b'\t');
                            i = i3;
                        }
                        // \$ : a literal dollar that never opens a hole.
                        b'"' | b'\\' | b'$' => {
                            racc.push(e);
                            i = i3;
                        }
                        b'u' => {
                            let (i4, cp) = self.scan_unicode(i, i3)?;
                            push_cp(&mut racc, cp);
                            i = i4;
                        }
                        _ => return Err(self.bad_escape(i, i3)),
                    },
                },
                _ => {
                    racc.push(b);
                    i = i2;
                }
            }
        }
    }

    /// `\u{...}`: 1-6 hex digits, a valid non-surrogate code point.
    /// `str_start` is the backslash position, for blame.
    fn scan_unicode(&self, str_start: Cursor, i0: Cursor) -> Result<(Cursor, u32), Diagnostic> {
        let Some((b'{', i1)) = self.get(i0) else {
            return Err(self.bad_escape(str_start, i0));
        };
        let mut i = i1;
        let mut acc: u32 = 0;
        let mut count = 0;
        loop {
            match self.get(i) {
                Some((b'}', i2)) => {
                    return if count > 0 && valid_cp(acc) {
                        Ok((i2, acc))
                    } else {
                        Err(self.bad_escape(str_start, i2))
                    };
                }
                Some((b, i2)) if count < 6 && hex_val(b).is_some() => {
                    acc = acc * 16 + hex_val(b).unwrap();
                    count += 1;
                    i = i2;
                }
                _ => return Err(self.bad_escape(str_start, i)),
            }
        }
    }

    /// Block comments are non-nesting (as in JS/TS).
    fn scan_block_comment(&self, open_inp: Cursor, i0: Cursor) -> Result<Cursor, Diagnostic> {
        let mut i = i0;
        loop {
            match self.get(i) {
                None => {
                    return Err(Diagnostic::error(
                        "E1004",
                        "UNTERMINATED COMMENT",
                        self.span_between(open_inp, i),
                        "this `/*` comment never finds its `*/`",
                    ));
                }
                Some((b'*', i2)) => match self.get(i2) {
                    Some((b'/', i3)) => return Ok(i3),
                    _ => i = i2,
                },
                Some((_, i2)) => i = i2,
            }
        }
    }

    // Diagnostics ------------------------------------------------------------

    fn unterm_string(&self, open_inp: Cursor, i: Cursor) -> Diagnostic {
        Diagnostic::error(
            "E1002",
            "UNTERMINATED STRING",
            self.span_between(open_inp, i),
            "this string never closes",
        )
        .with_hints(vec![
            "Strings are single-line in Maxim; escape newlines as `\\n`.".to_string(),
        ])
    }

    fn unterm_element(&self, open: Cursor, name: &str, i: Cursor) -> Diagnostic {
        Diagnostic::error(
            "E1006",
            "UNTERMINATED ELEMENT",
            self.span_between(open, i),
            format!("`<{name}>` never closes"),
        )
        .with_hints(vec![
            format!("Every element needs a matching `</{name}>` or a self-closing `/>`."),
            "A mismatched closing tag above can leave an element open.".to_string(),
        ])
    }

    fn unterm_tag(&self, open: Cursor, name: &str, i: Cursor) -> Diagnostic {
        Diagnostic::error(
            "E1006",
            "UNTERMINATED ELEMENT",
            self.span_between(open, i),
            format!("this `<{name}` tag never reaches its `>`"),
        )
        .with_hints(vec![
            "Finish the tag with `>` (children follow) or `/>` (no children).".to_string(),
        ])
    }

    fn unterm_hole(&self, open: Cursor, dollar: bool, i: Cursor) -> Diagnostic {
        let what = if dollar { "`${…}` hole" } else { "`{…}` splice" };
        Diagnostic::error(
            "E1006",
            "UNTERMINATED ELEMENT",
            self.span_between(open, i),
            format!("this {what} never closes"),
        )
    }

    fn stray_brace(&self, i: Cursor) -> Diagnostic {
        let here = self.pos_of(i);
        Diagnostic::error(
            "E1007",
            "STRAY BRACE IN ELEMENT TEXT",
            Span::new(
                self.fid,
                here,
                Pos::new(here.line, here.col + 1, here.offset + 1),
            ),
            "a bare `}` cannot appear in element text",
        )
        .with_hints(vec![
            "Write `\\}` for a literal `}` in element text.".to_string(),
        ])
    }

    fn bad_tag(&self, from: Cursor, to: Cursor, msg: &str, hints: Vec<String>) -> Diagnostic {
        // A zero-width span renders poorly; widen to one character.
        let to = if to.byte == from.byte {
            Cursor {
                line: to.line,
                col: to.col + 1,
                cp: to.cp + 1,
                byte: to.byte + 1,
            }
        } else {
            to
        };
        Diagnostic::error("E1008", "MALFORMED TAG", self.span_between(from, to), msg)
            .with_hints(hints)
    }

    fn interp_newline(&self, i: Cursor) -> Diagnostic {
        let here = self.pos_of(i);
        Diagnostic::error(
            "E1005",
            "INTERPOLATION SPANS LINES",
            Span::new(
                self.fid,
                here,
                Pos::new(here.line, here.col + 1, here.offset + 1),
            ),
            "a `${…}` hole cannot contain a line break",
        )
        .with_hints(vec![
            "Bind the value with a `let` above the string, then splice the name.".to_string(),
        ])
    }

    fn bad_escape(&self, from: Cursor, to: Cursor) -> Diagnostic {
        Diagnostic::error(
            "E1003",
            "UNKNOWN ESCAPE",
            self.span_between(from, to),
            "I don't recognize this escape sequence",
        )
        .with_hints(vec![
            "Valid escapes are `\\n`, `\\t`, `\\\"`, `\\\\`, `\\$`, and `\\u{1F389}`; element text also takes `\\<`, `\\{`, and `\\}`."
                .to_string(),
        ])
    }

    fn unexpected_char(&self, i: Cursor) -> Diagnostic {
        let here = self.pos_of(i);
        let rest = &self.buf[i.byte.min(self.buf.len())..self.buf.len().min(i.byte + 4)];
        let ch = String::from_utf8_lossy(rest).chars().next().unwrap_or('?');
        Diagnostic::error(
            "E1001",
            "UNRECOGNIZED CHARACTER",
            Span::new(
                self.fid,
                here,
                Pos::new(here.line, here.col + 1, here.offset + 1),
            ),
            format!("I don't know what to do with `{ch}`"),
        )
    }
}

/// Can this token be the last token of a complete expression? The `<`
/// rule keys on it: after an expression, `<` compares; elsewhere it opens
/// an element. `Upper` is here so `List<a>` in type position keeps
/// lexing as `<`.
fn can_end_expr(k: &TokenKind) -> bool {
    match k {
        TokenKind::Lower(_) => true,
        TokenKind::Upper(_) => true,
        TokenKind::Hole => true,
        TokenKind::Int(_) => true,
        TokenKind::Float(_) => true,
        TokenKind::Str(_) => true,
        TokenKind::StringEnd(_) => true,
        TokenKind::RParen => true,
        TokenKind::RBrace => true,
        TokenKind::RBracket => true,
        TokenKind::Question => true,
        TokenKind::JsxSelfClose => true,
        TokenKind::JsxClose(_) => true,
        TokenKind::StringBegin(_)
        | TokenKind::StringMid(_)
        | TokenKind::JsxTagStart(_)
        | TokenKind::JsxTagEnd
        | TokenKind::JsxAttrName(_)
        | TokenKind::JsxText(_)
        | TokenKind::DollarBrace
        | TokenKind::Module
        | TokenKind::Import
        | TokenKind::Type
        | TokenKind::Fn
        | TokenKind::Cast
        | TokenKind::Component
        | TokenKind::Let
        | TokenKind::Match
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::LParen
        | TokenKind::LParenBOL
        | TokenKind::LBrace
        | TokenKind::LBracket
        | TokenKind::Comma
        | TokenKind::Colon
        | TokenKind::ColonColon
        | TokenKind::Dot
        | TokenKind::DotDot
        | TokenKind::Eq
        | TokenKind::Bar
        | TokenKind::Bang
        | TokenKind::Arrow
        | TokenKind::FatArrow
        | TokenKind::Pipeline
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::Op(_)
        | TokenKind::Eof => false,
    }
}

/// Would a `<` here start an element? Yes when the `<` is line-initial
/// (some pending trivia crosses a line) or when the last meaningful
/// token cannot end an expression.
fn jsx_starts(items: &[Item]) -> bool {
    let mut crossed = false;
    for item in items.iter().rev() {
        match item {
            Item::Trivia(tr) => {
                if tr.crosses_line() {
                    crossed = true;
                }
            }
            Item::Token(_, k, _) => return crossed || !can_end_expr(k),
        }
    }
    true
}

fn keyword(txt: &str) -> Option<TokenKind> {
    match txt {
        "module" => Some(TokenKind::Module),
        "import" => Some(TokenKind::Import),
        "type" => Some(TokenKind::Type),
        "fn" => Some(TokenKind::Fn),
        "cast" => Some(TokenKind::Cast),
        "component" => Some(TokenKind::Component),
        "let" => Some(TokenKind::Let),
        "match" => Some(TokenKind::Match),
        "if" => Some(TokenKind::If),
        "else" => Some(TokenKind::Else),
        _ => None,
    }
}

fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

fn push_cp(out: &mut Vec<u8>, cp: u32) {
    let c = char::from_u32(cp).expect("valid_cp guaranteed by scan_unicode");
    let mut buf = [0u8; 4];
    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}

fn valid_cp(n: u32) -> bool {
    n <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&n)
}

fn hex_val(b: u8) -> Option<u32> {
    match b {
        b'0'..=b'9' => Some((b - b'0') as u32),
        b'a'..=b'f' => Some((b - b'a' + 10) as u32),
        b'A'..=b'F' => Some((b - b'A' + 10) as u32),
        _ => None,
    }
}

// Trivia attachment -----------------------------------------------------------

/// Attach trivia: a token's leading is everything pending before it; its
/// trailing is the trivia after it up to (excluding) the first newline or
/// the next token. The EOF token carries the file's final trivia as
/// leading. A `(` whose leading crosses a line becomes `LParenBOL`.
fn fold_trivia(eof_span: Span, items: Vec<Item>) -> Vec<Token> {
    let mut out = Vec::new();
    let mut leading: Vec<Trivia> = Vec::new();
    let mut iter = items.into_iter().peekable();
    while let Some(item) = iter.next() {
        match item {
            Item::Trivia(tr) => leading.push(tr),
            Item::Token(sp, k, txt) => {
                // Trailing trivia never crosses a line: stop at a newline
                // AND at any multi-line block comment (which then leads
                // the next token).
                let mut trailing = Vec::new();
                while let Some(Item::Trivia(tr)) = iter.peek() {
                    if tr.crosses_line() {
                        break;
                    }
                    let Some(Item::Trivia(tr)) = iter.next() else {
                        unreachable!()
                    };
                    trailing.push(tr);
                }
                let kind = match k {
                    TokenKind::LParen if leading.iter().any(Trivia::crosses_line) => {
                        TokenKind::LParenBOL
                    }
                    other => other,
                };
                out.push(Token {
                    leading: std::mem::take(&mut leading),
                    kind,
                    text: txt,
                    span: sp,
                    trailing,
                });
            }
        }
    }
    out.push(Token {
        leading,
        kind: TokenKind::Eof,
        text: String::new(),
        span: eof_span,
        trailing: Vec::new(),
    });
    out
}

#[cfg(test)]
mod tests;
