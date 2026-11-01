//! Ported from `compiler/test/TypesTests.hs` — test names match the
//! Haskell cases for diffability. Full front-end: parse, lower, rename,
//! typecheck.

use std::collections::BTreeMap;

use super::check_module;
use crate::builtin::{builtin_ctors, builtin_modules, builtin_types};
use crate::cst::lower::lower_module;
use crate::diag::Diagnostic;
use crate::intrinsics::base_tc_env;
use crate::parser::parse_module;
use crate::rename::{RenameEnv, rename_module};
use crate::span::FileId;
use crate::types::ty::render_scheme;

fn rn_env() -> RenameEnv {
    RenameEnv {
        auto_modules: builtin_modules(),
        user_modules: BTreeMap::new(),
        prelude_ctors: builtin_ctors(),
        prelude_types: builtin_types(),
    }
}

/// Full front-end: parse, lower, rename, typecheck. Returns rendered
/// schemes by surface name on success.
fn tc(src: &str) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let cst = parse_module(FileId(0), src).map_err(|d| vec![d])?;
    let ast = lower_module(&cst).map_err(|d| vec![d])?;
    let (m, exports, _seed) = rename_module(&rn_env(), 0, &ast)?;
    let (res, _arts, mut arena) = check_module(&base_tc_env(), &m);
    match res {
        Err(ds) => Err(ds),
        Ok(iface) => Ok(exports
            .values
            .iter()
            .map(|(txt, n)| {
                let rendered = match iface.values.get(&n.unique) {
                    Some(sch) => render_scheme(&mut arena, sch),
                    None => "<missing>".to_string(),
                };
                (txt.clone(), rendered)
            })
            .collect()),
    }
}

fn tc_ok(src: &str) -> BTreeMap<String, String> {
    match tc(src) {
        Ok(m) => m,
        Err(ds) => panic!("unexpected type errors: {ds:#?}"),
    }
}

#[track_caller]
fn expect_scheme(name: &str, want: &str, schemes: &BTreeMap<String, String>) {
    assert_eq!(
        schemes.get(name).map(String::as_str),
        Some(want),
        "scheme of {name}"
    );
}

#[track_caller]
fn expect_error(code: &str, src: &str) {
    match tc(src) {
        Err(ds) => assert!(
            ds.iter().any(|d| d.code.0 == code),
            "expected {code}, got {:?}",
            ds.iter().map(|d| d.code.0).collect::<Vec<_>>()
        ),
        Ok(schemes) => panic!("expected an error, got: {schemes:?}"),
    }
}

#[track_caller]
fn expect_error_with(code: &str, p: impl Fn(&Diagnostic) -> bool, src: &str) {
    match tc(src) {
        Err(ds) => {
            assert!(
                ds.iter().any(|d| d.code.0 == code),
                "expected {code}, got {:?}",
                ds.iter().map(|d| d.code.0).collect::<Vec<_>>()
            );
            assert!(ds.iter().any(p), "predicate over diagnostics: {ds:#?}");
        }
        Ok(_) => panic!("expected an error"),
    }
}

fn hints_contain(d: &Diagnostic, needle: &str) -> bool {
    d.hints.iter().any(|h| h.contains(needle))
}

// -- well-typed programs --------------------------------------------------------

#[test]
fn the_canonical_program_self_contained() {
    let schemes = tc_ok(
        "module Geometry\ntype Shape =\n  | Circle(Float)\n  | Rect(Float, Float)\ntype User = { name: String, age: Int }\nfn area(s: Shape) -> Float {\n  match s {\n    Circle(r) -> 3.14 * r * r\n    Rect(w, h) -> w * h\n  }\n}\nfn map(xs: List<a>, f: (a) -> b) -> List<b> {\n  match xs {\n    [] -> []\n    [x, ..rest] -> [f(x), ..map(rest, f)]\n  }\n}\nfn sum(xs: List<number>) -> number {\n  match xs {\n    [] -> 0\n    [x, ..rest] -> x + sum(rest)\n  }\n}\nfn main() {\n  [Circle(1.0), Rect(2.0, 3.0)]\n  |> map(area)\n  |> sum\n  |> Debug.log\n}\n",
    );
    expect_scheme("area", "(s: Shape) -> Float", &schemes);
    expect_scheme("sum", "(xs: List<number>) -> number", &schemes);
    // The Num variable instantiates to Float through the pipeline.
    expect_scheme("main", "() -> Float", &schemes);
}

