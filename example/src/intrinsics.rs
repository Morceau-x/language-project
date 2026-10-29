//! Type schemes for the wired-in values, and the base typing environment
//! every module starts from. The JS emission half of each intrinsic lives
//! in the backend.

use std::collections::BTreeMap;

use crate::builtin as b;
use crate::types::env::{CtorInfo, TcEnv, TyConInfo};
use crate::types::ty::*;
use crate::unique::Unique;

// Fixed rigid variables for intrinsic schemes (negative unique space).

fn var_a() -> TyVar {
    TyVar {
        unique: Unique(-50),
        text: "a".to_string(),
        flavor: Flavor::Star,
    }
}

fn list_elem() -> TyVar {
    TyVar {
        unique: Unique(-51),
        text: "a".to_string(),
        flavor: Flavor::Star,
    }
}

fn mount_root() -> TyVar {
    TyVar {
        unique: Unique(-52),
        text: "a".to_string(),
        flavor: Flavor::Star,
    }
}

fn resolve_var() -> TyVar {
    TyVar {
        unique: Unique(-53),
        text: "a".to_string(),
        flavor: Flavor::Star,
    }
}

pub fn intrinsic_values() -> BTreeMap<Unique, Scheme> {
    let mut out = BTreeMap::new();
    out.insert(
        b::intr_debug_log().unique,
        Scheme {
            vars: vec![var_a()],
            ty: t_fun(vec![Type::Var(var_a())], Type::Var(var_a())),
        },
    );
    out.insert(
        b::intr_string_length().unique,
        mono_scheme(t_fun(vec![t_string()], t_int())),
    );
    out.insert(
        b::intr_string_replace().unique,
        mono_scheme(t_fun(vec![t_string(), t_string(), t_string()], t_string())),
    );
    out.insert(
        b::intr_int_to_string().unique,
        mono_scheme(t_fun(vec![t_int()], t_string())),
    );
    out.insert(
        b::intr_float_to_string().unique,
        mono_scheme(t_fun(vec![t_float()], t_string())),
    );
    out.insert(
        b::intr_float_from_int().unique,
        mono_scheme(t_fun(vec![t_int()], t_float())),
    );
    out.insert(
        b::intr_int_from_float().unique,
        mono_scheme(t_fun(vec![t_float()], t_int())),
    );
    for intr in [
        b::intr_float_trunc(),
        b::intr_float_floor(),
        b::intr_float_ceil(),
        b::intr_float_round(),
    ] {
        out.insert(intr.unique, mono_scheme(t_fun(vec![t_float()], t_int())));
    }
    // The mounted/resolved tree's types are deliberately loose
    // (`(a) -> App`, `(a) -> a`): `Html` is a stdlib type, unnameable
    // from these wired-ins.
    out.insert(
        b::intr_app_mount().unique,
        Scheme {
            vars: vec![mount_root()],
            ty: t_fun(vec![Type::Var(mount_root())], t_app()),
        },
    );
    out.insert(
        b::intr_html_resolve().unique,
        Scheme {
            vars: vec![resolve_var()],
            ty: t_fun(vec![Type::Var(resolve_var())], Type::Var(resolve_var())),
        },
    );
    out
}

/// The wired-in casts. Users cannot declare casts between built-in types
/// (no user module is their home), so this table is closed here.
fn builtin_casts() -> BTreeMap<(Unique, Unique), (crate::names::Name, Scheme)> {
    let mut out = BTreeMap::new();
    out.insert(
        (b::ty_int().unique, b::ty_float().unique),
        (
            b::intr_float_from_int(),
            mono_scheme(t_fun(vec![t_int()], t_float())),
        ),
    );
    out.insert(
        (b::ty_float().unique, b::ty_int().unique),
        (
            b::intr_int_from_float(),
            mono_scheme(t_fun(vec![t_float()], t_int())),
        ),
    );
    out.insert(
        (b::ty_int().unique, b::ty_string().unique),
        (
            b::intr_int_to_string(),
            mono_scheme(t_fun(vec![t_int()], t_string())),
        ),
    );
    out.insert(
        (b::ty_float().unique, b::ty_string().unique),
        (
            b::intr_float_to_string(),
            mono_scheme(t_fun(vec![t_float()], t_string())),
        ),
    );
    out
}

fn true_info() -> CtorInfo {
    CtorInfo {
        name: b::ctor_true(),
        ty_con: tc_bool(),
        params: Vec::new(),
        args: Vec::new(),
        fields: None,
    }
}

fn false_info() -> CtorInfo {
    CtorInfo {
        name: b::ctor_false(),
        ty_con: tc_bool(),
        params: Vec::new(),
        args: Vec::new(),
        fields: None,
    }
}

fn cons_info() -> CtorInfo {
    CtorInfo {
        name: b::ctor_cons(),
        ty_con: tc_list(),
        params: vec![list_elem()],
        args: vec![Type::Var(list_elem()), t_list(Type::Var(list_elem()))],
        fields: None,
    }
}

fn nil_info() -> CtorInfo {
    CtorInfo {
        name: b::ctor_nil(),
        ty_con: tc_list(),
        params: vec![list_elem()],
        args: Vec::new(),
        fields: None,
    }
}

pub fn base_tc_env() -> TcEnv {
    let ctors: BTreeMap<Unique, CtorInfo> =
        [true_info(), false_info(), cons_info(), nil_info()]
            .into_iter()
            .map(|ci| (ci.name.unique, ci))
            .collect();
    let ty_cons: BTreeMap<Unique, TyConInfo> = [
        (
            b::ty_int().unique,
            TyConInfo {
                con: tc_int(),
                params: Vec::new(),
                ctors: Vec::new(),
                fields: None,
            },
        ),
        (
            b::ty_float().unique,
            TyConInfo {
                con: tc_float(),
                params: Vec::new(),
                ctors: Vec::new(),
                fields: None,
            },
        ),
        (
            b::ty_string().unique,
            TyConInfo {
                con: tc_string(),
                params: Vec::new(),
                ctors: Vec::new(),
                fields: None,
            },
        ),
        (
            b::ty_bool().unique,
            TyConInfo {
                con: tc_bool(),
                params: Vec::new(),
                ctors: vec![true_info(), false_info()],
                fields: None,
            },
        ),
        (
            b::ty_list().unique,
            TyConInfo {
                con: tc_list(),
                params: vec![list_elem()],
                ctors: vec![cons_info(), nil_info()],
                fields: None,
            },
        ),
        (
            b::ty_app().unique,
            TyConInfo {
                con: tc_app(),
                params: Vec::new(),
                ctors: Vec::new(),
                fields: None,
            },
        ),
    ]
    .into_iter()
    .collect();
    TcEnv {
        values: intrinsic_values(),
        ctors,
        ty_cons,
        type_names: b::builtin_types(),
        casts: builtin_casts(),
        jsx_html: None,
        components: BTreeMap::new(),
    }
}
