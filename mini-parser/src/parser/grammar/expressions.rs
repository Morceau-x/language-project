//! Expressions: operators by Pratt parsing, postfix operators, and the
//! arrow-lambda mistake. The atoms are in `atoms.rs`.

use super::*;
use crate::parser::CompletedMarker;

/// The tokens that start an expression.
pub(super) const EXPR_FIRST: TokenSet = TokenSet::new(&[
    LOWER_IDENT,
    UPPER_IDENT,
    INT_NUMBER,
    FLOAT_NUMBER,
    STRING,
    STRING_START,
    T![_],
    T!['('],
    T!['['],
    T!['{'],
    T![fn],
    T![if],
    T![match],
    T![-],
    T![!],
]);

/// `Expr`: a whole expression.
pub(super) fn expr(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    let lhs = expr_bp(p, 0);
    if p.at(T![=>]) {
        return Some(arrow_lambda(p, lhs));
    }
    lhs
}

/// `(x) => x + 1`: not Maxim, but common enough to deserve its own
/// message. Parse it anyway, in an `ERROR` node, so that the rest of the
/// file still parses. `lhs` is what came before the `=>`.
fn arrow_lambda(p: &mut Parser<'_>, lhs: Option<CompletedMarker>) -> CompletedMarker {
    todo!("Part 12b: arrow_lambda")
}

/// How tightly a binary operator holds its left and right operands: the
/// precedence table of the spec, as numbers. Higher binds tighter. The
/// right number is higher for left-associative operators (`a - b - c`
/// is `(a - b) - c`), lower for right-associative ones.
fn infix_binding_power(kind: SyntaxKind) -> Option<(u8, u8)> {
    todo!("Part 9a: infix_binding_power")
}

/// How tightly `-` and `!` hold their operand: tighter than `*`, looser
/// than `**`, so `-x ** 2` is `-(x ** 2)`.
const PREFIX_BINDING_POWER: u8 = 17;

/// Comparisons don't chain: `a < b < c` is an error.
fn is_comparison(kind: SyntaxKind) -> bool {
    matches!(kind, T![==] | T![!=] | T![<] | T![>] | T![<=] | T![>=])
}

/// An expression whose binary operators all bind at least as tightly as
/// `min_bp`.
fn expr_bp(p: &mut Parser<'_>, min_bp: u8) -> Option<CompletedMarker> {
    todo!("Part 9a: expr_bp")
}

/// The left side of a binary operator: a prefix operator and its
/// operand, or an atom and its postfix operators.
fn lhs(p: &mut Parser<'_>) -> Option<CompletedMarker> {
    todo!("Part 9a: lhs")
}

/// The postfix operators after an atom, which bind tightest of all:
/// `.field`, `(args)`, `?` and `:: Type`.
fn postfix(p: &mut Parser<'_>, mut lhs: CompletedMarker) -> CompletedMarker {
    // Part 9b: parse the postfix operators after `lhs`. Until then,
    // there are none.
    lhs
}

/// `ArgList ::= "(" (Arg ("," Arg)*)? ")"`
fn arg_list(p: &mut Parser<'_>) {
    todo!("Part 9b: arg_list")
}

