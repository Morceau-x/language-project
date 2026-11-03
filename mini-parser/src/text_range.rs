//! Positions in the source text.

use std::fmt;

/// A range of bytes in the source text: `start..end`, end excluded.
/// Byte offsets, as in rust-analyzer: cheap to compute, and slicing the
/// text with them is free. Lines and columns are computed only when an
/// error is shown.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TextRange {
    start: u32,
    end: u32,
}

impl TextRange {
    pub fn new(start: u32, end: u32) -> TextRange {
        assert!(start <= end, "a range can't end before it starts");
        TextRange { start, end }
    }

    /// The empty range at `offset`: where something is missing.
    pub fn empty(offset: u32) -> TextRange {
        TextRange::new(offset, offset)
    }

    pub fn start(self) -> u32 {
        self.start
    }

    pub fn end(self) -> u32 {
        self.end
    }

    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// The text this range covers.
    pub fn slice(self, text: &str) -> &str {
        &text[self.start as usize..self.end as usize]
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Debug for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
