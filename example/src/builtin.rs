//! Wired-in names: primitive type constructors, the builtin Bool and
//! List ADTs, and the intrinsic "virtual modules" (`Debug`, `String`,
//! `Int`, `Float`) whose functions are compiler-known — no FFI syntax
//! exists in v0.
//!
//! Conversions follow the cast convention: `toString` lives on the source
//! type's module (`Int.toString`, `Float.toString` — `String` has no
//! `fromX`); every other conversion is `from<Source>` on the target's
//! module (`Float.fromInt`, `Int.fromFloat`).
//!
//! Builtin uniques are negative and fixed; user names start at 0.

use std::collections::BTreeMap;

use crate::names::{ModuleName, Name, NameSort, ModuleExports};
use crate::span::{FileId, START_POS, Span};
use crate::unique::Unique;

/// The definition site of everything wired in.
pub fn builtin_span() -> Span {
    Span::new(FileId(-1), START_POS, START_POS)
}

fn mk(u: i64, txt: &str, sort: NameSort) -> Name {
    Name {
        unique: Unique(u),
        text: txt.to_string(),
        sort,
        def_span: builtin_span(),
    }
}

fn basics() -> ModuleName {
    ModuleName("Basics".to_string())
}

fn m(s: &str) -> ModuleName {
    ModuleName(s.to_string())
}

// Type constructors -----------------------------------------------------------

pub fn ty_int() -> Name {
    mk(-1, "Int", NameSort::TyCon(basics()))
}

pub fn ty_float() -> Name {
    mk(-2, "Float", NameSort::TyCon(basics()))
}

pub fn ty_string() -> Name {
    mk(-3, "String", NameSort::TyCon(m("String")))
}

pub fn ty_bool() -> Name {
    mk(-4, "Bool", NameSort::TyCon(basics()))
}

pub fn ty_list() -> Name {
    mk(-5, "List", NameSort::TyCon(m("List")))
}

/// The opaque handle a mounted application returns — `fn main() -> App`.
pub fn ty_app() -> Name {
    mk(-23, "App", NameSort::TyCon(m("App")))
}

// Constructors ----------------------------------------------------------------

pub fn ctor_true() -> Name {
    mk(-6, "True", NameSort::Ctor(basics()))
}

pub fn ctor_false() -> Name {
    mk(-7, "False", NameSort::Ctor(basics()))
}

pub fn ctor_cons() -> Name {
    mk(-8, "Cons", NameSort::Ctor(m("List")))
}

pub fn ctor_nil() -> Name {
    mk(-9, "Nil", NameSort::Ctor(m("List")))
}

// Intrinsic values ------------------------------------------------------------
// Unique -14 belonged to Int.div, retired when `~/` arrived in M3.

pub fn intr_debug_log() -> Name {
    mk(-10, "log", NameSort::TopVal(m("Debug")))
}

pub fn intr_string_length() -> Name {
    mk(-11, "length", NameSort::TopVal(m("String")))
}

pub fn intr_int_to_string() -> Name {
    mk(-12, "toString", NameSort::TopVal(m("Int")))
}

pub fn intr_float_to_string() -> Name {
    mk(-13, "toString", NameSort::TopVal(m("Float")))
}

pub fn intr_float_from_int() -> Name {
    mk(-15, "fromInt", NameSort::TopVal(m("Float")))
}

pub fn intr_int_from_float() -> Name {
    mk(-16, "fromFloat", NameSort::TopVal(m("Int")))
}

pub fn intr_float_trunc() -> Name {
    mk(-17, "trunc", NameSort::TopVal(m("Float")))
}

pub fn intr_float_floor() -> Name {
    mk(-18, "floor", NameSort::TopVal(m("Float")))
}

pub fn intr_float_ceil() -> Name {
    mk(-19, "ceil", NameSort::TopVal(m("Float")))
}

pub fn intr_float_round() -> Name {
    mk(-20, "round", NameSort::TopVal(m("Float")))
}

pub fn intr_string_replace() -> Name {
    mk(-21, "replace", NameSort::TopVal(m("String")))
}

/// `App.mount(root)` — hand an `Html` tree to the runtime. In a browser
/// it mounts and runs the update loop; under Node it renders the tree at
/// its init state and prints it.
pub fn intr_app_mount() -> Name {
    mk(-22, "mount", NameSort::TopVal(m("App")))
}