#[test]
fn let_polymorphism_one_id_two_types() {
    let schemes = tc_ok(
        "module M\nfn f() -> Bool {\n  let id = fn(x) { x }\n  id(1) == 1 && id(\"s\") == \"s\"\n}\n",
    );
    expect_scheme("f", "() -> Bool", &schemes);
}

#[test]
fn records_construction_and_field_access() {
    let schemes = tc_ok(
        "module M\ntype User = { name: String, age: Int }\nfn f() -> String {\n  let u = User(name: \"Ada\", age: 36)\n  \"${u.name}${Int.toString(u.age)}\"\n}\n",
    );
    expect_scheme("f", "() -> String", &schemes);
}

#[test]
fn field_access_resolves_through_an_annotation() {
    tc_ok(
        "module M\ntype User = { name: String, age: Int }\nfn name(u: User) -> String {\n  u.name\n}\n",
    );
}

#[test]
fn generic_option_like_type_round_trips() {
    let schemes = tc_ok(
        "module M\ntype Opt<a> =\n  | Just2(a)\n  | Nothing2\nfn withDefault(o: Opt<a>, d: a) -> a {\n  match o {\n    Just2(x) -> x\n    Nothing2 -> d\n  }\n}\nfn g() -> Int {\n  withDefault(Just2(1), 0)\n}\n",
    );
    expect_scheme("withDefault", "(o: Opt<a>, d: a) -> a", &schemes);
}

#[test]
fn unannotated_fn_generalizes() {
    let schemes = tc_ok("module M\nfn pair(x) {\n  [x, x]\n}\n");
    expect_scheme("pair", "(x: a) -> List<a>", &schemes);
}

#[test]
fn top_level_numeric_let_stays_a_number() {
    let schemes = tc_ok("module M\nlet n = 1 + 1\n");
    expect_scheme("n", "number", &schemes);
}

#[test]
fn record_update_preserves_the_record_type() {
    let schemes = tc_ok(
        "module M\ntype User = { name: String, age: Int }\nfn birthday(u: User) -> User {\n  User(..u, age: u.age + 1)\n}\nfn copy(u: User) -> User {\n  User(..u)\n}\n",
    );
    expect_scheme("birthday", "(u: User) -> User", &schemes);
    expect_scheme("copy", "(u: User) -> User", &schemes);
}

#[test]
fn match_guards_see_pattern_binders() {
    let schemes = tc_ok(
        "module M\nfn sign(n: Int) -> Int {\n  match n {\n    x if x > 0 -> 1\n    0 -> 0\n    _ -> 0 - 1\n  }\n}\n",
    );
    expect_scheme("sign", "(n: Int) -> Int", &schemes);
}

#[test]
fn update_of_a_generic_record_keeps_its_parameter() {
    let schemes = tc_ok(
        "module M\ntype Box<a> = { value: a, tag: String }\nfn retag(b: Box<a>, t: String) -> Box<a> {\n  Box(..b, tag: t)\n}\n",
    );
    expect_scheme("retag", "(b: Box<a>, t: String) -> Box<a>", &schemes);
}

// -- overloaded numerics ----------------------------------------------------------

#[test]
fn int_literal_blends_with_float_operand() {
    let schemes = tc_ok("module M\nfn f() -> Float {\n  1.5 + 1\n}\n");
    expect_scheme("f", "() -> Float", &schemes);
}

#[test]
fn int_and_float_values_do_not_mix() {
    expect_error_with(
        "E3002",
        |d| hints_contain(d, "never converts"),
        "module M\nfn f(x: Int, y: Float) -> Float {\n  x + y\n}\n",
    );
}

#[test]
fn strings_are_not_numbers_hint_points_at_interpolation() {
    expect_error_with(
        "E3003",
        |d| hints_contain(d, "${a}${b}"),
        "module M\nlet s = \"a\" + \"b\"\n",
    );
}

#[test]
fn comparison_needs_numbers() {
    expect_error("E3003", "module M\nlet b = \"a\" < \"b\"\n");
}

