//! The typed nodes, one per node kind, and the enums that group them.
//! rust-analyzer generates this file from a grammar of the tree's shape
//! (`rust.ungram`); here it's written by hand, with two macros.

use super::support::{child, children, token};
use super::{AstNode, ast_enum, ast_node};
use crate::SyntaxKind::*;
use crate::{SyntaxKind, SyntaxNode, SyntaxToken, T};

// Names -------------------------------------------------------------------

ast_node!(
    /// A name being defined.
    Name, NAME
);
ast_node!(
    /// A name being used.
    NameRef, NAME_REF
);

impl Name {
    pub fn text(&self) -> String {
        self.syntax.text()
    }
}

impl NameRef {
    pub fn text(&self) -> String {
        self.syntax.text()
    }
}

// The file and its declarations --------------------------------------------

ast_node!(SourceFile, SOURCE_FILE);
ast_node!(ModuleHeader, MODULE_HEADER);

impl SourceFile {
    pub fn module_header(&self) -> Option<ModuleHeader> {
        child(&self.syntax)
    }

    pub fn decls(&self) -> impl Iterator<Item = Decl> + '_ {
        children(&self.syntax)
    }
}

impl ModuleHeader {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }
}

ast_enum!(
    /// A declaration, at the top level of a file.
    Decl { Import, TypeDecl, Fn, Cast, Component, LetDecl }
);

ast_node!(Import, IMPORT);
ast_node!(TypeDecl, TYPE_DECL);
ast_node!(TypeParamList, TYPE_PARAM_LIST);
ast_node!(VariantList, VARIANT_LIST);
ast_node!(Variant, VARIANT);
ast_node!(TupleFieldList, TUPLE_FIELD_LIST);
ast_node!(RecordFieldList, RECORD_FIELD_LIST);
ast_node!(RecordField, RECORD_FIELD);
ast_node!(Fn, FN);
ast_node!(Cast, CAST);
ast_node!(Component, COMPONENT);
ast_node!(LetDecl, LET_DECL);
ast_node!(ParamList, PARAM_LIST);
ast_node!(Param, PARAM);
ast_node!(RetType, RET_TYPE);

impl Import {
    pub fn name_ref(&self) -> Option<NameRef> {
        child(&self.syntax)
    }
}

impl TypeDecl {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn type_param_list(&self) -> Option<TypeParamList> {
        child(&self.syntax)
    }

    pub fn variant_list(&self) -> Option<VariantList> {
        child(&self.syntax)
    }

    pub fn record_field_list(&self) -> Option<RecordFieldList> {
        child(&self.syntax)
    }
}

impl TypeParamList {
    pub fn names(&self) -> impl Iterator<Item = Name> + '_ {
        children(&self.syntax)
    }
}

impl VariantList {
    pub fn variants(&self) -> impl Iterator<Item = Variant> + '_ {
        children(&self.syntax)
    }
}

impl Variant {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn tuple_field_list(&self) -> Option<TupleFieldList> {
        child(&self.syntax)
    }
}

impl TupleFieldList {
    pub fn types(&self) -> impl Iterator<Item = Type> + '_ {
        children(&self.syntax)
    }
}

impl RecordFieldList {
    pub fn fields(&self) -> impl Iterator<Item = RecordField> + '_ {
        children(&self.syntax)
    }
}

impl RecordField {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

impl Fn {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.syntax)
    }

    pub fn ret_type(&self) -> Option<RetType> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl Cast {
    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.syntax)
    }

    pub fn ret_type(&self) -> Option<RetType> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl Component {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.syntax)
    }
}

impl LetDecl {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl ParamList {
    pub fn params(&self) -> impl Iterator<Item = Param> + '_ {
        children(&self.syntax)
    }
}

impl Param {
    /// `None` for `_`, or if the name is missing.
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

impl RetType {
    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

// Types --------------------------------------------------------------------

ast_enum!(
    /// A type: `Int`, `List<a>`, `(Int) -> Bool`.
    Type { NamedType, FnType }
);

ast_node!(NamedType, NAMED_TYPE);
ast_node!(TypeArgList, TYPE_ARG_LIST);
ast_node!(FnType, FN_TYPE);
ast_node!(ParamTypeList, PARAM_TYPE_LIST);
ast_node!(ParamType, PARAM_TYPE);

impl NamedType {
    pub fn name_ref(&self) -> Option<NameRef> {
        child(&self.syntax)
    }

