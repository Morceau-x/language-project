//! Ported from `compiler/test/MatchTests.hs`.

use std::collections::BTreeMap;

use super::*;
use crate::ast::Lit;
use crate::builtin::{builtin_ctors, builtin_modules, builtin_span, builtin_types};
use crate::cst::lower::lower_module;
use crate::intrinsics::base_tc_env;
use crate::lexer::token::F64;
use crate::names::{ModuleName, NameSort};
use crate::parser::parse_module;
use crate::rename::{RenameEnv, rename_module};
use crate::span::FileId;
use crate::types::env::merge_iface;
use crate::types::infer::check_module;
use crate::unique::Unique;

/// Front-end pipeline, then analyze every match in the module.
fn analyze_all(src: &str) -> Vec<Diagnostic> {
    let rn_env = RenameEnv {
        auto_modules: builtin_modules(),
        user_modules: BTreeMap::new(),
        prelude_ctors: builtin_ctors(),
        prelude_types: builtin_types(),
    };
    let cst = match parse_module(FileId(0), src) {
        Ok(c) => c,
        Err(d) => return vec![d],
    };
    let ast = match lower_module(&cst) {
        Ok(a) => a,
        Err(d) => return vec![d],
    };
    let (m, _, _) = match rename_module(&rn_env, 0, &ast) {
        Ok(r) => r,
        Err(ds) => return ds,
    };
    let (res, _arts, _arena) = check_module(&base_tc_env(), &m);
    match res {
        Err(ds) => ds,
        Ok(iface) => {
            let mut env = base_tc_env();
            merge_iface(&iface, &mut env);
            collect_matches(&m)
                .into_iter()
                .flat_map(|(sp, arms)| analyze_match(&env, sp, arms).0)
                .collect()
        }
    }
}

#[track_caller]
fn expect_clean(src: &str) {
    let ds = analyze_all(src);
    assert!(ds.is_empty(), "expected no diagnostics, got {ds:#?}");
}

#[track_caller]
fn expect_diag(code: &str, needle: &str, src: &str) {
    let ds = analyze_all(src);
    assert!(
        ds.iter()
            .any(|d| d.code.0 == code && d.primary.1.contains(needle)),
        "expected {code} mentioning {needle:?}, got {ds:#?}"
    );
}

const SHAPE_PRELUDE: &str = "module M\ntype Shape =\n  | Circle(Float)\n  | Rect(Float, Float)\n";

// -- exhaustiveness -------------------------------------------------------------

#[test]
fn complete_adt_match_is_clean() {
    expect_clean(&format!(
        "{SHAPE_PRELUDE}fn f(s: Shape) -> Int {{\n  match s {{\n    Circle(_) -> 1\n    Rect(_, _) -> 2\n  }}\n}}\n"
    ));
}

#[test]
fn missing_constructor_is_named_with_a_witness() {
    expect_diag(
        "E4001",
        "Rect(_, _)",
        &format!(
            "{SHAPE_PRELUDE}fn f(s: Shape) -> Int {{\n  match s {{\n    Circle(_) -> 1\n  }}\n}}\n"
        ),
    );
}

#[test]
fn nested_witness() {
    expect_diag(
        "E4001",
        "Some2(Rect(_, _))",
        &format!(
            "{SHAPE_PRELUDE}type Opt =\n  | Some2(Shape)\n  | None2\nfn f(o: Opt) -> Int {{\n  match o {{\n    Some2(Circle(_)) -> 1\n    None2 -> 2\n  }}\n}}\n"
        ),
    );
}

#[test]
fn bool_match_missing_false() {
    expect_diag(
        "E4001",
        "False",
        "module M\nfn f(b: Bool) -> Int {\n  match b {\n    True -> 1\n  }\n}\n",
    );
}

#[test]
fn list_nil_and_cons_are_exhaustive() {
    expect_clean(
        "module M\nfn f(xs: List<Int>) -> Int {\n  match xs {\n    [] -> 0\n    [_, ..rest] -> 1 + f(rest)\n  }\n}\n",
    );
}

#[test]
fn list_missing_the_cons_case_renders_as_list_syntax() {
    expect_diag(
        "E4001",
        "[_, ..]",
        "module M\nfn f(xs: List<Int>) -> Int {\n  match xs {\n    [] -> 0\n  }\n}\n",
    );
}

#[test]
fn list_singleton_plus_nil_misses_two_or_more() {
    expect_diag(
        "E4001",
        "[_, _, ..]",
        "module M\nfn f(xs: List<Int>) -> Int {\n  match xs {\n    [] -> 0\n    [_] -> 1\n  }\n}\n",
    );
}

