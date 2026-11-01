//! The `%error.expected`-style handler that turns expected-token lists
//! into friendly diagnostics, and the cover-grammar validators (lambda
//! params vs parenthesized expressions — the ECMAScript technique).

use crate::cst::print::span_of;
use crate::cst::{CstExpr, Separated, Wrapped};
use crate::diag::{Diagnostic, dtext};
use crate::lexer::token::{Token, TokenKind};

/// Build the syntax-error diagnostic from the stuck token and the names
/// of the tokens the parser would have accepted.
pub fn parse_error(stuck: &Token, expected: &[&str]) -> Diagnostic {
    let (sp, label) = if !matches!(stuck.kind, TokenKind::Eof) {
        (
            stuck.span,
            format!("I got stuck on this {}", describe(&stuck.kind)),
        )
    } else {
        (
            stuck.span,
            "I got to the end of the file but I was still expecting more".to_string(),
        )
    };
    let mut names: Vec<String> = Vec::new();
    for e in expected {
        let h = human_token_name(e);
        if !names.contains(&h) {
            names.push(h);
        }
    }
    let body = if names.is_empty() {
        "I am not sure what went wrong.".to_string()
    } else {
        format!(
            "I was expecting {}{}",
            sentence_list(&names[..names.len().min(8)]),
            if names.len() > 8 {
                ", among other things."
            } else {
                "."
            }
        )
    };
    Diagnostic::error("E1101", "SYNTAX ERROR", sp, label).with_body(vec![dtext(body)])
}

fn describe(k: &TokenKind) -> String {
    match k {
        TokenKind::Lower(n) | TokenKind::Upper(n) => format!("name `{n}`"),
        TokenKind::Int(_) | TokenKind::Float(_) => "number".to_string(),
        TokenKind::Str(_) => "string".to_string(),
        k => format!("`{}`", token_kind_text(k)),
    }
}

pub fn sentence_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [x] => x.clone(),
        [x, y] => format!("{x} or {y}"),
        xs => format!("{}, or {}", xs[..xs.len() - 1].join(", "), xs[xs.len() - 1]),
    }
}

/// Map grammar token names (as written in the `%token` block / quoted in
/// productions) to prose.
pub fn human_token_name(s: &str) -> String {
    match s {
        "lower" => "a lowercase name like `area`".to_string(),
        "upper" => "a capitalized name like `Shape`".to_string(),
        "hole" => "`_`".to_string(),
        "int" => "a number".to_string(),
        "float" => "a number".to_string(),
        "string" => "a string".to_string(),
        "lparenBOL" => "`(`".to_string(),
        "stringBegin" => "an interpolated string".to_string(),
        "stringMid" => "`}` continuing the string".to_string(),
        "stringEnd" => "`}` closing the string".to_string(),
        "eof" => "the end of the file".to_string(),
        // grmtools reports expected sets from the pre-reduction state, so
        // JSX terminals can surface where happy's narrower post-default-
        // reduction sets never did; give them prose instead of internal
        // names.
        "jsxTagStart" => "an element like `<div>`".to_string(),
        "jsxClose" => "a closing tag like `</div>`".to_string(),
        "jsxTagEnd" => "`>`".to_string(),
        "jsxSelfClose" => "`/>`".to_string(),
        "jsxAttrName" => "an attribute name".to_string(),
        "jsxText" => "element text".to_string(),
        "dollarBrace" => "`${`".to_string(),
        s => {
            // Happy passed quoted aliases (`'('`); grmtools token names
            // arrive unquoted, so the strip is a no-op kept for parity.
            let stripped = s
                .strip_prefix('\'')
                .and_then(|r| r.strip_suffix('\''))
                .unwrap_or(s);
            format!("`{stripped}`")
        }
    }
}