    pub fn type_arg_list(&self) -> Option<TypeArgList> {
        child(&self.syntax)
    }
}

impl TypeArgList {
    pub fn types(&self) -> impl Iterator<Item = Type> + '_ {
        children(&self.syntax)
    }
}

impl FnType {
    pub fn param_type_list(&self) -> Option<ParamTypeList> {
        child(&self.syntax)
    }

    /// The result type, after `->`.
    pub fn ret(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

impl ParamTypeList {
    pub fn params(&self) -> impl Iterator<Item = ParamType> + '_ {
        children(&self.syntax)
    }
}

impl ParamType {
    /// The label in `(label: Int) -> Bool`.
    pub fn label(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }
}

// Patterns -----------------------------------------------------------------

ast_enum!(
    /// A pattern, on the left of a match arm.
    Pat { WildcardPat, IdentPat, LiteralPat, CtorPat, ListPat }
);

ast_node!(WildcardPat, WILDCARD_PAT);
ast_node!(IdentPat, IDENT_PAT);
ast_node!(LiteralPat, LITERAL_PAT);
ast_node!(CtorPat, CTOR_PAT);
ast_node!(ListPat, LIST_PAT);
ast_node!(RestPat, REST_PAT);

impl IdentPat {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }
}

impl LiteralPat {
    pub fn token(&self) -> Option<SyntaxToken> {
        literal_token(&self.syntax)
    }
}

impl CtorPat {
    /// `Geo` in `Geo.Circle(r)`.
    pub fn qualifier(&self) -> Option<NameRef> {
        todo!("Part 11a: qualifier")
    }

    /// `Circle` in `Geo.Circle(r)`.
    pub fn name_ref(&self) -> Option<NameRef> {
        children(&self.syntax).last()
    }

    pub fn args(&self) -> impl Iterator<Item = Pat> + '_ {
        children(&self.syntax)
    }
}

impl ListPat {
    pub fn pats(&self) -> impl Iterator<Item = Pat> + '_ {
        children(&self.syntax)
    }

    pub fn rest(&self) -> Option<RestPat> {
        child(&self.syntax)
    }
}

impl RestPat {
    /// `None` for `.._`.
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }
}

// Expressions --------------------------------------------------------------

ast_enum!(
    /// An expression.
    Expr {
        Literal,
        PathExpr,
        HoleExpr,
        ParenExpr,
        ListExpr,
        BlockExpr,
        LambdaExpr,
        IfExpr,
        MatchExpr,
        InterpolatedString,
        BinExpr,
        PrefixExpr,
        CallExpr,
        FieldExpr,
        TryExpr,
        CastExpr,
    }
);

ast_node!(Literal, LITERAL);
ast_node!(PathExpr, PATH_EXPR);
ast_node!(HoleExpr, HOLE_EXPR);
ast_node!(ParenExpr, PAREN_EXPR);
ast_node!(ListExpr, LIST_EXPR);
ast_node!(Spread, SPREAD);
ast_node!(BlockExpr, BLOCK_EXPR);
ast_node!(LetStmt, LET_STMT);
ast_node!(ExprStmt, EXPR_STMT);
ast_node!(LambdaExpr, LAMBDA_EXPR);
ast_node!(IfExpr, IF_EXPR);
ast_node!(MatchExpr, MATCH_EXPR);
ast_node!(MatchArmList, MATCH_ARM_LIST);
ast_node!(MatchArm, MATCH_ARM);
ast_node!(MatchGuard, MATCH_GUARD);
ast_node!(InterpolatedString, INTERPOLATED_STRING);
ast_node!(BinExpr, BIN_EXPR);
ast_node!(PrefixExpr, PREFIX_EXPR);
ast_node!(CallExpr, CALL_EXPR);
ast_node!(ArgList, ARG_LIST);
ast_node!(LabeledArg, LABELED_ARG);
ast_node!(FieldExpr, FIELD_EXPR);
ast_node!(TryExpr, TRY_EXPR);
ast_node!(CastExpr, CAST_EXPR);

ast_enum!(
    /// A statement, in a block.
    Stmt { LetStmt, ExprStmt }
);

ast_enum!(
    /// What follows `else`.
    ElseBranch { BlockExpr, IfExpr }
);

ast_enum!(
    /// A call argument: `x`, `label: x` or `..x`.
    Arg { Expr, LabeledArg, Spread }
);

