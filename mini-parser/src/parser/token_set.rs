//! Sets of token kinds, as in rust-analyzer.

use crate::SyntaxKind;

/// A set of token kinds, as a bitset: one bit per kind. Token kinds come
/// first in `SyntaxKind`, so they all fit in 128 bits.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenSet(u128);

impl TokenSet {
    pub(crate) const EMPTY: TokenSet = TokenSet(0);

    /// `const`, so sets can be constants: `const DECL_FIRST: TokenSet = …`.
    pub(crate) const fn new(kinds: &[SyntaxKind]) -> TokenSet {
        let mut bits = 0u128;
        let mut i = 0;
        // No `for` loops in a `const fn`: iterators aren't `const` yet.
        while i < kinds.len() {
            bits |= mask(kinds[i]);
            i += 1;
        }
        TokenSet(bits)
    }

    pub(crate) const fn union(self, other: TokenSet) -> TokenSet {
        TokenSet(self.0 | other.0)
    }

    pub(crate) const fn contains(self, kind: SyntaxKind) -> bool {
        self.0 & mask(kind) != 0
    }
}

const fn mask(kind: SyntaxKind) -> u128 {
    // `UNDERSCORE` is the last token kind: nodes come after it.
    assert!(
        (kind as u8) <= (SyntaxKind::UNDERSCORE as u8),
        "only token kinds go in a TokenSet, not node kinds"
    );
    1u128 << (kind as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::T;

    #[test]
    fn contains_what_was_added() {
        const SET: TokenSet = TokenSet::new(&[T![fn], T![let]]);
        assert!(SET.contains(T![fn]));
        assert!(SET.contains(T![let]));
        assert!(!SET.contains(T![type]));
        assert!(SET.union(TokenSet::new(&[T![type]])).contains(T![type]));
        assert!(!TokenSet::EMPTY.contains(T![fn]));
    }

    #[test]
    fn every_token_kind_fits() {
        assert!((SyntaxKind::UNDERSCORE as u8) < 128);
    }

    #[test]
    #[should_panic(expected = "only token kinds")]
    fn node_kinds_dont_go_in_a_token_set() {
        TokenSet::new(&[SyntaxKind::FN]);
    }
}
