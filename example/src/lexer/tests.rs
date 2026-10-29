//! Ported from `compiler/test/LexerTests.hs` — test names match the
//! Haskell cases for diffability.

use super::token::NewlineKind::*;
use super::token::TokenKind::{self, *};
use super::token::Trivia::*;
use super::token::{F64, IntLit, Operator, Token};
use super::lex_tokens;
use crate::span::FileId;

const FID: FileId = FileId(0);

fn lex_ok(src: &str) -> Vec<Token> {
    match lex_tokens(FID, src) {
        Ok(toks) => toks,
        Err(d) => panic!("unexpected lex failure: {d:?}"),
    }
}

fn kinds(src: &str) -> Vec<TokenKind> {
    lex_ok(src).into_iter().map(|t| t.kind).collect()
}

fn reconstruct(toks: &[Token]) -> String {
    let mut s = String::new();
    for t in toks {
        t.write_full_text(&mut s);
    }
    s
}

fn round_trips(src: &str) {
    assert_eq!(reconstruct(&lex_ok(src)), src, "round trip of {src:?}");
}

fn lower(s: &str) -> TokenKind {
    Lower(s.to_string())
}

fn upper(s: &str) -> TokenKind {
    Upper(s.to_string())
}

fn int(s: &str) -> TokenKind {
    Int(IntLit::from_raw(s))
}

fn string(s: &str) -> TokenKind {
    Str(s.to_string())
}

// The nasty corpus: every case must lex and reproduce the source exactly.
const NASTY_CORPUS: &[&str] = &[
    "fn main() {\n  Debug.log(\"hi\")\n}\n",
    "// comment at eof",
    "// comment, no newline after code\nx",
    "a /* multi\nline */ b",
    "x = 1 // trailing\ny = 2",
    "1_000_000 + 3.14",
    "a\r\nb\r\n",
    "\"\\u{1F389} unicode\" /* é in comment */",
    "",
    "  \t ",
    "[x, ..rest]",
    "/**/x/**/",
    "/* ** */",
    "x|>f(y)|>g",
    "type Option<a> =\n  | Some(a)\n  | None\n",
    "a<b\na>b\na<=b",
    "match s {\n  Circle(r) -> 3.14 * r * r\n  Rect(w, h) -> w * h\n}\n",
    "fn v() {\n  <div class=\"a\">\n    hi ${x} there\n    <br/>\n  </div>\n}\n",
    "<ul>{x}{..xs |> List.map(f)}</ul>",
    "<p>escapes: \\< \\{ \\} \\$ \\\\</p>",
    "<Card task={t} compact={True}></Card>",
    "\"${<b>bold</b>}\"",
    "<div /* why not */ data-id=\"7\" >text</div>",
];

#[test]
fn losslessness_corpus() {
    for src in NASTY_CORPUS {
        round_trips(src);
    }
}

// -- trivia attachment --------------------------------------------------------

#[test]
fn trailing_stops_at_newline() {
    let toks = lex_ok("x = 1 // c\ny");
    // x: trailing is the single space
    assert_eq!(toks[0].trailing, vec![Spaces(1)]);
    // 1: trailing is space + line comment, newline goes to y's leading
    let one = &toks[2];
    assert_eq!(one.kind, int("1"));
    assert_eq!(
        one.trailing,
        vec![Spaces(1), LineComment("// c".to_string())]
    );
    let y = &toks[3];
    assert_eq!(y.leading, vec![Newline(Lf)]);
}

#[test]
fn eof_carries_final_trivia() {
    let toks = lex_ok("x\n// bye\n");
    let eof = toks.last().unwrap();
    assert_eq!(eof.kind, Eof);
    assert_eq!(
        eof.leading,
        vec![
            Newline(Lf),
            LineComment("// bye".to_string()),
            Newline(Lf)
        ]
    );
}

#[test]
fn crlf_is_preserved_as_one_newline_trivia() {
    let toks = lex_ok("a\r\nb");
    assert_eq!(toks[1].leading, vec![Newline(CrLf)]);
}

// -- TokLParenBOL -------------------------------------------------------------

#[test]
fn same_line_paren_stays_lparen() {
    assert_eq!(
        kinds("f(x)"),
        vec![lower("f"), LParen, lower("x"), RParen, Eof]
    );
}

#[test]
fn space_before_paren_stays_lparen() {
    assert_eq!(kinds("f (x)")[1], LParen);
}