#[test]
fn int_literals_need_a_catch_all() {
    expect_diag(
        "E4001",
        "_",
        "module M\nfn f(n: Int) -> Int {\n  match n {\n    1 -> 1\n    2 -> 2\n  }\n}\n",
    );
}

#[test]
fn int_literals_with_a_final_binder_are_exhaustive() {
    expect_clean(
        "module M\nfn f(n: Int) -> Int {\n  match n {\n    1 -> 1\n    other -> other\n  }\n}\n",
    );
}

// -- redundancy -------------------------------------------------------------------

#[test]
fn arm_after_a_wildcard_is_redundant() {
    expect_diag(
        "W4002",
        "never match",
        &format!(
            "{SHAPE_PRELUDE}fn f(s: Shape) -> Int {{\n  match s {{\n    _ -> 0\n    Circle(_) -> 1\n  }}\n}}\n"
        ),
    );
}

#[test]
fn wildcard_after_a_complete_signature_is_redundant() {
    expect_diag(
        "W4002",
        "never match",
        &format!(
            "{SHAPE_PRELUDE}fn f(s: Shape) -> Int {{\n  match s {{\n    Circle(_) -> 1\n    Rect(_, _) -> 2\n    _ -> 3\n  }}\n}}\n"
        ),
    );
}

#[test]
fn duplicate_literal_arm_is_redundant() {
    expect_diag(
        "W4002",
        "never match",
        "module M\nfn f(n: Int) -> Int {\n  match n {\n    1 -> 1\n    1 -> 2\n    _ -> 0\n  }\n}\n",
    );
}

// -- guards -----------------------------------------------------------------------

#[test]
fn a_guarded_arm_does_not_count_as_coverage() {
    expect_diag(
        "E4001",
        "Rect(_, _)",
        &format!(
            "{SHAPE_PRELUDE}fn f(s: Shape) -> Float {{\n  match s {{\n    Circle(r) -> r\n    Rect(w, h) if w > h -> w\n  }}\n}}\n"
        ),
    );
}

#[test]
fn guarded_arms_plus_an_unguarded_catch_all_are_exhaustive() {
    expect_clean(&format!(
        "{SHAPE_PRELUDE}fn f(s: Shape) -> Float {{\n  match s {{\n    Rect(w, h) if w > h -> w\n    _ -> 0.0\n  }}\n}}\n"
    ));
}

#[test]
fn repeating_a_guarded_arms_pattern_is_not_redundant() {
    expect_clean(
        "module M\nfn f(n: Int) -> Int {\n  match n {\n    x if x > 0 -> x\n    x -> 0 - x\n  }\n}\n",
    );
}

#[test]
fn a_guarded_arm_under_an_earlier_unguarded_wildcard_is_redundant() {
    expect_diag(
        "W4002",
        "never match",
        "module M\nfn f(n: Int) -> Int {\n  match n {\n    _ -> 0\n    x if x > 0 -> x\n  }\n}\n",
    );
}

// -- tree-vs-naive agreement property ------------------------------------------------
// Over a synthetic Shape-like ADT with Float literal sub-patterns and
// optional guards: the compiled decision tree must pick exactly the arm
// a top-to-bottom first-match-whose-guard-passes walk picks.

/// A guard the naive matcher can evaluate: a predicate over the root.
#[derive(Clone, Copy, Debug)]
enum G {
    True,
    False,
    IsCircle,
}

#[derive(Clone, Debug)]
enum Val {
    Con(Name, Vec<Val>),
    Lit(Lit),
}

fn mk_n(u: i64, t: &str) -> Name {
    Name {
        unique: Unique(u),
        text: t.to_string(),
        sort: NameSort::Ctor(ModuleName("T".to_string())),
        def_span: builtin_span(),
    }
}

fn c_circle() -> Name {
    mk_n(-201, "Circle")
}

fn c_rect() -> Name {
    mk_n(-202, "Rect")
}

fn siblings_t(n: &Name) -> Option<Vec<(Name, usize)>> {
    if *n == c_circle() || *n == c_rect() {
        Some(vec![(c_circle(), 1), (c_rect(), 2)])
    } else {
        None
    }
}

fn g_pred(g: G, v: &Val) -> bool {
    match g {
        G::True => true,
        G::False => false,
        G::IsCircle => matches!(v, Val::Con(c, _) if *c == c_circle()),
    }
}

