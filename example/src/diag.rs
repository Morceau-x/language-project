//! The one diagnostic type, produced by every compiler phase and consumed
//! by the CLI renderer.
//!
//! Code conventions: `E1xxx` lex/parse, `E2xxx` names, `E3xxx` types,
//! `E4xxx`/`W4xxx` pattern matching, `E5xxx` driver.

pub mod render;

use crate::span::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DiagCode(pub &'static str);

/// Body prose interleaved with inline code/type snippets. Types are
/// pre-rendered to text by their producers, so this module sits at the
/// bottom of the dependency graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagPart {
    Text(String),
    Code(String),
}

/// Shorthand for building `DiagPart::Text` from a literal.
pub fn dtext(t: impl Into<String>) -> DiagPart {
    DiagPart::Text(t.into())
}

/// Shorthand for building `DiagPart::Code` from a literal.
pub fn dcode(t: impl Into<String>) -> DiagPart {
    DiagPart::Code(t.into())
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    /// Header, Elm style: `TYPE MISMATCH`, `NAMING ERROR`, ...
    pub title: String,
    /// The main span and the label rendered under its carets.
    pub primary: (Span, String),
    pub secondary: Vec<(Span, String)>,
    pub body: Vec<DiagPart>,
    pub hints: Vec<String>,
}

impl Diagnostic {
    pub fn error(
        code: &'static str,
        title: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: DiagCode(code),
            title: title.into(),
            primary: (span, label.into()),
            secondary: Vec::new(),
            body: Vec::new(),
            hints: Vec::new(),
        }
    }

    pub fn warning(
        code: &'static str,
        title: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, title, span, label)
        }
    }

    pub fn with_secondary(mut self, xs: Vec<(Span, String)>) -> Diagnostic {
        self.secondary.extend(xs);
        self
    }

    pub fn with_body(mut self, ps: Vec<DiagPart>) -> Diagnostic {
        self.body.extend(ps);
        self
    }

    pub fn with_hints(mut self, hs: Vec<String>) -> Diagnostic {
        self.hints.extend(hs);
        self
    }
}