/// Render the fixed spelling of a token kind (for error prose).
pub fn token_kind_text(k: &TokenKind) -> String {
    match k {
        TokenKind::Module => "module".to_string(),
        TokenKind::Import => "import".to_string(),
        TokenKind::Type => "type".to_string(),
        TokenKind::Fn => "fn".to_string(),
        TokenKind::Cast => "cast".to_string(),
        TokenKind::Component => "component".to_string(),
        TokenKind::Let => "let".to_string(),
        TokenKind::Match => "match".to_string(),
        TokenKind::If => "if".to_string(),
        TokenKind::Else => "else".to_string(),
        TokenKind::LParen => "(".to_string(),
        TokenKind::LParenBOL => "(".to_string(),
        TokenKind::RParen => ")".to_string(),
        TokenKind::LBrace => "{".to_string(),
        TokenKind::RBrace => "}".to_string(),
        TokenKind::LBracket => "[".to_string(),
        TokenKind::RBracket => "]".to_string(),
        TokenKind::Comma => ",".to_string(),
        TokenKind::Colon => ":".to_string(),
        TokenKind::ColonColon => "::".to_string(),
        TokenKind::Dot => ".".to_string(),
        TokenKind::DotDot => "..".to_string(),
        TokenKind::Eq => "=".to_string(),
        TokenKind::Bar => "|".to_string(),
        TokenKind::Bang => "!".to_string(),
        TokenKind::Arrow => "->".to_string(),
        TokenKind::FatArrow => "=>".to_string(),
        TokenKind::Pipeline => "|>".to_string(),
        TokenKind::Question => "?".to_string(),
        TokenKind::Lt => "<".to_string(),
        TokenKind::Gt => ">".to_string(),
        TokenKind::Op(op) => op.text().to_string(),
        TokenKind::Hole => "_".to_string(),
        TokenKind::Lower(n) => n.clone(),
        TokenKind::Upper(n) => n.clone(),
        TokenKind::Int(_) => "number".to_string(),
        TokenKind::Float(_) => "number".to_string(),
        TokenKind::Str(_) => "string".to_string(),
        TokenKind::StringBegin(_) => "string".to_string(),
        TokenKind::StringMid(_) => "}".to_string(),
        TokenKind::StringEnd(_) => "}".to_string(),
        TokenKind::JsxTagStart(n) => format!("<{n}"),
        TokenKind::JsxClose(n) => format!("</{n}>"),
        TokenKind::JsxTagEnd => ">".to_string(),
        TokenKind::JsxSelfClose => "/>".to_string(),
        TokenKind::JsxAttrName(n) => n.clone(),
        TokenKind::JsxText(_) => "element text".to_string(),
        TokenKind::DollarBrace => "${".to_string(),
        TokenKind::Eof => "end of file".to_string(),
    }
}

// Cover-grammar validation ----------------------------------------------------

/// `( … )` standing alone as an expression: must be exactly one
/// expression, and not `_`.
pub fn validate_group(w: Wrapped<Option<Separated<CstExpr>>>) -> Result<CstExpr, Diagnostic> {
    let whole_span = span_of(&w);
    let Wrapped { open, value, close } = w;
    match value {
        None => Err(Diagnostic::error(
            "E1102",
            "EMPTY PARENS",
            whole_span,
            "Maxim has no `()` value",
        )
        .with_hints(vec![
            "For a function with no parameters, write `fn() { body }`.".to_string(),
        ])),
        Some(sep) if sep.tail.is_empty() => match sep.head {
            CstExpr::Hole(t) => Err(Diagnostic::error(
                "E1103",
                "UNEXPECTED UNDERSCORE",
                t.span,
                "I found `_` where an expression should be",
            )
            .with_hints(vec![
                "`_` only makes sense as a lambda parameter, like `fn(_) { 0 }`.".to_string(),
            ])),
            e => Ok(CstExpr::Group(Wrapped {
                open,
                value: Box::new(e),
                close,
            })),
        },
        Some(sep) => Err(Diagnostic::error(
            "E1104",
            "NO TUPLES",
            span_of(&sep),
            "this looks like a tuple",
        )
        .with_hints(vec![
            "Maxim has no tuples. To group several values, declare a record type".to_string(),
            "or pass them as separate function arguments.".to_string(),
        ])),
    }
}

/// `( … ) => body`: the arrow lambda was removed when the fn expression
/// form arrived; the production still parses so we can say exactly that.
pub fn arrow_lambda_error(arrow: &Token) -> Diagnostic {
    Diagnostic::error(
        "E1111",
        "ARROW LAMBDA",
        arrow.span,
        "Maxim lambdas are written with `fn`",
    )
    .with_hints(vec![
        "`(x, y) => body` is written `fn(x, y) { body }` now.".to_string(),
    ])
}
