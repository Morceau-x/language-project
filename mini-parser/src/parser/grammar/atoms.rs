//! Atoms: the expressions operators work on. Literals, names, and every
//! expression that starts with its own keyword or bracket.

use super::*;
use crate::parser::CompletedMarker;
use expressions::{EXPR_FIRST, expr};

/// Where to go on when an expression is missing.
const EXPR_RECOVERY: TokenSet = TokenSet::new(&[
    T![,],
    T![')'],
    T![']'],
    T!['}'],
    T![->],
    T![=],
    T![else],
    STRING_MIDDLE,
    STRING_END,
])
.union(items::DECL_FIRST);

/// The keywords that can only start a declaration: inside a block, one
/// of them means its `}` is missing.
const DECL_ONLY: TokenSet = TokenSet::new(&[T![import], T![type], T![cast], T![component]]);

/// `Atom ::= Literal | Name | "_" | Paren | List | Block | Lambda | If | Match | Interpolation`
pub(super) fn atom(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let done = match p.current() {
        INT_NUMBER | FLOAT_NUMBER | STRING => {
            let m = p.start();
            p.bump_any();
            m.complete(p, LITERAL)
        }
        kind @ (LOWER_IDENT | UPPER_IDENT) => {
            let m = p.start();
            name_ref(p, kind);
            m.complete(p, PATH_EXPR)
        }
        T![_] => {
            let m = p.start();
            p.bump(T![_]);
            m.complete(p, HOLE_EXPR)
        }
        STRING_START => interpolated_string(p),
        T!['('] => paren_expr(p),
        T!['['] => list_expr(p),
        T!['{'] => block_expr(p),
        T![fn] => lambda_expr(p),
        T![if] => if_expr(p),
        T![match] => match_expr(p),
        _ => {
            p.err_recover("expected an expression", EXPR_RECOVERY);
            return None;
        }
    };
    Some(done)
}

/// `Interpolation ::= STRING_START Expr (STRING_MIDDLE Expr)* STRING_END`
fn interpolated_string(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9g: interpolated_string")
}

/// `Paren ::= "(" Expr ")"`. Parsed more loosely, `"(" (Expr ("," Expr)*)?
/// ")"`, so that the validation can say what's wrong with `()` and
/// `(a, b)`.
fn paren_expr(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9c: paren_expr")
}

/// `List ::= "[" (Expr ("," Expr)* ("," Spread)?)? "]"`
fn list_expr(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9c: list_expr")
}

/// `Spread ::= ".." Expr`
pub(super) fn spread(p: &mut Parser<'_>) {
    todo!("Part 9b: spread")
}

/// `Block ::= "{" Stmt* "}"`
pub(super) fn block_expr(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9d: block_expr")
}

/// `Stmt ::= "let" LOWER (":" Type)? "=" Expr | Expr`
fn stmt(p: &mut Parser<'_>) {
    todo!("Part 9d: stmt")
}

/// `Lambda ::= "fn" ParamList RetType? Block`
fn lambda_expr(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9e: lambda_expr")
}

/// `If ::= "if" Expr Block "else" (Block | If)`
fn if_expr(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9f: if_expr")
}

/// `Match ::= "match" Expr "{" MatchArm+ "}"`
fn match_expr(p: &mut Parser<'_>) -> CompletedMarker {
    todo!("Part 9f: match_expr")
}

fn match_arm_list(p: &mut Parser<'_>) {
    todo!("Part 9f: match_arm_list")
}

