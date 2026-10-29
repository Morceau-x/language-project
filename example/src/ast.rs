//! The surface abstract syntax: trivia gone, spans kept. Parameterized by
//! the name type — [`RawName`](crate::names::RawName) straight out of
//! lowering, [`Name`](crate::names::Name) after renaming.

use crate::lexer::token::{F64, IntLit};
use crate::names::ModuleName;
use crate::span::Span;

#[derive(Clone, Debug)]
pub struct Module<N> {
    pub name: ModuleName,
    pub name_span: Span,
    pub imports: Vec<Import>,
    pub decls: Vec<Decl<N>>,
}

#[derive(Clone, Debug)]
pub struct Import {
    pub module: ModuleName,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Decl<N> {
    Type(TypeDecl<N>),
    Fn(FnDef<N>),
    Let(Span, N, Option<TypeAnn>, Expr<N>),
    Cast(CastDef<N>),
    Component(ComponentDef<N>),
}

/// `component Counter(init: init, update: update, view: view)` — the
/// component manifest: a declaration that WIRES ordinary functions. The
/// name is what uppercase JSX tags resolve to; the wired names are plain
/// top-level value references. Only `view` is mandatory; the checker
/// reads Props/Model/Msg/Out off the wired signatures.
#[derive(Clone, Debug)]
pub struct ComponentDef<N> {
    pub span: Span,
    pub name: N,
    pub init: Option<(Span, N)>,
    pub update: Option<(Span, N)>,
    pub view: (Span, N),
}

/// `cast (d: Float) -> Celsius { … }` — the signature is the identity
/// (dispatch is by head-constructor pair), the name is synthesized for
/// the emitted JS (`floatToCelsius`) and is NOT in the value namespace.
#[derive(Clone, Debug)]
pub struct CastDef<N> {
    pub span: Span,
    pub name: N,
    /// Head type-constructor texts, straight off the annotations —
    /// rename-time material for the synthesized name.
    pub src_head: String,
    pub tgt_head: String,
    pub param: FnParam<N>,
    pub ret: TypeAnn,
    /// Always an `Expr::Block`.
    pub body: Expr<N>,
}

#[derive(Clone, Debug)]
pub struct TypeDecl<N> {
    pub span: Span,
    pub name: N,
    /// Type parameters as written: spans and texts (`a`, `b`).
    pub params: Vec<(Span, String)>,
    pub rhs: TypeRhs<N>,
}

#[derive(Clone, Debug)]
pub enum TypeRhs<N> {
    Variants(Vec<Variant<N>>),
    /// A nominal record: one implicit constructor named like the type.
    Record(N, Vec<Field>),
}

#[derive(Clone, Debug)]
pub struct Variant<N> {
    pub span: Span,
    pub name: N,
    pub args: Vec<TypeAnn>,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub span: Span,
    pub name: String,
    pub ty: TypeAnn,
}

/// Surface types. Names stay textual; the type checker resolves them
/// against the module environment (so "unknown type" errors carry type
/// context).
#[derive(Clone, Debug)]
pub enum TypeAnn {
    /// `Shape`, `Option<a>`.
    Name(Span, String, Vec<TypeAnn>),
    /// A type variable. The reserved name `number` ranges over Int and
    /// Float only.
    Var(Span, String),
    /// `(T1, T2) -> T`, optionally labeled: `(x: Int, y: Int) -> Int`.
    Fun(Span, Vec<(Option<String>, TypeAnn)>, Box<TypeAnn>),
}

impl TypeAnn {
    pub fn span(&self) -> Span {
        match self {
            TypeAnn::Name(sp, _, _) | TypeAnn::Var(sp, _) | TypeAnn::Fun(sp, _, _) => *sp,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FnDef<N> {
    pub span: Span,
    pub name: N,
    pub params: Vec<FnParam<N>>,
    pub ret: Option<TypeAnn>,
    /// Always an `Expr::Block`.
    pub body: Expr<N>,
}

/// A function parameter; `None` name is `_`.
#[derive(Clone, Debug)]
pub struct FnParam<N> {
    pub span: Span,
    pub name: Option<N>,
    pub ann: Option<TypeAnn>,
}

#[derive(Clone, Debug)]
pub enum Expr<N> {
    Var(Span, N),
    /// Bare constructor reference (callable and first-class).
    Con(Span, N),
    Lit(Span, Lit),
    /// `expr.name` as parsed; the renamer eliminates this into a
    /// qualified `Var`/`Con` or a `Field`.
    Dot(Span, Box<Expr<N>>, Span, String),
    /// Field access, post-rename only.
    Field(Span, Box<Expr<N>>, Span, String),
    Call(Span, Box<Expr<N>>, Vec<Arg<N>>),
    /// Record update `User(..base, age: 31)`: the constructor and its
    /// span, the base record, and the labeled replacements (label span,
    /// label, value).
    Update(Span, N, Span, Box<Expr<N>>, Vec<(Span, String, Expr<N>)>),
    /// Operator span kept separately for error blame.
    Bin(Span, BinOp, Span, Box<Expr<N>>, Box<Expr<N>>),
    Unary(Span, UnOp, Box<Expr<N>>),
    /// `x |> f(a)`; kept through type checking for first-class error
    /// spans, desugared to a call in Core lowering.
    Pipe(Span, PipeStyle, Box<Expr<N>>, Box<Expr<N>>),
    If(Span, Box<Expr<N>>, Box<Expr<N>>, Box<Expr<N>>),
    Match(Span, Box<Expr<N>>, Vec<Arm<N>>),
    /// `expr::Type` — a cast: dispatches to `toString` on the source
    /// type's module (target String) or `from<Source>` on the target's.
    /// Carries the target NAME (span + text); resolution happens in Infer.
    Cast(Span, Box<Expr<N>>, Span, String),
    /// `fn(x: Int) -> Int { body }` — the params are declaration
    /// params (annotations optional), the body a lowered block.
    Lam(Span, Vec<FnParam<N>>, Option<TypeAnn>, Box<Expr<N>>),
    /// Elements plus optional spread tail: `[a, b, ..rest]`.
    List(Span, Vec<Expr<N>>, Option<Box<Expr<N>>>),
    /// Statements plus the mandatory result expression.
    Block(Span, Vec<Stmt<N>>, Box<Expr<N>>),
    /// `expr?`: unwrap `Ok`/`Some` or early-return the `Err`/`None`
    /// from the nearest enclosing fn or lambda. The second span is the
    /// `?` token itself (blame).
    Try(Span, Span, Box<Expr<N>>),
    /// `"a ${x} b"`: literal chunks and spliced expressions, in order.
    /// Every spliced expression must be a String; empty chunks are elided.
    Interp(Span, Vec<InterpPart<N>>),
    /// A JSX element: sugar for building `Html<msg>` values, first-class
    /// through renaming and type checking so diagnostics speak JSX, then
    /// desugared in Core lowering.
    Jsx(Span, JsxElem<N>),
    /// A stateful component tag after desugaring: the component, its
    /// Props construction, and its out-event handlers. SYNTHESIZED by
    /// Core lowering only — never present before it.
    Instance(Span, N, Option<Box<Expr<N>>>, Vec<(String, Expr<N>)>),
}

impl<N> Expr<N> {
    pub fn span(&self) -> Span {
        match self {
            Expr::Var(sp, _)
            | Expr::Con(sp, _)
            | Expr::Lit(sp, _)
            | Expr::Dot(sp, _, _, _)
            | Expr::Field(sp, _, _, _)
            | Expr::Call(sp, _, _)
            | Expr::Update(sp, _, _, _, _)
            | Expr::Bin(sp, _, _, _, _)
            | Expr::Unary(sp, _, _)
            | Expr::Pipe(sp, _, _, _)
            | Expr::If(sp, _, _, _)
            | Expr::Match(sp, _, _)
            | Expr::Cast(sp, _, _, _)
            | Expr::Lam(sp, _, _, _)
            | Expr::List(sp, _, _)
            | Expr::Block(sp, _, _)
            | Expr::Try(sp, _, _)
            | Expr::Interp(sp, _)
            | Expr::Jsx(sp, _)
            | Expr::Instance(sp, _, _, _) => *sp,
        }
    }
}

#[derive(Clone, Debug)]
pub enum InterpPart<N> {
    Str(String),
    Expr(Expr<N>),
}

#[derive(Clone, Debug)]
pub struct JsxElem<N> {
    pub span: Span,
    pub tag: JsxTag<N>,
    pub attrs: Vec<JsxAttr<N>>,
    /// Text already cooked by the whitespace rule (newline-bearing
    /// whitespace-only runs dropped, boundary whitespace trimmed at
    /// newlines, interior newline runs collapsed to one space).
    pub children: Vec<JsxChild<N>>,
    /// Written with a closing tag? Drives the component `children:`
    /// rule: `<Card></Card>` passes `children: []`, `<Card/>` passes none.
    pub has_close: bool,
}

#[derive(Clone, Debug)]
pub enum JsxTag<N> {
    /// A lowercase tag: an HTML (or custom) element.
    Intrinsic(Span, String),
    /// An uppercase tag: the module's `view` function — `<Card …>` calls
    /// `Card.view`. Pre-rename the name is `RawName (Just mod) "view"`;
    /// the renamer resolves it like any qualified use.
    Component(Span, N),
}

impl<N> JsxTag<N> {
    pub fn span(&self) -> Span {
        match self {
            JsxTag::Intrinsic(sp, _) | JsxTag::Component(sp, _) => *sp,
        }
    }
}

#[derive(Clone, Debug)]
pub enum JsxAttr<N> {
    /// `name=value` (or bare `name`, `None`): a String attribute on an
    /// intrinsic element, a labeled argument on a component.
    Attr(Span, String, Option<Expr<N>>),
    /// `on:event={msg}` — the name is the DOM event, verbatim.
    On(Span, String, Expr<N>),
}

impl<N> JsxAttr<N> {
    pub fn span(&self) -> Span {
        match self {
            JsxAttr::Attr(sp, _, _) | JsxAttr::On(sp, _, _) => *sp,
        }
    }
}

#[derive(Clone, Debug)]
pub enum JsxChild<N> {
    /// A cooked text run.
    Text(Span, String),
    Elem(JsxElem<N>),
    /// `{expr}` — one `Html<msg>` child.
    Splice(Span, Expr<N>),
    /// `{..expr}` — a `List<Html<msg>>` spread in place.
    Spread(Span, Expr<N>),
    /// `${expr}` — a String hole, wrapped as text.
    Hole(Span, Expr<N>),
}

#[derive(Clone, Debug)]
pub enum Stmt<N> {
    Let(Span, N, Option<TypeAnn>, Expr<N>),
    /// A non-final expression statement; its value is discarded.
    Discard(Expr<N>),
}

#[derive(Clone, Debug)]
pub enum Arg<N> {
    Pos(Expr<N>),
    /// `label: expr` in a record-constructor call.
    Lab(Span, String, Expr<N>),
}

impl<N> Arg<N> {
    pub fn expr(&self) -> &Expr<N> {
        match self {
            Arg::Pos(e) | Arg::Lab(_, _, e) => e,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Arm<N> {
    pub span: Span,
    pub pat: Pat<N>,
    /// `pattern if guard ->`; the guard sees the pattern's binders and
    /// must be `Bool`. A guarded arm never witnesses exhaustiveness.
    pub guard: Option<Expr<N>>,
    pub body: Expr<N>,
}

#[derive(Clone, Debug)]
pub enum Pat<N> {
    Wild(Span),
    Var(Span, N),
    Lit(Span, Lit),
    Ctor(Span, N, Vec<Pat<N>>),
    /// `[]`, `[p, q]`, `[p, ..rest]`.
    List(Span, Vec<Pat<N>>, Option<PatRest<N>>),
}

impl<N> Pat<N> {
    pub fn span(&self) -> Span {
        match self {
            Pat::Wild(sp)
            | Pat::Var(sp, _)
            | Pat::Lit(sp, _)
            | Pat::Ctor(sp, _, _)
            | Pat::List(sp, _, _) => *sp,
        }
    }
}

/// The `..rest` tail binder; `None` name is `_`.
#[derive(Clone, Debug)]
pub struct PatRest<N> {
    pub span: Span,
    pub name: Option<N>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lit {
    Int(IntLit),
    Float(F64),
    Str(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
    Pow,
    Concat,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn text(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::IntDiv => "~/",
            BinOp::Mod => "%",
            BinOp::Pow => "**",
            BinOp::Concat => "++",
            BinOp::Eq => "==",
            BinOp::Neq => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnOp {
    Neg,
    Not,
}

/// How a pipeline treats its right-hand side. A call written directly
/// after `|>` takes the subject as its FIRST argument (`x |> f(a)` is
/// `f(x, a)`); parenthesizing the stage opts out — `x |> (f(a))` applies
/// the value of `f(a)` (say, a partial application) to the subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeStyle {
    Into,
    Apply,
}

/// Which failure type a `?` unwraps — decided by the checker, consumed
/// by Core lowering and the JS emitter (the tag test and payload field
/// depend on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryKind {
    Result,
    Option,
}