fn matches_p(p: &SP, v: &Val) -> bool {
    match (p, v) {
        (SP::Wild, _) => true,
        (SP::Lit(l), Val::Lit(l2)) => l == l2,
        (SP::Lit(_), _) => false,
        (SP::Ctor(c, _, ps), Val::Con(c2, vs)) => {
            c == c2 && ps.iter().zip(vs).all(|(p2, v2)| matches_p(p2, v2))
        }
        (SP::Ctor(_, _, _), _) => false,
    }
}

fn naive(arms: &[(SP, Option<G>)], val: &Val) -> Option<usize> {
    arms.iter()
        .enumerate()
        .find(|(_, (p, g))| matches_p(p, val) && g.map_or(true, |g2| g_pred(g2, val)))
        .map(|(i, _)| i)
}

fn val_at(occ: &Occ, v: &Val) -> Val {
    match occ {
        Occ::Root => v.clone(),
        Occ::Arg(i, _, p) => match val_at(p, v) {
            Val::Con(_, args) => args[*i].clone(),
            other => other,
        },
    }
}

fn eval_tree(guards: &[Option<G>], tree: &DTree, val: &Val) -> Option<usize> {
    match tree {
        DTree::Fail => None,
        DTree::Leaf(i, _) => Some(*i),
        DTree::Guard(i, _, fb) => match guards[*i] {
            Some(g) if g_pred(g, val) => Some(*i),
            _ => eval_tree(guards, fb, val),
        },
        DTree::SwitchCtor(occ, branches, deflt) => match val_at(occ, val) {
            Val::Con(c, _) => match branches.iter().find(|(c2, _, _)| *c2 == c) {
                Some((_, _, t)) => eval_tree(guards, t, val),
                None => deflt.as_ref().and_then(|t| eval_tree(guards, t, val)),
            },
            Val::Lit(_) => deflt.as_ref().and_then(|t| eval_tree(guards, t, val)),
        },
        DTree::SwitchLit(occ, branches, deflt) => match val_at(occ, val) {
            Val::Lit(l) => match branches.iter().find(|(l2, _)| *l2 == l) {
                Some((_, t)) => eval_tree(guards, t, val),
                None => eval_tree(guards, deflt, val),
            },
            Val::Con(_, _) => eval_tree(guards, deflt, val),
        },
    }
}

mod agreement {
    use super::*;
    use proptest::prelude::*;

    fn gen_lit() -> impl Strategy<Value = Lit> {
        prop_oneof![Just(Lit::Float(F64(1.0))), Just(Lit::Float(F64(2.0))),]
    }

    fn gen_arg_pat() -> impl Strategy<Value = SP> {
        prop_oneof![
            2 => Just(SP::Wild),
            3 => gen_lit().prop_map(SP::Lit),
        ]
    }

    fn gen_pat() -> impl Strategy<Value = SP> {
        prop_oneof![
            2 => Just(SP::Wild),
            3 => gen_arg_pat().prop_map(|a| SP::Ctor(c_circle(), 1, vec![a])),
            3 => (gen_arg_pat(), gen_arg_pat())
                .prop_map(|(a, b)| SP::Ctor(c_rect(), 2, vec![a, b])),
        ]
    }

    fn gen_guard() -> impl Strategy<Value = Option<G>> {
        prop_oneof![
            5 => Just(None),
            1 => Just(Some(G::True)),
            1 => Just(Some(G::False)),
            2 => Just(Some(G::IsCircle)),
        ]
    }

    fn gen_val() -> impl Strategy<Value = Val> {
        prop_oneof![
            gen_lit().prop_map(|l| Val::Con(c_circle(), vec![Val::Lit(l)])),
            (gen_lit(), gen_lit())
                .prop_map(|(a, b)| Val::Con(c_rect(), vec![Val::Lit(a), Val::Lit(b)])),
        ]
    }

    proptest! {
        #[test]
        fn decision_tree_agrees_with_first_match_semantics(
            arms_g in proptest::collection::vec((gen_pat(), gen_guard()), 1..7),
            val in gen_val(),
        ) {
            let guards: Vec<Option<G>> = arms_g.iter().map(|(_, g)| *g).collect();
            let guarded = |i: usize| guards[i].is_some();
            let rows: Vec<(Vec<SP>, usize, Vec<(Name, Occ)>)> = arms_g
                .iter()
                .enumerate()
                .map(|(i, (p, _))| (vec![p.clone()], i, Vec::new()))
                .collect();
            let tree = compile_tree(&siblings_t, &guarded, &[Occ::Root], &rows);
            prop_assert_eq!(eval_tree(&guards, &tree, &val), naive(&arms_g, &val));
        }
    }
}