/// `MatchArm ::= Pattern ("if" Expr)? "->" Expr`
fn match_arm(p: &mut Parser<'_>) {
    todo!("Part 9f: match_arm")
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn literals_names_and_holes() {
        check_expr(
            "42",
            expect![[r#"
            SOURCE_FILE@0..2
              LITERAL@0..2
                INT_NUMBER@0..2 "42"
        "#]],
        );
        check_expr(
            "Shape",
            expect![[r#"
            SOURCE_FILE@0..5
              PATH_EXPR@0..5
                NAME_REF@0..5
                  UPPER_IDENT@0..5 "Shape"
        "#]],
        );
        check_expr(
            "_",
            expect![[r#"
            SOURCE_FILE@0..1
              HOLE_EXPR@0..1
                UNDERSCORE@0..1 "_"
        "#]],
        );
    }

    #[test]
    fn interpolated_strings() {
        check_expr(
            r#""a${x}b${"c${y}"}d""#,
            expect![[r#"
            SOURCE_FILE@0..19
              INTERPOLATED_STRING@0..19
                STRING_START@0..4 "\"a${"
                PATH_EXPR@4..5
                  NAME_REF@4..5
                    LOWER_IDENT@4..5 "x"
                STRING_MIDDLE@5..9 "}b${"
                INTERPOLATED_STRING@9..16
                  STRING_START@9..13 "\"c${"
                  PATH_EXPR@13..14
                    NAME_REF@13..14
                      LOWER_IDENT@13..14 "y"
                  STRING_END@14..16 "}\""
                STRING_END@16..19 "}d\""
        "#]],
        );
    }

    #[test]
    fn parens_hold_one_expression_but_parse_more() {
        check_expr(
            "(a)",
            expect![[r#"
            SOURCE_FILE@0..3
              PAREN_EXPR@0..3
                L_PAREN@0..1 "("
                PATH_EXPR@1..2
                  NAME_REF@1..2
                    LOWER_IDENT@1..2 "a"
                R_PAREN@2..3 ")"
        "#]],
        );
        check_expr(
            "(a, b)",
            expect![[r#"
            SOURCE_FILE@0..6
              PAREN_EXPR@0..6
                L_PAREN@0..1 "("
                PATH_EXPR@1..2
                  NAME_REF@1..2
                    LOWER_IDENT@1..2 "a"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                PATH_EXPR@4..5
                  NAME_REF@4..5
                    LOWER_IDENT@4..5 "b"
                R_PAREN@5..6 ")"
        "#]],
        );
        check_expr(
            "()",
            expect![[r#"
            SOURCE_FILE@0..2
              PAREN_EXPR@0..2
                L_PAREN@0..1 "("
                R_PAREN@1..2 ")"
        "#]],
        );
    }

    #[test]
    fn lists() {
        check_expr(
            "[]",
            expect![[r#"
            SOURCE_FILE@0..2
              LIST_EXPR@0..2
                L_BRACK@0..1 "["
                R_BRACK@1..2 "]"
        "#]],
        );
        check_expr(
            "[a, b, ..rest]",
            expect![[r#"
            SOURCE_FILE@0..14
              LIST_EXPR@0..14
                L_BRACK@0..1 "["
                PATH_EXPR@1..2
                  NAME_REF@1..2
                    LOWER_IDENT@1..2 "a"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                PATH_EXPR@4..5
                  NAME_REF@4..5
                    LOWER_IDENT@4..5 "b"
                COMMA@5..6 ","
                WHITESPACE@6..7 " "
                SPREAD@7..13
                  DOT2@7..9 ".."
                  PATH_EXPR@9..13
                    NAME_REF@9..13
                      LOWER_IDENT@9..13 "rest"
                R_BRACK@13..14 "]"
        "#]],
        );
    }

    #[test]
    fn broken_lists() {
        check_expr(
            "[1, ]",
            expect![[r#"
            SOURCE_FILE@0..5
              LIST_EXPR@0..5
                L_BRACK@0..1 "["
                LITERAL@1..2
                  INT_NUMBER@1..2 "1"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                R_BRACK@4..5 "]"
            error 4..5: expected an expression
        "#]],
        );
        check_expr(
            "[..xs]",
            expect![[r#"
            SOURCE_FILE@0..6
              LIST_EXPR@0..3
                L_BRACK@0..1 "["
                ERROR@1..3
                  DOT2@1..3 ".."
              LOWER_IDENT@3..5 "xs"
              R_BRACK@5..6 "]"
            error 1..3: expected an expression
            error 3..5: expected `]`
        "#]],
        );
    }

    #[test]
    fn blocks_and_statements() {
        check_expr(
            "{\n  let a: Int = 1\n  f(a) g\n}",
            expect![[r#"
            SOURCE_FILE@0..29
              BLOCK_EXPR@0..29
                L_CURLY@0..1 "{"
                WHITESPACE@1..4 "\n  "
                LET_STMT@4..18
                  LET_KW@4..7 "let"
                  WHITESPACE@7..8 " "
                  NAME@8..9
                    LOWER_IDENT@8..9 "a"
                  COLON@9..10 ":"
                  WHITESPACE@10..11 " "
                  NAMED_TYPE@11..14
                    NAME_REF@11..14
                      UPPER_IDENT@11..14 "Int"
                  WHITESPACE@14..15 " "
                  EQ@15..16 "="
                  WHITESPACE@16..17 " "
                  LITERAL@17..18
                    INT_NUMBER@17..18 "1"
                WHITESPACE@18..21 "\n  "
                EXPR_STMT@21..25
                  CALL_EXPR@21..25
                    PATH_EXPR@21..22
                      NAME_REF@21..22
                        LOWER_IDENT@21..22 "f"
                    ARG_LIST@22..25
                      L_PAREN@22..23 "("
                      PATH_EXPR@23..24
                        NAME_REF@23..24
                          LOWER_IDENT@23..24 "a"
                      R_PAREN@24..25 ")"
                WHITESPACE@25..26 " "
                EXPR_STMT@26..27
                  PATH_EXPR@26..27
                    NAME_REF@26..27
                      LOWER_IDENT@26..27 "g"
                WHITESPACE@27..28 "\n"
                R_CURLY@28..29 "}"
        "#]],
        );
    }

    #[test]
    fn statements_take_as_much_as_they_can() {
        // `- 1` continues `a`; `(b)` on its own line doesn't continue it.
        check_expr(
            "{\n  a\n  - 1\n  (b)\n}",
            expect![[r#"
            SOURCE_FILE@0..19
              BLOCK_EXPR@0..19
                L_CURLY@0..1 "{"
                WHITESPACE@1..4 "\n  "
                EXPR_STMT@4..11
                  BIN_EXPR@4..11
                    PATH_EXPR@4..5
                      NAME_REF@4..5
                        LOWER_IDENT@4..5 "a"
                    WHITESPACE@5..8 "\n  "
                    MINUS@8..9 "-"
                    WHITESPACE@9..10 " "
                    LITERAL@10..11
                      INT_NUMBER@10..11 "1"
                WHITESPACE@11..14 "\n  "
                EXPR_STMT@14..17
                  PAREN_EXPR@14..17
                    L_PAREN@14..15 "("
                    PATH_EXPR@15..16
                      NAME_REF@15..16
                        LOWER_IDENT@15..16 "b"
                    R_PAREN@16..17 ")"
                WHITESPACE@17..18 "\n"
                R_CURLY@18..19 "}"
        "#]],
        );
    }

    #[test]
    fn lambdas() {
        check_expr(
            "fn(x, _: Int) -> Int { x }",
            expect![[r#"
            SOURCE_FILE@0..26
              LAMBDA_EXPR@0..26
                FN_KW@0..2 "fn"
                PARAM_LIST@2..13
                  L_PAREN@2..3 "("
                  PARAM@3..4
                    NAME@3..4
                      LOWER_IDENT@3..4 "x"
                  COMMA@4..5 ","
                  WHITESPACE@5..6 " "
                  PARAM@6..12
                    UNDERSCORE@6..7 "_"
                    COLON@7..8 ":"
                    WHITESPACE@8..9 " "
                    NAMED_TYPE@9..12
                      NAME_REF@9..12
                        UPPER_IDENT@9..12 "Int"
                  R_PAREN@12..13 ")"
                WHITESPACE@13..14 " "
                RET_TYPE@14..20
                  THIN_ARROW@14..16 "->"
                  WHITESPACE@16..17 " "
                  NAMED_TYPE@17..20
                    NAME_REF@17..20
                      UPPER_IDENT@17..20 "Int"
                WHITESPACE@20..21 " "
                BLOCK_EXPR@21..26
                  L_CURLY@21..22 "{"
                  WHITESPACE@22..23 " "
                  EXPR_STMT@23..24
                    PATH_EXPR@23..24
                      NAME_REF@23..24
                        LOWER_IDENT@23..24 "x"
                  WHITESPACE@24..25 " "
                  R_CURLY@25..26 "}"
        "#]],
        );
    }

    #[test]
    fn ifs() {
        check_expr(
            "if a { b } else if c { d } else { e }",
            expect![[r#"
            SOURCE_FILE@0..37
              IF_EXPR@0..37
                IF_KW@0..2 "if"
                WHITESPACE@2..3 " "
                PATH_EXPR@3..4
                  NAME_REF@3..4
                    LOWER_IDENT@3..4 "a"
                WHITESPACE@4..5 " "
                BLOCK_EXPR@5..10
                  L_CURLY@5..6 "{"
                  WHITESPACE@6..7 " "
                  EXPR_STMT@7..8
                    PATH_EXPR@7..8
                      NAME_REF@7..8
                        LOWER_IDENT@7..8 "b"
                  WHITESPACE@8..9 " "
                  R_CURLY@9..10 "}"
                WHITESPACE@10..11 " "
                ELSE_KW@11..15 "else"
                WHITESPACE@15..16 " "
                IF_EXPR@16..37
                  IF_KW@16..18 "if"
                  WHITESPACE@18..19 " "
                  PATH_EXPR@19..20
                    NAME_REF@19..20
                      LOWER_IDENT@19..20 "c"
                  WHITESPACE@20..21 " "
                  BLOCK_EXPR@21..26
                    L_CURLY@21..22 "{"
                    WHITESPACE@22..23 " "
                    EXPR_STMT@23..24
                      PATH_EXPR@23..24
                        NAME_REF@23..24
                          LOWER_IDENT@23..24 "d"
                    WHITESPACE@24..25 " "
                    R_CURLY@25..26 "}"
                  WHITESPACE@26..27 " "
                  ELSE_KW@27..31 "else"
                  WHITESPACE@31..32 " "
                  BLOCK_EXPR@32..37
                    L_CURLY@32..33 "{"
                    WHITESPACE@33..34 " "
                    EXPR_STMT@34..35
                      PATH_EXPR@34..35
                        NAME_REF@34..35
                          LOWER_IDENT@34..35 "e"
                    WHITESPACE@35..36 " "
                    R_CURLY@36..37 "}"
        "#]],
        );
        check_expr(
            "if a { b }",
            expect![[r#"
            SOURCE_FILE@0..10
              IF_EXPR@0..10
                IF_KW@0..2 "if"
                WHITESPACE@2..3 " "
                PATH_EXPR@3..4
                  NAME_REF@3..4
                    LOWER_IDENT@3..4 "a"
                WHITESPACE@4..5 " "
                BLOCK_EXPR@5..10
                  L_CURLY@5..6 "{"
                  WHITESPACE@6..7 " "
                  EXPR_STMT@7..8
                    PATH_EXPR@7..8
                      NAME_REF@7..8
                        LOWER_IDENT@7..8 "b"
                  WHITESPACE@8..9 " "
                  R_CURLY@9..10 "}"
            error 10..10: expected `else`: an `if` is an expression, so it needs both branches
        "#]],
        );
    }

    #[test]
    fn matches() {
        check_expr(
            "match x { A(y) if y > 0 -> y _ -> 0 }",
            expect![[r#"
            SOURCE_FILE@0..37
              MATCH_EXPR@0..37
                MATCH_KW@0..5 "match"
                WHITESPACE@5..6 " "
                PATH_EXPR@6..7
                  NAME_REF@6..7
                    LOWER_IDENT@6..7 "x"
                WHITESPACE@7..8 " "
                MATCH_ARM_LIST@8..37
                  L_CURLY@8..9 "{"
                  WHITESPACE@9..10 " "
                  MATCH_ARM@10..28
                    CTOR_PAT@10..14
                      NAME_REF@10..11
                        UPPER_IDENT@10..11 "A"
                      L_PAREN@11..12 "("
                      IDENT_PAT@12..13
                        NAME@12..13
                          LOWER_IDENT@12..13 "y"
                      R_PAREN@13..14 ")"
                    WHITESPACE@14..15 " "
                    MATCH_GUARD@15..23
                      IF_KW@15..17 "if"
                      WHITESPACE@17..18 " "
                      BIN_EXPR@18..23
                        PATH_EXPR@18..19
                          NAME_REF@18..19
                            LOWER_IDENT@18..19 "y"
                        WHITESPACE@19..20 " "
                        R_ANGLE@20..21 ">"
                        WHITESPACE@21..22 " "
                        LITERAL@22..23
                          INT_NUMBER@22..23 "0"
                    WHITESPACE@23..24 " "
                    THIN_ARROW@24..26 "->"
                    WHITESPACE@26..27 " "
                    PATH_EXPR@27..28
                      NAME_REF@27..28
                        LOWER_IDENT@27..28 "y"
                  WHITESPACE@28..29 " "
                  MATCH_ARM@29..35
                    WILDCARD_PAT@29..30
                      UNDERSCORE@29..30 "_"
                    WHITESPACE@30..31 " "
                    THIN_ARROW@31..33 "->"
                    WHITESPACE@33..34 " "
                    LITERAL@34..35
                      INT_NUMBER@34..35 "0"
                  WHITESPACE@35..36 " "
                  R_CURLY@36..37 "}"
        "#]],
        );
    }

    #[test]
    fn broken_matches() {
        check_expr(
            "match x { }",
            expect![[r#"
            SOURCE_FILE@0..11
              MATCH_EXPR@0..11
                MATCH_KW@0..5 "match"
                WHITESPACE@5..6 " "
                PATH_EXPR@6..7
                  NAME_REF@6..7
                    LOWER_IDENT@6..7 "x"
                WHITESPACE@7..8 " "
                MATCH_ARM_LIST@8..11
                  L_CURLY@8..9 "{"
                  WHITESPACE@9..10 " "
                  R_CURLY@10..11 "}"
            error 10..11: expected a match arm: a `match` needs at least one
        "#]],
        );
        check_expr(
            "match x { -1 -> a }",
            expect![[r#"
            SOURCE_FILE@0..19
              MATCH_EXPR@0..19
                MATCH_KW@0..5 "match"
                WHITESPACE@5..6 " "
                PATH_EXPR@6..7
                  NAME_REF@6..7
                    LOWER_IDENT@6..7 "x"
                WHITESPACE@7..8 " "
                MATCH_ARM_LIST@8..19
                  L_CURLY@8..9 "{"
                  WHITESPACE@9..10 " "
                  ERROR@10..11
                    MINUS@10..11 "-"
                  MATCH_ARM@11..17
                    LITERAL_PAT@11..12
                      INT_NUMBER@11..12 "1"
                    WHITESPACE@12..13 " "
                    THIN_ARROW@13..15 "->"
                    WHITESPACE@15..16 " "
                    PATH_EXPR@16..17
                      NAME_REF@16..17
                        LOWER_IDENT@16..17 "a"
                  WHITESPACE@17..18 " "
                  R_CURLY@18..19 "}"
            error 10..11: expected a match arm: negative numbers can't be patterns
        "#]],
        );
        check_expr(
            "match x { y }",
            expect![[r#"
            SOURCE_FILE@0..13
              MATCH_EXPR@0..13
                MATCH_KW@0..5 "match"
                WHITESPACE@5..6 " "
                PATH_EXPR@6..7
                  NAME_REF@6..7
                    LOWER_IDENT@6..7 "x"
                WHITESPACE@7..8 " "
                MATCH_ARM_LIST@8..13
                  L_CURLY@8..9 "{"
                  WHITESPACE@9..10 " "
                  MATCH_ARM@10..11
                    IDENT_PAT@10..11
                      NAME@10..11
                        LOWER_IDENT@10..11 "y"
                  WHITESPACE@11..12 " "
                  R_CURLY@12..13 "}"
            error 12..13: expected `->`
        "#]],
        );
    }
}
