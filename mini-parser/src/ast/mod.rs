//! A typed view of the syntax tree, as in rust-analyzer's `ast` module.
//!
//! The syntax tree is untyped: every node is a `SyntaxNode`, and its
//! kind says what it is. Code that uses the tree wants types instead:
//! a `Fn` with a `name()` and a `body()`, an `Expr` that is one of the
//! expression kinds. Each type here wraps a `SyntaxNode` of the right
//! kind and finds its parts in its children. Nothing is copied: it's
//! the same tree, seen through types.
//!
//! Every accessor returns an `Option` or an iterator: the tree may come
//! from a broken program, and any part may be missing.

mod nodes;

pub use nodes::*;

use crate::{SyntaxKind, SyntaxNode, SyntaxToken};

/// A typed view of a syntax node.
pub trait AstNode {
    /// Can a node of this kind be seen as `Self`?
    fn can_cast(kind: SyntaxKind) -> bool
    where
        Self: Sized;

    /// See `syntax` as `Self`, if its kind allows it.
    fn cast(syntax: SyntaxNode) -> Option<Self>
    where
        Self: Sized;

    /// The node underneath.
    fn syntax(&self) -> &SyntaxNode;
}

/// Find the parts of a node, for the accessors.
mod support {
    use super::*;

    /// The first child that is an `N`.
    pub(super) fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
        parent.children().find_map(N::cast)
    }

    /// All the children that are `N`s.
    pub(super) fn children<N: AstNode>(parent: &SyntaxNode) -> impl Iterator<Item = N> + use<N> {
        parent.children().filter_map(N::cast)
    }

    /// The first child token of this kind.
    pub(super) fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
        parent.children_with_tokens().find_map(|child| match child {
            crate::SyntaxElement::Token(token) if token.kind() == kind => Some(token),
            _ => None,
        })
    }
}

/// A struct for the nodes of one kind: `ast_node!(Fn, FN)`.
macro_rules! ast_node {
    ($(#[$meta:meta])* $name:ident, $kind:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name {
            syntax: SyntaxNode,
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                kind == SyntaxKind::$kind
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                Self::can_cast(syntax.kind()).then_some($name { syntax })
            }

            fn syntax(&self) -> &SyntaxNode {
                &self.syntax
            }
        }
    };
}

/// An enum for nodes of several kinds, one variant per type:
/// `ast_enum!(Type { NamedType, FnType })`.
macro_rules! ast_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant($variant),)*
        }

        impl AstNode for $name {
            fn can_cast(kind: SyntaxKind) -> bool {
                $($variant::can_cast(kind))||*
            }

            fn cast(syntax: SyntaxNode) -> Option<Self> {
                $(
                    if $variant::can_cast(syntax.kind()) {
                        return $variant::cast(syntax).map($name::$variant);
                    }
                )*
                None
            }

            fn syntax(&self) -> &SyntaxNode {
                match self {
                    $($name::$variant(it) => it.syntax(),)*
                }
            }
        }
    };
}

use {ast_enum, ast_node};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    /// The first node of type `N` in a parsed file.
    fn find<N: AstNode>(text: &str) -> N {
        parse(text)
            .syntax_node()
            .descendants()
            .find_map(N::cast)
            .unwrap_or_else(|| panic!("no such node in {text:?}"))
    }

    #[test]
    fn cast_checks_the_kind() {
        let root = parse("module M").syntax_node();
        assert!(SourceFile::cast(root.clone()).is_some());
        assert!(Fn::cast(root.clone()).is_none());
        assert!(Decl::cast(root).is_none());
    }

    #[test]
    fn enums_cast_to_the_right_variant() {
        let decls: Vec<Decl> = parse("module M\nimport A\nfn f() {}\nlet x = 1")
            .tree()
            .decls()
            .collect();
        assert!(matches!(
            decls.as_slice(),
            [Decl::Import(_), Decl::Fn(_), Decl::LetDecl(_)]
        ));
    }

    #[test]
    fn a_function() {
        let f: Fn = find("module M\nfn area(s: Shape, _) -> Float { s }");
        assert_eq!(f.name().unwrap().text(), "area");
        let params: Vec<Param> = f.param_list().unwrap().params().collect();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name().unwrap().text(), "s");
        assert_eq!(params[0].ty().unwrap().syntax().text(), "Shape");
        assert!(params[1].name().is_none());
        assert_eq!(f.ret_type().unwrap().ty().unwrap().syntax().text(), "Float");
        assert_eq!(f.body().unwrap().stmts().count(), 1);
    }

    #[test]
    fn missing_parts_are_none() {
        let f: Fn = find("module M\nfn (");
        assert!(f.name().is_none());
        assert!(f.ret_type().is_none());
        assert!(f.body().is_none());
    }

    #[test]
    fn binary_expressions() {
        let e: BinExpr = find("module M\nlet x = a + b * c");
        assert_eq!(e.lhs().unwrap().syntax().text(), "a");
        assert_eq!(e.op_kind(), Some(crate::T![+]));
        assert_eq!(e.op_token().unwrap().text(), "+");
        let Some(Expr::BinExpr(rhs)) = e.rhs() else {
            panic!("the right side is `b * c`");
        };
        assert_eq!(rhs.op_kind(), Some(crate::T![*]));
    }

    #[test]
    fn if_branches() {
        let e: IfExpr = find("module M\nlet x = if { a } { b } else if c { d } else { e }");
        assert_eq!(e.condition().unwrap().syntax().text(), "{ a }");
        assert_eq!(e.then_branch().unwrap().syntax().text(), "{ b }");
        let Some(ElseBranch::IfExpr(inner)) = e.else_branch() else {
            panic!("the else branch is another if");
        };
        let Some(ElseBranch::BlockExpr(last)) = inner.else_branch() else {
            panic!("the last else branch is a block");
        };
        assert_eq!(last.syntax().text(), "{ e }");
    }

    #[test]
    fn constructor_patterns() {
        let p: CtorPat = find("module M\nlet x = match s { Geo.Circle(r, _) -> r }");
        assert_eq!(p.qualifier().unwrap().text(), "Geo");
        assert_eq!(p.name_ref().unwrap().text(), "Circle");
        assert_eq!(p.args().count(), 2);
        let p: CtorPat = find("module M\nlet x = match s { None -> 0 }");
        assert!(p.qualifier().is_none());
        assert_eq!(p.name_ref().unwrap().text(), "None");
    }

    #[test]
    fn call_arguments() {
        let call: CallExpr = find("module M\nlet x = f(a, label: b, ..c)");
        assert_eq!(call.callee().unwrap().syntax().text(), "f");
        let args: Vec<Arg> = call.arg_list().unwrap().args().collect();
        assert!(matches!(
            args.as_slice(),
            [Arg::Expr(_), Arg::LabeledArg(_), Arg::Spread(_)]
        ));
        let Arg::LabeledArg(labeled) = &args[1] else {
            unreachable!()
        };
        assert_eq!(labeled.label().unwrap().text(), "label");
        assert_eq!(labeled.expr().unwrap().syntax().text(), "b");
    }
}
