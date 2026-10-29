//! Source files and the line index used to slice excerpts for diagnostics.

use crate::span::FileId;

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub id: FileId,
    pub path: String,
    pub text: String,
}

/// The lines of a file, 1-based, with line endings stripped.
#[derive(Clone, Debug)]
pub struct LineIndex {
    lines: Vec<String>,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let lines = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
            .collect();
        LineIndex { lines }
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// 1-based and total: out-of-range lines render as empty (only
    /// reachable from malformed spans).
    pub fn line_text(&self, n: i64) -> &str {
        if n >= 1 && n <= self.lines.len() as i64 {
            &self.lines[(n - 1) as usize]
        } else {
            ""
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_1_based() {
        let li = LineIndex::new("alpha\nbeta\ngamma");
        assert_eq!(li.line_text(1), "alpha");
        assert_eq!(li.line_text(2), "beta");
        assert_eq!(li.line_text(3), "gamma");
        assert_eq!(li.line_count(), 3);
    }

    #[test]
    fn crlf_endings_are_stripped() {
        let li = LineIndex::new("alpha\r\nbeta\r\n");
        assert_eq!(li.line_text(1), "alpha");
        assert_eq!(li.line_text(2), "beta");
    }

    #[test]
    fn out_of_range_is_empty() {
        let li = LineIndex::new("only");
        assert_eq!(li.line_text(0), "");
        assert_eq!(li.line_text(2), "");
    }
}