#[test]
fn newline_before_paren_becomes_lparen_bol() {
    assert_eq!(kinds("f\n(x)")[1], LParenBOL);
}

#[test]
fn multi_line_block_comment_before_paren_counts_as_a_line_break() {
    assert_eq!(kinds("f /* a\nb */ (x)")[1], LParenBOL);
}

#[test]
fn single_line_block_comment_does_not() {
    assert_eq!(kinds("f /* a */ (x)")[1], LParen);
}

// -- token payloads -----------------------------------------------------------

#[test]
fn keywords_vs_identifiers() {
    assert_eq!(
        kinds("fn fnx if iffy"),
        vec![Fn, lower("fnx"), If, lower("iffy"), Eof]
    );
}

#[test]
fn cast_is_a_keyword_castx_is_not() {
    assert_eq!(kinds("cast castx"), vec![Cast, lower("castx"), Eof]);
}

#[test]
fn numbers() {
    assert_eq!(
        kinds("1_0 3.14 1..2"),
        vec![int("10"), Float(F64(3.14)), int("1"), DotDot, int("2"), Eof]
    );
}

#[test]
fn string_escapes_decode() {
    let toks = lex_ok("\"a\\nb\\u{1F389}\"");
    match &toks[..] {
        [s, _eof] => {
            assert_eq!(s.kind, string("a\nb\u{1F389}"));
            assert_eq!(s.text, "\"a\\nb\\u{1F389}\"");
        }
        toks => panic!("expected one token + EOF, got {}", toks.len()),
    }
}

#[test]
fn hole_vs_underscore_prefixed_name() {
    assert_eq!(kinds("_ _x"), vec![Hole, lower("_x"), Eof]);
}

#[test]
fn operators_munch_maximally() {
    assert_eq!(
        kinds("a|>b||c|d"),
        vec![
            lower("a"),
            Pipeline,
            lower("b"),
            Op(Operator::Or),
            lower("c"),
            Bar,
            lower("d"),
            Eof
        ]
    );
}

#[test]
fn arrow_vs_minus() {
    assert_eq!(
        kinds("a->b-c"),
        vec![
            lower("a"),
            Arrow,
            lower("b"),
            Op(Operator::Sub),
            lower("c"),
            Eof
        ]
    );
}

#[test]
fn lt_gt_are_distinct_kinds() {
    assert_eq!(
        kinds("a<b>=c"),
        vec![
            lower("a"),
            Lt,
            lower("b"),
            Op(Operator::Ge),
            lower("c"),
            Eof
        ]
    );
}

#[test]
fn pow_munches_before_mul() {
    assert_eq!(
        kinds("a**b***c"),
        vec![
            lower("a"),
            Op(Operator::Pow),
            lower("b"),
            Op(Operator::Pow),
            Op(Operator::Mul),
            lower("c"),
            Eof
        ]
    );
}

#[test]
fn intdiv_is_one_token() {
    assert_eq!(
        kinds("a~/b"),
        vec![lower("a"), Op(Operator::IntDiv), lower("b"), Eof]
    );
}

#[test]
fn coloncolon_munches_before_colon() {
    assert_eq!(
        kinds("a::B:c"),
        vec![lower("a"), ColonColon, upper("B"), Colon, lower("c"), Eof]
    );
}

#[test]
fn intdiv_munches_before_a_block_comment_opener() {
    // `a ~/* b` is `~/` then `*`, NOT `~` before a comment — the same
    // maximal munch the JetBrains lexer mirrors.
    assert_eq!(
        kinds("a~/*b"),
        vec![
            lower("a"),
            Op(Operator::IntDiv),
            Op(Operator::Mul),
            lower("b"),
            Eof
        ]
    );
}

// -- lex errors ---------------------------------------------------------------

fn expect_error(src: &str) {
    match lex_tokens(FID, src) {
        Err(_) => {}
        Ok(toks) => panic!(
            "expected a lex error, got: {:?}",
            toks.into_iter().map(|t| t.kind).collect::<Vec<_>>()
        ),
    }
}

fn expect_code(src: &str, code: &str) {
    match lex_tokens(FID, src) {
        Err(d) => assert_eq!(d.code.0, code, "for source {src:?}"),
        Ok(toks) => panic!(
            "expected {code}, got: {:?}",
            toks.into_iter().map(|t| t.kind).collect::<Vec<_>>()
        ),
    }
}

#[test]
fn unterminated_string() {
    expect_error("\"abc");
}

#[test]
fn newline_in_string() {
    expect_error("\"a\nb\"");
}