/// `Html.resolve(html)` — replace every stateful component instance with
/// its view at init state (deeply). What SSR renders; the identity on
/// instance-free trees. Wired in (the shape is runtime-known), merged
/// into the compiled `Html` module's exports.
pub fn intr_html_resolve() -> Name {
    mk(-24, "resolve", NameSort::TopVal(m("Html")))
}

// Assemblies ------------------------------------------------------------------

fn vals(ns: Vec<Name>) -> BTreeMap<String, Name> {
    ns.into_iter().map(|n| (n.text.clone(), n)).collect()
}

/// The virtual modules, always importable without an `import`. The driver
/// merges the compiled stdlib's own exports on top (e.g. `List.map` from
/// `List.mx` joins the wired-in `List` below).
pub fn builtin_modules() -> BTreeMap<ModuleName, ModuleExports> {
    let mut out = BTreeMap::new();
    out.insert(
        m("Debug"),
        ModuleExports {
            values: vals(vec![intr_debug_log()]),
            ..Default::default()
        },
    );
    out.insert(
        m("String"),
        ModuleExports {
            values: vals(vec![intr_string_length(), intr_string_replace()]),
            ..Default::default()
        },
    );
    out.insert(
        m("Int"),
        ModuleExports {
            values: vals(vec![intr_int_to_string(), intr_int_from_float()]),
            ..Default::default()
        },
    );
    out.insert(
        m("Float"),
        ModuleExports {
            values: vals(vec![
                intr_float_to_string(),
                intr_float_from_int(),
                intr_float_trunc(),
                intr_float_floor(),
                intr_float_ceil(),
                intr_float_round(),
            ]),
            ..Default::default()
        },
    );
    out.insert(
        m("List"),
        ModuleExports {
            ctors: vals(vec![ctor_cons(), ctor_nil()]),
            types: vals(vec![ty_list()]),
            ..Default::default()
        },
    );
    out.insert(
        m("App"),
        ModuleExports {
            values: vals(vec![intr_app_mount()]),
            types: vals(vec![ty_app()]),
            ..Default::default()
        },
    );
    // The compiled Html.mx's own exports merge ON TOP of this (the
    // driver unions them), so the wired-in `resolve` rides along.
    out.insert(
        m("Html"),
        ModuleExports {
            values: vals(vec![intr_html_resolve()]),
            ..Default::default()
        },
    );
    out
}

/// Types in scope unqualified everywhere.
pub fn builtin_types() -> BTreeMap<String, Name> {
    vals(vec![
        ty_int(),
        ty_float(),
        ty_string(),
        ty_bool(),
        ty_list(),
        ty_app(),
    ])
}

/// Constructors exposed unqualified everywhere. `Cons`/`Nil` are not in
/// this list — lists are written with `[ … ]` sugar.
pub fn builtin_ctors() -> BTreeMap<String, Name> {
    vals(vec![ctor_true(), ctor_false()])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin every wired-in unique: iface keys and the `unique < 0` builtin
    /// predicate depend on these never drifting.
    #[test]
    fn builtin_uniques_are_pinned() {
        for (name, u) in [
            (ty_int(), -1),
            (ty_float(), -2),
            (ty_string(), -3),
            (ty_bool(), -4),
            (ty_list(), -5),
            (ctor_true(), -6),
            (ctor_false(), -7),
            (ctor_cons(), -8),
            (ctor_nil(), -9),
            (intr_debug_log(), -10),
            (intr_string_length(), -11),
            (intr_int_to_string(), -12),
            (intr_float_to_string(), -13),
            (intr_float_from_int(), -15),
            (intr_int_from_float(), -16),
            (intr_float_trunc(), -17),
            (intr_float_floor(), -18),
            (intr_float_ceil(), -19),
            (intr_float_round(), -20),
            (intr_string_replace(), -21),
            (intr_app_mount(), -22),
            (ty_app(), -23),
            (intr_html_resolve(), -24),
        ] {
            assert_eq!(name.unique, Unique(u), "unique of {}", name.text);
            assert!(name.unique.is_builtin());
        }
    }
}
