//! Ported from `compiler/test/ParserTests.hs` — test names match the
//! Haskell cases for diffability. The lowering-shape assertions (try
//! hoisting, interp lowering, E1107–E1109) live with `cst::lower`.

use super::{parse_module, parse_module_recovering};
use crate::cst::print::print_module;
use crate::cst::*;
use crate::span::FileId;

const FID: FileId = FileId(0);

fn parse_ok(src: &str) -> CstModule {
    match parse_module(FID, src) {
        Ok(m) => m,
        Err(d) => panic!("unexpected parse failure: {d:?}"),
    }
}

fn round_trips(src: &str) {
    assert_eq!(print_module(&parse_ok(src)), src);
}

// The approved canonical sample, verbatim.
const GEOMETRY_SAMPLE: &str = "module Geometry\n\ntype Shape =\n  | Circle(Float)\n  | Rect(Float, Float)\n\ntype User = { name: String, age: Int }\n\nfn area(s: Shape) -> Float {\n  match s {\n    Circle(r) -> 3.14 * r * r\n    Rect(w, h) -> w * h\n  }\n}\n\nfn main() {\n  [Circle(1.0), Rect(2.0, 3.0)]\n  |> List.map(area)\n  |> List.sum\n  |> Debug.log\n}\n";

// -- exact print . parse round-trip -------------------------------------------

#[test]
fn the_canonical_geometry_sample() {
    round_trips(GEOMETRY_SAMPLE);
}

#[test]
fn hello_world() {
    round_trips("module Main\n\nfn main() {\n  Debug.log(\"Hello, world!\")\n}\n");
}

#[test]
fn comments_survive_everywhere() {
    round_trips(
        "module M\n\n// before fn\nfn f(x: Int) -> Int { // open\n  x + 1 // result\n} /* after */\n",
    );
}

#[test]
fn let_declarations_and_annotations() {
    round_trips("module M\n\nlet pi: Float = 3.14\n\nlet tau = pi * 2.0\n");
}

#[test]
fn lambdas_holes_pipelines() {
    round_trips("module M\n\nlet f = fn(x, _) { x + 1 }\n\nlet g = fn() { 0 }\n");
}

#[test]
fn if_else_chains() {
    round_trips(
        "module M\n\nfn f(x: Int) -> Int {\n  if x > 0 { 1 } else if x == 0 { 0 } else { -1 }\n}\n",
    );
}

#[test]
fn list_spread_expressions_and_patterns() {
    round_trips(
        "module M\n\nfn f(xs: List<a>) -> List<a> {\n  match xs {\n    [] -> []\n    [x, ..rest] -> [x, ..f(rest)]\n  }\n}\n",
    );
}

#[test]
fn labeled_constructor_call() {
    round_trips("module M\n\nlet u = User(name: \"Alice\", age: 30)\n");
}

#[test]
fn record_update_spread() {
    round_trips("module M\n\nfn birthday(u: User) -> User {\n  User(..u, age: u.age + 1)\n}\n");
}

#[test]
fn qualified_record_update_bare_copy() {
    round_trips(
        "module M\n\nimport Geo\n\nlet q = Geo.Point(..p, x: 1.0)\n\nlet r = Point(..p)\n",
    );
}

#[test]
fn match_guards() {
    round_trips(
        "module M\n\nfn f(n: Int) -> Int {\n  match n {\n    x if x > 0 -> x\n    _ -> 0\n  }\n}\n",
    );
}

#[test]
fn qualified_names_in_expressions_and_patterns() {
    round_trips(
        "module M\n\nimport Geometry\n\nfn f(s: Shape) -> Float {\n  match s {\n    Geometry.Circle(r) -> Geometry.area(s)\n    _ -> 0.0\n  }\n}\n",
    );
}

#[test]
fn record_type_declaration() {
    round_trips("module M\n\ntype P = { x: Float, y: Float }\n");
}

#[test]
fn function_types_and_generics() {
    round_trips("module M\n\nfn map(xs: List<a>, f: (a) -> b) -> List<b> {\n  xs\n}\n");
}

#[test]
fn block_expression_as_a_value() {
    round_trips("module M\n\nlet x = {\n  let a = 1\n  a + 1\n}\n");
}

#[test]
fn cast_declarations() {
    round_trips("module M\n\ncast (d: Float) -> Celsius {\n  Celsius(degrees: d)\n}\n");
}

#[test]
fn jsx_self_closing_attrs_comments_in_the_tag() {
    round_trips("module M\n\nlet x = <input type=\"text\" /* why */ disabled data-id=\"7\"/>\n");
}