#[test]
fn div_escapes_to_float_even_on_int_operands() {
    let schemes = tc_ok("module M\nfn f() -> Float {\n  7 / 2\n}\n");
    expect_scheme("f", "() -> Float", &schemes);
}

#[test]
fn pow_escapes_to_float_and_stays_homogeneous() {
    let schemes = tc_ok("module M\nfn f(x) {\n  x ** 2\n}\n");
    expect_scheme("f", "(x: number) -> Float", &schemes);
}

#[test]
fn intdiv_takes_either_flavor_yields_int() {
    let schemes = tc_ok("module M\nfn halve(n) {\n  n ~/ 2\n}\n");
    expect_scheme("halve", "(n: number) -> Int", &schemes);
}

#[test]
fn mod_preserves_the_operand_flavor() {
    let schemes = tc_ok("module M\nfn wrap(n) {\n  n % 2\n}\n");
    expect_scheme("wrap", "(n: number) -> number", &schemes);
}

#[test]
fn div_still_rejects_mixed_operands() {
    expect_error(
        "E3002",
        "module M\nfn f(x: Int, y: Float) -> Float {\n  x / y\n}\n",
    );
}

// -- type errors ------------------------------------------------------------------

#[test]
fn skolem_escape_caller_chooses_a() {
    expect_error_with(
        "E3002",
        |d| hints_contain(d, "caller"),
        "module M\nfn f(x: a) -> a {\n  1\n}\n",
    );
}

#[test]
fn wrong_number_of_arguments() {
    expect_error(
        "E3005",
        "module M\nfn g(x: Int) -> Int {\n  x\n}\nfn h() -> Int {\n  g(1, 2)\n}\n",
    );
}

#[test]
fn calling_a_non_function() {
    expect_error("E3006", "module M\nlet x = 1(2)\n");
}

#[test]
fn return_type_mismatch() {
    expect_error_with(
        "E3002",
        |d| d.title == "RETURN MISMATCH",
        "module M\nfn f() -> String {\n  1\n}\n",
    );
}

#[test]
fn if_branches_must_agree() {
    expect_error_with(
        "E3002",
        |d| d.title == "BRANCH MISMATCH",
        "module M\nlet x = if True { 1 } else { \"one\" }\n",
    );
}

#[test]
fn match_arms_must_agree() {
    expect_error_with(
        "E3002",
        |d| d.title == "ARM MISMATCH",
        "module M\nfn f(b: Bool) -> Int {\n  match b {\n    True -> 1\n    False -> \"zero\"\n  }\n}\n",
    );
}

#[test]
fn infinite_type() {
    expect_error(
        "E3004",
        "module M\nfn f() -> Int {\n  let g = fn(x) { x(x) }\n  1\n}\n",
    );
}

#[test]
fn pattern_arity() {
    expect_error(
        "E3012",
        "module M\ntype Shape =\n  | Circle(Float)\nfn f(s: Shape) -> Int {\n  match s {\n    Circle -> 1\n  }\n}\n",
    );
}

#[test]
fn unknown_type_with_suggestion() {
    expect_error_with(
        "E3001",
        |d| hints_contain(d, "Shape"),
        "module M\ntype Shape =\n  | Circle(Float)\nfn f(s: Shap) -> Int {\n  1\n}\n",
    );
}

#[test]
fn wrong_number_of_type_arguments() {
    expect_error("E3013", "module M\nfn f(xs: List) -> Int {\n  1\n}\n");
}

#[test]
fn record_a_missing_field_builds_a_partial_constructor() {
    let schemes = tc_ok(
        "module M\ntype User = { name: String, age: Int }\nlet partial = User(name: \"A\")\nlet complete = partial(age: 30)\n",
    );
    expect_scheme("partial", "(age: Int) -> User", &schemes);
    expect_scheme("complete", "User", &schemes);
}

#[test]
fn record_unknown_field_with_suggestion() {
    expect_error(
        "E3017",
        "module M\ntype User = { name: String, age: Int }\nlet u = User(name: \"A\", age: 1, agee: 2)\n",
    );
}

#[test]
fn field_access_on_unknown_record_is_ambiguous() {
    expect_error("E3010", "module M\nfn f(u) -> String {\n  u.name\n}\n");
}

