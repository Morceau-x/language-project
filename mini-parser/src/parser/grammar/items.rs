//! Declarations: everything at the top level of a file.

use super::*;

/// The tokens that start a declaration.
pub(super) const DECL_FIRST: TokenSet = TokenSet::new(&[
    T![import],
    T![type],
    T![fn],
    T![cast],
    T![component],
    T![let],
]);

/// `Decl ::= Import | TypeDecl | Fn | Cast | Component | LetDecl`
pub(super) fn decl(p: &mut Parser<'_>) {
    match p.current() {
        T![import] => import(p),
        T![type] => type_decl(p),
        T![fn] => fn_decl(p),
        T![cast] => cast_decl(p),
        T![component] => component(p),
        T![let] => let_binding(p, LET_DECL),
        _ => {
            // Skip to the next declaration, in one ERROR node.
            let m = p.start();
            if p.at(T!['(']) && p.at_line_start() {
                p.error(
                    "expected a declaration: a call's `(` must be on the same line as the function",
                );
            } else {
                p.error(
                    "expected a declaration: `fn`, `let`, `type`, `import`, `cast` or `component`",
                );
            }
            while !p.at(EOF) && !p.at_ts(DECL_FIRST) {
                p.bump_any();
            }
            m.complete(p, ERROR);
        }
    }
}

/// `Import ::= "import" UPPER`
fn import(p: &mut Parser<'_>) {
    let m = p.start();
    p.bump(T![import]);
    name_ref(p, UPPER_IDENT);
    m.complete(p, IMPORT);
}

/// `TypeDecl ::= "type" UPPER TypeParamList? "=" (VariantList | RecordFieldList)`
fn type_decl(p: &mut Parser<'_>) {
    todo!("Part 10a: type_decl")
}

/// `TypeParamList ::= "<" LOWER ("," LOWER)* ">"`
fn type_param_list(p: &mut Parser<'_>) {
    todo!("Part 10a: type_param_list")
}

/// `VariantList ::= "|"? Variant ("|" Variant)*`
fn variant_list(p: &mut Parser<'_>) {
    todo!("Part 10a: variant_list")
}

/// `Variant ::= UPPER TupleFieldList?`
/// `TupleFieldList ::= "(" Type ("," Type)* ")"`
fn variant(p: &mut Parser<'_>) {
    todo!("Part 10a: variant")
}

/// `RecordFieldList ::= "{" RecordField ("," RecordField)* "}"`
fn record_field_list(p: &mut Parser<'_>) {
    todo!("Part 10a: record_field_list")
}

/// `RecordField ::= LOWER ":" Type`
fn record_field(p: &mut Parser<'_>) {
    todo!("Part 10a: record_field")
}

/// `Fn ::= "fn" LOWER ParamList RetType? Block`
fn fn_decl(p: &mut Parser<'_>) {
    todo!("Part 10b: fn_decl")
}

/// `Cast ::= "cast" ParamList RetType? Block`
fn cast_decl(p: &mut Parser<'_>) {
    todo!("Part 10b: cast_decl")
}

/// `Component ::= "component" UPPER ParamList`
fn component(p: &mut Parser<'_>) {
    todo!("Part 10b: component")
}