#[test]
fn bad_escape() {
    expect_error("\"a\\qb\"");
}

#[test]
fn bad_unicode_escape() {
    expect_error("\"\\u{D800}\"");
}

#[test]
fn unterminated_block_comment() {
    expect_error("/* abc");
}

#[test]
fn unexpected_character() {
    expect_error("@");
}

#[test]
fn lone_tilde() {
    expect_error("a ~ b");
}

#[test]
fn non_ascii_identifier() {
    expect_error("café");
}

// -- string interpolation -----------------------------------------------------

#[test]
fn chunks_carry_decoded_payloads() {
    assert_eq!(
        kinds("\"a${x}b\""),
        vec![
            StringBegin("a".to_string()),
            lower("x"),
            StringEnd("b".to_string()),
            Eof
        ]
    );
}

#[test]
fn mid_chunks_between_holes() {
    assert_eq!(
        kinds("\"a${x}m${y}z\""),
        vec![
            StringBegin("a".to_string()),
            lower("x"),
            StringMid("m".to_string()),
            lower("y"),
            StringEnd("z".to_string()),
            Eof
        ]
    );
}

#[test]
fn empty_chunks_still_tokenize() {
    assert_eq!(
        kinds("\"${x}\""),
        vec![
            StringBegin(String::new()),
            lower("x"),
            StringEnd(String::new()),
            Eof
        ]
    );
}

#[test]
fn braces_nest_inside_a_hole() {
    assert_eq!(
        kinds("\"${ {let a = 1 a} }!\""),
        vec![
            StringBegin(String::new()),
            LBrace,
            Let,
            lower("a"),
            TokenKind::Eq,
            int("1"),
            lower("a"),
            RBrace,
            StringEnd("!".to_string()),
            Eof
        ]
    );
}

#[test]
fn nested_strings_inside_holes() {
    assert_eq!(
        kinds("\"${ \"inner ${y} deep\" }\""),
        vec![
            StringBegin(String::new()),
            StringBegin("inner ".to_string()),
            lower("y"),
            StringEnd(" deep".to_string()),
            StringEnd(String::new()),
            Eof
        ]
    );
}

#[test]
fn escaped_and_lone_dollars_stay_literal() {
    assert_eq!(
        kinds("\"cost: \\${ or $x\""),
        vec![string("cost: ${ or $x"), Eof]
    );
}

#[test]
fn interpolation_round_trips_exactly() {
    round_trips("\"a${ x + 1 }b\"");
    round_trips("\"${ \"inner ${y}\" }end\"");
    round_trips("fn f() {\n  \"n = ${ /* here */ n }\"\n}");
}

#[test]
fn a_newline_inside_a_hole_is_an_error() {
    expect_code("\"a${\nx}b\"", "E1005");
}

#[test]
fn a_multi_line_comment_inside_a_hole_is_an_error() {
    expect_code("\"a${ /* x\ny */ n }b\"", "E1005");
}

#[test]
fn eof_inside_a_hole_is_an_unterminated_string() {
    expect_code("\"a${x", "E1002");
}

// -- jsx start rule -----------------------------------------------------------
// The `<` decision: an element starts exactly where an expression can
// begin — after a token that cannot end an expression, or line-initially
// (layout rule 3). Everything else is the comparison operator.

#[test]
fn a_lt_b_stays_a_comparison() {
    assert_eq!(kinds("a < b"), vec![lower("a"), Lt, lower("b"), Eof]);
}

#[test]
fn a_lt_div_no_spaces_stays_a_comparison() {
    assert_eq!(kinds("a<div"), vec![lower("a"), Lt, lower("div"), Eof]);
}

#[test]
fn call_then_lt_stays_a_comparison() {
    assert_eq!(kinds("f(x) < y")[4..], [Lt, lower("y"), Eof]);
}

#[test]
fn question_then_lt_stays_a_comparison() {
    assert_eq!(
        kinds("x? < y"),
        vec![lower("x"), Question, Lt, lower("y"), Eof]
    );
}

#[test]
fn cast_target_then_lt_stays_a_comparison() {
    assert_eq!(
        kinds("x::Celsius < y")[2..],
        [upper("Celsius"), Lt, lower("y"), Eof]
    );
}

#[test]
fn generics_keep_lexing_as_angle_brackets() {
    assert_eq!(
        kinds("xs: List<a>"),
        vec![lower("xs"), Colon, upper("List"), Lt, lower("a"), Gt, Eof]
    );
}

