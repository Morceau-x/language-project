//! Source positions and spans.
//!
//! Lines and columns are 1-based and counted in Unicode code points (not
//! bytes, not UTF-16 units). `offset` is the code-point index into the
//! file. The LSP layer converts to UTF-16 at its own boundary; nothing in
//! the compiler core thinks about UTF-16.

/// A source position. Ordering is lexicographic in `(line, col, offset)`,
/// matching the Haskell derived `Ord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
    pub offset: u32,
}

impl Pos {
    pub const fn new(line: u32, col: u32, offset: u32) -> Pos {
        Pos { line, col, offset }
    }
}

/// The position of the first character of a file.
pub const START_POS: Pos = Pos {
    line: 1,
    col: 1,
    offset: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub i32);

/// A contiguous source region; `end` is exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub const fn new(file: FileId, start: Pos, end: Pos) -> Span {
        Span { file, start, end }
    }

    /// Smallest span covering both arguments (which must be in the same file).
    pub fn merge(self, other: Span) -> Span {
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// 1-based first and last line touched by this span.
    pub fn lines(self) -> (u32, u32) {
        (self.start.line, self.end.line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(l1: u32, c1: u32, l2: u32, c2: u32) -> Span {
        Span::new(FileId(0), Pos::new(l1, c1, 0), Pos::new(l2, c2, 0))
    }

    #[test]
    fn merge_span_is_the_hull() {
        assert_eq!(sp(1, 5, 1, 8).merge(sp(1, 1, 1, 3)), sp(1, 1, 1, 8));
    }
}
