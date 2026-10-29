//! Ported from `compiler/test/RenamerTests.hs`.

use std::collections::BTreeMap;

use super::{RenameEnv, rename_module};
use crate::ast::Module;
use crate::builtin::*;
use crate::cst::lower::lower_module;
use crate::diag::Diagnostic;
use crate::names::{ModuleExports, ModuleName, Name, NameSort};
use crate::parser::parse_module;
use crate::span::FileId;
use crate::unique::Unique;

const FID: FileId = FileId(0);

fn fake(u: i64, t: &str, sort: NameSort) -> Name {
    Name {
        unique: Unique(u),
        text: t.to_string(),
        sort,
        def_span: builtin_span(),
    }
}

fn fake_val(u: i64, t: &str) -> Name {
    fake(u, t, NameSort::TopVal(ModuleName("Test".to_string())))
}

fn fake_ctor(u: i64, t: &str) -> Name {
    fake(u, t, NameSort::Ctor(ModuleName("Test".to_string())))
}

fn fake_ty(u: i64, t: &str) -> Name {
    fake(u, t, NameSort::TyCon(ModuleName("Test".to_string())))
}

fn named(ns: Vec<Name>) -> BTreeMap<String, Name> {
    ns.into_iter().map(|n| (n.text.clone(), n)).collect()
}

/// A test environment: builtins plus a fake stdlib List and prelude
/// Option/Result, plus an importable user module Geometry.
fn test_env() -> RenameEnv {
    let list_exports = ModuleExports {
        values: named(vec![
            fake_val(-110, "map"),
            fake_val(-111, "sum"),
            fake_val(-112, "foldl"),
        ]),
        ctors: named(vec![ctor_cons(), ctor_nil()]),
        types: named(vec![ty_list()]),
        components: BTreeMap::new(),
    };
    let geom_exports = ModuleExports {
        values: named(vec![fake_val(-120, "area")]),
        ctors: named(vec![fake_ctor(-121, "Circle"), fake_ctor(-122, "Rect")]),
        types: named(vec![fake_ty(-123, "Shape")]),
        components: BTreeMap::new(),
    };
    let mut auto = builtin_modules();
    auto.insert(ModuleName("List".to_string()), list_exports);
    let mut prelude_ctors = builtin_ctors();
    for n in [
        fake_ctor(-101, "Some"),
        fake_ctor(-102, "None"),
        fake_ctor(-103, "Ok"),
        fake_ctor(-104, "Err"),
    ] {
        prelude_ctors.insert(n.text.clone(), n);
    }
    RenameEnv {
        auto_modules: auto,
        user_modules: [(ModuleName("Geometry".to_string()), geom_exports)]
            .into_iter()
            .collect(),
        prelude_ctors,
        prelude_types: builtin_types(),
    }
}

fn rn(src: &str) -> Result<(Module<Name>, ModuleExports), Vec<Diagnostic>> {
    let cst = parse_module(FID, src).map_err(|d| vec![d])?;
    let ast = lower_module(&cst).map_err(|d| vec![d])?;
    let (m, exports, _seed) = rename_module(&test_env(), 0, &ast)?;
    Ok((m, exports))
}

fn rn_ok(src: &str) -> (Module<Name>, ModuleExports) {
    match rn(src) {
        Ok(r) => r,
        Err(ds) => panic!("unexpected rename failure: {ds:?}"),
    }
}

fn has_code<T>(c: &str, r: &Result<T, Vec<Diagnostic>>) -> bool {
    match r {
        Err(ds) => ds.iter().any(|d| d.code.0 == c),
        Ok(_) => false,
    }
}

fn hints_contain<T>(needle: &str, r: &Result<T, Vec<Diagnostic>>) -> bool {
    match r {
        Err(ds) => ds
            .iter()
            .any(|d| d.hints.iter().any(|h| h.contains(needle))),
        Ok(_) => false,
    }
}

const GEOMETRY_SAMPLE: &str = "module Geometry\n\ntype Shape =\n  | Circle(Float)\n  | Rect(Float, Float)\n\ntype User = { name: String, age: Int }\n\nfn area(s: Shape) -> Float {\n  match s {\n    Circle(r) -> 3.14 * r * r\n    Rect(w, h) -> w * h\n  }\n}\n\nfn main() {\n  [Circle(1.0), Rect(2.0, 3.0)]\n  |> List.map(area)\n  |> List.sum\n  |> Debug.log\n}\n";

// -- successful renames ---------------------------------------------------------