#[test]
fn an_element_after_eq() {
    assert_eq!(
        kinds("let x = <br/>"),
        vec![
            Let,
            lower("x"),
            TokenKind::Eq,
            JsxTagStart("br".to_string()),
            JsxSelfClose,
            Eof
        ]
    );
}

#[test]
fn an_element_after_a_match_arrow() {
    assert_eq!(
        kinds("-> <br/>")[1..],
        [JsxTagStart("br".to_string()), JsxSelfClose, Eof]
    );
}

#[test]
fn elements_as_list_items() {
    assert_eq!(
        kinds("[<br/>, <hr/>]"),
        vec![
            LBracket,
            JsxTagStart("br".to_string()),
            JsxSelfClose,
            Comma,
            JsxTagStart("hr".to_string()),
            JsxSelfClose,
            RBracket,
            Eof
        ]
    );
}

#[test]
fn a_line_initial_element_after_a_complete_statement_layout_rule_3() {
    assert_eq!(
        kinds("let x = 1\n<br/>"),
        vec![
            Let,
            lower("x"),
            TokenKind::Eq,
            int("1"),
            JsxTagStart("br".to_string()),
            JsxSelfClose,
            Eof
        ]
    );
}

#[test]
fn an_element_opening_a_string_hole() {
    assert_eq!(
        kinds("\"${<b>x</b>}\""),
        vec![
            StringBegin(String::new()),
            JsxTagStart("b".to_string()),
            JsxTagEnd,
            JsxText("x".to_string()),
            JsxClose("b".to_string()),
            StringEnd(String::new()),
            Eof
        ]
    );
}

#[test]
fn lt_before_a_non_letter_is_never_an_element() {
    assert_eq!(
        kinds("x = <3"),
        vec![lower("x"), TokenKind::Eq, Lt, int("3"), Eof]
    );
}

// -- jsx modes ----------------------------------------------------------------

#[test]
fn attributes_string_values_keywords_and_dashes_as_names() {
    assert_eq!(
        kinds("<input type=\"text\" data-id=\"7\"/>"),
        vec![
            JsxTagStart("input".to_string()),
            JsxAttrName("type".to_string()),
            TokenKind::Eq,
            string("text"),
            JsxAttrName("data-id".to_string()),
            TokenKind::Eq,
            string("7"),
            JsxSelfClose,
            Eof
        ]
    );
}

#[test]
fn event_attributes_keep_the_colon_in_the_name() {
    assert_eq!(
        kinds("<button on:click={m}>x</button>"),
        vec![
            JsxTagStart("button".to_string()),
            JsxAttrName("on:click".to_string()),
            TokenKind::Eq,
            LBrace,
            lower("m"),
            RBrace,
            JsxTagEnd,
            JsxText("x".to_string()),
            JsxClose("button".to_string()),
            Eof
        ]
    );
}

#[test]
fn bare_attribute() {
    assert_eq!(
        kinds("<input disabled/>"),
        vec![
            JsxTagStart("input".to_string()),
            JsxAttrName("disabled".to_string()),
            JsxSelfClose,
            Eof
        ]
    );
}

#[test]
fn attribute_value_interpolation() {
    assert_eq!(
        kinds("<div class=\"a${x}b\"/>"),
        vec![
            JsxTagStart("div".to_string()),
            JsxAttrName("class".to_string()),
            TokenKind::Eq,
            StringBegin("a".to_string()),
            lower("x"),
            StringEnd("b".to_string()),
            JsxSelfClose,
            Eof
        ]
    );
}

#[test]
fn children_text_holes_splices_spread() {
    assert_eq!(
        kinds("<p>n = ${n}!{x}{..xs}</p>"),
        vec![
            JsxTagStart("p".to_string()),
            JsxTagEnd,
            JsxText("n = ".to_string()),
            DollarBrace,
            lower("n"),
            RBrace,
            JsxText("!".to_string()),
            LBrace,
            lower("x"),
            RBrace,
            LBrace,
            DotDot,
            lower("xs"),
            RBrace,
            JsxClose("p".to_string()),
            Eof
        ]
    );
}

#[test]
fn nested_elements() {
    assert_eq!(
        kinds("<div><br/></div>"),
        vec![
            JsxTagStart("div".to_string()),
            JsxTagEnd,
            JsxTagStart("br".to_string()),
            JsxSelfClose,
            JsxClose("div".to_string()),
            Eof
        ]
    );
}