#[test]
fn jsx_children_with_text_holes_splices_spreads() {
    round_trips(
        "module M\n\nfn v(n: Int) -> Html<msg> {\n  <ul id=\"all\">\n    count: ${Int.toString(n)}\n    {header}\n    {..items |> List.map(f)}\n    <li>done</li>\n  </ul>\n}\n",
    );
}

#[test]
fn jsx_component_tags_with_expression_attrs_and_children() {
    round_trips(
        "module M\n\nimport Card\n\nlet x = <Card title=\"a\" urgent={True}>\n  <p>body</p>\n</Card>\n",
    );
}

#[test]
fn jsx_multiline_splice_with_a_match() {
    round_trips(
        "module M\n\nfn v(s: Shape) -> Html<msg> {\n  <div>{\n    match s {\n      Circle(r) -> <b>round</b>\n      Rect(w, h) -> <b>boxy</b>\n    }\n  }</div>\n}\n",
    );
}

#[test]
fn jsx_text_escapes_and_crlf_survive_byte_for_byte() {
    round_trips("module M\r\n\r\nlet x = <p>a \\{ b \\} c \\< d \\$ e</p>\r\n");
}

#[test]
fn jsx_attr_value_interpolation() {
    round_trips("module M\n\nlet x = <div class=\"a${cls}b\">t</div>\n");
}

#[test]
fn jsx_text_containing_keyword_looking_runs() {
    round_trips("module M\n\nlet x = <p>fn main() let type // not a comment</p>\n");
}

// -- repo .mx files round-trip -------------------------------------------------
// Every committed .mx file must round-trip (the stdlib is the dogfood).

#[test]
fn repo_mx_files_round_trip() {
    for path in [
        "../../language-project/stdlib/core/List.mx",
        "../../language-project/stdlib/core/Math.mx",
        "../../language-project/stdlib/core/Option.mx",
        "../../language-project/stdlib/core/Result.mx",
        "../../language-project/examples/hello-world/Main.mx",
        "../../language-project/examples/shapes/Geometry.mx",
        "../../language-project/examples/shapes/Main.mx",
    ] {
        let full = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path);
        let src = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
        assert_eq!(print_module(&parse_ok(&src)), src, "round trip of {path}");
    }
}

// -- juxtaposed match arms -----------------------------------------------------

#[test]
fn bare_variable_body_before_constructor_pattern() {
    round_trips(
        "module M\n\nfn f(s: S) -> Int {\n  match s {\n    Circle(r) -> g\n    Rect(w, h) -> h\n  }\n}\n",
    );
}

#[test]
fn nested_match_one_liner_followed_by_outer_arm() {
    let m = parse_ok("module M\n\nfn f(x: T) -> Int { match x { A -> match y { B -> 1 } C -> 2 } }\n");
    match &m.decls[..] {
        [CstDecl::Fn(_, _, _, _, CstBlock(_, stmts, _))] => match &stmts[..] {
            [CstStmt::Expr(CstExpr::Match(_, _, _, arms, _))] => assert_eq!(arms.len(), 2),
            _ => panic!("unexpected shape"),
        },
        _ => panic!("unexpected shape"),
    }
}

#[test]
fn operators_continue_across_lines_no_asi() {
    let m = parse_ok("module M\n\nlet x = 1 +\n  2\n");
    assert_eq!(m.decls.len(), 1);
}

#[test]
fn pipeline_continues_across_lines() {
    round_trips("module M\n\nlet x = a\n  |> f\n  |> g\n");
}

// -- operator precedence shapes ------------------------------------------------
// The first precedence shape tests: '**' vs unary minus and the
// multiplicative row are the only places the table has a surprise.

fn let_rhs(decl: &str) -> CstExpr {
    let m = parse_ok(&format!("module M\n\n{decl}\n"));
    match m.decls.into_iter().next() {
        Some(CstDecl::Let(_, _, _, _, rhs)) => rhs,
        _ => panic!("unexpected declaration shape"),
    }
}

#[test]
fn neg_x_pow_y_is_neg_of_pow() {
    match let_rhs("let a = -x ** y") {
        CstExpr::Unary(_, inner) => match *inner {
            CstExpr::Bin(_, op, _) => assert_eq!(op.text, "**"),
            e => panic!("unexpected shape: {e:?}"),
        },
        e => panic!("unexpected shape: {e:?}"),
    }
}