impl Literal {
    pub fn token(&self) -> Option<SyntaxToken> {
        literal_token(&self.syntax)
    }
}

impl PathExpr {
    pub fn name_ref(&self) -> Option<NameRef> {
        child(&self.syntax)
    }
}

impl ParenExpr {
    /// The expressions inside: exactly one in a valid program.
    pub fn exprs(&self) -> impl Iterator<Item = Expr> + '_ {
        children(&self.syntax)
    }
}

impl ListExpr {
    pub fn elements(&self) -> impl Iterator<Item = Expr> + '_ {
        children(&self.syntax)
    }

    pub fn spread(&self) -> Option<Spread> {
        child(&self.syntax)
    }
}

impl Spread {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl BlockExpr {
    pub fn stmts(&self) -> impl Iterator<Item = Stmt> + '_ {
        children(&self.syntax)
    }
}

impl LetStmt {
    pub fn name(&self) -> Option<Name> {
        child(&self.syntax)
    }

    pub fn ty(&self) -> Option<Type> {
        child(&self.syntax)
    }

    pub fn value(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl ExprStmt {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl LambdaExpr {
    pub fn param_list(&self) -> Option<ParamList> {
        child(&self.syntax)
    }

    pub fn ret_type(&self) -> Option<RetType> {
        child(&self.syntax)
    }

    pub fn body(&self) -> Option<BlockExpr> {
        child(&self.syntax)
    }
}

impl IfExpr {
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn then_branch(&self) -> Option<BlockExpr> {
        todo!("Part 11a: then_branch")
    }

    pub fn else_branch(&self) -> Option<ElseBranch> {
        todo!("Part 11a: else_branch")
    }
}

impl MatchExpr {
    pub fn scrutinee(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn arm_list(&self) -> Option<MatchArmList> {
        child(&self.syntax)
    }
}

impl MatchArmList {
    pub fn arms(&self) -> impl Iterator<Item = MatchArm> + '_ {
        children(&self.syntax)
    }
}

impl MatchArm {
    pub fn pat(&self) -> Option<Pat> {
        child(&self.syntax)
    }

    pub fn guard(&self) -> Option<MatchGuard> {
        child(&self.syntax)
    }

    /// The expression after `->`.
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl MatchGuard {
    pub fn condition(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl InterpolatedString {
    /// The expressions in the holes.
    pub fn exprs(&self) -> impl Iterator<Item = Expr> + '_ {
        children(&self.syntax)
    }
}

impl BinExpr {
    pub fn lhs(&self) -> Option<Expr> {
        todo!("Part 11a: lhs")
    }

    pub fn rhs(&self) -> Option<Expr> {
        todo!("Part 11a: rhs")
    }

    /// The operator token: the first token that isn't trivia.
    pub fn op_token(&self) -> Option<SyntaxToken> {
        todo!("Part 11a: op_token")
    }

    pub fn op_kind(&self) -> Option<SyntaxKind> {
        self.op_token().map(|token| token.kind())
    }
}

impl PrefixExpr {
    pub fn op_token(&self) -> Option<SyntaxToken> {
        todo!("Part 11a: op_token")
    }

    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl CallExpr {
    pub fn callee(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn arg_list(&self) -> Option<ArgList> {
        child(&self.syntax)
    }
}

impl ArgList {
    pub fn args(&self) -> impl Iterator<Item = Arg> + '_ {
        children(&self.syntax)
    }
}

impl LabeledArg {
    pub fn label(&self) -> Option<NameRef> {
        child(&self.syntax)
    }

    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl FieldExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    pub fn name_ref(&self) -> Option<NameRef> {
        child(&self.syntax)
    }
}

impl TryExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }
}

impl CastExpr {
    pub fn expr(&self) -> Option<Expr> {
        child(&self.syntax)
    }

    /// The target type's name, after `::`.
    pub fn name_ref(&self) -> Option<NameRef> {
        child(&self.syntax)
    }
}

/// The literal token of a `LITERAL` or a `LITERAL_PAT`.
fn literal_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    first_token(node).filter(|token| matches!(token.kind(), INT_NUMBER | FLOAT_NUMBER | STRING))
}

/// The first child token that isn't trivia.
fn first_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens().find_map(|child| match child {
        crate::SyntaxElement::Token(token) if !token.kind().is_trivia() => Some(token),
        _ => None,
    })
}
