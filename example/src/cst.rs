//! The lossless concrete syntax tree: every token of the source —
//! delimiters, keywords, commas — is retained, each carrying its trivia.
//! The formatter consumes this tree; `cst::print` reproduces the source
//! exactly (`print_module . parse_module == id`, golden-tested).
//!
//! Design follows PureScript's typed CST (an LALR grammar over
//! trivia-carrying tokens), not an untyped green tree.
//!
//! Direct `CstExpr`-in-node fields are boxed to keep the enum sized;
//! recursion behind `Vec` needs no box.

pub mod lower;
pub mod print;

use crate::lexer::token::Token;

/// A value between two delimiter tokens: `( … )`, `{ … }`, `[ … ]`, `< … >`.
#[derive(Clone, Debug)]
pub struct Wrapped<T> {
    pub open: Token,
    pub value: T,
    pub close: Token,
}

/// One-or-more items with their separator tokens (commas or bars).
#[derive(Clone, Debug)]
pub struct Separated<T> {
    pub head: T,
    pub tail: Vec<(Token, T)>,
}

impl<T> Separated<T> {
    pub fn single(head: T) -> Separated<T> {
        Separated {
            head,
            tail: Vec::new(),
        }
    }

    pub fn snoc(mut self, tok: Token, x: T) -> Separated<T> {
        self.tail.push((tok, x));
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.head).chain(self.tail.iter().map(|(_, x)| x))
    }

    pub fn into_iter_items(self) -> impl Iterator<Item = T> {
        std::iter::once(self.head).chain(self.tail.into_iter().map(|(_, x)| x))
    }

    pub fn len(&self) -> usize {
        1 + self.tail.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
pub struct CstModule {
    pub module_kw: Token,
    pub name: Token,
    pub decls: Vec<CstDecl>,
    pub eof: Token,
}

pub type ParamParens = Wrapped<Option<Separated<CstParam>>>;

#[derive(Clone, Debug)]
pub enum CstDecl {
    /// `import Geometry`
    Import(Token, Token),
    /// `type Name<a, b> = rhs`
    Type(Token, Token, Option<CstTyParams>, Token, CstTypeRhs),
    /// `fn name(params) -> Ret { … }`
    Fn(
        Token,
        Token,
        ParamParens,
        Option<(Token, CstType)>,
        CstBlock,
    ),
    /// `let name: T = expr`
    Let(Token, Token, Option<(Token, CstType)>, Token, CstExpr),
    /// `cast (d: Float) -> Celsius { … }` — a conversion declaration:
    /// the signature is the identity, so param and return annotations
    /// are validated as mandatory during lowering.
    Cast(Token, ParamParens, Option<(Token, CstType)>, CstBlock),
    /// `component Counter(init: init, update: update, view: view)` — a
    /// manifest wiring ordinary functions. Syntactically it reuses the
    /// parameter list (each "annotation" is a lowercase type name that is
    /// really a value reference); lowering validates the labels and shapes.
    Component(Token, Token, ParamParens),
    /// A declaration segment that failed to parse (error recovery): its
    /// tokens, verbatim. Its one diagnostic is reported by the recovering
    /// parser; lowering drops the node, the exact printer and the
    /// formatter pass it through.
    Error(Vec<Token>),
}

/// `<a, b>` on a type declaration.
#[derive(Clone, Debug)]
pub struct CstTyParams(pub Token, pub Separated<Token>, pub Token);

/// A function parameter: name (or `_`) with optional annotation.
#[derive(Clone, Debug)]
pub struct CstParam(pub Token, pub Option<(Token, CstType)>);

#[derive(Clone, Debug)]
pub enum CstTypeRhs {
    /// Optional leading `|`, then bar-separated variants.
    Variants(Option<Token>, Separated<CstVariant>),
    /// `{ name: T, … }` — a nominal record.
    Record(Wrapped<Separated<CstField>>),
}

/// `Circle(Float)` or bare `None`.
#[derive(Clone, Debug)]
pub struct CstVariant(pub Token, pub Option<Wrapped<Separated<CstType>>>);

/// `name: Type` inside a record declaration.
#[derive(Clone, Debug)]
pub struct CstField(pub Token, pub Token, pub CstType);

#[derive(Clone, Debug)]
pub enum CstType {
    /// `Shape`, `Option<a>`, or a type variable `a` / `number`.
    Name(Token, Option<Box<CstTyArgs>>),
    /// `(T1, T2) -> T` — the only use of parens in v0 types. Elements
    /// may carry a call label: `(x: Int, y: Int) -> Int`.
    Fun(
        Box<Wrapped<Option<Separated<CstLabType>>>>,
        Token,
        Box<CstType>,
    ),
}

/// One element of a function type's parameter list: an optional
/// `label ':'` prefix, then the type.
#[derive(Clone, Debug)]
pub struct CstLabType(pub Option<(Token, Token)>, pub CstType);

#[derive(Clone, Debug)]
pub struct CstTyArgs(pub Token, pub Separated<CstType>, pub Token);

#[derive(Clone, Debug)]
pub enum CstPat {
    Wild(Token),
    Var(Token),
    Lit(Token),
    /// Optionally qualified constructor with optional arguments:
    /// `Circle(r)`, `Geometry.Circle(r)`, `None`.
    Ctor(
        Option<(Token, Token)>,
        Token,
        Option<Box<Wrapped<Separated<CstPat>>>>,
    ),
    /// `[]`, `[p, q]`, `[p, ..rest]`.
    List(Wrapped<Option<Box<CstListPat>>>),
}

#[derive(Clone, Debug)]
pub struct CstListPat {
    pub elems: Separated<CstPat>,
    /// `, ..binder` — comma, dotdot, lower/hole.
    pub rest: Option<(Token, Token, Token)>,
}

#[derive(Clone, Debug)]
pub enum CstExpr {
    Var(Token),
    Con(Token),
    Lit(Token),
    /// A lone `_`. Only legal as a lambda parameter; rejected elsewhere
    /// during lowering with a targeted message.
    Hole(Token),
    /// `expr.field` or `Module.member` — the renamer decides which.
    Dot(Box<CstExpr>, Token, Token),
    /// `callee(args)`; the open paren is always same-line (`LParen`).
    Call(Box<CstExpr>, Wrapped<Option<Separated<CstArg>>>),
    Bin(Box<CstExpr>, Token, Box<CstExpr>),
    Unary(Token, Box<CstExpr>),
    /// `(e)` — validated single-element cover parens.
    Group(Wrapped<Box<CstExpr>>),
    /// `fn(x: Int) -> Int { body }` — a lambda: declaration params
    /// (annotations optional), optional return annotation, block body.
    FnLam(Token, ParamParens, Option<(Token, CstType)>, CstBlock),
    If(Token, Box<CstExpr>, CstBlock, Token, Box<CstElse>),
    Match(Token, Box<CstExpr>, Token, Vec<CstArm>, Token),
    List(Wrapped<Option<Box<CstListExpr>>>),
    Block(CstBlock),
    /// `expr?` — unwrap a Result/Option or early-return its failure.
    Try(Box<CstExpr>, Token),
    /// `expr::Type` — a cast, dispatching on the conversion convention.
    Cast(Box<CstExpr>, Token, Token),
    /// `"chunk${expr}chunk"` — the begin token, then each hole's
    /// expression with the mid/end chunk that closes it.
    Interp(Token, Vec<(CstExpr, Token)>),
    /// A JSX element.
    Jsx(CstJsx),
}

/// `<tag attrs/>` or `<tag attrs>children</tag>`. The tag-start and
/// closing tokens carry the tag names; the lowering checks they match.
#[derive(Clone, Debug)]
pub enum CstJsx {
    SelfClose(Token, Vec<CstJsxAttr>, Token),
    Element(Token, Vec<CstJsxAttr>, Token, Vec<CstJsxChild>, Token),
}

#[derive(Clone, Debug)]
pub enum CstJsxAttr {
    /// `disabled` — HTML boolean-attribute shorthand.
    Bare(Token),
    /// `name=value`: the name, the `=`, the value.
    Value(Token, Token, CstJsxAttrValue),
}

#[derive(Clone, Debug)]
pub enum CstJsxAttrValue {
    /// `name="literal"`.
    Str(Token),
    /// `name="a${x}b"` — an interpolated literal, begin + interp tail.
    Interp(Token, Vec<(CstExpr, Token)>),
    /// `name={expr}`.
    Expr(Wrapped<Box<CstExpr>>),
}

#[derive(Clone, Debug)]
pub enum CstJsxChild {
    /// A run of element text.
    Text(Token),
    /// A nested element.
    Elem(CstJsx),
    /// `{expr}` — one child.
    Splice(Wrapped<Box<CstExpr>>),
    /// `{..expr}` — a list of children, spread in place.
    Spread(Token, Token, Box<CstExpr>, Token),
    /// `${expr}` — a String hole, wrapped as text.
    Hole(Token, Box<CstExpr>, Token),
}

#[derive(Clone, Debug)]
pub struct CstListExpr {
    pub elems: Separated<CstExpr>,
    /// `, ..tailExpr`.
    pub spread: Option<(Token, Token, CstExpr)>,
}

#[derive(Clone, Debug)]
pub enum CstArg {
    Pos(Box<CstExpr>),
    /// `label: expr` — record construction via labeled constructor call.
    Lab(Token, Token, Box<CstExpr>),
    /// `..base` — record update, first argument of a constructor call.
    Spread(Token, Box<CstExpr>),
}

/// `pattern -> body` or `pattern if guard -> body`; arms are
/// juxtaposed, no separators.
#[derive(Clone, Debug)]
pub struct CstArm(
    pub CstPat,
    pub Option<(Token, CstExpr)>,
    pub Token,
    pub CstExpr,
);

#[derive(Clone, Debug)]
pub struct CstBlock(pub Token, pub Vec<CstStmt>, pub Token);

#[derive(Clone, Debug)]
pub enum CstStmt {
    Let(Token, Token, Option<(Token, CstType)>, Token, CstExpr),
    Expr(CstExpr),
}

#[derive(Clone, Debug)]
pub enum CstElse {
    Block(CstBlock),
    /// `else if …` chains.
    If(Box<CstExpr>),
}
