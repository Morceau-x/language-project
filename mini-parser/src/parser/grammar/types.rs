//! Types: `Int`, `List<a>`, `(Int, label: String) -> Bool`.

use super::*;

/// Where to go on after a broken type: what comes after types.
const TYPE_RECOVERY: TokenSet =
    TokenSet::new(&[T![,], T![')'], T![>], T![=], T!['{'], T!['}'], T![->]])
        .union(items::DECL_FIRST);

/// `Type ::= UPPER TypeArgList? | LOWER | FnType`
pub(super) fn type_(p: &mut Parser<'_>) {
    todo!("Part 8a: type_")
}

/// `TypeArgList ::= "<" Type ("," Type)* ">"`
fn type_arg_list(p: &mut Parser<'_>) {
    todo!("Part 8a: type_arg_list")
}

/// `FnType ::= ParamTypeList "->" Type`
fn fn_type(p: &mut Parser<'_>) {
    todo!("Part 8a: fn_type")
}

/// `ParamTypeList ::= "(" (ParamType ("," ParamType)*)? ")"`
fn param_type_list(p: &mut Parser<'_>) {
    todo!("Part 8a: param_type_list")
}

/// `ParamType ::= (LOWER ":")? Type`. Two tokens of lookahead: `a` alone
/// is a type variable, `a:` is a label.
fn param_type(p: &mut Parser<'_>) {
    todo!("Part 8a: param_type")
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn named_types() {
        check(
            type_,
            "Int",
            expect![[r#"
            SOURCE_FILE@0..3
              NAMED_TYPE@0..3
                NAME_REF@0..3
                  UPPER_IDENT@0..3 "Int"
        "#]],
        );
        check(
            type_,
            "a",
            expect![[r#"
            SOURCE_FILE@0..1
              NAMED_TYPE@0..1
                NAME_REF@0..1
                  LOWER_IDENT@0..1 "a"
        "#]],
        );
        check(
            type_,
            "Dict<String, List<a>>",
            expect![[r#"
            SOURCE_FILE@0..21
              NAMED_TYPE@0..21
                NAME_REF@0..4
                  UPPER_IDENT@0..4 "Dict"
                TYPE_ARG_LIST@4..21
                  L_ANGLE@4..5 "<"
                  NAMED_TYPE@5..11
                    NAME_REF@5..11
                      UPPER_IDENT@5..11 "String"
                  COMMA@11..12 ","
                  WHITESPACE@12..13 " "
                  NAMED_TYPE@13..20
                    NAME_REF@13..17
                      UPPER_IDENT@13..17 "List"
                    TYPE_ARG_LIST@17..20
                      L_ANGLE@17..18 "<"
                      NAMED_TYPE@18..19
                        NAME_REF@18..19
                          LOWER_IDENT@18..19 "a"
                      R_ANGLE@19..20 ">"
                  R_ANGLE@20..21 ">"
        "#]],
        );
    }

    #[test]
    fn function_types() {
        check(
            type_,
            "(a) -> b",
            expect![[r#"
            SOURCE_FILE@0..8
              FN_TYPE@0..8
                PARAM_TYPE_LIST@0..3
                  L_PAREN@0..1 "("
                  PARAM_TYPE@1..2
                    NAMED_TYPE@1..2
                      NAME_REF@1..2
                        LOWER_IDENT@1..2 "a"
                  R_PAREN@2..3 ")"
                WHITESPACE@3..4 " "
                THIN_ARROW@4..6 "->"
                WHITESPACE@6..7 " "
                NAMED_TYPE@7..8
                  NAME_REF@7..8
                    LOWER_IDENT@7..8 "b"
        "#]],
        );
        check(
            type_,
            "() -> Int",
            expect![[r#"
            SOURCE_FILE@0..9
              FN_TYPE@0..9
                PARAM_TYPE_LIST@0..2
                  L_PAREN@0..1 "("
                  R_PAREN@1..2 ")"
                WHITESPACE@2..3 " "
                THIN_ARROW@3..5 "->"
                WHITESPACE@5..6 " "
                NAMED_TYPE@6..9
                  NAME_REF@6..9
                    UPPER_IDENT@6..9 "Int"
        "#]],
        );
        check(
            type_,
            "(acc: b, a) -> (Int) -> b",
            expect![[r#"
            SOURCE_FILE@0..25
              FN_TYPE@0..25
                PARAM_TYPE_LIST@0..11
                  L_PAREN@0..1 "("
                  PARAM_TYPE@1..7
                    NAME@1..4
                      LOWER_IDENT@1..4 "acc"
                    COLON@4..5 ":"
                    WHITESPACE@5..6 " "
                    NAMED_TYPE@6..7
                      NAME_REF@6..7
                        LOWER_IDENT@6..7 "b"
                  COMMA@7..8 ","
                  WHITESPACE@8..9 " "
                  PARAM_TYPE@9..10
                    NAMED_TYPE@9..10
                      NAME_REF@9..10
                        LOWER_IDENT@9..10 "a"
                  R_PAREN@10..11 ")"
                WHITESPACE@11..12 " "
                THIN_ARROW@12..14 "->"
                WHITESPACE@14..15 " "
                FN_TYPE@15..25
                  PARAM_TYPE_LIST@15..20
                    L_PAREN@15..16 "("
                    PARAM_TYPE@16..19
                      NAMED_TYPE@16..19
                        NAME_REF@16..19
                          UPPER_IDENT@16..19 "Int"
                    R_PAREN@19..20 ")"
                  WHITESPACE@20..21 " "
                  THIN_ARROW@21..23 "->"
                  WHITESPACE@23..24 " "
                  NAMED_TYPE@24..25
                    NAME_REF@24..25
                      LOWER_IDENT@24..25 "b"
        "#]],
        );
    }

    #[test]
    fn broken_types() {
        check(
            type_,
            "(Int)",
            expect![[r#"
            SOURCE_FILE@0..5
              FN_TYPE@0..5
                PARAM_TYPE_LIST@0..5
                  L_PAREN@0..1 "("
                  PARAM_TYPE@1..4
                    NAMED_TYPE@1..4
                      NAME_REF@1..4
                        UPPER_IDENT@1..4 "Int"
                  R_PAREN@4..5 ")"
            error 5..5: expected `->`
        "#]],
        );
        check(
            type_,
            "List<>",
            expect![[r#"
            SOURCE_FILE@0..6
              NAMED_TYPE@0..6
                NAME_REF@0..4
                  UPPER_IDENT@0..4 "List"
                TYPE_ARG_LIST@4..6
                  L_ANGLE@4..5 "<"
                  R_ANGLE@5..6 ">"
            error 5..6: expected a type
        "#]],
        );
        check(
            type_,
            "5",
            expect![[r#"
            SOURCE_FILE@0..1
              ERROR@0..1
                INT_NUMBER@0..1 "5"
            error 0..1: expected a type
        "#]],
        );
    }
}