#[test]
fn unknown_field_access_with_suggestion() {
    expect_error_with(
        "E3009",
        |d| hints_contain(d, "name"),
        "module M\ntype User = { name: String, age: Int }\nfn f(u: User) -> String {\n  u.nam\n}\n",
    );
}

#[test]
fn reserved_number_as_a_type_parameter() {
    expect_error("E3015", "module M\ntype Box<number> =\n  | B(number)\n");
}

#[test]
fn named_arguments_work_on_a_plain_function_any_order() {
    let schemes = tc_ok(
        "module M\nfn scale(dx: Int, dy: Int) -> Int {\n  dx * 10 + dy\n}\nlet a = scale(dy: 2, dx: 1)\nlet b = scale(1, 2)\n",
    );
    expect_scheme("a", "Int", &schemes);
    expect_scheme("b", "Int", &schemes);
}

#[test]
fn named_arguments_work_on_a_let_bound_lambda() {
    let schemes = tc_ok("module M\nlet myfn = fn(x: Int) -> Int { x + 1 }\nlet y = myfn(x: 1)\n");
    expect_scheme("y", "Int", &schemes);
}

#[test]
fn unknown_parameter_label_with_a_suggestion() {
    expect_error_with(
        "E3025",
        |d| hints_contain(d, "dx"),
        "module M\nfn g(dx: Int) -> Int {\n  dx\n}\nlet y = g(dz: 1)\n",
    );
}

#[test]
fn duplicate_labeled_argument() {
    expect_error(
        "E3018",
        "module M\nfn g(x: Int, y: Int) -> Int {\n  x + y\n}\nlet z = g(x: 1, x: 2)\n",
    );
}

#[test]
fn labeled_call_on_an_unresolved_callee_asks_for_an_annotation() {
    expect_error("E3026", "module M\nfn h(f) {\n  f(x: 1)\n}\n");
}

#[test]
fn a_labeled_call_leaves_unnamed_parameters_for_the_closure() {
    let schemes = tc_ok("module M\nlet g = fn(_: Int, y: Int) -> Int { y }\nlet z = g(y: 2)\n");
    expect_scheme("z", "(Int) -> Int", &schemes);
}

#[test]
fn labels_never_block_assignability() {
    let schemes = tc_ok(
        "module M\nfn apply(f: (Int) -> Int, v: Int) -> Int {\n  f(v)\n}\nfn double(amount: Int) -> Int {\n  amount * 2\n}\nlet r = apply(double, 3)\n",
    );
    expect_scheme("r", "Int", &schemes);
}

#[test]
fn branch_labels_that_disagree_are_stripped_at_the_join() {
    let schemes = tc_ok(
        "module M\nfn add(x: Int, y: Int) -> Int {\n  x + y\n}\nfn sub(a: Int, b: Int) -> Int {\n  a - b\n}\nfn pick(neg: Bool) -> (Int, Int) -> Int {\n  if neg { sub } else { add }\n}\n",
    );
    expect_scheme("pick", "(neg: Bool) -> (Int, Int) -> Int", &schemes);
}

#[test]
fn positional_partial_application_takes_the_prefix() {
    let schemes = tc_ok(
        "module M\nfn add(x: Int, y: Int) -> Int {\n  x + y\n}\nlet inc = add(1)\nlet three = inc(2)\n",
    );
    expect_scheme("inc", "(y: Int) -> Int", &schemes);
    expect_scheme("three", "Int", &schemes);
}

#[test]
fn labeled_partial_keeps_the_remainder_in_declaration_order() {
    let schemes = tc_ok(
        "module M\nfn f(a: Int, b: String, c: Bool) -> Int {\n  a\n}\nlet g = f(b: \"s\")\nlet h = g(c: True)\nlet i = h(1)\n",
    );
    expect_scheme("g", "(a: Int, c: Bool) -> Int", &schemes);
    expect_scheme("h", "(a: Int) -> Int", &schemes);
    expect_scheme("i", "Int", &schemes);
}

#[test]
fn over_applying_a_partial_is_an_arity_error() {
    expect_error(
        "E3005",
        "module M\nfn add(x: Int, y: Int) -> Int {\n  x + y\n}\nlet bad = add(y: 2)(5, 6)\n",
    );
}