/// `Arg ::= Expr | LOWER ":" Expr | ".." Expr`. Two tokens of lookahead
/// tell a label from a variable.
fn arg(p: &mut Parser<'_>) {
    todo!("Part 9b: arg")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyntaxNode;
    use expect_test::expect;

    /// Operators in parentheses, everything else as written:
    /// `a + b * c` is `(a + (b * c))`.
    fn parenthesize(node: &SyntaxNode) -> String {
        let parts: Vec<String> = node
            .children_with_tokens()
            .filter(|child| !child.kind().is_trivia())
            .map(|child| match child {
                crate::SyntaxElement::Node(node) => parenthesize(&node),
                crate::SyntaxElement::Token(token) => token.text().to_string(),
            })
            .collect();
        match node.kind() {
            BIN_EXPR => format!("({})", parts.join(" ")),
            PREFIX_EXPR => format!("({})", parts.concat()),
            _ => node.text().trim().to_string(),
        }
    }

    /// Parse `text` as an expression; it must be valid, and parenthesize
    /// as `expected`.
    #[track_caller]
    fn check_parens(text: &str, expected: &str) {
        let (root, errors) = parse_with(
            |p| {
                expr(p);
            },
            text,
        );
        assert_eq!(errors, [], "{text}");
        let value = root.children().next().unwrap();
        assert_eq!(parenthesize(&value), expected, "{text}");
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        check_parens("a + b * c", "(a + (b * c))");
        check_parens("a * b + c", "((a * b) + c)");
        check_parens("a % b ~/ c * d / e", "((((a % b) ~/ c) * d) / e)");
    }

    #[test]
    fn most_operators_are_left_associative() {
        check_parens("a - b - c", "((a - b) - c)");
        check_parens("a |> f |> g", "((a |> f) |> g)");
        check_parens("a && b && c", "((a && b) && c)");
    }

    #[test]
    fn power_and_concatenation_are_right_associative() {
        check_parens("a ** b ** c", "(a ** (b ** c))");
        check_parens("a ++ b ++ c", "(a ++ (b ++ c))");
    }

    #[test]
    fn the_precedence_ladder() {
        check_parens(
            "a |> b || c && d == e < f ++ g + h * i ** j",
            "(a |> (b || (c && (d == (e < (f ++ (g + (h * (i ** j)))))))))",
        );
        check_parens(
            "a ** b * c + d ++ e < f == g && h || i |> j",
            "(((((((((a ** b) * c) + d) ++ e) < f) == g) && h) || i) |> j)",
        );
    }

    #[test]
    fn prefix_operators() {
        check_parens("-a * b", "((-a) * b)");
        check_parens("!a && b", "((!a) && b)");
        check_parens("a - -b", "(a - (-b))");
        check_parens("--a", "(-(-a))");
    }

    #[test]
    fn power_binds_tighter_than_prefix_minus() {
        check_parens("-x ** 2", "(-(x ** 2))");
        check_parens("x ** -y", "(x ** (-y))");
        check_parens("x ** -y ** z", "(x ** (-(y ** z)))");
        check_parens("x ** -y * z", "((x ** (-y)) * z)");
    }

    #[test]
    fn comparisons_dont_chain() {
        check_parens("a == b < c", "(a == (b < c))");
        check_parens("a < b == c", "((a < b) == c)");
        check_expr(
            "a < b > c",
            expect![[r#"
                SOURCE_FILE@0..9
                  BIN_EXPR@0..9
                    BIN_EXPR@0..5
                      PATH_EXPR@0..1
                        NAME_REF@0..1
                          LOWER_IDENT@0..1 "a"
                      WHITESPACE@1..2 " "
                      L_ANGLE@2..3 "<"
                      WHITESPACE@3..4 " "
                      PATH_EXPR@4..5
                        NAME_REF@4..5
                          LOWER_IDENT@4..5 "b"
                    WHITESPACE@5..6 " "
                    R_ANGLE@6..7 ">"
                    WHITESPACE@7..8 " "
                    PATH_EXPR@8..9
                      NAME_REF@8..9
                        LOWER_IDENT@8..9 "c"
                error 6..7: comparisons can't be chained: write `a < b && b < c`
            "#]],
        );
        check_expr(
            "a == b != c",
            expect![[r#"
                SOURCE_FILE@0..11
                  BIN_EXPR@0..11
                    BIN_EXPR@0..6
                      PATH_EXPR@0..1
                        NAME_REF@0..1
                          LOWER_IDENT@0..1 "a"
                      WHITESPACE@1..2 " "
                      EQ2@2..4 "=="
                      WHITESPACE@4..5 " "
                      PATH_EXPR@5..6
                        NAME_REF@5..6
                          LOWER_IDENT@5..6 "b"
                    WHITESPACE@6..7 " "
                    NEQ@7..9 "!="
                    WHITESPACE@9..10 " "
                    PATH_EXPR@10..11
                      NAME_REF@10..11
                        LOWER_IDENT@10..11 "c"
                error 7..9: comparisons can't be chained: write `a < b && b < c`
            "#]],
        );
    }

    #[test]
    fn postfix_operators_bind_tightest() {
        check_parens("-x.y", "(-x.y)");
        check_parens("a + f(b)? :: T", "(a + f(b)? :: T)");
        check_expr(
            "x.f(1)?.G :: T",
            expect![[r#"
                SOURCE_FILE@0..14
                  CAST_EXPR@0..14
                    FIELD_EXPR@0..9
                      TRY_EXPR@0..7
                        CALL_EXPR@0..6
                          FIELD_EXPR@0..3
                            PATH_EXPR@0..1
                              NAME_REF@0..1
                                LOWER_IDENT@0..1 "x"
                            DOT@1..2 "."
                            NAME_REF@2..3
                              LOWER_IDENT@2..3 "f"
                          ARG_LIST@3..6
                            L_PAREN@3..4 "("
                            LITERAL@4..5
                              INT_NUMBER@4..5 "1"
                            R_PAREN@5..6 ")"
                        QUESTION@6..7 "?"
                      DOT@7..8 "."
                      NAME_REF@8..9
                        UPPER_IDENT@8..9 "G"
                    WHITESPACE@9..10 " "
                    COLON2@10..12 "::"
                    WHITESPACE@12..13 " "
                    NAME_REF@13..14
                      UPPER_IDENT@13..14 "T"
            "#]],
        );
    }

    #[test]
    fn lambdas_are_atoms() {
        check_parens("fn(x) { x } |> g", "(fn(x) { x } |> g)");
        check_parens("-fn(x) { x }(1)", "(-fn(x) { x }(1))");
    }

    #[test]
    fn call_arguments() {
        check_expr(
            "f(x, label: y, ..z)",
            expect![[r#"
                SOURCE_FILE@0..19
                  CALL_EXPR@0..19
                    PATH_EXPR@0..1
                      NAME_REF@0..1
                        LOWER_IDENT@0..1 "f"
                    ARG_LIST@1..19
                      L_PAREN@1..2 "("
                      PATH_EXPR@2..3
                        NAME_REF@2..3
                          LOWER_IDENT@2..3 "x"
                      COMMA@3..4 ","
                      WHITESPACE@4..5 " "
                      LABELED_ARG@5..13
                        NAME_REF@5..10
                          LOWER_IDENT@5..10 "label"
                        COLON@10..11 ":"
                        WHITESPACE@11..12 " "
                        PATH_EXPR@12..13
                          NAME_REF@12..13
                            LOWER_IDENT@12..13 "y"
                      COMMA@13..14 ","
                      WHITESPACE@14..15 " "
                      SPREAD@15..18
                        DOT2@15..17 ".."
                        PATH_EXPR@17..18
                          NAME_REF@17..18
                            LOWER_IDENT@17..18 "z"
                      R_PAREN@18..19 ")"
            "#]],
        );
    }

    #[test]
    fn a_paren_on_the_next_line_is_not_a_call() {
        check_expr(
            "f\n(x)",
            expect![[r#"
                SOURCE_FILE@0..5
                  PATH_EXPR@0..1
                    NAME_REF@0..1
                      LOWER_IDENT@0..1 "f"
                  WHITESPACE@1..2 "\n"
                  L_PAREN@2..3 "("
                  LOWER_IDENT@3..4 "x"
                  R_PAREN@4..5 ")"
                error 2..3: unexpected tokens at the end
            "#]],
        );
    }

    #[test]
    fn line_breaks_continue_after_operators_and_before_pipelines() {
        check_parens("a +\n  b", "(a + b)");
        check_parens("a\n  |> f\n  |> g", "((a |> f) |> g)");
        check_parens("f(\n  a\n  - b\n)", "f(\n  a\n  - b\n)");
    }

    #[test]
    fn a_line_break_before_a_minus_ends_the_expression() {
        check_expr(
            "a\n- b",
            expect![[r#"
            SOURCE_FILE@0..5
              PATH_EXPR@0..1
                NAME_REF@0..1
                  LOWER_IDENT@0..1 "a"
              WHITESPACE@1..2 "\n"
              MINUS@2..3 "-"
              WHITESPACE@3..4 " "
              LOWER_IDENT@4..5 "b"
            error 2..3: unexpected tokens at the end
        "#]],
        );
    }

    #[test]
    fn missing_operands() {
        check_expr(
            "1 +",
            expect![[r#"
                SOURCE_FILE@0..3
                  BIN_EXPR@0..3
                    LITERAL@0..1
                      INT_NUMBER@0..1 "1"
                    WHITESPACE@1..2 " "
                    PLUS@2..3 "+"
                error 3..3: expected an expression
            "#]],
        );
        check_expr(
            "x. :: ",
            expect![[r#"
                SOURCE_FILE@0..6
                  CAST_EXPR@0..5
                    FIELD_EXPR@0..2
                      PATH_EXPR@0..1
                        NAME_REF@0..1
                          LOWER_IDENT@0..1 "x"
                      DOT@1..2 "."
                    WHITESPACE@2..3 " "
                    COLON2@3..5 "::"
                  WHITESPACE@5..6 " "
                error 3..5: expected a name after `.`
                error 6..6: expected a capitalized name
            "#]],
        );
    }

    #[test]
    fn arrow_lambdas_get_their_own_error() {
        check_expr(
            "(x) => x + 1",
            expect![[r#"
                SOURCE_FILE@0..12
                  ERROR@0..12
                    PAREN_EXPR@0..3
                      L_PAREN@0..1 "("
                      PATH_EXPR@1..2
                        NAME_REF@1..2
                          LOWER_IDENT@1..2 "x"
                      R_PAREN@2..3 ")"
                    WHITESPACE@3..4 " "
                    FAT_ARROW@4..6 "=>"
                    WHITESPACE@6..7 " "
                    BIN_EXPR@7..12
                      PATH_EXPR@7..8
                        NAME_REF@7..8
                          LOWER_IDENT@7..8 "x"
                      WHITESPACE@8..9 " "
                      PLUS@9..10 "+"
                      WHITESPACE@10..11 " "
                      LITERAL@11..12
                        INT_NUMBER@11..12 "1"
                error 4..6: lambdas are written `fn(x) { … }` in Maxim
            "#]],
        );
    }
}