#[test]
fn pow_is_right_associative() {
    match let_rhs("let a = x ** y ** z") {
        CstExpr::Bin(l, op, r) => {
            assert!(matches!(*l, CstExpr::Var(_)));
            assert_eq!(op.text, "**");
            match *r {
                CstExpr::Bin(_, op2, _) => assert_eq!(op2.text, "**"),
                e => panic!("unexpected shape: {e:?}"),
            }
        }
        e => panic!("unexpected shape: {e:?}"),
    }
}

#[test]
fn mul_keeps_pow_tighter() {
    match let_rhs("let a = x * y ** z") {
        CstExpr::Bin(l, op, r) => {
            assert!(matches!(*l, CstExpr::Var(_)));
            assert_eq!(op.text, "*");
            match *r {
                CstExpr::Bin(_, op2, _) => assert_eq!(op2.text, "**"),
                e => panic!("unexpected shape: {e:?}"),
            }
        }
        e => panic!("unexpected shape: {e:?}"),
    }
}

#[test]
fn pow_allows_unary_on_the_right() {
    match let_rhs("let a = x ** -y") {
        CstExpr::Bin(l, op, r) => {
            assert!(matches!(*l, CstExpr::Var(_)));
            assert_eq!(op.text, "**");
            assert!(matches!(*r, CstExpr::Unary(_, _)));
        }
        e => panic!("unexpected shape: {e:?}"),
    }
}

#[test]
fn a_cast_is_postfix() {
    match let_rhs("let a = x + y::Float") {
        CstExpr::Bin(_, op, r) => {
            assert_eq!(op.text, "+");
            match *r {
                CstExpr::Cast(_, _, ty) => assert_eq!(ty.text, "Float"),
                e => panic!("unexpected shape: {e:?}"),
            }
        }
        e => panic!("unexpected shape: {e:?}"),
    }
    match let_rhs("let a = x::Foo::Bar") {
        CstExpr::Cast(inner, _, ty2) => {
            assert_eq!(ty2.text, "Bar");
            match *inner {
                CstExpr::Cast(_, _, ty1) => assert_eq!(ty1.text, "Foo"),
                e => panic!("unexpected shape: {e:?}"),
            }
        }
        e => panic!("unexpected shape: {e:?}"),
    }
    match let_rhs("let a = x::Foo.bar") {
        CstExpr::Dot(inner, _, name) => {
            assert_eq!(name.text, "bar");
            assert!(matches!(*inner, CstExpr::Cast(_, _, _)));
        }
        e => panic!("unexpected shape: {e:?}"),
    }
}

#[test]
fn intdiv_sits_on_the_multiplicative_row() {
    match let_rhs("let a = x ~/ y - z") {
        CstExpr::Bin(l, op2, _) => {
            assert_eq!(op2.text, "-");
            match *l {
                CstExpr::Bin(_, op, _) => assert_eq!(op.text, "~/"),
                e => panic!("unexpected shape: {e:?}"),
            }
        }
        e => panic!("unexpected shape: {e:?}"),
    }
}

// -- call paren must be same-line (TokLParenBOL) --------------------------------

fn stmt_count(m: &CstModule) -> usize {
    match &m.decls[..] {
        [CstDecl::Fn(_, _, _, _, CstBlock(_, stmts, _))] => stmts.len(),
        _ => panic!("unexpected shape"),
    }
}

#[test]
fn same_line_paren_is_a_call_one_statement() {
    let m = parse_ok("module M\n\nfn f() {\n  g(x)\n}\n");
    assert_eq!(stmt_count(&m), 1);
}

#[test]
fn next_line_paren_starts_a_new_statement_two_statements() {
    let m = parse_ok("module M\n\nfn f() {\n  g\n  (x) |> h\n}\n");
    assert_eq!(stmt_count(&m), 2);
}

// -- parse shapes ---------------------------------------------------------------

#[test]
fn pipeline_binds_loosest() {
    let m = parse_ok("module M\n\nlet x = a |> f(b) + c\n");
    match &m.decls[..] {
        [CstDecl::Let(_, _, _, _, CstExpr::Bin(_, op, _))] => assert_eq!(op.text, "|>"),
        _ => panic!("unexpected shape"),
    }
}

#[test]
fn fn_lambda_is_an_atom_pipeline_continues_past_it() {
    let m = parse_ok("module M\n\nlet f = fn(x) { x + 1 } |> g\n");
    match &m.decls[..] {
        [CstDecl::Let(_, _, _, _, CstExpr::Bin(l, op, _))] => {
            assert!(matches!(**l, CstExpr::FnLam(_, _, _, _)));
            assert_eq!(op.text, "|>");
        }
        _ => panic!("unexpected shape"),
    }
}

