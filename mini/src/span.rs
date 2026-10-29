//! Source locations.

/// A region of the source text, as byte offsets `start..end` (`end` is
/// exclusive). Every token and AST node carries one, so errors can point
/// at the exact code they are about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    /// The smallest span covering both `self` and `other`: merging the
    /// spans of `1` and `2` in `1 + 2` gives the span of the whole sum.
    pub fn merge(&self, other: &Span) -> Span {
        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        Span { start, end }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_covers_both_spans() {
        assert_eq!(Span::new(0, 1).merge(&Span::new(4, 5)), Span::new(0, 5));
    }

    #[test]
    fn merge_works_in_either_order() {
        assert_eq!(Span::new(4, 5).merge(&Span::new(0, 1)), Span::new(0, 5));
    }

    #[test]
    fn merge_with_a_span_inside_keeps_the_outer_one() {
        assert_eq!(Span::new(0, 10).merge(&Span::new(3, 4)), Span::new(0, 10));
    }
}