#[test]
fn zero_arguments_is_never_a_partial() {
    expect_error(
        "E3005",
        "module M\nfn add(x: Int, y: Int) -> Int {\n  x + y\n}\nlet bad = add()\n",
    );
}

#[test]
fn unresolved_callee_means_exact_arity() {
    expect_error_with(
        "E3002",
        |d| hints_contain(d, "partial application"),
        "module M\nfn use(f) {\n  let g = f(1)\n  g(2)\n}\nfn add(x: Int, y: Int) -> Int {\n  x + y\n}\nlet bad = use(add)\n",
    );
}

#[test]
fn a_pipe_subject_fills_the_first_parameter_of_a_labeled_call() {
    let schemes = tc_ok(
        "module M\nfn scale(v: Int, by: Int, plus: Int) -> Int {\n  v * by + plus\n}\nlet r = 3 |> scale(plus: 1, by: 2)\n",
    );
    expect_scheme("r", "Int", &schemes);
}

#[test]
fn the_pipe_subjects_parameter_cannot_be_renamed() {
    expect_error(
        "E3025",
        "module M\nfn scale(v: Int, by: Int) -> Int {\n  v * by\n}\nlet bad = 3 |> scale(v: 2)\n",
    );
}

#[test]
fn parens_opt_a_piped_call_out_of_prepending() {
    let schemes =
        tc_ok("module M\nfn add(x: Int, y: Int) -> Int {\n  x + y\n}\nlet r = 3 |> (add(1))\n");
    expect_scheme("r", "Int", &schemes);
}

#[test]
fn partial_construction_completes_later() {
    let schemes = tc_ok(
        "module M\ntype Point = { x: Int, y: Int, tag: String }\nlet part = Point(tag: \"origin\")\nlet done = part(x: 0, y: 0)\n",
    );
    expect_scheme("part", "(x: Int, y: Int) -> Point", &schemes);
    expect_scheme("done", "Point", &schemes);
}

#[test]
fn casts_numeric_sugar_identity_and_user_conversions() {
    let schemes = tc_ok(
        "module M\ntype Celsius = { degrees: Float }\ncast (d: Float) -> Celsius {\n  Celsius(degrees: d)\n}\ncast (c: Celsius) -> String {\n  \"${c.degrees::String} degrees\"\n}\nlet widened = 1::Float\nlet truncated = 1.9::Int\nlet shown = 42::String\nlet celsius = 21.5::Celsius\nlet round = 21.5::Celsius::String\nlet same = 7::Int\n",
    );
    expect_scheme("widened", "Float", &schemes);
    expect_scheme("truncated", "Int", &schemes);
    expect_scheme("shown", "String", &schemes);
    expect_scheme("celsius", "Celsius", &schemes);
    expect_scheme("round", "String", &schemes);
    expect_scheme("same", "Int", &schemes);
}

#[test]
fn a_cast_without_its_conversion_names_the_missing_function() {
    expect_error(
        "E3028",
        "module M\ntype User = { name: String }\nlet bad = 1::User\n",
    );
}

#[test]
fn an_ordinary_function_is_never_a_cast() {
    expect_error(
        "E3028",
        "module M\ntype Port = { value: Int }\nfn fromInt(n: Int) -> Port {\n  Port(value: n)\n}\nlet bad = 1::Port\n",
    );
}

#[test]
fn duplicate_casts_for_one_pair_are_a_declaration_error() {
    expect_error(
        "E3030",
        "module M\ntype Port = { value: Int }\ncast (n: Int) -> Port {\n  Port(value: n)\n}\ncast (m: Int) -> Port {\n  Port(value: m + 1)\n}\n",
    );
}

#[test]
fn casts_between_built_in_types_have_no_home() {
    expect_error("E3031", "module M\ncast (n: Int) -> Bool {\n  n > 0\n}\n");
}

#[test]
fn an_identity_cast_is_rejected() {
    expect_error(
        "E3032",
        "module M\ntype Port = { value: Int }\ncast (p: Port) -> Port {\n  p\n}\n",
    );
}

