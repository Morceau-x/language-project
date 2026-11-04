//! Patterns, on the left of a match arm: `Circle(r)`, `[x, ..rest]`.

use super::*;

/// The tokens that start a pattern.
pub(super) const PAT_FIRST: TokenSet = TokenSet::new(&[
    T![_],
    LOWER_IDENT,
    INT_NUMBER,
    FLOAT_NUMBER,
    STRING,
    UPPER_IDENT,
    T!['['],
]);

/// Where to go on after a broken pattern.
const PAT_RECOVERY: TokenSet = TokenSet::new(&[T![->], T![if], T![,], T![')'], T![']'], T!['}']]);

/// `Pattern ::= "_" | LOWER | INT | FLOAT | STRING | CtorPat | ListPat`
pub(super) fn pattern(p: &mut Parser<'_>) {
    todo!("Part 8b: pattern")
}

/// `CtorPat ::= (UPPER ".")? UPPER ("(" Pattern ("," Pattern)* ")")?`
fn ctor_pat(p: &mut Parser<'_>) {
    todo!("Part 8b: ctor_pat")
}

/// `ListPat ::= "[" (Pattern ("," Pattern)* ("," RestPat)?)? "]"`
fn list_pat(p: &mut Parser<'_>) {
    todo!("Part 8b: list_pat")
}

/// `RestPat ::= ".." (LOWER | "_")`
fn rest_pat(p: &mut Parser<'_>) {
    todo!("Part 8b: rest_pat")
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn simple_patterns() {
        check(
            pattern,
            "_",
            expect![[r#"
            SOURCE_FILE@0..1
              WILDCARD_PAT@0..1
                UNDERSCORE@0..1 "_"
        "#]],
        );
        check(
            pattern,
            "x",
            expect![[r#"
            SOURCE_FILE@0..1
              IDENT_PAT@0..1
                NAME@0..1
                  LOWER_IDENT@0..1 "x"
        "#]],
        );
        check(
            pattern,
            "\"s\"",
            expect![[r#"
            SOURCE_FILE@0..3
              LITERAL_PAT@0..3
                STRING@0..3 "\"s\""
        "#]],
        );
    }

    #[test]
    fn constructor_patterns() {
        check(
            pattern,
            "None",
            expect![[r#"
            SOURCE_FILE@0..4
              CTOR_PAT@0..4
                NAME_REF@0..4
                  UPPER_IDENT@0..4 "None"
        "#]],
        );
        check(
            pattern,
            "Geo.Circle(r)",
            expect![[r#"
            SOURCE_FILE@0..13
              CTOR_PAT@0..13
                NAME_REF@0..3
                  UPPER_IDENT@0..3 "Geo"
                DOT@3..4 "."
                NAME_REF@4..10
                  UPPER_IDENT@4..10 "Circle"
                L_PAREN@10..11 "("
                IDENT_PAT@11..12
                  NAME@11..12
                    LOWER_IDENT@11..12 "r"
                R_PAREN@12..13 ")"
        "#]],
        );
        check(
            pattern,
            "Rect(_, 1.5)",
            expect![[r#"
            SOURCE_FILE@0..12
              CTOR_PAT@0..12
                NAME_REF@0..4
                  UPPER_IDENT@0..4 "Rect"
                L_PAREN@4..5 "("
                WILDCARD_PAT@5..6
                  UNDERSCORE@5..6 "_"
                COMMA@6..7 ","
                WHITESPACE@7..8 " "
                LITERAL_PAT@8..11
                  FLOAT_NUMBER@8..11 "1.5"
                R_PAREN@11..12 ")"
        "#]],
        );
    }

    #[test]
    fn a_constructors_paren_stays_on_its_line() {
        check(
            pattern,
            "Some\n(x)",
            expect![[r#"
                SOURCE_FILE@0..8
                  CTOR_PAT@0..4
                    NAME_REF@0..4
                      UPPER_IDENT@0..4 "Some"
                  WHITESPACE@4..5 "\n"
                  L_PAREN@5..6 "("
                  LOWER_IDENT@6..7 "x"
                  R_PAREN@7..8 ")"
                error 5..6: unexpected tokens at the end
            "#]],
        );
    }

    #[test]
    fn list_patterns() {
        check(
            pattern,
            "[]",
            expect![[r#"
            SOURCE_FILE@0..2
              LIST_PAT@0..2
                L_BRACK@0..1 "["
                R_BRACK@1..2 "]"
        "#]],
        );
        check(
            pattern,
            "[x, Some(y), ..rest]",
            expect![[r#"
            SOURCE_FILE@0..20
              LIST_PAT@0..20
                L_BRACK@0..1 "["
                IDENT_PAT@1..2
                  NAME@1..2
                    LOWER_IDENT@1..2 "x"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                CTOR_PAT@4..11
                  NAME_REF@4..8
                    UPPER_IDENT@4..8 "Some"
                  L_PAREN@8..9 "("
                  IDENT_PAT@9..10
                    NAME@9..10
                      LOWER_IDENT@9..10 "y"
                  R_PAREN@10..11 ")"
                COMMA@11..12 ","
                WHITESPACE@12..13 " "
                REST_PAT@13..19
                  DOT2@13..15 ".."
                  NAME@15..19
                    LOWER_IDENT@15..19 "rest"
                R_BRACK@19..20 "]"
        "#]],
        );
        check(
            pattern,
            "[a, .._]",
            expect![[r#"
            SOURCE_FILE@0..8
              LIST_PAT@0..8
                L_BRACK@0..1 "["
                IDENT_PAT@1..2
                  NAME@1..2
                    LOWER_IDENT@1..2 "a"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                REST_PAT@4..7
                  DOT2@4..6 ".."
                  UNDERSCORE@6..7 "_"
                R_BRACK@7..8 "]"
        "#]],
        );
    }

    #[test]
    fn broken_patterns() {
        check(
            pattern,
            "Some()",
            expect![[r#"
            SOURCE_FILE@0..6
              CTOR_PAT@0..6
                NAME_REF@0..4
                  UPPER_IDENT@0..4 "Some"
                L_PAREN@4..5 "("
                R_PAREN@5..6 ")"
            error 5..6: expected a pattern
        "#]],
        );
        check(
            pattern,
            "[..rest]",
            expect![[r#"
            SOURCE_FILE@0..8
              LIST_PAT@0..3
                L_BRACK@0..1 "["
                ERROR@1..3
                  DOT2@1..3 ".."
              LOWER_IDENT@3..7 "rest"
              R_BRACK@7..8 "]"
            error 1..3: expected a pattern
            error 3..7: expected `]`
        "#]],
        );
        check(
            pattern,
            "[x, ..]",
            expect![[r#"
            SOURCE_FILE@0..7
              LIST_PAT@0..7
                L_BRACK@0..1 "["
                IDENT_PAT@1..2
                  NAME@1..2
                    LOWER_IDENT@1..2 "x"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                REST_PAT@4..6
                  DOT2@4..6 ".."
                R_BRACK@6..7 "]"
            error 6..7: expected a name or `_` after `..`
        "#]],
        );
        check(
            pattern,
            "Geo.circle",
            expect![[r#"
            SOURCE_FILE@0..10
              CTOR_PAT@0..4
                NAME_REF@0..3
                  UPPER_IDENT@0..3 "Geo"
                DOT@3..4 "."
              LOWER_IDENT@4..10 "circle"
            error 4..10: expected a capitalized name
        "#]],
        );
    }
}
