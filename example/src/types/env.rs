//! Typing environments and the cross-module interface (`Iface`) — the
//! ONLY artifact a module's dependents ever see, which is the seam where
//! incremental compilation can slot in later.

use std::collections::{BTreeMap, BTreeSet};

use crate::names::Name;
use crate::types::ty::{Scheme, TyCon, TyVar, Type};
use crate::unique::Unique;

#[derive(Clone, Debug)]
pub struct CtorInfo {
    pub name: Name,
    pub ty_con: TyCon,
    /// The owning type's parameters; `args` is expressed in them.
    pub params: Vec<TyVar>,
    pub args: Vec<Type>,
    /// `Some(labels)` for the implicit constructor of a record type.
    pub fields: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct TyConInfo {
    pub con: TyCon,
    pub params: Vec<TyVar>,
    pub ctors: Vec<CtorInfo>,
    /// `Some(fields)` for record types (drives field-access resolution).
    pub fields: Option<Vec<(String, Type)>>,
}

/// A checked `component` manifest — everything a JSX tag (the use site)
/// and Core lowering need. Derived from the wired functions' signatures;
/// crosses modules through the `Iface`.
#[derive(Clone, Debug)]
pub struct CompSig {
    pub name: Name,
    /// The wired view — a stateless tag is a saturated call of it.
    pub view: Name,
    pub stateful: bool,
    /// Stateful props: label, the type an attribute value must have (the
    /// payload for optional props), and whether it is optional (an
    /// `Option` field — omission passes `None`, a value wraps in `Some`).
    pub props: Vec<(String, Type, bool)>,
    /// The Props record's implicit constructor and its function type
    /// (recorded at the tag span so lowering reorders labels as usual).
    pub props_ctor: Option<(Name, Type)>,
    /// `update` returns a step (`Step(Model, List<Out>)` by shape)
    /// rather than a bare model — the emitted manifest says so.
    pub step: bool,
    /// Out events: name (the lowercased `Out` constructor) and the
    /// constructor's payload types. `on:name={h}` on the tag needs
    /// `h : (payloads) -> msg` (bare `msg` when payload-free).
    pub outs: Vec<(String, Vec<Type>)>,
}

/// How one component tag lowers; the checker records this at the tag
/// span for Core lowering.
#[derive(Clone, Debug)]
pub enum InstLower {
    /// Stateless: a saturated labeled call of the view.
    Call(Name),
    /// Stateful: a runtime instance node — component, and (for a
    /// props-taking one) the Props constructor with its optional labels
    /// (Some/None fills happen in lowering).
    Node(Name, Option<(Name, BTreeSet<String>)>),
}

/// Everything the checker can see, keyed by `Unique` (names are already
/// resolved). `type_names` resolves the textual type names in
/// annotations: prelude types plus the module's own.
#[derive(Clone, Debug, Default)]
pub struct TcEnv {
    pub values: BTreeMap<Unique, Scheme>,
    pub ctors: BTreeMap<Unique, CtorInfo>,
    pub ty_cons: BTreeMap<Unique, TyConInfo>,
    pub type_names: BTreeMap<String, Name>,
    /// Declared casts, keyed by (source head, target head) TyCon-name
    /// uniques: the conversion's synthesized name and its scheme (always
    /// a 1-ary function type).
    pub casts: BTreeMap<(Unique, Unique), (Name, Scheme)>,
    /// The stdlib `Html` type constructor — what a JSX element builds.
    /// Pinned to the stdlib by the driver (a local `type Html` shadow
    /// must not change what elements construct, because the emitted code
    /// calls the stdlib's `Html.node`). `None` when the stdlib has no
    /// `Html` module: JSX then reports E3033.
    pub jsx_html: Option<TyCon>,
    /// Checked component manifests, by the component name's unique.
    pub components: BTreeMap<Unique, CompSig>,
}

/// What one module contributes to its dependents' `TcEnv`.
#[derive(Clone, Debug, Default)]
pub struct Iface {
    pub values: BTreeMap<Unique, Scheme>,
    pub ctors: BTreeMap<Unique, CtorInfo>,
    pub ty_cons: BTreeMap<Unique, TyConInfo>,
    pub casts: BTreeMap<(Unique, Unique), (Name, Scheme)>,
    pub components: BTreeMap<Unique, CompSig>,
}

/// Left-biased merge, iface wins — mirrors the Haskell `Map.union i env`.
pub fn merge_iface(i: &Iface, env: &mut TcEnv) {
    for (k, v) in &i.values {
        env.values.insert(*k, v.clone());
    }
    for (k, v) in &i.ctors {
        env.ctors.insert(*k, v.clone());
    }
    for (k, v) in &i.ty_cons {
        env.ty_cons.insert(*k, v.clone());
    }
    for (k, v) in &i.casts {
        env.casts.insert(*k, v.clone());
    }
    for (k, v) in &i.components {
        env.components.insert(*k, v.clone());
    }
}