/// `LetDecl ::= "let" LOWER (":" Type)? "=" Expr`, and the same for
/// `LET_STMT` in a block.
pub(super) fn let_binding(p: &mut Parser<'_>, kind: SyntaxKind) {
    todo!("Part 9d: let_binding")
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn imports() {
        check(
            decl,
            "import List",
            expect![[r#"
            SOURCE_FILE@0..11
              IMPORT@0..11
                IMPORT_KW@0..6 "import"
                WHITESPACE@6..7 " "
                NAME_REF@7..11
                  UPPER_IDENT@7..11 "List"
        "#]],
        );
    }

    #[test]
    fn lets() {
        check(
            decl,
            "let pi: Float = 3.14",
            expect![[r#"
            SOURCE_FILE@0..20
              LET_DECL@0..20
                LET_KW@0..3 "let"
                WHITESPACE@3..4 " "
                NAME@4..6
                  LOWER_IDENT@4..6 "pi"
                COLON@6..7 ":"
                WHITESPACE@7..8 " "
                NAMED_TYPE@8..13
                  NAME_REF@8..13
                    UPPER_IDENT@8..13 "Float"
                WHITESPACE@13..14 " "
                EQ@14..15 "="
                WHITESPACE@15..16 " "
                LITERAL@16..20
                  FLOAT_NUMBER@16..20 "3.14"
        "#]],
        );
        check(
            decl,
            "let x 1",
            expect![[r#"
            SOURCE_FILE@0..7
              LET_DECL@0..7
                LET_KW@0..3 "let"
                WHITESPACE@3..4 " "
                NAME@4..5
                  LOWER_IDENT@4..5 "x"
                WHITESPACE@5..6 " "
                LITERAL@6..7
                  INT_NUMBER@6..7 "1"
            error 6..7: expected `=`
        "#]],
        );
    }

    #[test]
    fn variant_types() {
        check(
            decl,
            "type Shape = | Circle(Float) | Rect(Float, Float)",
            expect![[r#"
            SOURCE_FILE@0..49
              TYPE_DECL@0..49
                TYPE_KW@0..4 "type"
                WHITESPACE@4..5 " "
                NAME@5..10
                  UPPER_IDENT@5..10 "Shape"
                WHITESPACE@10..11 " "
                EQ@11..12 "="
                WHITESPACE@12..13 " "
                VARIANT_LIST@13..49
                  PIPE@13..14 "|"
                  WHITESPACE@14..15 " "
                  VARIANT@15..28
                    NAME@15..21
                      UPPER_IDENT@15..21 "Circle"
                    TUPLE_FIELD_LIST@21..28
                      L_PAREN@21..22 "("
                      NAMED_TYPE@22..27
                        NAME_REF@22..27
                          UPPER_IDENT@22..27 "Float"
                      R_PAREN@27..28 ")"
                  WHITESPACE@28..29 " "
                  PIPE@29..30 "|"
                  WHITESPACE@30..31 " "
                  VARIANT@31..49
                    NAME@31..35
                      UPPER_IDENT@31..35 "Rect"
                    TUPLE_FIELD_LIST@35..49
                      L_PAREN@35..36 "("
                      NAMED_TYPE@36..41
                        NAME_REF@36..41
                          UPPER_IDENT@36..41 "Float"
                      COMMA@41..42 ","
                      WHITESPACE@42..43 " "
                      NAMED_TYPE@43..48
                        NAME_REF@43..48
                          UPPER_IDENT@43..48 "Float"
                      R_PAREN@48..49 ")"
        "#]],
        );
        check(
            decl,
            "type Option<a> = Some(a) | None",
            expect![[r#"
            SOURCE_FILE@0..31
              TYPE_DECL@0..31
                TYPE_KW@0..4 "type"
                WHITESPACE@4..5 " "
                NAME@5..11
                  UPPER_IDENT@5..11 "Option"
                TYPE_PARAM_LIST@11..14
                  L_ANGLE@11..12 "<"
                  NAME@12..13
                    LOWER_IDENT@12..13 "a"
                  R_ANGLE@13..14 ">"
                WHITESPACE@14..15 " "
                EQ@15..16 "="
                WHITESPACE@16..17 " "
                VARIANT_LIST@17..31
                  VARIANT@17..24
                    NAME@17..21
                      UPPER_IDENT@17..21 "Some"
                    TUPLE_FIELD_LIST@21..24
                      L_PAREN@21..22 "("
                      NAMED_TYPE@22..23
                        NAME_REF@22..23
                          LOWER_IDENT@22..23 "a"
                      R_PAREN@23..24 ")"
                  WHITESPACE@24..25 " "
                  PIPE@25..26 "|"
                  WHITESPACE@26..27 " "
                  VARIANT@27..31
                    NAME@27..31
                      UPPER_IDENT@27..31 "None"
        "#]],
        );
    }

    #[test]
    fn record_types() {
        check(
            decl,
            "type Point = { x: Float, y: Float }",
            expect![[r#"
            SOURCE_FILE@0..35
              TYPE_DECL@0..35
                TYPE_KW@0..4 "type"
                WHITESPACE@4..5 " "
                NAME@5..10
                  UPPER_IDENT@5..10 "Point"
                WHITESPACE@10..11 " "
                EQ@11..12 "="
                WHITESPACE@12..13 " "
                RECORD_FIELD_LIST@13..35
                  L_CURLY@13..14 "{"
                  WHITESPACE@14..15 " "
                  RECORD_FIELD@15..23
                    NAME@15..16
                      LOWER_IDENT@15..16 "x"
                    COLON@16..17 ":"
                    WHITESPACE@17..18 " "
                    NAMED_TYPE@18..23
                      NAME_REF@18..23
                        UPPER_IDENT@18..23 "Float"
                  COMMA@23..24 ","
                  WHITESPACE@24..25 " "
                  RECORD_FIELD@25..33
                    NAME@25..26
                      LOWER_IDENT@25..26 "y"
                    COLON@26..27 ":"
                    WHITESPACE@27..28 " "
                    NAMED_TYPE@28..33
                      NAME_REF@28..33
                        UPPER_IDENT@28..33 "Float"
                  WHITESPACE@33..34 " "
                  R_CURLY@34..35 "}"
        "#]],
        );
    }

    #[test]
    fn broken_types() {
        check(
            decl,
            "type T = A()",
            expect![[r#"
            SOURCE_FILE@0..12
              TYPE_DECL@0..12
                TYPE_KW@0..4 "type"
                WHITESPACE@4..5 " "
                NAME@5..6
                  UPPER_IDENT@5..6 "T"
                WHITESPACE@6..7 " "
                EQ@7..8 "="
                WHITESPACE@8..9 " "
                VARIANT_LIST@9..12
                  VARIANT@9..12
                    NAME@9..10
                      UPPER_IDENT@9..10 "A"
                    TUPLE_FIELD_LIST@10..12
                      L_PAREN@10..11 "("
                      R_PAREN@11..12 ")"
            error 11..12: expected a type
        "#]],
        );
        check(
            decl,
            "type T =",
            expect![[r#"
            SOURCE_FILE@0..8
              TYPE_DECL@0..8
                TYPE_KW@0..4 "type"
                WHITESPACE@4..5 " "
                NAME@5..6
                  UPPER_IDENT@5..6 "T"
                WHITESPACE@6..7 " "
                EQ@7..8 "="
                VARIANT_LIST@8..8
                  VARIANT@8..8
            error 8..8: expected a capitalized name
        "#]],
        );
    }

    #[test]
    fn functions() {
        check(
            decl,
            "fn area(s: Shape) -> Float { s }",
            expect![[r#"
            SOURCE_FILE@0..32
              FN@0..32
                FN_KW@0..2 "fn"
                WHITESPACE@2..3 " "
                NAME@3..7
                  LOWER_IDENT@3..7 "area"
                PARAM_LIST@7..17
                  L_PAREN@7..8 "("
                  PARAM@8..16
                    NAME@8..9
                      LOWER_IDENT@8..9 "s"
                    COLON@9..10 ":"
                    WHITESPACE@10..11 " "
                    NAMED_TYPE@11..16
                      NAME_REF@11..16
                        UPPER_IDENT@11..16 "Shape"
                  R_PAREN@16..17 ")"
                WHITESPACE@17..18 " "
                RET_TYPE@18..26
                  THIN_ARROW@18..20 "->"
                  WHITESPACE@20..21 " "
                  NAMED_TYPE@21..26
                    NAME_REF@21..26
                      UPPER_IDENT@21..26 "Float"
                WHITESPACE@26..27 " "
                BLOCK_EXPR@27..32
                  L_CURLY@27..28 "{"
                  WHITESPACE@28..29 " "
                  EXPR_STMT@29..30
                    PATH_EXPR@29..30
                      NAME_REF@29..30
                        LOWER_IDENT@29..30 "s"
                  WHITESPACE@30..31 " "
                  R_CURLY@31..32 "}"
        "#]],
        );
        check(
            decl,
            "fn main() {}",
            expect![[r#"
            SOURCE_FILE@0..12
              FN@0..12
                FN_KW@0..2 "fn"
                WHITESPACE@2..3 " "
                NAME@3..7
                  LOWER_IDENT@3..7 "main"
                PARAM_LIST@7..9
                  L_PAREN@7..8 "("
                  R_PAREN@8..9 ")"
                WHITESPACE@9..10 " "
                BLOCK_EXPR@10..12
                  L_CURLY@10..11 "{"
                  R_CURLY@11..12 "}"
        "#]],
        );
    }

    #[test]
    fn casts_and_components() {
        check(
            decl,
            "cast (d: Float) -> Celsius { d }",
            expect![[r#"
            SOURCE_FILE@0..32
              CAST@0..32
                CAST_KW@0..4 "cast"
                WHITESPACE@4..5 " "
                PARAM_LIST@5..15
                  L_PAREN@5..6 "("
                  PARAM@6..14
                    NAME@6..7
                      LOWER_IDENT@6..7 "d"
                    COLON@7..8 ":"
                    WHITESPACE@8..9 " "
                    NAMED_TYPE@9..14
                      NAME_REF@9..14
                        UPPER_IDENT@9..14 "Float"
                  R_PAREN@14..15 ")"
                WHITESPACE@15..16 " "
                RET_TYPE@16..26
                  THIN_ARROW@16..18 "->"
                  WHITESPACE@18..19 " "
                  NAMED_TYPE@19..26
                    NAME_REF@19..26
                      UPPER_IDENT@19..26 "Celsius"
                WHITESPACE@26..27 " "
                BLOCK_EXPR@27..32
                  L_CURLY@27..28 "{"
                  WHITESPACE@28..29 " "
                  EXPR_STMT@29..30
                    PATH_EXPR@29..30
                      NAME_REF@29..30
                        LOWER_IDENT@29..30 "d"
                  WHITESPACE@30..31 " "
                  R_CURLY@31..32 "}"
        "#]],
        );
        check(
            decl,
            "component Card(title: String)",
            expect![[r#"
            SOURCE_FILE@0..29
              COMPONENT@0..29
                COMPONENT_KW@0..9 "component"
                WHITESPACE@9..10 " "
                NAME@10..14
                  UPPER_IDENT@10..14 "Card"
                PARAM_LIST@14..29
                  L_PAREN@14..15 "("
                  PARAM@15..28
                    NAME@15..20
                      LOWER_IDENT@15..20 "title"
                    COLON@20..21 ":"
                    WHITESPACE@21..22 " "
                    NAMED_TYPE@22..28
                      NAME_REF@22..28
                        UPPER_IDENT@22..28 "String"
                  R_PAREN@28..29 ")"
        "#]],
        );
    }

    #[test]
    fn a_lambda_is_not_a_declaration() {
        check(
            decl,
            "fn(x) { x }",
            expect![[r#"
            SOURCE_FILE@0..11
              FN@0..11
                FN_KW@0..2 "fn"
                PARAM_LIST@2..5
                  L_PAREN@2..3 "("
                  PARAM@3..4
                    NAME@3..4
                      LOWER_IDENT@3..4 "x"
                  R_PAREN@4..5 ")"
                WHITESPACE@5..6 " "
                BLOCK_EXPR@6..11
                  L_CURLY@6..7 "{"
                  WHITESPACE@7..8 " "
                  EXPR_STMT@8..9
                    PATH_EXPR@8..9
                      NAME_REF@8..9
                        LOWER_IDENT@8..9 "x"
                  WHITESPACE@9..10 " "
                  R_CURLY@10..11 "}"
            error 2..3: expected the function's name: a lambda can't stand alone at the top level
        "#]],
        );
    }

    #[test]
    fn skips_to_the_next_declaration() {
        check_file(
            "module M\nfoo bar\nfn f() {}",
            expect![[r#"
            SOURCE_FILE@0..26
              MODULE_HEADER@0..8
                MODULE_KW@0..6 "module"
                WHITESPACE@6..7 " "
                NAME@7..8
                  UPPER_IDENT@7..8 "M"
              WHITESPACE@8..9 "\n"
              ERROR@9..16
                LOWER_IDENT@9..12 "foo"
                WHITESPACE@12..13 " "
                LOWER_IDENT@13..16 "bar"
              WHITESPACE@16..17 "\n"
              FN@17..26
                FN_KW@17..19 "fn"
                WHITESPACE@19..20 " "
                NAME@20..21
                  LOWER_IDENT@20..21 "f"
                PARAM_LIST@21..23
                  L_PAREN@21..22 "("
                  R_PAREN@22..23 ")"
                WHITESPACE@23..24 " "
                BLOCK_EXPR@24..26
                  L_CURLY@24..25 "{"
                  R_CURLY@25..26 "}"
            error 9..12: expected a declaration: `fn`, `let`, `type`, `import`, `cast` or `component`
        "#]],
        );
    }

    #[test]
    fn a_block_stops_at_a_declaration() {
        check_file(
            "module M\nfn f() {\n  1\nfn g() {}\n",
            expect![[r#"
            SOURCE_FILE@0..32
              MODULE_HEADER@0..8
                MODULE_KW@0..6 "module"
                WHITESPACE@6..7 " "
                NAME@7..8
                  UPPER_IDENT@7..8 "M"
              WHITESPACE@8..9 "\n"
              FN@9..21
                FN_KW@9..11 "fn"
                WHITESPACE@11..12 " "
                NAME@12..13
                  LOWER_IDENT@12..13 "f"
                PARAM_LIST@13..15
                  L_PAREN@13..14 "("
                  R_PAREN@14..15 ")"
                WHITESPACE@15..16 " "
                BLOCK_EXPR@16..21
                  L_CURLY@16..17 "{"
                  WHITESPACE@17..20 "\n  "
                  EXPR_STMT@20..21
                    LITERAL@20..21
                      INT_NUMBER@20..21 "1"
              WHITESPACE@21..22 "\n"
              FN@22..31
                FN_KW@22..24 "fn"
                WHITESPACE@24..25 " "
                NAME@25..26
                  LOWER_IDENT@25..26 "g"
                PARAM_LIST@26..28
                  L_PAREN@26..27 "("
                  R_PAREN@27..28 ")"
                WHITESPACE@28..29 " "
                BLOCK_EXPR@29..31
                  L_CURLY@29..30 "{"
                  R_CURLY@30..31 "}"
              WHITESPACE@31..32 "\n"
            error 22..24: expected `}`
        "#]],
        );
    }
}