#[test]
fn fn_lambda_with_annotations_and_return_type() {
    let m = parse_ok("module M\n\nlet f = fn(x: Int, _: Bool) -> Int { x }\n");
    match &m.decls[..] {
        [CstDecl::Let(_, _, _, _, CstExpr::FnLam(_, _, Some(_), _))] => {}
        _ => panic!("unexpected shape"),
    }
}

#[test]
fn fn_at_statement_start_after_an_expression() {
    let m = parse_ok("module M\n\nfn f() {\n  g\n  fn(x) { x }\n}\n");
    assert_eq!(stmt_count(&m), 2);
}

// -- parse errors ----------------------------------------------------------------

fn expect_code(src: &str, code: &str) {
    match parse_module(FID, src) {
        Err(d) => assert_eq!(d.code.0, code),
        Ok(_) => panic!("expected a parse error"),
    }
}

#[test]
fn tuples_are_rejected_with_a_hint() {
    expect_code("module M\n\nlet x = (a, b)\n", "E1104");
}

#[test]
fn arrow_lambdas_are_rejected_with_the_fn_rewrite() {
    expect_code("module M\n\nlet f = (x) => x + 1\n", "E1111");
}

#[test]
fn empty_parens_as_expression() {
    expect_code("module M\n\nlet x = ()\n", "E1102");
}

#[test]
fn lone_underscore_as_expression() {
    expect_code("module M\n\nlet x = (_)\n", "E1103");
}

#[test]
fn expected_token_list_is_reported() {
    match parse_module(FID, "module M\n\nfn f(\n") {
        Err(d) => assert!(
            format!("{:?}", d.body).contains("expecting"),
            "mentions expecting: {:?}",
            d.body
        ),
        Ok(_) => panic!("expected a syntax error"),
    }
}

// -- postfix ? -------------------------------------------------------------------

#[test]
fn postfix_try_round_trips() {
    round_trips("module M\n\nlet a = f(x)?\n");
    round_trips("module M\n\nlet a = r.field?\n");
    round_trips("module M\n\nlet a = f(x)?.g\n");
    round_trips("module M\n\nlet a = x??\n");
    round_trips("module M\n\nlet a = xs |> parse? |> use\n");
    round_trips("module M\n\nlet a = xs |> (h?)\n");
    round_trips("module M\n\nlet a = f(x) ?\n"); // exact print keeps the space
    round_trips("module M\n\nlet a = -x?\n");
    round_trips("module M\n\nfn f(r: R) -> R {\n  match r? {\n    A -> 1\n  }\n}\n");
}

// -- string interpolation ---------------------------------------------------------

#[test]
fn interpolation_round_trips() {
    round_trips("module M\n\nlet a = \"hello ${name}!\"\n");
    round_trips("module M\n\nlet a = \"${x}\"\n");
    round_trips("module M\n\nlet a = \"a${ x + 1 }b${ f(y) }c\"\n");
    round_trips("module M\n\nlet a = \"${ \"inner ${y} deep\" }\"\n");
    round_trips("module M\n\nlet a = \"n = ${ /* why */ n }\"\n");
    round_trips("module M\n\nlet a = \"${ if p { \"y\" } else { \"n\" } }\"\n");
}

// -- error recovery (R1: declaration resync) --------------------------------------

fn recover_ok(src: &str) -> (CstModule, Vec<crate::diag::Diagnostic>) {
    match parse_module_recovering(FID, src) {
        Ok(r) => r,
        Err(d) => panic!("recovery unexpectedly failed outright: {d:?}"),
    }
}

fn recover_round_trips(src: &str) {
    match parse_module_recovering(FID, src) {
        Ok((m, _)) => assert_eq!(print_module(&m), src),
        Err(_) => {} // lexical failure: fail-fast is the contract
    }
}

fn decl_tag(d: &CstDecl) -> &'static str {
    match d {
        CstDecl::Import(..) => "import",
        CstDecl::Type(..) => "type",
        CstDecl::Fn(..) => "fn",
        CstDecl::Let(..) => "let",
        CstDecl::Cast(..) => "cast",
        CstDecl::Component(..) => "component",
        CstDecl::Error(..) => "error",
    }
}

#[test]
fn two_broken_declarations_yield_two_cderrors_in_order() {
    let (m, ds) = recover_ok(
        "module M\nfn broken(\nfn ok(n: Int) -> Int {\n  n\n}\nfn alsoBroken( ->\nlet x = 1\n",
    );
    assert_eq!(ds.len(), 2);
    let tags: Vec<_> = m.decls.iter().map(decl_tag).collect();
    assert_eq!(tags, vec!["error", "fn", "error", "let"]);
}