#[test]
fn a_generic_cast_owns_its_head_pair() {
    let schemes = tc_ok(
        "module M\ntype Size = { count: Int }\ncast (xs: List<a>) -> Size {\n  match xs {\n    [] -> Size(count: 0)\n    [_, ..rest] -> Size(count: 1 + rest::Size.count)\n  }\n}\nlet s = [1, 2, 3]::Size\n",
    );
    expect_scheme("s", "Size", &schemes);
}

#[test]
fn a_cast_needs_a_resolved_source() {
    expect_error("E3027", "module M\nfn f(x) {\n  x::String\n}\n");
}

#[test]
fn an_annotation_can_name_parameters_for_a_lambda_value() {
    let schemes = tc_ok(
        "module M\nlet f: (x: Int, y: Int) -> Int = fn(a, b) { a + b }\nlet z = f(y: 2, x: 1)\n",
    );
    expect_scheme("z", "Int", &schemes);
}

#[test]
fn update_base_must_be_the_record() {
    expect_error_with(
        "E3002",
        |d| d.title == "UPDATE MISMATCH",
        "module M\ntype User = { name: String, age: Int }\ntype P = { x: Float }\nfn f(p: P) -> User {\n  User(..p, age: 1)\n}\n",
    );
}

#[test]
fn update_with_an_unknown_field() {
    expect_error(
        "E3017",
        "module M\ntype User = { name: String, age: Int }\nfn f(u: User) -> User {\n  User(..u, agee: 1)\n}\n",
    );
}

#[test]
fn update_cannot_change_a_type_parameter() {
    expect_error(
        "E3002",
        "module M\ntype Box<a> = { value: a }\nfn f(b: Box<Int>) -> Box<String> {\n  Box(..b, value: \"s\")\n}\n",
    );
}

#[test]
fn update_on_a_variant_constructor() {
    expect_error(
        "E3020",
        "module M\ntype Shape =\n  | Circle(Float)\nfn f(s: Shape) -> Shape {\n  Circle(..s)\n}\n",
    );
}

#[test]
fn a_match_guard_must_be_bool() {
    expect_error(
        "E3002",
        "module M\nfn f(n: Int) -> Int {\n  match n {\n    x if x + 1 -> x\n    _ -> 0\n  }\n}\n",
    );
}

#[test]
fn a_field_written_twice_in_an_update_is_duplicate_never_unknown() {
    match tc(
        "module M\ntype User = { name: String, age: Int }\nfn f(u: User) -> User {\n  User(..u, age: 1, age: 2)\n}\n",
    ) {
        Err(ds) => {
            let codes: Vec<&str> = ds.iter().map(|d| d.code.0).collect();
            assert!(codes.contains(&"E3018"), "reports DUPLICATE FIELD");
            assert!(
                !codes.contains(&"E3017"),
                "does not falsely report UNKNOWN FIELD"
            );
        }
        Ok(_) => panic!("expected a duplicate-field error"),
    }
}

// -- postfix ? --------------------------------------------------------------------
// Self-contained: these run without the stdlib, so each module declares
// its own Result/Option — which also proves the checker's structural
// resolution (same shape = same behavior as the prelude's).

const RESULT_DECL: &str = "type Result<e, a> =\n  | Ok(a)\n  | Err(e)\n";

#[test]
fn a_try_chain_unwraps_and_re_wraps() {
    let schemes = tc_ok(&format!(
        "module M\n{RESULT_DECL}fn parse(s: String) -> Result<String, Int> {{\n  Ok(String.length(s))\n}}\nfn total(a: String, b: String) -> Result<String, Int> {{\n  let x = parse(a)?\n  let y = parse(b)?\n  Ok(x + y)\n}}\n"
    ));
    expect_scheme(
        "total",
        "(a: String, b: String) -> Result<String, Int>",
        &schemes,
    );
}

#[test]
fn the_return_type_drives_an_unannotated_operand() {
    let schemes = tc_ok(&format!(
        "module M\n{RESULT_DECL}fn bump(x) -> Result<String, Int> {{\n  Ok(x? + 1)\n}}\n"
    ));
    expect_scheme(
        "bump",
        "(x: Result<String, Int>) -> Result<String, Int>",
        &schemes,
    );
}

