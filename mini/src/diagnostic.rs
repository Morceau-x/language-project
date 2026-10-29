//! Compiler errors that point at the source code they are about.

use crate::span::Span;

/// An error found while compiling: what went wrong, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            message: message.into(),
            span,
        }
    }

    /// Format the error for humans, quoting the offending line the way
    /// rustc does:
    ///
    /// ```text
    /// error: unexpected character `$`
    ///  --> input.mini:1:5
    ///   |
    /// 1 | 1 + $
    ///   |     ^
    /// ```
    pub fn render(&self, file_name: &str, source: &str) -> String {
        let start = self.span.start;
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_text = source[line_start..].lines().next().unwrap_or("");
        let line_number = source[..line_start].matches('\n').count() + 1;
        let column = source[line_start..start].chars().count() + 1;

        // Underline the span, but only the part on this line, and always
        // with at least one `^`: an empty span (end of input) needs a mark too.
        let line_end = line_start + line_text.len();
        let underline_end = self.span.end.clamp(start, line_end.max(start));
        let width = source[start..underline_end].chars().count().max(1);

        let pad = " ".repeat(line_number.to_string().len());
        let marker = " ".repeat(column - 1) + &"^".repeat(width);
        let mut out = String::new();
        out += &format!("error: {}\n", self.message);
        out += &format!("{pad}--> {file_name}:{line_number}:{column}\n");
        out += &format!("{pad} |\n");
        out += &format!("{line_number} | {line_text}\n");
        out += &format!("{pad} | {marker}\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_quotes_the_line_and_marks_the_span() {
        let diagnostic = Diagnostic::new("unexpected character `$`", Span::new(4, 5));
        assert_eq!(
            diagnostic.render("input.mini", "1 + $"),
            "\
error: unexpected character `$`
 --> input.mini:1:5
  |
1 | 1 + $
  |     ^
"
        );
    }

    #[test]
    fn render_finds_the_line_and_underlines_the_whole_span() {
        let source = "1 +\n  99999999999999999999\n";
        let diagnostic = Diagnostic::new("integer literal is too large", Span::new(6, 26));
        assert_eq!(
            diagnostic.render("input.mini", source),
            "\
error: integer literal is too large
 --> input.mini:2:3
  |
2 |   99999999999999999999
  |   ^^^^^^^^^^^^^^^^^^^^
"
        );
    }

    #[test]
    fn render_marks_an_empty_span_at_end_of_input() {
        let diagnostic = Diagnostic::new("expected an expression", Span::new(3, 3));
        assert_eq!(
            diagnostic.render("input.mini", "1 +"),
            "\
error: expected an expression
 --> input.mini:1:4
  |
1 | 1 +
  |    ^
"
        );
    }
}
