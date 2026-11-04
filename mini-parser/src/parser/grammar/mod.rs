//! The grammar: one function per rule of the spec, calling each other
//! the way the rules do. Recursive descent, as in rustc and
//! rust-analyzer, with a Pratt loop for binary operators.
//!
//! Each function starts at the first token of its rule and leaves the
//! parser after the last one. Where a rule is optional, the caller
//! checks the first token before calling.

mod atoms;
mod expressions;
mod items;
mod patterns;
mod types;

use super::{Parser, TokenSet};
use crate::SyntaxKind::{self, *};
use crate::T;

/// The tokens that can start a statement, a match arm or a declaration:
/// a line break before one of them can end what came before.
pub(crate) const LINE_START: TokenSet = expressions::EXPR_FIRST
    .union(patterns::PAT_FIRST)
    .union(items::DECL_FIRST);

/// `Module ::= "module" UPPER ( NEWLINE Decl )*`
pub(crate) fn source_file(p: &mut Parser<'_>) {
    let m = p.start();
    module_header(p);
    // After junk that `decl` skipped, a missing line break isn't news.
    let mut needs_newline = true;
    while !p.at(EOF) {
        // Only a declaration can be on the wrong line: anything else is
        // `decl`'s error to report.
        if needs_newline && !p.at_newline() && p.at_ts(items::DECL_FIRST) {
            p.error("expected a line break: each declaration starts on its own line");
        }
        needs_newline = items::decl(p);
    }
    m.complete(p, SOURCE_FILE);
}

/// `ModuleHeader ::= "module" UPPER`
fn module_header(p: &mut Parser<'_>) {
    if !p.at(T![module]) {
        p.error("a file starts with `module` and the module's name");
        return;
    }
    let m = p.start();
    p.bump(T![module]);
    name(p, UPPER_IDENT);
    m.complete(p, MODULE_HEADER);
}

/// A name being defined: a `NAME` node around a `kind` token.
fn name(p: &mut Parser<'_>, kind: SyntaxKind) {
    if !p.at(kind) {
        let found = p.current();
        if found.is_keyword() {
            p.error(format!(
                "expected {}, but {} is a keyword",
                kind.describe(),
                found.describe()
            ));
        } else {
            p.error(format!("expected {}", kind.describe()));
        }
        return;
    }
    let m = p.start();
    p.bump(kind);
    m.complete(p, NAME);
}

/// A name being used: a `NAME_REF` node around a `kind` token.
fn name_ref(p: &mut Parser<'_>, kind: SyntaxKind) {
    if !p.at(kind) {
        p.error(format!("expected {}", kind.describe()));
        return;
    }
    let m = p.start();
    p.bump(kind);
    m.complete(p, NAME_REF);
}

/// Where a list of parameters, arguments or elements goes on after a
/// broken item: the next item, or the end of the list.
const LIST_RECOVERY: TokenSet = TokenSet::new(&[T![,], T![')'], T![']'], T!['}'], T!['{']]);

/// `ParamList ::= "(" (Param ("," Param)*)? ")"`
fn param_list(p: &mut Parser<'_>) {
    todo!("Part 9e: param_list")
}

/// `Param ::= (LOWER | "_") (":" Type)?`
fn param(p: &mut Parser<'_>) {
    todo!("Part 9e: param")
}

/// `RetType ::= "->" Type`, if there is one.
fn opt_ret_type(p: &mut Parser<'_>) {
    todo!("Part 9e: opt_ret_type")
}

/// A block, which must be there: a function's body, an `if`'s branch…
fn block(p: &mut Parser<'_>) {
    if p.at(T!['{']) {
        not_on_new_line(p);
        atoms::block_expr(p);
    } else {
        p.error("expected a block `{ … }`");
    }
}

/// The current token, a `(` or a `{`, belongs to what's before it. If a
/// line break ends the statement before it, say so, and parse on as if it
/// didn't: `fn f()` then `{` on the next line.
fn not_on_new_line(p: &mut Parser<'_>) {
    if p.at_newline() {
        p.error(format!(
            "a line break here ends the statement: put this {} at the end of the line before",
            p.current().describe()
        ));
    }
}