#[test]
fn broken_files_still_print_byte_for_byte() {
    for src in [
        "module M\n\nfn broken(\n\nfn ok() {\n  1\n}\n",
        "module M\n\nfn ok() {\n  1\n}\n\nfn broken(\n",
        "module M\n\ngarbage here\n\nfn ok() {\n  1\n}\n",
        "module M\n\nfn ok() {\n  1\n} trailing garbage\n\nlet y = 2\n",
        "module M\n\nfn broken( // comment\n/* kept */\nfn ok() {\n  1\n}\n",
        "module M\n\nfn swallow() {\n  let a = 1\n\nfn nextIsSwallowed() {\n  2\n}\n",
        "module M\n\n}\n\nfn ok() {\n  1\n}\n",
        "module M\n\nlet s = \"a${broken\n", // lex error: stays Err, tested below
    ] {
        recover_round_trips(src);
    }
}

#[test]
fn an_unclosed_brace_swallows_the_rest_into_one_segment() {
    let (m, ds) = recover_ok("module M\n\nfn swallow() {\n  let a = 1\n\nfn hidden() {\n  2\n}\n");
    assert_eq!(ds.len(), 1);
    let errors = m
        .decls
        .iter()
        .filter(|d| matches!(d, CstDecl::Error(_)))
        .count();
    assert_eq!(errors, 1);
}

#[test]
fn broken_jsx_does_not_poison_the_next_declaration() {
    // `{}` is a parse error (a splice needs an expression) but lexes
    // fine; the segmenter's brace counter keeps the resync exact.
    let src = "module M\n\nlet bad = <div>{}</div>\n\nfn ok() {\n  1\n}\n";
    let (m, ds) = recover_ok(src);
    assert_eq!(ds.len(), 1);
    let tags: Vec<_> = m.decls.iter().map(decl_tag).collect();
    assert_eq!(tags, vec!["error", "fn"]);
    recover_round_trips(src);
}

#[test]
fn a_stray_closing_brace_cannot_poison_later_declarations() {
    let (m, ds) = recover_ok("module M\n\n}\n\nfn ok() {\n  1\n}\n");
    assert_eq!(ds.len(), 1);
    let tags: Vec<_> = m.decls.iter().map(decl_tag).collect();
    assert_eq!(tags, vec!["error", "fn"]);
}

#[test]
fn recovering_parse_of_a_healthy_file_equals_the_fail_fast_parse() {
    for src in [
        GEOMETRY_SAMPLE,
        "module Main\n\nfn main() {\n  Debug.log(\"hi\")\n}\n",
    ] {
        let (m, ds) = recover_ok(src);
        assert_eq!(ds.len(), 0);
        assert_eq!(print_module(&m), print_module(&parse_ok(src)));
    }
}

#[test]
fn repo_files_parse_identically_under_both_modes() {
    for path in [
        "../../language-project/stdlib/core/List.mx",
        "../../language-project/stdlib/core/Option.mx",
        "../../language-project/examples/calc/Main.mx",
        "../../language-project/examples/shapes/Main.mx",
    ] {
        let full = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path);
        let src = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
        let (m, ds) = recover_ok(&src);
        assert_eq!(ds.len(), 0, "{path}");
        assert_eq!(print_module(&m), src, "{path}");
    }
}

mod properties {
    use super::*;
    use proptest::prelude::*;

    const SOUP: &[char] = &[
        ' ', '\t', '\n', 'a', 'b', 'z', 'A', 'Z', '0', '9', '_', '(', ')', '{', '}', '[', ']',
        '<', '>', '=', '+', '-', '*', '/', '%', '|', ',', '.', ':', '"', '\\', ' ', '!', '&',
        '$', '?', 'f', 'n', 'l', 'e', 't', 't', 'y', 'p', 'e', 'i', 'm', 'p', 'o', 'r', 't',
        'm', 'a', 't', 'c', 'h',
    ];

    proptest! {
        #[test]
        fn recovery_keeps_losslessness_on_arbitrary_suffixes(
            chars in proptest::collection::vec(proptest::sample::select(SOUP), 0..80)
        ) {
            let suffix: String = chars.into_iter().collect();
            let src = format!("module M\n{suffix}");
            match parse_module_recovering(FID, &src) {
                Err(_) => {} // lex errors stay fail-fast
                Ok((m, _)) => prop_assert_eq!(print_module(&m), src),
            }
        }
    }
}