#[test]
fn component_tags_are_upper_tag_starts() {
    assert_eq!(
        kinds("<Card task={t}/>"),
        vec![
            JsxTagStart("Card".to_string()),
            JsxAttrName("task".to_string()),
            TokenKind::Eq,
            LBrace,
            lower("t"),
            RBrace,
            JsxSelfClose,
            Eof
        ]
    );
}

#[test]
fn text_escapes_decode_raw_text_is_preserved() {
    let toks = lex_ok("<p>\\{a\\} \\< \\$</p>");
    match &toks[..] {
        [_, _, t, _, _] => {
            assert_eq!(t.kind, JsxText("{a} < $".to_string()));
            assert_eq!(t.text, "\\{a\\} \\< \\$");
        }
        toks => panic!(
            "unexpected shape: {:?}",
            toks.iter().map(|t| &t.kind).collect::<Vec<_>>()
        ),
    }
}

#[test]
fn multiline_text_keeps_its_newlines_in_one_token() {
    let toks = lex_ok("<pre>\n  a\n</pre>");
    match &toks[..] {
        [_, _, t, _, _] => assert_eq!(t.kind, JsxText("\n  a\n".to_string())),
        toks => panic!(
            "unexpected shape: {:?}",
            toks.iter().map(|t| &t.kind).collect::<Vec<_>>()
        ),
    }
}

#[test]
fn a_splice_may_span_lines() {
    round_trips("<div>{\n  match x {\n    A -> y\n  }\n}</div>");
}

#[test]
fn a_mismatched_closing_tag_still_lexes_the_parser_blames_it() {
    assert_eq!(
        kinds("<div></span>"),
        vec![
            JsxTagStart("div".to_string()),
            JsxTagEnd,
            JsxClose("span".to_string()),
            Eof
        ]
    );
}

// -- jsx errors ---------------------------------------------------------------

#[test]
fn stray_rbrace_in_text() {
    expect_code("<p>}</p>", "E1007");
}

#[test]
fn unterminated_element() {
    expect_code("<div>never closes", "E1006");
}

#[test]
fn unterminated_tag() {
    expect_code("<div class=\"x\"", "E1006");
}

#[test]
fn unterminated_splice() {
    expect_code("<div>{x", "E1006");
}

#[test]
fn trailing_dash_in_a_tag_name() {
    expect_code("<my-/>", "E1008");
}

#[test]
fn dotted_component_tag() {
    expect_code("<Card.sub/>", "E1008");
}

#[test]
fn non_letter_after_lt_in_text() {
    expect_code("<p><3</p>", "E1008");
}

#[test]
fn newline_in_a_dollar_hole_in_text() {
    expect_code("<p>${\nx}</p>", "E1005");
}

#[test]
fn newline_in_text_inside_a_string_hole() {
    expect_code("\"${<p>\na</p>}\"", "E1005");
}

#[test]
fn unknown_escape_in_text() {
    expect_code("<p>\\q</p>", "E1003");
}

// -- properties ---------------------------------------------------------------

mod properties {
    use super::*;
    use proptest::prelude::*;

    // Random printable-ASCII soup: whenever it lexes, it must round-trip.
    const CHARS: &[char] = &[
        ' ', '\t', '\n', 'a', 'b', 'z', 'A', 'Z', '0', '9', '_', '(', ')', '{', '}', '[', ']',
        '<', '>', '=', '+', '-', '*', '/', '%', '|', ',', '.', ':', '"', '\\', ' ', '!', '&',
        '$', '?',
    ];

    proptest! {
        #[test]
        fn lex_success_implies_exact_round_trip(
            chars in proptest::collection::vec(proptest::sample::select(CHARS), 0..120)
        ) {
            let src: String = chars.into_iter().collect();
            if let Ok(toks) = lex_tokens(FID, &src) {
                prop_assert_eq!(reconstruct(&toks), src);
            }
        }
    }
}

// -- repo corpus --------------------------------------------------------------

/// Every committed `.mx` file that lexes must round-trip byte for byte.
#[test]
fn repo_corpus_round_trips() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../language-project");
    let mut checked = 0;
    for dir in ["stdlib", "examples", "compiler/test/fixtures"] {
        let mut stack = vec![std::path::PathBuf::from(root).join(dir)];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "mx") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    if let Ok(toks) = lex_tokens(FID, &src) {
                        assert_eq!(reconstruct(&toks), src, "round trip of {}", p.display());
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked >= 10, "corpus should contain .mx files, found {checked}");
}