/// Parse `text` with one grammar function, in a root node: the tests of
/// each grammar file use it.
#[cfg(test)]
fn parse_with<R>(
    entry: impl FnOnce(&mut Parser<'_>) -> R,
    text: &str,
) -> (crate::SyntaxNode, Vec<crate::SyntaxError>) {
    let lexed = crate::lexer::lex(text);
    let input = super::Input::from_lexed(&lexed);
    let mut p = Parser::new(&input);
    let root = p.start();
    entry(&mut p);
    if !p.at(EOF) {
        p.error("unexpected tokens at the end");
        while !p.at(EOF) {
            p.bump_any();
        }
    }
    root.complete(&mut p, SOURCE_FILE);
    let (green, mut errors) = crate::tree::build_tree(&lexed, p.events);
    // As `parse` does: one error per place.
    errors.sort_by_key(|error| error.range().start());
    errors.dedup_by_key(|error| error.range().start());
    (crate::SyntaxNode::new_root(green), errors)
}

/// Parse `text` with one grammar function, and dump the tree.
#[cfg(test)]
fn check<R>(entry: impl FnOnce(&mut Parser<'_>) -> R, text: &str, expect: expect_test::Expect) {
    let (root, errors) = parse_with(entry, text);
    let mut out = root.debug_dump();
    for error in errors {
        out += &format!("{error}\n");
    }
    expect.assert_eq(&out);
}

/// Parse `text` as an expression, and dump the tree.
#[cfg(test)]
fn check_expr(text: &str, expect: expect_test::Expect) {
    check(
        |p| {
            expressions::expr(p);
        },
        text,
        expect,
    );
}

/// Parse a whole file, and dump the tree.
#[cfg(test)]
fn check_file(text: &str, expect: expect_test::Expect) {
    expect.assert_eq(&crate::parse(text).debug_dump());
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;

    #[test]
    fn a_whole_file() {
        check_file(
            "module Main\n\nimport List\n",
            expect![[r#"
            SOURCE_FILE@0..25
              MODULE_HEADER@0..11
                MODULE_KW@0..6 "module"
                WHITESPACE@6..7 " "
                NAME@7..11
                  UPPER_IDENT@7..11 "Main"
              WHITESPACE@11..13 "\n\n"
              IMPORT@13..24
                IMPORT_KW@13..19 "import"
                WHITESPACE@19..20 " "
                NAME_REF@20..24
                  UPPER_IDENT@20..24 "List"
              WHITESPACE@24..25 "\n"
        "#]],
        );
    }

    #[test]
    fn a_file_without_its_header() {
        check_file(
            "import List",
            expect![[r#"
            SOURCE_FILE@0..11
              IMPORT@0..11
                IMPORT_KW@0..6 "import"
                WHITESPACE@6..7 " "
                NAME_REF@7..11
                  UPPER_IDENT@7..11 "List"
            error 0..6: a file starts with `module` and the module's name
        "#]],
        );
    }

    #[test]
    fn param_lists() {
        check(
            param_list,
            "(x, _: Int, items: List<a>)",
            expect![[r#"
            SOURCE_FILE@0..27
              PARAM_LIST@0..27
                L_PAREN@0..1 "("
                PARAM@1..2
                  NAME@1..2
                    LOWER_IDENT@1..2 "x"
                COMMA@2..3 ","
                WHITESPACE@3..4 " "
                PARAM@4..10
                  UNDERSCORE@4..5 "_"
                  COLON@5..6 ":"
                  WHITESPACE@6..7 " "
                  NAMED_TYPE@7..10
                    NAME_REF@7..10
                      UPPER_IDENT@7..10 "Int"
                COMMA@10..11 ","
                WHITESPACE@11..12 " "
                PARAM@12..26
                  NAME@12..17
                    LOWER_IDENT@12..17 "items"
                  COLON@17..18 ":"
                  WHITESPACE@18..19 " "
                  NAMED_TYPE@19..26
                    NAME_REF@19..23
                      UPPER_IDENT@19..23 "List"
                    TYPE_ARG_LIST@23..26
                      L_ANGLE@23..24 "<"
                      NAMED_TYPE@24..25
                        NAME_REF@24..25
                          LOWER_IDENT@24..25 "a"
                      R_ANGLE@25..26 ">"
                R_PAREN@26..27 ")"
        "#]],
        );
        check(
            param_list,
            "()",
            expect![[r#"
            SOURCE_FILE@0..2
              PARAM_LIST@0..2
                L_PAREN@0..1 "("
                R_PAREN@1..2 ")"
        "#]],
        );
    }

    #[test]
    fn broken_param_lists() {
        check(
            param_list,
            "(x,)",
            expect![[r#"
            SOURCE_FILE@0..4
              PARAM_LIST@0..4
                L_PAREN@0..1 "("
                PARAM@1..2
                  NAME@1..2
                    LOWER_IDENT@1..2 "x"
                COMMA@2..3 ","
                R_PAREN@3..4 ")"
            error 3..4: expected a parameter
        "#]],
        );
        check(
            param_list,
            "(x y)",
            expect![[r#"
            SOURCE_FILE@0..5
              PARAM_LIST@0..2
                L_PAREN@0..1 "("
                PARAM@1..2
                  NAME@1..2
                    LOWER_IDENT@1..2 "x"
              WHITESPACE@2..3 " "
              LOWER_IDENT@3..4 "y"
              R_PAREN@4..5 ")"
            error 3..4: expected `)`
        "#]],
        );
        check(
            param_list,
            "x",
            expect![[r#"
            SOURCE_FILE@0..1
              LOWER_IDENT@0..1 "x"
            error 0..1: expected `(` and the parameters
        "#]],
        );
    }

    #[test]
    fn names_that_are_keywords() {
        check(
            |p| name(p, LOWER_IDENT),
            "type",
            expect![[r#"
            SOURCE_FILE@0..4
              TYPE_KW@0..4 "type"
            error 0..4: expected a lowercase name, but `type` is a keyword
        "#]],
        );
    }
}