#[test]
fn try_in_an_unannotated_fn_forces_its_result_return() {
    let schemes = tc_ok(&format!(
        "module M\n{RESULT_DECL}fn shout(r: Result<String, String>) {{\n  Ok(\"${{r?}}!\")\n}}\n"
    ));
    expect_scheme(
        "shout",
        "(r: Result<String, String>) -> Result<String, String>",
        &schemes,
    );
}

#[test]
fn try_inside_a_lambda_targets_the_lambda() {
    // f returns Int; if the ? escaped to f this would be E3023. It
    // targets the lambda instead, whose return becomes the Result.
    tc_ok(&format!(
        "module M\n{RESULT_DECL}fn parse(s: String) -> Result<String, Int> {{\n  Ok(String.length(s))\n}}\nfn f(s: String) -> Int {{\n  let h = fn(t) {{ Ok(parse(t)? * 2) }}\n  match h(s) {{\n    Ok(n) -> n\n    Err(_) -> 0\n  }}\n}}\n"
    ));
}

#[test]
fn option_try_unwraps() {
    let schemes = tc_ok(
        "module M\ntype Option<a> =\n  | Some(a)\n  | None\nfn tenfold(o: Option<String>) -> Option<String> {\n  Some(\"${o?}0\")\n}\n",
    );
    expect_scheme("tenfold", "(o: Option<String>) -> Option<String>", &schemes);
}

#[test]
fn e3021_try_in_a_top_level_let() {
    expect_error(
        "E3021",
        &format!(
            "module M\n{RESULT_DECL}fn parse(s: String) -> Result<String, Int> {{\n  Ok(1)\n}}\nlet a = parse(\"x\")?\n"
        ),
    );
}

#[test]
fn e3022_try_on_a_non_result() {
    expect_error(
        "E3022",
        &format!(
            "module M\n{RESULT_DECL}fn f(n: Int) -> Result<String, Int> {{\n  Ok(n? + 1)\n}}\n"
        ),
    );
}

#[test]
fn e3023_option_try_inside_a_result_fn() {
    expect_error(
        "E3023",
        &format!(
            "module M\n{RESULT_DECL}type Option<a> =\n  | Some(a)\n  | None\nfn f(o: Option<Int>) -> Result<String, Int> {{\n  Ok(o? + 1)\n}}\n"
        ),
    );
}

#[test]
fn e3023_try_in_a_fn_returning_a_plain_type() {
    expect_error(
        "E3023",
        &format!("module M\n{RESULT_DECL}fn f(r: Result<String, Int>) -> Int {{\n  r? + 1\n}}\n"),
    );
}

#[test]
fn e3024_neither_side_determines_the_kind() {
    expect_error(
        "E3024",
        &format!("module M\n{RESULT_DECL}fn f(x) {{\n  x?\n}}\n"),
    );
}

#[test]
fn mismatched_error_types_are_a_return_mismatch() {
    expect_error(
        "E3002",
        &format!(
            "module M\n{RESULT_DECL}fn f(r: Result<Int, Int>) -> Result<String, Int> {{\n  Ok(r? + 1)\n}}\n"
        ),
    );
}

#[test]
fn a_shadowing_result_with_the_wrong_shape_does_not_unwrap() {
    expect_error(
        "E3022",
        "module M\ntype Result<e, a> =\n  | Good(a)\n  | Bad(e)\nfn f(r: Result<String, Int>) -> Result<String, Int> {\n  Good(r? + 1)\n}\n",
    );
}

// -- string interpolation -----------------------------------------------------------

#[test]
fn string_splices_type_check_the_whole_literal_is_a_string() {
    let schemes = tc_ok(
        "module M\nfn greet(name: String) -> String {\n  \"hello ${name}!\"\n}\nfn solo(s: String) -> String {\n  \"${s}\"\n}\n",
    );
    expect_scheme("greet", "(name: String) -> String", &schemes);
    expect_scheme("solo", "(s: String) -> String", &schemes);
}

#[test]
fn a_non_string_splice_points_at_the_conversion() {
    expect_error_with(
        "E3002",
        |d| hints_contain(d, "Int.toString"),
        "module M\nfn f(n: Int) -> String {\n  \"n = ${n}\"\n}\n",
    );
}
