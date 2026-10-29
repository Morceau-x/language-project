//! Parser entry points: text in, lossless CST out.
//!
//! Two modes. `parse_module` is fail-fast: the first syntax error abandons
//! the file (right for `maxim build`'s all-or-nothing contract, and the
//! baseline the round-trip property pins). `parse_module_recovering`
//! implements R1 of the error-recovery design
//! (`docs/design/parser-error-recovery.md`): the token stream is split
//! into declaration segments OUTSIDE the grammar — at `fn`/`type`/
//! `import`/`let` tokens at brace depth 0 — and each segment parses
//! independently. A failing segment contributes a `CstDecl::Error` node
//! carrying its tokens verbatim (losslessness survives) plus its one
//! diagnostic; every healthy declaration still parses, renames, and
//! type-checks.

pub mod grammar;
pub mod support;

use crate::cst::{CstDecl, CstModule};
use crate::diag::Diagnostic;
use crate::lexer::lex_tokens;
use crate::lexer::token::{Token, TokenKind};
use crate::maxim_panic;
use crate::span::{FileId, Span};
use grammar::{EntryOut, ParseMode, run_parse};

/// Lex and parse one module. Pure; fail-fast per file (the driver still
/// compiles every other module in the graph on failure).
pub fn parse_module(fid: FileId, src: &str) -> Result<CstModule, Diagnostic> {
    parse_tokens(&lex_tokens(fid, src)?)
}

/// Parse a pre-lexed token stream (the stream must end with `Eof`, as
/// `lex_tokens` guarantees).
pub fn parse_tokens(toks: &[Token]) -> Result<CstModule, Diagnostic> {
    match run_parse(toks, ParseMode::Module)? {
        EntryOut::Module(m) => Ok(m),
        EntryOut::Decl(_) => maxim_panic!("module entry produced a declaration"),
    }
}

/// Lex and parse with declaration-level error recovery. `Err` only for
/// lexical errors and broken module headers (out of R1 scope — those stay
/// fail-fast); otherwise the module comes back with one diagnostic per
/// broken declaration segment (empty = the file is syntactically clean).
pub fn parse_module_recovering(
    fid: FileId,
    src: &str,
) -> Result<(CstModule, Vec<Diagnostic>), Diagnostic> {
    parse_tokens_recovering(lex_tokens(fid, src)?)
}

pub fn parse_tokens_recovering(
    toks: Vec<Token>,
) -> Result<(CstModule, Vec<Diagnostic>), Diagnostic> {
    if toks.len() >= 2
        && matches!(toks[0].kind, TokenKind::Module)
        && matches!(toks[1].kind, TokenKind::Upper(_))
    {
        let mut it = toks.into_iter();
        let module_kw = it.next().unwrap();
        let name = it.next().unwrap();
        let (segs, real_eof) = segment(it.collect());
        let mut decls = Vec::with_capacity(segs.len());
        let mut diags = Vec::new();
        for seg in segs {
            match parse_seg(&seg) {
                Ok(d) => decls.push(d),
                Err(diag) => {
                    decls.push(CstDecl::Error(seg));
                    diags.push(diag);
                }
            }
        }
        Ok((
            CstModule {
                module_kw,
                name,
                decls,
                eof: real_eof,
            },
            diags,
        ))
    } else {
        // No recognizable header: whole-file fail-fast (R1 recovers
        // between declarations, not inside the header).
        parse_tokens(&toks).map(|m| (m, Vec::new()))
    }
}

fn parse_seg(ts: &[Token]) -> Result<CstDecl, Diagnostic> {
    let mut with_eof = ts.to_vec();
    with_eof.push(synthetic_eof(ts));
    match run_parse(&with_eof, ParseMode::DeclSeg)? {
        EntryOut::Decl(d) => Ok(d),
        EntryOut::Module(_) => maxim_panic!("decl-segment entry produced a module"),
    }
}

/// A zero-width eof anchoring the segment's end: a completed declaration
/// followed by garbage must still fail (losslessness), and the error
/// machinery always has a token to point at.
fn synthetic_eof(ts: &[Token]) -> Token {
    let Some(t) = ts.last() else {
        maxim_panic!("parse_tokens_recovering: empty segment")
    };
    let e = t.span.end;
    Token {
        leading: Vec::new(),
        kind: TokenKind::Eof,
        text: String::new(),
        span: Span::new(t.span.file, e, e),
        trailing: Vec::new(),
    }
}

/// Split at declaration starters at brace depth 0, outside interpolation
/// holes. The brace depth clamps at 0 so a stray `}` cannot poison the
/// rest of the file; an unclosed `{` swallows everything after it into
/// one segment (accepted for R1 — statement resync is R2). Interpolation
/// holes hold ordinary tokens, so the Begin/End depth keeps a broken
/// hole from splitting a string.
fn segment(rest: Vec<Token>) -> (Vec<Vec<Token>>, Token) {
    let mut segs: Vec<Vec<Token>> = Vec::new();
    let mut cur: Vec<Token> = Vec::new();
    let mut braces: i64 = 0;
    let mut holes: i64 = 0;
    let mut iter = rest.into_iter().peekable();
    while let Some(t) = iter.next() {
        if matches!(t.kind, TokenKind::Eof) && iter.peek().is_none() {
            if !cur.is_empty() {
                segs.push(cur);
            }
            return (segs, t);
        }
        // `fn` only STARTS a declaration when a name follows: a bare
        // `fn (` at depth 0 is a lambda in a top-level let's RHS.
        let starts = match t.kind {
            TokenKind::Fn => matches!(iter.peek().map(|n| &n.kind), Some(TokenKind::Lower(_))),
            TokenKind::Type
            | TokenKind::Import
            | TokenKind::Let
            | TokenKind::Cast
            | TokenKind::Component => true,
            _ => false,
        };
        // `${` in element text opens a hole that a plain `}` closes, so
        // it rides the brace counter. No JSX counter is needed beyond
        // that: tag and text tokens contain no declaration-starter
        // keywords (`type` in a tag lexes as an attribute name, text is
        // one opaque token).
        let braces2 = match t.kind {
            TokenKind::LBrace | TokenKind::DollarBrace => braces + 1,
            TokenKind::RBrace => (braces - 1).max(0),
            _ => braces,
        };
        let holes2 = match t.kind {
            TokenKind::StringBegin(_) => holes + 1,
            TokenKind::StringEnd(_) => (holes - 1).max(0),
            _ => holes,
        };
        if starts && braces == 0 && holes == 0 && !cur.is_empty() {
            segs.push(std::mem::take(&mut cur));
        }
        cur.push(t);
        braces = braces2;
        holes = holes2;
    }
    maxim_panic!("parse_tokens_recovering: stream without EOF")
}

#[cfg(test)]
mod tests;
