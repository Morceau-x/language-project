//! Unique identifiers, threaded through the renamer and later phases.
//!
//! Uniques must be unique across ALL modules of one compilation (the type
//! environment is keyed by them), so the driver threads the supply from
//! one module's renaming into the next.
//!
//! Reserved negative ranges (never produced by the supply):
//! `-1..=-24` wired-in builtins, `-50..=-53` intrinsic rigid type
//! variables, `-1000-n` Core lowering temporaries.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unique(pub i64);

impl Unique {
    /// Wired-in names (builtins, intrinsics, lowering temps) live below
    /// zero; user names count up from zero.
    pub fn is_builtin(self) -> bool {
        self.0 < 0
    }
}

/// A monotone counter handing out fresh uniques.
#[derive(Clone, Copy, Debug)]
pub struct UniqueSupply(i64);

impl UniqueSupply {
    pub fn initial() -> UniqueSupply {
        UniqueSupply(0)
    }

    pub fn from_value(n: i64) -> UniqueSupply {
        UniqueSupply(n)
    }

    pub fn value(self) -> i64 {
        self.0
    }

    pub fn fresh(&mut self) -> Unique {
        let u = Unique(self.0);
        self.0 += 1;
        u
    }
}
