//! Names: raw (as written) before renaming, unique `Name`s after.

use std::collections::BTreeMap;

use crate::span::Span;
use crate::unique::Unique;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleName(pub String);

impl ModuleName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A name as written in the source, possibly module-qualified: `List.map`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawName {
    pub qualifier: Option<String>,
    pub base: String,
}

impl RawName {
    pub fn plain(base: impl Into<String>) -> RawName {
        RawName {
            qualifier: None,
            base: base.into(),
        }
    }

    pub fn text(&self) -> String {
        match &self.qualifier {
            Some(m) => format!("{}.{}", m, self.base),
            None => self.base.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NameSort {
    /// Top-level `fn` or `let` of a module.
    TopVal(ModuleName),
    /// Parameter, block-local `let`, or pattern binding.
    Local,
    /// Data constructor.
    Ctor(ModuleName),
    /// Type constructor.
    TyCon(ModuleName),
    /// A `component` declaration — what uppercase JSX tags resolve to.
    /// Not a value: it emits as the module's manifest const.
    Component(ModuleName),
    /// Placeholder issued for a name that failed to resolve; the type
    /// checker assigns it the error type, suppressing cascades.
    Error,
}

/// A resolved name. Identity is the unique; `def_span` points at the
/// definition site (go-to-definition for free).
#[derive(Clone, Debug)]
pub struct Name {
    pub unique: Unique,
    pub text: String,
    pub sort: NameSort,
    pub def_span: Span,
}

impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.unique == other.unique
    }
}

impl Eq for Name {}

impl PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.unique.cmp(&other.unique)
    }
}

impl std::hash::Hash for Name {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.unique.hash(state);
    }
}

/// What one module offers to its importers, by surface name. Built by the
/// renamer; the only name-level artifact that crosses module boundaries.
#[derive(Clone, Debug, Default)]
pub struct ModuleExports {
    pub values: BTreeMap<String, Name>,
    pub ctors: BTreeMap<String, Name>,
    pub types: BTreeMap<String, Name>,
    pub components: BTreeMap<String, Name>,
}