#[test]
fn the_canonical_sample_renames_cleanly() {
    let (_m, exports) = rn_ok(GEOMETRY_SAMPLE);
    let value_keys: Vec<&String> = exports.values.keys().collect();
    assert_eq!(value_keys, vec!["area", "main"]);
    let ctor_keys: Vec<&String> = exports.ctors.keys().collect();
    assert_eq!(ctor_keys, vec!["Circle", "Rect", "User"]);
    let type_keys: Vec<&String> = exports.types.keys().collect();
    assert_eq!(type_keys, vec!["Shape", "User"]);
}

#[test]
fn imports_make_qualified_names_work() {
    let src = "module Main\n\nimport Geometry\n\nfn f() {\n  Geometry.area(Geometry.Circle(1.0))\n}\n";
    rn(src).unwrap_or_else(|ds| panic!("{ds:?}"));
}

#[test]
fn qualified_constructors_in_patterns() {
    let src = "module Main\n\nimport Geometry\n\nfn f(s: Shape) -> Float {\n  match s {\n    Geometry.Circle(r) -> r\n    _ -> 0.0\n  }\n}\n";
    rn(src).unwrap_or_else(|ds| panic!("{ds:?}"));
}

#[test]
fn shadowing_let_over_param_lambda_over_let() {
    let src = "module M\n\nfn f(x: Int) -> Int {\n  let x = x + 1\n  let g = fn(x) { x * 2 }\n  g(x)\n}\n";
    rn(src).unwrap_or_else(|ds| panic!("{ds:?}"));
}

#[test]
fn top_level_let_can_use_any_fn_and_earlier_lets() {
    let src = "module M\n\nlet a = helper()\n\nlet b = a\n\nfn helper() -> Int {\n  1\n}\n";
    rn(src).unwrap_or_else(|ds| panic!("{ds:?}"));
}

#[test]
fn list_spread_binders_bind_in_body() {
    let src = "module M\n\nfn f(xs: List<Int>) -> Int {\n  match xs {\n    [] -> 0\n    [x, ..rest] -> x + f(rest)\n  }\n}\n";
    rn(src).unwrap_or_else(|ds| panic!("{ds:?}"));
}

// -- rename errors ----------------------------------------------------------------

#[test]
fn unbound_value_with_did_you_mean() {
    let r = rn("module M\n\nfn area(s: Int) -> Int {\n  s\n}\n\nfn f() {\n  aera(1)\n}\n");
    assert!(has_code("E2001", &r), "code E2001");
    assert!(hints_contain("area", &r), "suggests area");
}

#[test]
fn top_level_let_used_before_defined() {
    let r = rn("module M\n\nlet a = b\n\nlet b = 1\n");
    assert!(has_code("E2006", &r), "code E2006");
}

#[test]
fn duplicate_definitions() {
    let r = rn("module M\n\nfn f() {\n  1\n}\n\nfn f() {\n  2\n}\n");
    assert!(has_code("E2004", &r), "code E2004");
}

#[test]
fn module_member_typo_suggests_the_member() {
    let r = rn("module M\n\nfn f() {\n  List.mapp([], fn(x) { x })\n}\n");
    assert!(has_code("E2003", &r), "code E2003");
    assert!(hints_contain("map", &r), "suggests map");
}

#[test]
fn unimported_module_suggests_the_import() {
    let r = rn("module M\n\nfn f() {\n  Geometry.area(1.0)\n}\n");
    assert!(has_code("E2002", &r), "code E2002");
    assert!(hints_contain("import Geometry", &r), "suggests import");
}

#[test]
fn unknown_constructor_in_pattern_with_suggestion() {
    let r = rn(
        "module M\n\ntype Shape =\n  | Circle(Float)\n\nfn f(s: Shape) -> Float {\n  match s {\n    Circel(r) -> r\n  }\n}\n",
    );
    assert!(has_code("E2005", &r), "code E2005");
    assert!(hints_contain("Circle", &r), "suggests Circle");
}

#[test]
fn duplicate_pattern_binder() {
    let r = rn(
        "module M\n\ntype Shape =\n  | Rect(Float, Float)\n\nfn f(s: Shape) -> Float {\n  match s {\n    Rect(w, w) -> w\n  }\n}\n",
    );
    assert!(has_code("E2008", &r), "code E2008");
}

#[test]
fn block_without_result() {
    let r = rn("module M\n\nfn f() {\n  let x = 1\n}\n");
    assert!(has_code("E1106", &r), "code E1106");
}

#[test]
fn underscore_as_expression() {
    let r = rn("module M\n\nlet x = [_]\n");
    assert!(has_code("E1103", &r), "code E1103");
}

#[test]
fn multiple_independent_errors_in_one_run() {
    match rn("module M\n\nfn f() {\n  nope1\n}\n\nfn g() {\n  nope2\n}\n") {
        Err(ds) => assert_eq!(ds.len(), 2),
        Ok(_) => panic!("expected two errors"),
    }
}
