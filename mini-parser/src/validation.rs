//! Checks on the finished tree, for mistakes the parser lets through.
//!
//! The parser accepts a little more than the grammar, on purpose: `(a,
//! b)` parses like a parenthesized expression with two expressions in
//! it. Here, with the whole node in hand, the error message can say
//! what's wrong: Maxim has no tuples. rust-analyzer works the same way
//! (its `validation.rs`).

use crate::ast::{self, AstNode};
use crate::{SyntaxError, SyntaxNode};

/// The errors in the tree that the parser didn't report.
pub(crate) fn validate(root: &SyntaxNode) -> Vec<SyntaxError> {
    let mut errors = Vec::new();
    for node in root.descendants() {
        if let Some(paren) = ast::ParenExpr::cast(node) {
            validate_paren(&paren, &mut errors);
        }
    }
    errors
}

/// `(…)` holds exactly one expression, and not `_`.
fn validate_paren(paren: &ast::ParenExpr, errors: &mut Vec<SyntaxError>) {
    // Part 12a: report what's wrong with `()`, `(a, b)` and `(_)`.
}

#[cfg(test)]
mod tests {
    use crate::parse;

    fn errors(text: &str) -> Vec<String> {
        parse(text)
            .errors()
            .iter()
            .map(|error| error.to_string())
            .collect()
    }

    #[test]
    fn one_expression_in_parentheses_is_fine() {
        assert_eq!(
            errors("module M\nlet x = (a + b) * c"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn empty_parentheses() {
        assert_eq!(
            errors("module M\nlet x = ()"),
            [
                "error 17..19: Maxim has no `()` value: a function without parameters is `fn() { … }`"
            ]
        );
    }

    #[test]
    fn tuples() {
        assert_eq!(
            errors("module M\nlet x = (1, 2, 3)"),
            ["error 17..26: Maxim has no tuples: use a record type, or separate arguments"]
        );
    }

    #[test]
    fn a_hole_in_parentheses() {
        assert_eq!(
            errors("module M\nlet x = f((_))"),
            ["error 19..22: `_` stands for a missing argument: it can't be in parentheses"]
        );
    }

    #[test]
    fn nested_mistakes_are_all_found() {
        assert_eq!(errors("module M\nlet x = [(), (a, b)]").len(), 2);
    }
}
