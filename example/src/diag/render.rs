//! Elm-style terminal rendering of diagnostics: a titled header rule, the
//! offending source excerpt with a line-number gutter and caret underlines,
//! prose, and hints.
//!
//! Output is deliberately plain-text stable when color is off; golden tests
//! snapshot exactly that form.

use std::collections::BTreeMap;

use crate::diag::{DiagPart, Diagnostic, Severity};
use crate::source::{LineIndex, SourceFile};
use crate::span::{FileId, Span};

/// Where to find the text of each file mentioned by a diagnostic.
pub type SourceMap = BTreeMap<i32, (String, LineIndex)>;

pub fn mk_source_map(files: &[SourceFile]) -> SourceMap {
    files
        .iter()
        .map(|f| (f.id.0, (f.path.clone(), LineIndex::new(&f.text))))
        .collect()
}

const HEADER_WIDTH: i64 = 80;

/// Like `Data.Text.replicate`: a non-positive count yields the empty string.
fn rep(n: i64, s: &str) -> String {
    if n <= 0 {
        String::new()
    } else {
        s.repeat(n as usize)
    }
}

/// Code-point length, matching Haskell's `T.length`.
fn cp_len(s: &str) -> i64 {
    s.chars().count() as i64
}

/// Render one diagnostic. `color` enables ANSI color.
pub fn render_diagnostic(color: bool, srcs: &SourceMap, d: &Diagnostic) -> String {
    let sev_color = match d.severity {
        Severity::Error => RED,
        Severity::Warning => YELLOW,
    };

    let file_path_of = |FileId(fid): FileId| -> String {
        srcs.get(&fid)
            .map(|(path, _)| path.clone())
            .unwrap_or_else(|| "<unknown>".to_string())
    };

    let line_index_of = |FileId(fid): FileId| -> Option<&LineIndex> { srcs.get(&fid).map(|(_, li)| li) };

    let header = {
        let path = file_path_of(d.primary.0.file);
        let title = format!("-- {} ", d.title);
        let pad = (HEADER_WIDTH - cp_len(&title) - cp_len(&path) - 1).max(1);
        paint(color, CYAN, &format!("{}{} {}", title, rep(pad, "-"), path))
    };

    let excerpt_for = |caret_color: &str, sp: Span, label: &str| -> Vec<String> {
        let Some(li) = line_index_of(sp.file) else {
            return vec![label.to_string()];
        };
        let (l1, l2) = sp.lines();
        let (l1, l2) = (l1 as i64, l2 as i64);
        let start_col = sp.start.col as i64;
        let end_col = sp.end.col as i64;
        let gutter_w = cp_len(&l2.to_string());
        let gutter = |n: i64| format!("{:>width$}| ", n, width = gutter_w as usize);
        let src_line = |n: i64| format!("{}{}", gutter(n), li.line_text(n));
        // Caret width on the first line of the span: to the span end if
        // single-line, else to the end of the first line.
        let first_line = li.line_text(l1);
        let caret_end = if l1 == l2 {
            (start_col + 1).max(end_col)
        } else {
            cp_len(first_line) + 1
        };
        let carets = format!(
            "{}{}{}",
            rep(gutter_w + 2 + start_col - 1, " "),
            paint(color, caret_color, &rep(caret_end - start_col, "^")),
            if label.is_empty() {
                String::new()
            } else {
                format!(" {label}")
            }
        );
        if l1 == l2 {
            vec![src_line(l1), carets]
        } else {
            let mut out: Vec<String> = (l1..=l2.min(l1 + 8)).map(src_line).collect();
            out.push(carets);
            out
        }
    };

    let mut lines: Vec<String> = vec![header, String::new()];

    let body = render_parts(&d.body);
    if !body.is_empty() {
        lines.push(body);
        lines.push(String::new());
    }

    lines.extend(excerpt_for(sev_color, d.primary.0, &d.primary.1));
    for (sp, label) in &d.secondary {
        lines.push(String::new());
        lines.extend(excerpt_for(CYAN, *sp, label));
    }

    if !d.hints.is_empty() {
        lines.push(String::new());
        for h in &d.hints {
            lines.push(format!("{} {}", paint(color, GREEN, "Hint:"), h));
        }
    }

    lines.join("\n")
}

fn render_parts(parts: &[DiagPart]) -> String {
    parts
        .iter()
        .map(|p| match p {
            DiagPart::Text(t) => t.clone(),
            DiagPart::Code(t) => format!("`{t}`"),
        })
        .collect()
}

/// Render many diagnostics sorted by source position, blank-line separated.
pub fn render_diagnostics(color: bool, srcs: &SourceMap, ds: &[Diagnostic]) -> String {
    let mut sorted: Vec<&Diagnostic> = ds.iter().collect();
    sorted.sort_by_key(|d| (d.primary.0.file, d.primary.0.start));
    sorted
        .iter()
        .map(|d| render_diagnostic(color, srcs, d))
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ANSI helpers ---------------------------------------------------------------

const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";

fn paint(color: bool, c: &str, t: &str) -> String {
    if color {
        format!("{c}{t}\x1b[0m")
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Pos;

    #[test]
    fn single_line_excerpt_with_carets() {
        let file = SourceFile {
            id: FileId(0),
            path: "demo.mx".to_string(),
            text: "fn main() {\n  oops\n}\n".to_string(),
        };
        let srcs = mk_source_map(std::slice::from_ref(&file));
        let sp = Span::new(FileId(0), Pos::new(2, 3, 14), Pos::new(2, 7, 18));
        let d = Diagnostic::error("E2001", "NAMING ERROR", sp, "I don't know this name")
            .with_hints(vec!["Did you mean `area`?".to_string()]);
        let out = render_diagnostic(false, &srcs, &d);
        assert!(out.starts_with("-- NAMING ERROR"), "has header");
        assert!(out.contains("demo.mx"), "names the file");
        assert!(out.contains("2|   oops"), "has the source line");
        assert!(out.contains("^^^^"), "has carets");
        assert!(out.contains("Hint: Did you mean `area`?"), "has the hint");
    }
}
